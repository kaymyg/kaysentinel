use std::collections::BTreeMap;

use crate::ir::reduced::ReducedVariant;
use crate::ir::timeline::CanonicalKey;
use crate::lifecycle::canonical::CanonicalGeneration;
use crate::lifecycle::certificate::{
    AccountSnapshotSource, CanonicalAccountCertificate, GenerationResolutionError,
    ResolvedAccountSnapshot, StorageRootState, VerifiedGeneration, resolve_generation_for_address,
};

/// The single, immutable intermediate payload representing a fully hydrated account:
/// identity, base attributes, and generation all gathered up front, so certificate
/// assembly (`assemble_certificates`) can be a pure, no-I/O mapping instead of doing
/// lookups and fallback recovery inline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydratedAccountState {
    pub address: [u8; 20],
    pub snapshot: ResolvedAccountSnapshot,
    pub generation: VerifiedGeneration,
    pub storage_root: StorageRootState,
}

/// Errors specific to the pure certificate-assembly step — distinct from
/// `GenerationResolutionError`, which covers failures during hydration (the step
/// that actually does lookups/I/O). This only has one variant right now because
/// it's the only invariant assembly can actually violate: everything else that
/// could go wrong already happened (and was already reported) during hydration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateAssemblyError {
    /// A `HydratedAccountState`'s own `address` field didn't match the map key it
    /// was stored under. Can't happen via `hydrate_accounts` (which always keys by
    /// the same address it stores), but a hand-built or externally-supplied
    /// `BTreeMap<[u8;20], HydratedAccountState>` could violate it, so assembly
    /// checks for real rather than assuming it's impossible.
    IdentityMismatchedKey { address: [u8; 20], key: [u8; 20] },
}

/// Hydrates every address touched by a terminal projection (`state_table`, as
/// produced by `reduce.rs`) into a `HydratedAccountState`: resolves its generation
/// and gathers its base attributes, falling back to `snapshot_source` for anything
/// this transaction's projection didn't touch. This is the "gather everything,
/// possibly doing I/O" phase, split out from the pure assembly step below.
pub fn hydrate_accounts<S: AccountSnapshotSource>(
    state_table: &BTreeMap<CanonicalKey, ReducedVariant>,
    generations: &[CanonicalGeneration],
    snapshot_source: &S,
) -> Result<BTreeMap<[u8; 20], HydratedAccountState>, GenerationResolutionError> {
    struct PartialFields {
        nonce: Option<u64>,
        balance: Option<[u8; 32]>,
        code_hash: Option<[u8; 32]>,
    }

    let mut generation_by_address: BTreeMap<[u8; 20], VerifiedGeneration> = BTreeMap::new();
    let mut fields_by_address: BTreeMap<[u8; 20], PartialFields> = BTreeMap::new();

    for (key, variant) in state_table {
        let address = match *key {
            CanonicalKey::Balance { address }
            | CanonicalKey::Nonce { address }
            | CanonicalKey::Code { address }
            | CanonicalKey::Storage { address, .. }
            | CanonicalKey::Transient { address, .. } => address,
        };

        if !generation_by_address.contains_key(&address) {
            let generation = resolve_generation_for_address(generations, address)
                .or_else(|_| snapshot_source.resolve_identity_generation(address))?;
            generation_by_address.insert(address, generation);
        }

        let fields = fields_by_address.entry(address).or_insert(PartialFields {
            nonce: None,
            balance: None,
            code_hash: None,
        });

        match variant {
            ReducedVariant::Balance(t) => fields.balance = Some(*t.terminal()),
            ReducedVariant::Nonce(t) => fields.nonce = Some(*t.terminal()),
            ReducedVariant::Code(t) => fields.code_hash = Some(*t.terminal()),
            ReducedVariant::Storage(_) | ReducedVariant::Transient(_) => {}
        }
    }

    let mut hydrated = BTreeMap::new();
    for (address, mut fields) in fields_by_address {
        if fields.nonce.is_none() || fields.balance.is_none() || fields.code_hash.is_none() {
            let snapshot = snapshot_source.recover_account_snapshot(address)?;
            if fields.nonce.is_none() {
                fields.nonce = Some(snapshot.nonce);
            }
            if fields.balance.is_none() {
                fields.balance = Some(snapshot.balance);
            }
            if fields.code_hash.is_none() {
                fields.code_hash = Some(snapshot.code_hash);
            }
        }

        let nonce = fields.nonce.ok_or(GenerationResolutionError::InsufficientStateInformation {
            address,
            missing_field: "nonce",
        })?;
        let balance = fields.balance.ok_or(GenerationResolutionError::InsufficientStateInformation {
            address,
            missing_field: "balance",
        })?;
        let code_hash = fields.code_hash.ok_or(GenerationResolutionError::InsufficientStateInformation {
            address,
            missing_field: "code_hash",
        })?;

        hydrated.insert(
            address,
            HydratedAccountState {
                address,
                snapshot: ResolvedAccountSnapshot { nonce, balance, code_hash },
                generation: generation_by_address[&address],
                // Phase 5 (storage trie construction) doesn't exist yet — see
                // `StorageRootDeriver` below.
                storage_root: StorageRootState::AwaitingDerivation,
            },
        );
    }

    Ok(hydrated)
}

/// Pure compiler pass: maps already-hydrated account states into their final
/// canonical certificates. Does no I/O and can only fail on the identity-consistency
/// invariant, since everything else was already resolved during hydration.
pub fn assemble_certificates(
    hydrated: BTreeMap<[u8; 20], HydratedAccountState>,
) -> Result<BTreeMap<[u8; 20], CanonicalAccountCertificate>, CertificateAssemblyError> {
    hydrated
        .into_iter()
        .map(|(key, state)| {
            if state.address != key {
                return Err(CertificateAssemblyError::IdentityMismatchedKey {
                    address: state.address,
                    key,
                });
            }

            Ok((
                key,
                CanonicalAccountCertificate {
                    address: state.address,
                    generation: state.generation,
                    nonce: state.snapshot.nonce,
                    balance: state.snapshot.balance,
                    code_hash: state.snapshot.code_hash,
                    storage_root: state.storage_root,
                },
            ))
        })
        .collect()
}

// ============================================================================
// Phase 5 scaffolding (storage subtree / trie construction) — no implementation
// exists anywhere yet. These types exist so a real implementation has somewhere
// to plug in, matching the same "flag it, don't fake it" approach used for
// `AccountSnapshotSource` and `state_table_proof_root` elsewhere in this module.
// ============================================================================

/// Errors native to the underlying physical persistence or cryptographic backend —
/// as opposed to `StorageDerivationError`, which is a deterministic domain-level
/// failure. Kept separate so infrastructure failures (a DB timeout, a corrupt trie
/// node on disk) can never be silently conflated with "this data is semantically
/// invalid."
#[derive(Debug)]
pub enum BackendError {
    Database(Box<dyn std::error::Error + Send + Sync>),
    TrieCorruption(String),
    Io(std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageDerivationError {
    MissingBaseState,
    InvalidMutation,
    CorruptSnapshot,
}

#[derive(Debug)]
pub enum DerivationError {
    Domain(StorageDerivationError),
    Infrastructure(BackendError),
}

/// Computes an account's storage root as a pure, deterministic function of a base
/// state plus the terminal (already-reduced) slot values touched this transaction.
/// No implementation exists yet — this is the trait a real Phase 5 trie
/// implementation would fill in.
pub trait StorageRootDeriver: Send + Sync {
    type BaseState;

    fn derive(
        &self,
        base_state: &Self::BaseState,
        terminal_slots: &BTreeMap<[u8; 32], [u8; 32]>,
    ) -> Result<[u8; 32], DerivationError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::timeline::{Provenance, TraceProvenance};
    use crate::lifecycle::keys::GenerationKey;
    use crate::traits::ReducedTransition;

    fn make_prov(ord: u64) -> TraceProvenance {
        TraceProvenance { trace_ordinal: ord, tx_index: 0, frame_id: 0, call_depth: 0, frame_ordinal: 0 }
    }

    fn make_transition<T: Clone + PartialEq>(initial: T, terminal: T) -> ReducedTransition<T> {
        ReducedTransition::new(initial, terminal, Provenance::new())
    }

    struct MockSnapshotSource {
        snapshot: ResolvedAccountSnapshot,
    }

    impl AccountSnapshotSource for MockSnapshotSource {
        fn resolve_identity_generation(
            &self,
            _address: [u8; 20],
        ) -> Result<VerifiedGeneration, GenerationResolutionError> {
            Ok(VerifiedGeneration {
                key: GenerationKey { address: [0u8; 20], generation_id: 0 },
                state_table_proof_root: [0u8; 32],
            })
        }

        fn recover_account_snapshot(
            &self,
            _address: [u8; 20],
        ) -> Result<ResolvedAccountSnapshot, GenerationResolutionError> {
            Ok(self.snapshot)
        }
    }

    fn gens_for(address: [u8; 20]) -> Vec<CanonicalGeneration> {
        vec![CanonicalGeneration {
            key: GenerationKey { address, generation_id: 0 },
            begin: make_prov(0),
            end: None,
        }]
    }

    #[test]
    fn assembly_enforces_identity_consistency() {
        let key_addr = [42u8; 20];
        let mismatched_state_addr = [99u8; 20];

        let mut corrupted = BTreeMap::new();
        corrupted.insert(
            key_addr,
            HydratedAccountState {
                address: mismatched_state_addr, // invariant violation, on purpose
                snapshot: ResolvedAccountSnapshot { nonce: 0, balance: [0u8; 32], code_hash: [0u8; 32] },
                generation: VerifiedGeneration {
                    key: GenerationKey { address: mismatched_state_addr, generation_id: 1 },
                    state_table_proof_root: [0u8; 32],
                },
                storage_root: StorageRootState::AwaitingDerivation,
            },
        );

        let result = assemble_certificates(corrupted);

        assert_eq!(
            result,
            Err(CertificateAssemblyError::IdentityMismatchedKey {
                address: mismatched_state_addr,
                key: key_addr,
            })
        );
    }

    #[test]
    fn assembles_correct_certificate_from_hydrated_state() {
        let addr = [7u8; 20];
        let generation = VerifiedGeneration {
            key: GenerationKey { address: addr, generation_id: 12 },
            state_table_proof_root: [0xFFu8; 32],
        };
        let snapshot = ResolvedAccountSnapshot { nonce: 45, balance: [0x01u8; 32], code_hash: [0x11u8; 32] };

        let mut hydrated = BTreeMap::new();
        hydrated.insert(
            addr,
            HydratedAccountState {
                address: addr,
                snapshot,
                generation,
                storage_root: StorageRootState::Verified([0x99u8; 32]),
            },
        );

        let certificates = assemble_certificates(hydrated).unwrap();

        assert_eq!(certificates.len(), 1);
        let cert = certificates.get(&addr).unwrap();
        assert_eq!(cert.address, addr);
        assert_eq!(cert.generation, generation);
        assert_eq!(cert.nonce, 45);
        assert_eq!(cert.balance, [0x01u8; 32]);
        assert_eq!(cert.code_hash, [0x11u8; 32]);
        assert_eq!(cert.storage_root, StorageRootState::Verified([0x99u8; 32]));
    }

    #[test]
    fn hydrate_then_assemble_matches_direct_build() {
        let addr = [123u8; 20];
        let mut state_table = BTreeMap::new();
        state_table.insert(
            CanonicalKey::Nonce { address: addr },
            ReducedVariant::Nonce(make_transition(0, 9)),
        );

        let snapshot_source = MockSnapshotSource {
            snapshot: ResolvedAccountSnapshot { nonce: 0, balance: [0u8; 32], code_hash: [0u8; 32] },
        };

        let hydrated = hydrate_accounts(&state_table, &gens_for(addr), &snapshot_source).unwrap();
        let certs = assemble_certificates(hydrated).unwrap();

        let direct = crate::lifecycle::certificate::build_canonical_certificates(
            &state_table,
            &gens_for(addr),
            &snapshot_source,
        )
        .unwrap();

        assert_eq!(certs, direct);
    }
}
