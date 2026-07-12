use std::collections::BTreeMap;

use crate::ir::reduced::ReducedVariant;
use crate::ir::timeline::CanonicalKey;
use crate::lifecycle::canonical::CanonicalGeneration;
use crate::lifecycle::keys::GenerationKey;

/// Errors that can occur when resolving a flat address identity down to exactly one
/// verified generation certificate (formal spec Stage 3, "Generation Resolution
/// Soundness": every survivor key maps to exactly one verified generation, or
/// resolution fails deterministically).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerationResolutionError {
    /// No generation record exists at all for this address.
    IdentityAbsent { address: [u8; 20] },
    /// More than one generation claims to be the terminal one for this address.
    /// Shouldn't happen if `canonicalize.rs`'s key-uniqueness invariant holds — this
    /// is a defensive check against that invariant having been violated upstream,
    /// not a condition the current pipeline is expected to hit in practice.
    IdentityAmbiguous { address: [u8; 20] },
    /// Reserved for future provenance-chain checks (e.g. a generation whose declared
    /// ancestor doesn't match the prior generation's terminal state). Not raised by
    /// `resolve_generation_for_address` yet, since no cross-generation ancestry
    /// linkage is tracked anywhere in the pipeline.
    ProvenanceMetadataDivergence { address: [u8; 20], generation_id: u32 },
    /// Reserved for future acyclicity/chain-shape checks on generation succession.
    /// Not raised yet, for the same reason as above.
    MalformedProvenanceChain { address: [u8; 20] },
    /// The snapshot source itself reported an internal inconsistency for this
    /// address (e.g. conflicting records). Not raised by anything in this module —
    /// reserved for a real `AccountSnapshotSource` implementation to use.
    StateDatabaseCorruption { address: [u8; 20] },
    /// A certificate field was neither present in the terminal projection nor
    /// recoverable from the snapshot source.
    InsufficientStateInformation { address: [u8; 20], missing_field: &'static str },
}

/// A generation resolved as the unique terminal identity epoch for its address,
/// as of the end of an already-canonicalized generation list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedGeneration {
    pub key: GenerationKey,
    /// Placeholder commitment root. `kaysentinel-hash` doesn't exist yet (still an
    /// empty stub crate), so this is always `[0u8; 32]` rather than a real
    /// cryptographic commitment — wire this up once that crate is implemented.
    pub state_table_proof_root: [u8; 32],
}

/// Resolves the unique terminal generation for a given address out of an already
/// canonicalized (sorted, deduplicated) generation list.
pub fn resolve_generation_for_address(
    generations: &[CanonicalGeneration],
    address: [u8; 20],
) -> Result<VerifiedGeneration, GenerationResolutionError> {
    let matches: Vec<&CanonicalGeneration> =
        generations.iter().filter(|g| g.key.address == address).collect();

    if matches.is_empty() {
        return Err(GenerationResolutionError::IdentityAbsent { address });
    }

    let max_generation_id = matches.iter().map(|g| g.key.generation_id).max().unwrap();
    let terminal: Vec<&&CanonicalGeneration> = matches
        .iter()
        .filter(|g| g.key.generation_id == max_generation_id)
        .collect();

    if terminal.len() > 1 {
        return Err(GenerationResolutionError::IdentityAmbiguous { address });
    }

    Ok(VerifiedGeneration {
        key: terminal[0].key,
        state_table_proof_root: [0u8; 32],
    })
}

/// An authoritative base-attribute snapshot for an account, recovered from a source
/// external to the current transaction's projection (e.g. prior block state).
///
/// Deliberately excludes `storage_root`: the storage trie is a downstream Phase 5
/// derivation product, not a base account attribute, and folding it in here invites
/// exactly the bug `CanonicalAccountCertificate` used to have (see `StorageRootState`
/// below) — a fetched-then-discarded value silently replaced by a hardcoded one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedAccountSnapshot {
    pub nonce: u64,
    pub balance: [u8; 32],
    pub code_hash: [u8; 32],
}

/// A source of authoritative account data external to the current transaction's
/// terminal projection — e.g. a prior-block state snapshot or cache. Implementing
/// this trait is how a real backing store plugs into `build_canonical_certificates`;
/// nothing in this crate provides a real implementation yet.
pub trait AccountSnapshotSource {
    fn resolve_identity_generation(
        &self,
        address: [u8; 20],
    ) -> Result<VerifiedGeneration, GenerationResolutionError>;

    fn recover_account_snapshot(
        &self,
        address: [u8; 20],
    ) -> Result<ResolvedAccountSnapshot, GenerationResolutionError>;
}

/// Whether an account's storage root has actually been derived yet. Phase 5 (storage
/// subtree/trie construction) doesn't exist yet, so every certificate produced today
/// carries `AwaitingDerivation` — never a fabricated root standing in for a real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageRootState {
    Verified([u8; 32]),
    AwaitingDerivation,
}

/// The non-mutable evidentiary artifact for a fully resolved account's canonical
/// state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalAccountCertificate {
    pub address: [u8; 20],
    pub generation: VerifiedGeneration,
    pub nonce: u64,
    pub balance: [u8; 32],
    pub code_hash: [u8; 32],
    pub storage_root: StorageRootState,
}

#[derive(Default)]
struct CertificateFields {
    nonce: Option<u64>,
    balance: Option<[u8; 32]>,
    code_hash: Option<[u8; 32]>,
}

struct CertificateBuilder {
    address: [u8; 20],
    generation: VerifiedGeneration,
    fields: CertificateFields,
}

/// Builds canonical account certificates from the pipeline's real terminal
/// projection (`state_table`, as already produced by `reduce.rs`) and an already
/// canonicalized generation list, falling back to `snapshot_source` for any base
/// attribute (nonce/balance/code_hash) that this transaction's projection didn't
/// touch. Projection values always take precedence over snapshot values for fields
/// the projection *did* touch — a stale snapshot balance can never overwrite a
/// balance this transaction actually changed.
///
/// Storage/transient entries mark an address as needing a certificate (so an
/// account that was only ever touched via storage still gets one), but don't feed
/// `storage_root` directly — that stays `AwaitingDerivation` until Phase 5 exists.
pub fn build_canonical_certificates<S: AccountSnapshotSource>(
    state_table: &BTreeMap<CanonicalKey, ReducedVariant>,
    generations: &[CanonicalGeneration],
    snapshot_source: &S,
) -> Result<BTreeMap<[u8; 20], CanonicalAccountCertificate>, GenerationResolutionError> {
    let mut builders: BTreeMap<[u8; 20], CertificateBuilder> = BTreeMap::new();

    for (key, variant) in state_table {
        let address = match *key {
            CanonicalKey::Balance { address }
            | CanonicalKey::Nonce { address }
            | CanonicalKey::Code { address }
            | CanonicalKey::Storage { address, .. }
            | CanonicalKey::Transient { address, .. } => address,
        };

        let builder = if let Some(existing) = builders.get_mut(&address) {
            existing
        } else {
            let generation = resolve_generation_for_address(generations, address)
                .or_else(|_| snapshot_source.resolve_identity_generation(address))?;
            builders.entry(address).or_insert(CertificateBuilder {
                address,
                generation,
                fields: CertificateFields::default(),
            })
        };

        match variant {
            ReducedVariant::Balance(t) => builder.fields.balance = Some(*t.terminal()),
            ReducedVariant::Nonce(t) => builder.fields.nonce = Some(*t.terminal()),
            ReducedVariant::Code(t) => builder.fields.code_hash = Some(*t.terminal()),
            // Storage/Transient don't populate certificate fields directly; they only
            // ensure the address gets a certificate at all. Real subtree derivation
            // is Phase 5.
            ReducedVariant::Storage(_) | ReducedVariant::Transient(_) => {}
        }
    }

    builders
        .into_iter()
        .map(|(address, mut b)| {
            if b.fields.nonce.is_none() || b.fields.balance.is_none() || b.fields.code_hash.is_none() {
                let snapshot = snapshot_source.recover_account_snapshot(address)?;
                if b.fields.nonce.is_none() {
                    b.fields.nonce = Some(snapshot.nonce);
                }
                if b.fields.balance.is_none() {
                    b.fields.balance = Some(snapshot.balance);
                }
                if b.fields.code_hash.is_none() {
                    b.fields.code_hash = Some(snapshot.code_hash);
                }
            }

            let nonce = b.fields.nonce.ok_or(GenerationResolutionError::InsufficientStateInformation {
                address,
                missing_field: "nonce",
            })?;
            let balance = b.fields.balance.ok_or(GenerationResolutionError::InsufficientStateInformation {
                address,
                missing_field: "balance",
            })?;
            let code_hash = b.fields.code_hash.ok_or(GenerationResolutionError::InsufficientStateInformation {
                address,
                missing_field: "code_hash",
            })?;

            Ok((
                address,
                CanonicalAccountCertificate {
                    address: b.address,
                    generation: b.generation,
                    nonce,
                    balance,
                    code_hash,
                    storage_root: StorageRootState::AwaitingDerivation,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::timeline::{Provenance, TraceProvenance};
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
    fn resolves_single_generation() {
        let addr = [1u8; 20];
        let gens = vec![CanonicalGeneration {
            key: GenerationKey { address: addr, generation_id: 0 },
            begin: make_prov(10),
            end: None,
        }];

        let resolved = resolve_generation_for_address(&gens, addr).unwrap();
        assert_eq!(resolved.key.generation_id, 0);
    }

    #[test]
    fn picks_highest_generation_id_as_terminal() {
        let addr = [2u8; 20];
        let gens = vec![
            CanonicalGeneration {
                key: GenerationKey { address: addr, generation_id: 0 },
                begin: make_prov(10),
                end: Some(make_prov(20)),
            },
            CanonicalGeneration {
                key: GenerationKey { address: addr, generation_id: 1 },
                begin: make_prov(30),
                end: None,
            },
        ];

        let resolved = resolve_generation_for_address(&gens, addr).unwrap();
        assert_eq!(resolved.key.generation_id, 1);
    }

    #[test]
    fn errors_when_address_absent() {
        let addr = [3u8; 20];
        let gens: Vec<CanonicalGeneration> = vec![];
        let err = resolve_generation_for_address(&gens, addr).unwrap_err();
        assert_eq!(err, GenerationResolutionError::IdentityAbsent { address: addr });
    }

    #[test]
    fn errors_when_terminal_generation_ambiguous() {
        let addr = [4u8; 20];
        let gens = vec![
            CanonicalGeneration {
                key: GenerationKey { address: addr, generation_id: 1 },
                begin: make_prov(10),
                end: None,
            },
            CanonicalGeneration {
                key: GenerationKey { address: addr, generation_id: 1 },
                begin: make_prov(15),
                end: None,
            },
        ];

        let err = resolve_generation_for_address(&gens, addr).unwrap_err();
        assert_eq!(err, GenerationResolutionError::IdentityAmbiguous { address: addr });
    }

    #[test]
    fn projection_precedence_over_snapshot() {
        let addr = [111u8; 20];
        let mut state_table = BTreeMap::new();
        state_table.insert(
            CanonicalKey::Balance { address: addr },
            ReducedVariant::Balance(make_transition([0u8; 32], {
                let mut b = [0u8; 32];
                b[31] = 100;
                b
            })),
        );

        let snapshot_source = MockSnapshotSource {
            snapshot: ResolvedAccountSnapshot {
                nonce: 10,
                balance: {
                    let mut b = [0u8; 32];
                    b[31] = 255;
                    b
                },
                code_hash: [0xFAu8; 32],
            },
        };

        let certs = build_canonical_certificates(&state_table, &gens_for(addr), &snapshot_source).unwrap();
        let cert = certs.get(&addr).unwrap();

        let mut expected_balance = [0u8; 32];
        expected_balance[31] = 100;
        assert_eq!(cert.balance, expected_balance); // projection value wins
        assert_eq!(cert.nonce, 10); // untouched field recovered from snapshot
        assert_eq!(cert.storage_root, StorageRootState::AwaitingDerivation);
    }

    #[test]
    fn storage_and_balance_mixed_recovery() {
        let addr = [222u8; 20];
        let mut state_table = BTreeMap::new();
        state_table.insert(
            CanonicalKey::Storage { address: addr, slot: [0u8; 32] },
            ReducedVariant::Storage(make_transition([0u8; 32], [1u8; 32])),
        );
        let mut balance_after = [0u8; 32];
        balance_after[31] = 55;
        state_table.insert(
            CanonicalKey::Balance { address: addr },
            ReducedVariant::Balance(make_transition([0u8; 32], balance_after)),
        );

        let snapshot_source = MockSnapshotSource {
            snapshot: ResolvedAccountSnapshot {
                nonce: 3,
                balance: {
                    let mut b = [0u8; 32];
                    b[31] = 255; // should be bypassed
                    b
                },
                code_hash: [0xCCu8; 32],
            },
        };

        let certs = build_canonical_certificates(&state_table, &gens_for(addr), &snapshot_source).unwrap();
        let cert = certs.get(&addr).unwrap();

        assert_eq!(cert.balance, balance_after);
        assert_eq!(cert.nonce, 3);
        assert_eq!(cert.code_hash, [0xCCu8; 32]);
    }

    #[test]
    fn multi_account_isolated_certificate_generation() {
        let addr_a = [1u8; 20];
        let addr_b = [2u8; 20];

        let mut state_table = BTreeMap::new();
        state_table.insert(
            CanonicalKey::Storage { address: addr_a, slot: [0u8; 32] },
            ReducedVariant::Storage(make_transition([0u8; 32], [1u8; 32])),
        );
        state_table.insert(
            CanonicalKey::Nonce { address: addr_b },
            ReducedVariant::Nonce(make_transition(0, 88)),
        );

        let mut generations = gens_for(addr_a);
        generations.extend(gens_for(addr_b));

        let snapshot_source = MockSnapshotSource {
            snapshot: ResolvedAccountSnapshot { nonce: 0, balance: [0u8; 32], code_hash: [0u8; 32] },
        };

        let certs = build_canonical_certificates(&state_table, &generations, &snapshot_source).unwrap();

        assert_eq!(certs.len(), 2);
        assert_eq!(certs.get(&addr_a).unwrap().nonce, 0);
        assert_eq!(certs.get(&addr_b).unwrap().nonce, 88);
    }
}
