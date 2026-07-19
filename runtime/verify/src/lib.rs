//! # Kaysentinel Verify (Gate 2)
//!
//! Populates the previously-empty `runtime/verify` stub with Gate 2
//! (semantic replay verification): the batch artifact type, the
//! transport-envelope integrity check, and the actual replay reducer
//! (`replay_and_verify`) that executes `CsePayload` traces against an
//! in-memory state model and compares the result against
//! `CanonicalAccountCertificate`s field-for-field.
//!
//! This is a first version, verified only against this crate's own
//! hand-constructed fixtures -- it has not been run against real Geth/Reth
//! output, and doing so is the natural next step once the Layer 2 bridge
//! (`runtime/bridge`) is wired up to a live tracer.

use std::collections::{BTreeMap, BTreeSet};

use kaysentinel_builder::lifecycle::certificate::StorageRootState;
use kaysentinel_builder::lifecycle::hydration::StorageRootDeriver;
use kaysentinel_builder::{CanonicalAccountCertificate, SimpleStorageRootDeriver};
use kaysentinel_cse::CsePayload;
use kaysentinel_hash::{derive_commitment, Digest, Domain};

/// The boundary artifact Gate 2 verifies: a batch of `CsePayload` execution
/// traces alongside the `CanonicalAccountCertificate`s they're claimed to
/// produce. Deliberately holds `CsePayload` directly (not
/// `CanonicalSemanticEvent`) since the reducer only needs mutation content,
/// not `ExecutionContext`/`TraceContext` -- diagnostic isolation (PR1) means
/// trace metadata shouldn't affect replay outcomes anyway.
#[derive(Debug, Clone)]
pub struct ExecutionBatchReplay {
    pub account_certificates: Vec<CanonicalAccountCertificate>,
    pub execution_traces: Vec<CsePayload>,
}

pub struct ReplayVerifier;

impl ReplayVerifier {
    /// Verifies that `raw_ssz_bytes` matches `expected_commitment`, using the
    /// real domain-separated BLAKE3 primitive with the new `Domain::Gate2Replay`
    /// tag. This is transport/envelope-level integrity only -- it says nothing
    /// about whether the *contents* semantically replay correctly; see
    /// `replay_and_verify` for that.
    pub fn verify_envelope_integrity(raw_ssz_bytes: &[u8], expected_commitment: &Digest) -> bool {
        let computed = derive_commitment(Domain::Gate2Replay, raw_ssz_bytes);
        computed == *expected_commitment
    }
}

/// One field disagreement between the reducer's derived state and a
/// certificate's claimed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldMismatch {
    Nonce { expected: u64, found: u64 },
    Balance { expected: [u8; 32], found: [u8; 32] },
    CodeHash { expected: [u8; 32], found: [u8; 32] },
    StorageRoot { expected: [u8; 32], found: StorageRootState },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    /// A trace's own `previous_*` value didn't match what prior traces in
    /// the same batch already established for that address/slot -- the
    /// trace stream itself is internally inconsistent, independent of
    /// whether any certificate is even involved yet.
    TraceInconsistency { address: [u8; 20], detail: String },
    /// An address appears in `execution_traces` but has no certificate in
    /// `account_certificates` at all.
    MissingCertificate { address: [u8; 20] },
    /// An address has a certificate but was never touched by any trace --
    /// per `build_canonical_certificates`, every real certificate should
    /// correspond to at least one mutation.
    UntouchedCertificate { address: [u8; 20] },
    /// The reducer's derived state disagrees with the certificate's claimed
    /// state for this address.
    CertificateMismatch { address: [u8; 20], mismatches: Vec<FieldMismatch> },
    /// The certificate's `storage_root` was `AwaitingDerivation` -- not yet
    /// a valid terminal state to replay-verify against at all.
    UnresolvedStorageRoot { address: [u8; 20] },
}

#[derive(Debug, Clone, Default)]
struct AccountState {
    nonce: u64,
    balance: [u8; 32],
    code_hash: [u8; 32],
    storage: BTreeMap<[u8; 32], [u8; 32]>,
    touched: bool,
}

/// Replays `batch.execution_traces` against an in-memory per-address state
/// model, then asserts the result matches `batch.account_certificates`
/// field-for-field (including `storage_root`, re-derived via the same
/// `SimpleStorageRootDeriver` the real pipeline uses -- not a separate,
/// possibly-divergent implementation).
///
/// This is a first version: it has not been run against real Geth/Reth
/// output, only against the hand-constructed fixtures in this crate's tests.
pub fn replay_and_verify(batch: &ExecutionBatchReplay) -> Result<(), Vec<ReplayError>> {
    let mut state: BTreeMap<[u8; 20], AccountState> = BTreeMap::new();
    let mut trace_errors: Vec<ReplayError> = Vec::new();

    for payload in &batch.execution_traces {
        match payload {
            CsePayload::BalanceChanged(p) => {
                let acct = state.entry(p.address).or_default();
                if acct.touched && acct.balance != p.previous_balance {
                    trace_errors.push(ReplayError::TraceInconsistency {
                        address: p.address,
                        detail: "BalanceChanged.previous_balance did not match prior state".into(),
                    });
                }
                acct.balance = p.current_balance;
                acct.touched = true;
            }
            CsePayload::NonceUpdated(p) => {
                let acct = state.entry(p.address).or_default();
                if acct.touched && acct.nonce != p.previous_nonce {
                    trace_errors.push(ReplayError::TraceInconsistency {
                        address: p.address,
                        detail: "NonceUpdated.previous_nonce did not match prior state".into(),
                    });
                }
                acct.nonce = p.current_nonce;
                acct.touched = true;
            }
            CsePayload::CodeUpdated(p) => {
                let acct = state.entry(p.address).or_default();
                if acct.touched && acct.code_hash != p.previous_code_hash {
                    trace_errors.push(ReplayError::TraceInconsistency {
                        address: p.address,
                        detail: "CodeUpdated.previous_code_hash did not match prior state".into(),
                    });
                }
                acct.code_hash = p.current_code_hash;
                acct.touched = true;
            }
            CsePayload::StorageSlotUpdated(p) => {
                let acct = state.entry(p.address).or_default();
                let current = acct.storage.get(&p.slot).copied().unwrap_or([0u8; 32]);
                if acct.touched && current != p.previous_value {
                    trace_errors.push(ReplayError::TraceInconsistency {
                        address: p.address,
                        detail: format!("StorageSlotUpdated.previous_value mismatch at slot {:?}", p.slot),
                    });
                }
                acct.storage.insert(p.slot, p.current_value);
                acct.touched = true;
            }
            CsePayload::ContractCreated(p) => {
                state.entry(p.address).or_default().touched = true;
            }
            CsePayload::ContractDestroyed(p) => {
                state.entry(p.address).or_default().touched = true;
            }
            // Transient storage is not part of persistent state (EIP-1153;
            // already excluded from storage_root derivation in
            // runtime/builder) -- deliberately not tracked in `state` at all.
            CsePayload::TransientStorageUpdated(_) => {}
            // No replay effect: boundary markers, and payload kinds with no
            // real Go source / already-confirmed dead ends in the real
            // pipeline (see docs/emes/004-bridge-buffering-spec.md).
            CsePayload::BeginTransaction
            | CsePayload::EndTransaction
            | CsePayload::LogEmitted(_)
            | CsePayload::GasRefundChanged(_)
            | CsePayload::AccessListTouched(_) => {}
        }
    }

    if !trace_errors.is_empty() {
        return Err(trace_errors);
    }

    let touched_addresses: BTreeSet<[u8; 20]> =
        state.iter().filter(|(_, s)| s.touched).map(|(addr, _)| *addr).collect();
    let certified_addresses: BTreeSet<[u8; 20]> =
        batch.account_certificates.iter().map(|c| c.address).collect();

    let mut errors = Vec::new();

    for address in touched_addresses.difference(&certified_addresses) {
        errors.push(ReplayError::MissingCertificate { address: *address });
    }
    for address in certified_addresses.difference(&touched_addresses) {
        errors.push(ReplayError::UntouchedCertificate { address: *address });
    }

    let deriver = SimpleStorageRootDeriver;
    for cert in &batch.account_certificates {
        let Some(derived) = state.get(&cert.address) else { continue }; // already reported above

        let mut mismatches = Vec::new();
        if derived.nonce != cert.nonce {
            mismatches.push(FieldMismatch::Nonce { expected: derived.nonce, found: cert.nonce });
        }
        if derived.balance != cert.balance {
            mismatches.push(FieldMismatch::Balance { expected: derived.balance, found: cert.balance });
        }
        if derived.code_hash != cert.code_hash {
            mismatches.push(FieldMismatch::CodeHash { expected: derived.code_hash, found: cert.code_hash });
        }

        match cert.storage_root {
            StorageRootState::AwaitingDerivation => {
                errors.push(ReplayError::UnresolvedStorageRoot { address: cert.address });
            }
            StorageRootState::Verified(claimed_root) => {
                let expected_root = deriver
                    .derive(&(), &derived.storage)
                    .expect("SimpleStorageRootDeriver::derive is infallible for any input");
                if expected_root != claimed_root {
                    mismatches.push(FieldMismatch::StorageRoot {
                        expected: expected_root,
                        found: cert.storage_root,
                    });
                }
            }
        }

        if !mismatches.is_empty() {
            errors.push(ReplayError::CertificateMismatch { address: cert.address, mismatches });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_bytes_and_commitment_verify_true() {
        let bytes = b"pretend this is ssz-serialized ExecutionBatchReplay bytes";
        let commitment = derive_commitment(Domain::Gate2Replay, bytes);
        assert!(ReplayVerifier::verify_envelope_integrity(bytes, &commitment));
    }

    #[test]
    fn tampered_bytes_fail_verification() {
        let bytes = b"original bytes";
        let commitment = derive_commitment(Domain::Gate2Replay, bytes);
        let tampered = b"tampered bytes!";
        assert!(!ReplayVerifier::verify_envelope_integrity(tampered, &commitment));
    }

    #[test]
    fn gate2_replay_domain_is_distinct_from_existing_domains() {
        let bytes = b"same input bytes";
        let gate2 = derive_commitment(Domain::Gate2Replay, bytes);
        let life_cert = derive_commitment(Domain::LifeCert, bytes);
        let ssr_root = derive_commitment(Domain::SsrRoot, bytes);
        assert_ne!(gate2, life_cert);
        assert_ne!(gate2, ssr_root);
    }

    #[test]
    fn commitment_wrong_domain_fails_verification() {
        // Same bytes, but computed under a different domain than the one
        // verify_envelope_integrity checks against -- must fail, proving
        // domain separation is actually load-bearing here, not decorative.
        let bytes = b"identical bytes across domains";
        let wrong_domain_commitment = derive_commitment(Domain::LifeCert, bytes);
        assert!(!ReplayVerifier::verify_envelope_integrity(bytes, &wrong_domain_commitment));
    }

    // --- Reducer tests -----------------------------------------------------

    use kaysentinel_builder::lifecycle::keys::GenerationKey;
    use kaysentinel_builder::VerifiedGeneration;
    use kaysentinel_cse::payloads::{BalanceChanged, NonceUpdated, StorageSlotUpdated};

    fn cert(address: [u8; 20], nonce: u64, balance: [u8; 32], storage_root: StorageRootState) -> CanonicalAccountCertificate {
        CanonicalAccountCertificate {
            address,
            generation: VerifiedGeneration {
                key: GenerationKey { address, generation_id: 0 },
                state_table_proof_root: [0u8; 32],
            },
            nonce,
            balance,
            code_hash: [0u8; 32],
            storage_root,
        }
    }

    fn balance(v: u8) -> [u8; 32] {
        let mut b = [0u8; 32];
        b[31] = v;
        b
    }

    #[test]
    fn correct_certificate_replays_successfully() {
        let addr = [0xAAu8; 20];
        let traces = vec![
            CsePayload::NonceUpdated(NonceUpdated { address: addr, previous_nonce: 0, current_nonce: 1 }),
            CsePayload::BalanceChanged(BalanceChanged {
                address: addr,
                previous_balance: [0u8; 32],
                current_balance: balance(100),
            }),
        ];
        let expected_root = SimpleStorageRootDeriver.derive(&(), &BTreeMap::new()).unwrap();
        let batch = ExecutionBatchReplay {
            account_certificates: vec![cert(addr, 1, balance(100), StorageRootState::Verified(expected_root))],
            execution_traces: traces,
        };
        assert_eq!(replay_and_verify(&batch), Ok(()));
    }

    #[test]
    fn wrong_balance_in_certificate_is_caught() {
        let addr = [0xBBu8; 20];
        let traces = vec![CsePayload::BalanceChanged(BalanceChanged {
            address: addr,
            previous_balance: [0u8; 32],
            current_balance: balance(100),
        })];
        let expected_root = SimpleStorageRootDeriver.derive(&(), &BTreeMap::new()).unwrap();
        let batch = ExecutionBatchReplay {
            account_certificates: vec![cert(addr, 0, balance(199), StorageRootState::Verified(expected_root))],
            execution_traces: traces,
        };
        let errors = replay_and_verify(&batch).unwrap_err();
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            ReplayError::CertificateMismatch { address, mismatches } => {
                assert_eq!(*address, addr);
                assert!(mismatches.iter().any(|m| matches!(m, FieldMismatch::Balance { .. })));
            }
            other => panic!("expected CertificateMismatch, got {other:?}"),
        }
    }

    #[test]
    fn storage_root_is_independently_rederived_and_checked() {
        let addr = [0xCCu8; 20];
        let traces = vec![CsePayload::StorageSlotUpdated(StorageSlotUpdated {
            address: addr,
            slot: [1u8; 32],
            previous_value: [0u8; 32],
            current_value: [2u8; 32],
        })];
        // Deliberately wrong claimed root -- must be caught by re-derivation,
        // not accepted just because it's present.
        let batch = ExecutionBatchReplay {
            account_certificates: vec![cert(addr, 0, [0u8; 32], StorageRootState::Verified([0xFFu8; 32]))],
            execution_traces: traces,
        };
        let errors = replay_and_verify(&batch).unwrap_err();
        assert!(errors.iter().any(|e| matches!(
            e,
            ReplayError::CertificateMismatch { mismatches, .. }
                if mismatches.iter().any(|m| matches!(m, FieldMismatch::StorageRoot { .. }))
        )));
    }

    #[test]
    fn missing_certificate_for_touched_address_is_caught() {
        let addr = [0xDDu8; 20];
        let traces = vec![CsePayload::NonceUpdated(NonceUpdated { address: addr, previous_nonce: 0, current_nonce: 1 })];
        let batch = ExecutionBatchReplay { account_certificates: vec![], execution_traces: traces };
        let errors = replay_and_verify(&batch).unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, ReplayError::MissingCertificate { address } if *address == addr)));
    }

    #[test]
    fn certificate_for_untouched_address_is_caught() {
        let addr = [0xEEu8; 20];
        let expected_root = SimpleStorageRootDeriver.derive(&(), &BTreeMap::new()).unwrap();
        let batch = ExecutionBatchReplay {
            account_certificates: vec![cert(addr, 0, [0u8; 32], StorageRootState::Verified(expected_root))],
            execution_traces: vec![],
        };
        let errors = replay_and_verify(&batch).unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, ReplayError::UntouchedCertificate { address } if *address == addr)));
    }

    #[test]
    fn inconsistent_trace_chain_is_caught_before_certificate_comparison() {
        let addr = [0x11u8; 20];
        // Second BalanceChanged's previous_balance doesn't match what the
        // first one actually left behind.
        let traces = vec![
            CsePayload::BalanceChanged(BalanceChanged {
                address: addr,
                previous_balance: [0u8; 32],
                current_balance: balance(50),
            }),
            CsePayload::BalanceChanged(BalanceChanged {
                address: addr,
                previous_balance: balance(199), // wrong -- should be 50 (state left 50, this claims 199)
                current_balance: balance(100),
            }),
        ];
        let batch = ExecutionBatchReplay { account_certificates: vec![], execution_traces: traces };
        let errors = replay_and_verify(&batch).unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, ReplayError::TraceInconsistency { address, .. } if *address == addr)));
    }

    #[test]
    fn unresolved_storage_root_is_rejected() {
        let addr = [0x22u8; 20];
        let traces = vec![CsePayload::NonceUpdated(NonceUpdated { address: addr, previous_nonce: 0, current_nonce: 1 })];
        let batch = ExecutionBatchReplay {
            account_certificates: vec![cert(addr, 1, [0u8; 32], StorageRootState::AwaitingDerivation)],
            execution_traces: traces,
        };
        let errors = replay_and_verify(&batch).unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, ReplayError::UnresolvedStorageRoot { address } if *address == addr)));
    }
}
