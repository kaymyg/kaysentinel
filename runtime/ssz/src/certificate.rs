//! SSZ encoding for `kaysentinel_builder::CanonicalAccountCertificate`.
//!
//! `CanonicalAccountCertificate` is defined in `runtime/builder`, not this crate,
//! so it cannot receive an inherent `impl` here (Rust E0116 — verified directly:
//! attempting `impl CanonicalAccountCertificate { ... }` in this crate fails to
//! compile). Instead this module defines a local trait, `SszEncodeCertificate`,
//! and implements it for the foreign type — legal because the trait, not the
//! type, is local to this crate.

use std::collections::BTreeMap;

use kaysentinel_builder::{CanonicalAccountCertificate, StorageRootState};
use kaysentinel_hash::{Domain, Digest, derive_commitment};

use crate::{SszError, chunkify, merkleize};

/// Total serialized length of a fully resolved certificate:
/// 20 (address) + 20 (generation.key.address) + 4 (generation.key.generation_id)
/// + 32 (generation.state_table_proof_root) + 8 (nonce) + 32 (balance)
/// + 32 (code_hash) + 32 (storage_root) = 180 bytes.
///
/// `generation.key.address` duplicates the top-level `address` field in-memory;
/// this encoding preserves that duplication rather than flattening it, for exact
/// structural fidelity with the source type. `state_table_proof_root` is, as of
/// this writing, always `[0u8; 32]` in every certificate the builder produces —
/// encoding it as a real field costs 32 constant bytes today, but keeps the wire
/// format stable once that field is actually derived.
pub const CERTIFICATE_ENCODED_LEN: usize = 20 + 20 + 4 + 32 + 8 + 32 + 32 + 32;

pub trait SszEncodeCertificate {
    /// Serializes a fully resolved certificate into its canonical byte form.
    ///
    /// Returns `SszError::UnresolvedStorageRoot` if `storage_root` is still
    /// `StorageRootState::AwaitingDerivation` — per the certificate's own doc
    /// comment, an unresolved storage root means the account hasn't reached a
    /// canonical state-commitment boundary, so there is no valid wire value for
    /// it yet.
    fn ssz_encode(&self) -> Result<[u8; CERTIFICATE_ENCODED_LEN], SszError>;

    /// Computes the domain-separated BLAKE3 commitment over the certificate's
    /// canonical serialized bytes, using `Domain::LifeCert` (already reserved
    /// for exactly this purpose in `kaysentinel-hash`'s domain registry).
    fn commit_root(&self) -> Result<Digest, SszError>;
}

impl SszEncodeCertificate for CanonicalAccountCertificate {
    fn ssz_encode(&self) -> Result<[u8; CERTIFICATE_ENCODED_LEN], SszError> {
        let storage_root = match self.storage_root {
            StorageRootState::Verified(root) => root,
            StorageRootState::AwaitingDerivation => return Err(SszError::UnresolvedStorageRoot),
        };

        let mut out = [0u8; CERTIFICATE_ENCODED_LEN];
        let mut offset = 0;

        out[offset..offset + 20].copy_from_slice(&self.address);
        offset += 20;

        out[offset..offset + 20].copy_from_slice(&self.generation.key.address);
        offset += 20;

        out[offset..offset + 4].copy_from_slice(&self.generation.key.generation_id.to_le_bytes());
        offset += 4;

        out[offset..offset + 32].copy_from_slice(&self.generation.state_table_proof_root);
        offset += 32;

        out[offset..offset + 8].copy_from_slice(&self.nonce.to_le_bytes());
        offset += 8;

        out[offset..offset + 32].copy_from_slice(&self.balance);
        offset += 32;

        out[offset..offset + 32].copy_from_slice(&self.code_hash);
        offset += 32;

        out[offset..offset + 32].copy_from_slice(&storage_root);
        offset += 32;

        debug_assert_eq!(offset, CERTIFICATE_ENCODED_LEN);
        Ok(out)
    }

    fn commit_root(&self) -> Result<Digest, SszError> {
        let bytes = self.ssz_encode()?;
        Ok(derive_commitment(Domain::LifeCert, &bytes))
    }
}

/// Computes the Merkle root (per Merkle Profile MP1, reusing `chunkify`/`merkleize`
/// from this crate) over an ordered batch of certificates. `BTreeMap` iteration is
/// deterministically key-ordered, so the resulting chunk sequence — and therefore
/// the root — is reproducible across runs without any extra sorting step.
///
/// Fails on the first certificate with an unresolved storage root, rather than
/// silently skipping it — a partial commitment over an incomplete certificate set
/// would be worse than no commitment at all.
pub fn commit_certificate_batch(
    certificates: &BTreeMap<[u8; 20], CanonicalAccountCertificate>,
) -> Result<[u8; 32], SszError> {
    let mut flat_chunks = Vec::new();

    for cert in certificates.values() {
        let bytes = cert.ssz_encode()?;
        flat_chunks.extend(chunkify(&bytes));
    }

    Ok(merkleize(&flat_chunks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaysentinel_builder::VerifiedGeneration;
    use kaysentinel_builder::lifecycle::keys::GenerationKey;

    fn sample_certificate(address: [u8; 20], storage_root: StorageRootState) -> CanonicalAccountCertificate {
        let mut balance = [0u8; 32];
        balance[31] = 100;
        CanonicalAccountCertificate {
            address,
            generation: VerifiedGeneration {
                key: GenerationKey { address, generation_id: 0 },
                state_table_proof_root: [0u8; 32],
            },
            nonce: 7,
            balance,
            code_hash: [0xCCu8; 32],
            storage_root,
        }
    }

    #[test]
    fn encoded_length_is_exactly_180_bytes() {
        let cert = sample_certificate([0x11u8; 20], StorageRootState::Verified([0x22u8; 32]));
        let encoded = cert.ssz_encode().unwrap();
        assert_eq!(encoded.len(), 180);
        assert_eq!(encoded.len(), CERTIFICATE_ENCODED_LEN);
    }

    #[test]
    fn awaiting_derivation_is_rejected() {
        let cert = sample_certificate([0x11u8; 20], StorageRootState::AwaitingDerivation);
        assert_eq!(cert.ssz_encode(), Err(SszError::UnresolvedStorageRoot));
        assert_eq!(cert.commit_root(), Err(SszError::UnresolvedStorageRoot));
    }

    #[test]
    fn encoding_round_trips_field_values() {
        let address = [0xAAu8; 20];
        let cert = sample_certificate(address, StorageRootState::Verified([0x99u8; 32]));
        let encoded = cert.ssz_encode().unwrap();

        assert_eq!(&encoded[0..20], &address);
        assert_eq!(&encoded[20..40], &address); // duplicated generation.key.address
        assert_eq!(&encoded[40..44], &0u32.to_le_bytes()); // generation_id
        assert_eq!(&encoded[44..76], &[0u8; 32]); // state_table_proof_root placeholder
        assert_eq!(&encoded[76..84], &7u64.to_le_bytes()); // nonce
        assert_eq!(&encoded[116..148], &[0xCCu8; 32]); // code_hash
        assert_eq!(&encoded[148..180], &[0x99u8; 32]); // storage_root
    }

    #[test]
    fn different_certificates_produce_different_commitments() {
        let cert_a = sample_certificate([0x01u8; 20], StorageRootState::Verified([0x02u8; 32]));
        let cert_b = sample_certificate([0x01u8; 20], StorageRootState::Verified([0x03u8; 32]));
        assert_ne!(cert_a.commit_root().unwrap(), cert_b.commit_root().unwrap());
    }

    #[test]
    fn commitment_is_deterministic() {
        let cert = sample_certificate([0x05u8; 20], StorageRootState::Verified([0x06u8; 32]));
        assert_eq!(cert.commit_root().unwrap(), cert.commit_root().unwrap());
    }

    #[test]
    fn batch_commit_fails_on_any_unresolved_certificate() {
        let mut certs = BTreeMap::new();
        certs.insert([0x01u8; 20], sample_certificate([0x01u8; 20], StorageRootState::Verified([0u8; 32])));
        certs.insert([0x02u8; 20], sample_certificate([0x02u8; 20], StorageRootState::AwaitingDerivation));

        assert_eq!(commit_certificate_batch(&certs), Err(SszError::UnresolvedStorageRoot));
    }

    #[test]
    fn batch_commit_is_deterministic_regardless_of_insertion_order() {
        let mut certs_a = BTreeMap::new();
        certs_a.insert([0x01u8; 20], sample_certificate([0x01u8; 20], StorageRootState::Verified([0xAAu8; 32])));
        certs_a.insert([0x02u8; 20], sample_certificate([0x02u8; 20], StorageRootState::Verified([0xBBu8; 32])));

        let mut certs_b = BTreeMap::new();
        certs_b.insert([0x02u8; 20], sample_certificate([0x02u8; 20], StorageRootState::Verified([0xBBu8; 32])));
        certs_b.insert([0x01u8; 20], sample_certificate([0x01u8; 20], StorageRootState::Verified([0xAAu8; 32])));

        assert_eq!(commit_certificate_batch(&certs_a).unwrap(), commit_certificate_batch(&certs_b).unwrap());
    }

    /// Golden vector: pins the real, computed encoding and commitment for a fixed
    /// certificate, so any future change to the layout or hashing logic that
    /// silently alters output is caught immediately. Values below were printed via
    /// `cargo test -- --nocapture` and copied from actual output, not hand-typed.
    #[test]
    fn golden_vector_encoding_and_commitment() {
        let address = [0x42u8; 20];
        let cert = sample_certificate(address, StorageRootState::Verified([0x77u8; 32]));

        let encoded = cert.ssz_encode().unwrap();
        let commitment = cert.commit_root().unwrap();

        println!("golden_vector encoded_hex = {}", hex_encode(&encoded));
        println!("golden_vector commit_hex  = {}", commitment.to_hex());

        assert_eq!(
            hex_encode(&encoded),
            "4242424242424242424242424242424242424242424242424242424242424242424242424242424200000000000000000000000000000000000000000000000000000000000000000000000007000000000000000000000000000000000000000000000000000000000000000000000000000064cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc7777777777777777777777777777777777777777777777777777777777777777"
        );
        assert_eq!(
            commitment.to_hex(),
            "2900460538b2f458aaf17d71bd414d342a8002c9ec6607d04558c1011b5bdf9c"
        );
    }

    fn hex_encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
