//! # Kaysentinel Hash
//!
//! Reference implementation of `SPECIFICATION.md` (Normative Cryptographic Protocol,
//! RC1): domain-separated BLAKE3 commitments over canonical byte sequences.
//!
//! `Digest := BLAKE3(DomainBytes || CanonicalBytes)` — unkeyed BLAKE3, no
//! `derive_key` mode, no XOF, no padding or framing bytes between the domain
//! identifier and the canonical serialization.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigestError {
    InvalidLength,
}

impl std::fmt::Display for DigestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLength => write!(f, "Invalid digest length: must be exactly 32 bytes"),
        }
    }
}

impl std::error::Error for DigestError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest([u8; 32]);

impl Digest {
    /// Creates a Digest wrapper from an explicit 32-byte array.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the raw 32-byte digest.
    pub const fn to_bytes(&self) -> [u8; 32] {
        self.0
    }

    /// Lowercase hex encoding of the digest, matching the vector-file convention.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

impl AsRef<[u8]> for Digest {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl std::borrow::Borrow<[u8]> for Digest {
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; 32]> for Digest {
    fn from(bytes: [u8; 32]) -> Self {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<&[u8]> for Digest {
    type Error = DigestError;

    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() == 32 {
            let mut bytes = [0u8; 32];
            bytes.copy_from_slice(slice);
            Ok(Self::from_bytes(bytes))
        } else {
            Err(DigestError::InvalidLength)
        }
    }
}

/// Hash Domain Registry (Version: 1). Existing tags are permanently immutable once
/// published — new domains may be appended, but nothing here may be renamed,
/// reinterpreted, or recycled.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    TrieLeaf,
    TrieBranch,
    LifeCert,
    SsrRoot,
}

impl Domain {
    pub const fn as_bytes(&self) -> &'static [u8] {
        match self {
            Domain::TrieLeaf => b"KAY_TRIE_LEAF",
            Domain::TrieBranch => b"KAY_TRIE_BRANCH",
            Domain::LifeCert => b"KAY_LIFE_CERT",
            Domain::SsrRoot => b"KAY_SSR_ROOT",
        }
    }
}

/// Total function that computes the cryptographic commitment per the specification:
/// BLAKE3(DomainBytes || CanonicalBytes). Infallible once `canonical_bytes` exists —
/// there is no failure mode once you have valid input bytes.
pub fn derive_commitment(domain: Domain, canonical_bytes: &[u8]) -> Digest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain.as_bytes());
    hasher.update(canonical_bytes);
    Digest::from_bytes(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commitment_is_deterministic() {
        let bytes = [1u8, 2, 3, 4];
        let a = derive_commitment(Domain::TrieLeaf, &bytes);
        let b = derive_commitment(Domain::TrieLeaf, &bytes);
        assert_eq!(a, b);
    }

    #[test]
    fn different_domains_produce_different_digests() {
        let bytes = [1u8, 2, 3, 4];
        let leaf = derive_commitment(Domain::TrieLeaf, &bytes);
        let branch = derive_commitment(Domain::TrieBranch, &bytes);
        assert_ne!(leaf, branch);
    }

    #[test]
    fn different_canonical_bytes_produce_different_digests() {
        let a = derive_commitment(Domain::LifeCert, &[1, 2, 3]);
        let b = derive_commitment(Domain::LifeCert, &[1, 2, 4]);
        assert_ne!(a, b);
    }

    #[test]
    fn digest_try_from_rejects_wrong_length() {
        let short = [0u8; 16];
        assert_eq!(Digest::try_from(&short[..]), Err(DigestError::InvalidLength));

        let exact = [0u8; 32];
        assert!(Digest::try_from(&exact[..]).is_ok());
    }

    #[test]
    fn digest_round_trips_through_bytes() {
        let bytes = [0xABu8; 32];
        let digest = Digest::from_bytes(bytes);
        assert_eq!(digest.to_bytes(), bytes);
    }

    /// Computes the real BLAKE3 digest for the candidate vector in
    /// `vectors/candidate.json` (domain = KAY_TRIE_LEAF, canonical_bytes = 01020304)
    /// and prints it, so the actual output can be copied into `vectors/normative.json`
    /// rather than trusting a hand-typed value. Run with `cargo test -- --nocapture`.
    #[test]
    fn print_candidate_vector_1_digest() {
        let canonical_bytes = [0x01u8, 0x02, 0x03, 0x04];
        let digest = derive_commitment(Domain::TrieLeaf, &canonical_bytes);
        println!("candidate_vector_1 digest_hex = {}", digest.to_hex());
        // 32 bytes -> 64 hex characters, always.
        assert_eq!(digest.to_hex().len(), 64);
    }

    /// Regression pin for candidate_vector_1. The doc that specified this vector
    /// claimed a digest_hex value that was actually 66 hex characters long — an
    /// impossible length for a 32-byte BLAKE3 digest, so it was never real. This
    /// pins the value this implementation actually computes, so any future change
    /// to the hashing logic (or the blake3 dependency version) that silently
    /// changes output is caught immediately.
    #[test]
    fn candidate_vector_1_matches_pinned_digest() {
        let canonical_bytes = [0x01u8, 0x02, 0x03, 0x04];
        let digest = derive_commitment(Domain::TrieLeaf, &canonical_bytes);
        assert_eq!(
            digest.to_hex(),
            "80522501585b8ebf1831439e57add0002d22fe75521120104b000cc707b5a34a"
        );
    }

    /// One pinned regression vector per registered domain, all against the same
    /// canonical bytes — proves domain separation holds and gives every domain a
    /// real, computed (not hand-typed) reference value.
    #[test]
    fn per_domain_vectors_are_pinned_and_distinct() {
        let canonical_bytes = [0x01u8, 0x02, 0x03, 0x04];
        let leaf = derive_commitment(Domain::TrieLeaf, &canonical_bytes).to_hex();
        let branch = derive_commitment(Domain::TrieBranch, &canonical_bytes).to_hex();
        let cert = derive_commitment(Domain::LifeCert, &canonical_bytes).to_hex();
        let ssr = derive_commitment(Domain::SsrRoot, &canonical_bytes).to_hex();

        println!("leaf   = {leaf}");
        println!("branch = {branch}");
        println!("cert   = {cert}");
        println!("ssr    = {ssr}");

        assert_eq!(leaf, "80522501585b8ebf1831439e57add0002d22fe75521120104b000cc707b5a34a");
        assert_eq!(branch, "51614b8a4a5f5a2b5b7b858e626cd19b710d670ebad2e0b9b296bf282b4bfe93");
        assert_eq!(cert, "59fd735ed0c0923c4316ad197223aed69b6667f6a00a228aae1638a521d1f62f");
        assert_eq!(ssr, "1007c1368120e75ff7f8a47d6933f1440e1a6e0fd1b5e10f8721329dd5d47617");

        let all = [&leaf, &branch, &cert, &ssr];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j], "domain separation violated between vectors {i} and {j}");
            }
        }
    }
}
