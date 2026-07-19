//! SSZ decoding -- the mirror of the three encoders in this crate
//! (`CanonicalEventSummary::ssz_encode`, `EpochStateSnapshot::ssz_encode`,
//! and `SszEncodeCertificate::ssz_encode`), closing the gap flagged during
//! Gate 2 groundwork: encode existed everywhere, decode existed nowhere, so
//! an `ExecutionBatchReplay` could only be constructed in memory, never
//! ingested from bytes.
//!
//! Every decoder is written against the corresponding encoder's exact byte
//! layout as implemented (not as separately specified), and each is proven
//! by round-trip tests: `decode(encode(x)) == x` for valid values, plus
//! explicit rejection tests for truncated, oversized, and trailing-garbage
//! inputs.

use std::convert::TryInto;

use kaysentinel_builder::lifecycle::keys::GenerationKey;
use kaysentinel_builder::{CanonicalAccountCertificate, VerifiedGeneration};
use kaysentinel_builder::lifecycle::certificate::StorageRootState;

use crate::certificate::CERTIFICATE_ENCODED_LEN;
use crate::{CanonicalEventSummary, EpochStateSnapshot, MAX_EVENT_SUMMARIES};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SszDecodeError {
    /// Input shorter than the type's minimum/exact encoded length.
    Truncated { expected_at_least: usize, actual: usize },
    /// Input longer than the decoded content accounts for -- trailing bytes
    /// are rejected rather than silently ignored, so two different byte
    /// strings can never decode to the same value.
    TrailingBytes { consumed: usize, actual: usize },
    /// `EpochStateSnapshot`'s offset field didn't have its one legal value.
    InvalidOffset { expected: u32, found: u32 },
    /// The list body length wasn't an exact multiple of the element size.
    MisalignedList { body_len: usize, element_len: usize },
    /// More elements than the declared SSZ list bound.
    ListTooLong { max: usize, actual: usize },
}

fn read_u64_le(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes[..8].try_into().unwrap())
}

fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[..4].try_into().unwrap())
}

impl CanonicalEventSummary {
    /// Exact inverse of `ssz_encode`: requires exactly `ENCODED_LEN` bytes.
    pub fn ssz_decode(bytes: &[u8]) -> Result<Self, SszDecodeError> {
        if bytes.len() < Self::ENCODED_LEN {
            return Err(SszDecodeError::Truncated { expected_at_least: Self::ENCODED_LEN, actual: bytes.len() });
        }
        if bytes.len() > Self::ENCODED_LEN {
            return Err(SszDecodeError::TrailingBytes { consumed: Self::ENCODED_LEN, actual: bytes.len() });
        }
        let mut actor_address = [0u8; 20];
        actor_address.copy_from_slice(&bytes[16..36]);
        let mut payload_digest = [0u8; 32];
        payload_digest.copy_from_slice(&bytes[36..68]);
        Ok(Self {
            block_timestamp: read_u64_le(&bytes[0..8]),
            sequence_nonce: read_u64_le(&bytes[8..16]),
            actor_address,
            payload_digest,
        })
    }
}

impl EpochStateSnapshot {
    const FIXED_HEADER: usize = 8 + 4;

    /// Exact inverse of `ssz_encode`. Enforces the same invariants the
    /// encoder produces: the offset must be exactly `FIXED_HEADER` (12), the
    /// body must be a whole number of 68-byte elements, and the element
    /// count must respect `MAX_EVENT_SUMMARIES`.
    pub fn ssz_decode(bytes: &[u8]) -> Result<Self, SszDecodeError> {
        if bytes.len() < Self::FIXED_HEADER {
            return Err(SszDecodeError::Truncated { expected_at_least: Self::FIXED_HEADER, actual: bytes.len() });
        }
        let epoch_index = read_u64_le(&bytes[0..8]);
        let offset = read_u32_le(&bytes[8..12]);
        if offset != Self::FIXED_HEADER as u32 {
            return Err(SszDecodeError::InvalidOffset { expected: Self::FIXED_HEADER as u32, found: offset });
        }

        let body = &bytes[Self::FIXED_HEADER..];
        let elem = CanonicalEventSummary::ENCODED_LEN;
        if body.len() % elem != 0 {
            return Err(SszDecodeError::MisalignedList { body_len: body.len(), element_len: elem });
        }
        let count = body.len() / elem;
        if count > MAX_EVENT_SUMMARIES {
            return Err(SszDecodeError::ListTooLong { max: MAX_EVENT_SUMMARIES, actual: count });
        }

        let mut event_summaries = Vec::with_capacity(count);
        for chunk in body.chunks_exact(elem) {
            event_summaries.push(CanonicalEventSummary::ssz_decode(chunk)?);
        }
        Ok(Self { epoch_index, event_summaries })
    }
}

/// Exact inverse of `SszEncodeCertificate::ssz_encode`'s 180-byte layout.
///
/// Round-trip caveat, stated honestly rather than papered over: the encoder
/// refuses `StorageRootState::AwaitingDerivation` (there is no wire value
/// for it), so every decoded certificate necessarily has a `Verified`
/// storage root -- `decode(encode(x)) == x` holds for every *encodable* x,
/// which is exactly the set of certificates with resolved roots.
pub fn ssz_decode_certificate(bytes: &[u8]) -> Result<CanonicalAccountCertificate, SszDecodeError> {
    if bytes.len() < CERTIFICATE_ENCODED_LEN {
        return Err(SszDecodeError::Truncated { expected_at_least: CERTIFICATE_ENCODED_LEN, actual: bytes.len() });
    }
    if bytes.len() > CERTIFICATE_ENCODED_LEN {
        return Err(SszDecodeError::TrailingBytes { consumed: CERTIFICATE_ENCODED_LEN, actual: bytes.len() });
    }

    let mut address = [0u8; 20];
    address.copy_from_slice(&bytes[0..20]);
    let mut gen_address = [0u8; 20];
    gen_address.copy_from_slice(&bytes[20..40]);
    let generation_id = read_u32_le(&bytes[40..44]);
    let mut state_table_proof_root = [0u8; 32];
    state_table_proof_root.copy_from_slice(&bytes[44..76]);
    let nonce = read_u64_le(&bytes[76..84]);
    let mut balance = [0u8; 32];
    balance.copy_from_slice(&bytes[84..116]);
    let mut code_hash = [0u8; 32];
    code_hash.copy_from_slice(&bytes[116..148]);
    let mut storage_root = [0u8; 32];
    storage_root.copy_from_slice(&bytes[148..180]);

    Ok(CanonicalAccountCertificate {
        address,
        generation: VerifiedGeneration {
            key: GenerationKey { address: gen_address, generation_id },
            state_table_proof_root,
        },
        nonce,
        balance,
        code_hash,
        storage_root: StorageRootState::Verified(storage_root),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::certificate::SszEncodeCertificate;

    fn sample_summary() -> CanonicalEventSummary {
        CanonicalEventSummary {
            block_timestamp: 171717,
            sequence_nonce: 7,
            actor_address: [0xABu8; 20],
            payload_digest: [0xCDu8; 32],
        }
    }

    fn sample_certificate() -> CanonicalAccountCertificate {
        let address = [0x42u8; 20];
        let mut balance = [0u8; 32];
        balance[31] = 99;
        CanonicalAccountCertificate {
            address,
            generation: VerifiedGeneration {
                key: GenerationKey { address, generation_id: 3 },
                state_table_proof_root: [0x01u8; 32],
            },
            nonce: 12,
            balance,
            code_hash: [0xEEu8; 32],
            storage_root: StorageRootState::Verified([0x77u8; 32]),
        }
    }

    #[test]
    fn event_summary_round_trips() {
        let original = sample_summary();
        let encoded = original.ssz_encode();
        let decoded = CanonicalEventSummary::ssz_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn event_summary_rejects_truncated_and_trailing() {
        let encoded = sample_summary().ssz_encode();
        assert!(matches!(
            CanonicalEventSummary::ssz_decode(&encoded[..encoded.len() - 1]),
            Err(SszDecodeError::Truncated { .. })
        ));
        let mut too_long = encoded.clone();
        too_long.push(0);
        assert!(matches!(
            CanonicalEventSummary::ssz_decode(&too_long),
            Err(SszDecodeError::TrailingBytes { .. })
        ));
    }

    #[test]
    fn snapshot_round_trips_empty_and_nonempty() {
        for count in [0usize, 1, 3] {
            let original = EpochStateSnapshot {
                epoch_index: 5,
                event_summaries: vec![sample_summary(); count],
            };
            let encoded = original.ssz_encode().unwrap();
            let decoded = EpochStateSnapshot::ssz_decode(&encoded).unwrap();
            assert_eq!(decoded, original, "round-trip failed for {count} elements");
        }
    }

    #[test]
    fn snapshot_rejects_bad_offset_and_misaligned_body() {
        let mut encoded = EpochStateSnapshot { epoch_index: 1, event_summaries: vec![sample_summary()] }
            .ssz_encode()
            .unwrap();

        // Corrupt the offset field.
        let mut bad_offset = encoded.clone();
        bad_offset[8] = 0xFF;
        assert!(matches!(
            EpochStateSnapshot::ssz_decode(&bad_offset),
            Err(SszDecodeError::InvalidOffset { .. })
        ));

        // Chop one byte off the body -- no longer a whole element.
        encoded.pop();
        assert!(matches!(
            EpochStateSnapshot::ssz_decode(&encoded),
            Err(SszDecodeError::MisalignedList { .. })
        ));
    }

    #[test]
    fn certificate_round_trips() {
        let original = sample_certificate();
        let encoded = original.ssz_encode().unwrap();
        let decoded = ssz_decode_certificate(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn certificate_rejects_wrong_length() {
        let encoded = sample_certificate().ssz_encode().unwrap();
        assert!(matches!(
            ssz_decode_certificate(&encoded[..100]),
            Err(SszDecodeError::Truncated { .. })
        ));
        let mut too_long = encoded.to_vec();
        too_long.push(0);
        assert!(matches!(
            ssz_decode_certificate(&too_long),
            Err(SszDecodeError::TrailingBytes { .. })
        ));
    }

    #[test]
    fn decoded_certificate_preserves_generation_and_root_fields() {
        // The certificate's nested/duplicated fields (generation.key.address,
        // generation_id, state_table_proof_root) are the most likely to be
        // silently dropped by a lazy decoder -- pin each one explicitly.
        let original = sample_certificate();
        let decoded = ssz_decode_certificate(&original.ssz_encode().unwrap()).unwrap();
        assert_eq!(decoded.generation.key.address, original.address);
        assert_eq!(decoded.generation.key.generation_id, 3);
        assert_eq!(decoded.generation.state_table_proof_root, [0x01u8; 32]);
        assert_eq!(decoded.storage_root, StorageRootState::Verified([0x77u8; 32]));
    }
}
