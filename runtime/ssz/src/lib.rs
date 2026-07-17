//! # Kaysentinel SSZ
//!
//! Reference implementation of `docs/ssz_specification.md` (SSR-RC1 / SSZ-RC1 / MP1):
//! a two fixed containers (`CanonicalEventSummary`, `EpochStateSnapshot`), their
//! canonical wire encoding, and the Merkle Profile MP1 tree construction over the
//! resulting byte stream.

use kaysentinel_hash::{Domain, derive_commitment};

/// SSZ `List[CanonicalEventSummary, 16384]` bound from the spec.
pub const MAX_EVENT_SUMMARIES: usize = 16384;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SszError {
    /// A `List` field exceeded its declared maximum length.
    ListTooLong { max: usize, actual: usize },
    /// Raised when attempting to serialize a `CanonicalAccountCertificate` whose
    /// `storage_root` is still `StorageRootState::AwaitingDerivation`. Per the
    /// certificate's own doc comment ("the non-mutable evidentiary artifact for a
    /// fully resolved account's canonical state"), an unresolved storage root
    /// means the account hasn't reached a canonical state-commitment boundary yet
    /// — this is an out-of-order pipeline error, not a valid wire value.
    UnresolvedStorageRoot,
}

pub mod certificate;

/// `CanonicalEventSummary` — fixed-size container, exactly 68 bytes
/// (8 + 8 + 20 + 32): `block_timestamp`, `sequence_nonce`, `actor_address`,
/// `payload_digest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalEventSummary {
    pub block_timestamp: u64,
    pub sequence_nonce: u64,
    pub actor_address: [u8; 20],
    pub payload_digest: [u8; 32],
}

impl CanonicalEventSummary {
    pub const ENCODED_LEN: usize = 8 + 8 + 20 + 32;

    pub fn ssz_encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(Self::ENCODED_LEN);
        out.extend_from_slice(&self.block_timestamp.to_le_bytes());
        out.extend_from_slice(&self.sequence_nonce.to_le_bytes());
        out.extend_from_slice(&self.actor_address);
        out.extend_from_slice(&self.payload_digest);
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochStateSnapshot {
    pub epoch_index: u64,
    pub event_summaries: Vec<CanonicalEventSummary>,
}

impl EpochStateSnapshot {
    const FIXED_HEADER_LEN: usize = 8 + 4;

    pub fn ssz_encode(&self) -> Result<Vec<u8>, SszError> {
        if self.event_summaries.len() > MAX_EVENT_SUMMARIES {
            return Err(SszError::ListTooLong {
                max: MAX_EVENT_SUMMARIES,
                actual: self.event_summaries.len(),
            });
        }

        let mut out = Vec::with_capacity(
            Self::FIXED_HEADER_LEN + self.event_summaries.len() * CanonicalEventSummary::ENCODED_LEN,
        );
        out.extend_from_slice(&self.epoch_index.to_le_bytes());
        out.extend_from_slice(&(Self::FIXED_HEADER_LEN as u32).to_le_bytes());
        for summary in &self.event_summaries {
            out.extend_from_slice(&summary.ssz_encode());
        }
        Ok(out)
    }
}

pub fn chunkify(bytes: &[u8]) -> Vec<[u8; 32]> {
    if bytes.is_empty() {
        return vec![[0u8; 32]];
    }
    let mut chunks = Vec::with_capacity((bytes.len() + 31) / 32);
    let mut i = 0;
    while i < bytes.len() {
        let end = (i + 32).min(bytes.len());
        let mut chunk = [0u8; 32];
        chunk[..end - i].copy_from_slice(&bytes[i..end]);
        chunks.push(chunk);
        i += 32;
    }
    chunks
}

fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut canonical = Vec::with_capacity(64);
    canonical.extend_from_slice(left);
    canonical.extend_from_slice(right);
    derive_commitment(Domain::SsrRoot, &canonical).to_bytes()
}

pub fn merkleize(chunks: &[[u8; 32]]) -> [u8; 32] {
    let mut layer: Vec<[u8; 32]> = if chunks.is_empty() { vec![[0u8; 32]] } else { chunks.to_vec() };

    let mut size = 1usize;
    while size < layer.len() {
        size <<= 1;
    }
    while layer.len() < size {
        layer.push([0u8; 32]);
    }

    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len() / 2);
        for pair in layer.chunks(2) {
            next.push(hash_pair(&pair[0], &pair[1]));
        }
        layer = next;
    }
    layer[0]
}

pub fn ssr_root(serialized_bytes: &[u8]) -> [u8; 32] {
    merkleize(&chunkify(serialized_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_summary() -> CanonicalEventSummary {
        CanonicalEventSummary {
            block_timestamp: 171717,
            sequence_nonce: 1,
            actor_address: [0xaa; 20],
            payload_digest: [0xff; 32],
        }
    }

    #[test]
    fn canonical_event_summary_is_exactly_68_bytes() {
        let encoded = sample_summary().ssz_encode();
        assert_eq!(encoded.len(), CanonicalEventSummary::ENCODED_LEN);
        assert_eq!(encoded.len(), 68);
    }

    #[test]
    fn epoch_state_snapshot_matches_correctly_computed_vector() {
        let snapshot = EpochStateSnapshot { epoch_index: 5, event_summaries: vec![sample_summary()] };
        let encoded = snapshot.ssz_encode().unwrap();

        assert_eq!(encoded.len(), 80);
        assert_eq!(
            hex_encode(&encoded),
            "05000000000000000c000000c59e0200000000000100000000000000aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
        );
    }

    #[test]
    fn epoch_state_snapshot_rejects_oversized_list() {
        let too_many = vec![sample_summary(); MAX_EVENT_SUMMARIES + 1];
        let snapshot = EpochStateSnapshot { epoch_index: 0, event_summaries: too_many };
        assert_eq!(
            snapshot.ssz_encode(),
            Err(SszError::ListTooLong { max: MAX_EVENT_SUMMARIES, actual: MAX_EVENT_SUMMARIES + 1 })
        );
    }

    #[test]
    fn chunkify_pads_final_chunk_with_zeros() {
        let bytes = vec![1u8, 2, 3];
        let chunks = chunkify(&bytes);
        assert_eq!(chunks.len(), 1);
        assert_eq!(&chunks[0][..3], &[1, 2, 3]);
        assert!(chunks[0][3..].iter().all(|&b| b == 0));
    }

    #[test]
    fn chunkify_exact_multiple_has_no_padding_chunk() {
        let bytes = vec![7u8; 64];
        let chunks = chunkify(&bytes);
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn merkleize_is_deterministic() {
        let chunks = chunkify(&[1, 2, 3, 4, 5]);
        assert_eq!(merkleize(&chunks), merkleize(&chunks));
    }

    #[test]
    fn merkleize_differs_for_different_input() {
        let a = merkleize(&chunkify(&[1, 2, 3]));
        let b = merkleize(&chunkify(&[1, 2, 4]));
        assert_ne!(a, b);
    }

    #[test]
    fn merkleize_single_chunk_equals_the_chunk_itself() {
        let chunks = chunkify(&[9u8; 10]);
        assert_eq!(chunks.len(), 1);
        assert_eq!(merkleize(&chunks), chunks[0]);
    }

    #[test]
    fn ssr_root_for_vec_ssr_rc1_001_is_pinned() {
        let snapshot = EpochStateSnapshot { epoch_index: 5, event_summaries: vec![sample_summary()] };
        let encoded = snapshot.ssz_encode().unwrap();
        let root = ssr_root(&encoded);
        assert_eq!(
            hex_encode(&root),
            "84fc188deeb381f7d1ac8454af2d21e37c8891112f6c37b4977e7d1c9b80820d"
        );
    }

    fn hex_encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
