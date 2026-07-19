//! # Kaysentinel Verify (Gate 2 groundwork)
//!
//! Populates the previously-empty `runtime/verify` stub with real, compiling
//! groundwork for Gate 2 (semantic replay verification): the batch artifact
//! type, and the transport-envelope integrity check. The actual replay
//! reducer (executing `CsePayload` traces and comparing the result against
//! `CanonicalAccountCertificate`) is **not implemented yet** -- this is
//! deliberately scoped to groundwork only, per the same "verify it compiles
//! before building on it" discipline used for `runtime/bridge`.

use kaysentinel_builder::CanonicalAccountCertificate;
use kaysentinel_cse::CsePayload;
use kaysentinel_hash::{derive_commitment, Digest, Domain};

/// The boundary artifact Gate 2 will eventually verify: a batch of
/// `CsePayload` execution traces alongside the `CanonicalAccountCertificate`s
/// they're claimed to produce. Deliberately holds `CsePayload` directly
/// (not `CanonicalSemanticEvent`) since a replay reducer only needs the
/// mutation content, not `ExecutionContext`/`TraceContext` -- diagnostic
/// isolation (PR1) means trace metadata shouldn't affect replay outcomes
/// anyway.
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
    /// about whether the *contents* semantically replay correctly; that's the
    /// reducer's job (§ "Real Gate 2 Execution Flow", not yet implemented).
    pub fn verify_envelope_integrity(raw_ssz_bytes: &[u8], expected_commitment: &Digest) -> bool {
        let computed = derive_commitment(Domain::Gate2Replay, raw_ssz_bytes);
        computed == *expected_commitment
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
}
