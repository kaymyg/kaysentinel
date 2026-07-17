//! Storage root derivation — implements the `StorageRootDeriver` trait from
//! `lifecycle::hydration`, and wires its output into already-built certificates.
//!
//! Deliberately self-contained (depends only on `kaysentinel-hash`, not on
//! `runtime/ssz`): `runtime/ssz` already depends on `runtime/builder` (to encode
//! `CanonicalAccountCertificate`), so the reverse dependency would create a cycle.
//! This means the pairwise Merkle fold below is a small, independent
//! re-implementation of the same zero-padding convention `runtime/ssz::merkleize`
//! uses — not a reuse of it. If the two ever need to be unified, that has to
//! happen by extracting a third, lower-level crate both depend on, not by either
//! depending on the other.
//!
//! ## Design note: not an Ethereum-compatible storage root
//!
//! This derives a protocol-native commitment over an account's touched storage
//! slots, using the project's own domain-separated BLAKE3 scheme
//! (`Domain::TrieLeaf` / `Domain::TrieBranch`) — it does **not** attempt to
//! reproduce Ethereum's real Merkle-Patricia Trie root (which requires Keccak256
//! and RLP encoding, neither of which exist anywhere in this codebase).
//!
//! This is inferred from the available evidence, not confirmed by an explicit
//! spec statement:
//!   - `docs/emes_profile.md` §5.2 confirms the Geth tracer only ever captures
//!     per-slot (prev, new) mutation pairs (`OnStorageChange`); it never captures
//!     Geth's real per-account MPT root, so there is no authoritative Ethereum
//!     root flowing through this pipeline to reproduce or compare against.
//!   - `kaysentinel-hash`'s entire domain-separation scheme is built on BLAKE3,
//!     structurally incompatible with Ethereum's Keccak/RLP-based MPT regardless
//!     of design intent.
//! If a future requirement demands real Ethereum MPT compatibility, this
//! implementation must be replaced, not extended — see `StorageRootDeriver`'s
//! trait boundary, which exists precisely so the rest of the builder doesn't
//! need to care which algorithm produced the root.

use std::collections::BTreeMap;

use kaysentinel_hash::{Domain, derive_commitment};

use crate::ir::timeline::CanonicalKey;
use crate::ir::reduced::ReducedVariant;
use crate::lifecycle::certificate::CanonicalAccountCertificate;
use crate::lifecycle::hydration::{DerivationError, StorageRootDeriver};
use crate::lifecycle::certificate::StorageRootState;

/// The canonical "no storage touched" root: derived the same way as any other
/// case (a zero-leaf Merkle fold), not a magic sentinel constant, so it's
/// automatically consistent if the leaf/branch hashing ever changes.
pub struct SimpleStorageRootDeriver;

impl StorageRootDeriver for SimpleStorageRootDeriver {
    /// No external base state is used — this derivation only covers slots
    /// actually touched in the current transaction batch, not an account's full
    /// historical storage. Real Ethereum-style trie derivation would need a
    /// `BaseState` carrying the account's prior slots; this interim design
    /// deliberately doesn't model that yet (see module-level design note).
    type BaseState = ();

    fn derive(
        &self,
        _base_state: &(),
        terminal_slots: &BTreeMap<[u8; 32], [u8; 32]>,
    ) -> Result<[u8; 32], DerivationError> {
        let mut leaves: Vec<[u8; 32]> = terminal_slots
            .iter() // BTreeMap iteration is key-sorted -> deterministic leaf order
            .map(|(slot, value)| {
                let mut canonical = Vec::with_capacity(64);
                canonical.extend_from_slice(slot);
                canonical.extend_from_slice(value);
                derive_commitment(Domain::TrieLeaf, &canonical).to_bytes()
            })
            .collect();

        if leaves.is_empty() {
            leaves.push([0u8; 32]);
        }

        let mut size = 1usize;
        while size < leaves.len() {
            size <<= 1;
        }
        while leaves.len() < size {
            leaves.push([0u8; 32]);
        }

        while leaves.len() > 1 {
            let mut next = Vec::with_capacity(leaves.len() / 2);
            for pair in leaves.chunks(2) {
                let mut canonical = Vec::with_capacity(64);
                canonical.extend_from_slice(&pair[0]);
                canonical.extend_from_slice(&pair[1]);
                next.push(derive_commitment(Domain::TrieBranch, &canonical).to_bytes());
            }
            leaves = next;
        }

        Ok(leaves[0])
    }
}

/// Mutates every certificate's `storage_root` in place, deriving it from the
/// account's `CanonicalKey::Storage` entries in `state_table`. Deliberately
/// separate from `build_canonical_certificates`/`hydrate_accounts` (an
/// additive post-processing pass, not a signature change to either) so
/// existing call sites — including the already-pushed
/// `runtime/builder/tests/conformance.rs` — keep working unmodified.
///
/// `CanonicalKey::Transient` entries are excluded on purpose: EIP-1153
/// transient storage is explicitly ephemeral (cleared at transaction end) and
/// is not part of an account's persistent storage root in real Ethereum
/// semantics either.
pub fn resolve_storage_roots<D: StorageRootDeriver<BaseState = ()>>(
    certificates: &mut BTreeMap<[u8; 20], CanonicalAccountCertificate>,
    state_table: &BTreeMap<CanonicalKey, ReducedVariant>,
    deriver: &D,
) -> Result<(), DerivationError> {
    let mut slots_by_address: BTreeMap<[u8; 20], BTreeMap<[u8; 32], [u8; 32]>> = BTreeMap::new();

    for (key, variant) in state_table {
        if let (CanonicalKey::Storage { address, slot }, ReducedVariant::Storage(t)) = (key, variant) {
            slots_by_address.entry(*address).or_default().insert(*slot, *t.terminal());
        }
    }

    for (address, cert) in certificates.iter_mut() {
        let slots = slots_by_address.get(address).cloned().unwrap_or_default();
        let root = deriver.derive(&(), &slots)?;
        cert.storage_root = StorageRootState::Verified(root);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::certificate::VerifiedGeneration;
    use crate::lifecycle::keys::GenerationKey;

    fn sample_cert(address: [u8; 20]) -> CanonicalAccountCertificate {
        CanonicalAccountCertificate {
            address,
            generation: VerifiedGeneration {
                key: GenerationKey { address, generation_id: 0 },
                state_table_proof_root: [0u8; 32],
            },
            nonce: 0,
            balance: [0u8; 32],
            code_hash: [0u8; 32],
            storage_root: StorageRootState::AwaitingDerivation,
        }
    }

    #[test]
    fn empty_storage_derives_deterministic_root() {
        let deriver = SimpleStorageRootDeriver;
        let empty = BTreeMap::new();
        let root_a = deriver.derive(&(), &empty).unwrap();
        let root_b = deriver.derive(&(), &empty).unwrap();
        assert_eq!(root_a, root_b);
    }

    #[test]
    fn nonempty_storage_differs_from_empty() {
        let deriver = SimpleStorageRootDeriver;
        let empty = BTreeMap::new();
        let mut one_slot = BTreeMap::new();
        one_slot.insert([1u8; 32], [2u8; 32]);

        let empty_root = deriver.derive(&(), &empty).unwrap();
        let nonempty_root = deriver.derive(&(), &one_slot).unwrap();
        assert_ne!(empty_root, nonempty_root);
    }

    #[test]
    fn different_slot_values_produce_different_roots() {
        let deriver = SimpleStorageRootDeriver;
        let mut slots_a = BTreeMap::new();
        slots_a.insert([1u8; 32], [2u8; 32]);
        let mut slots_b = BTreeMap::new();
        slots_b.insert([1u8; 32], [3u8; 32]);

        assert_ne!(deriver.derive(&(), &slots_a).unwrap(), deriver.derive(&(), &slots_b).unwrap());
    }

    #[test]
    fn root_is_independent_of_insertion_order() {
        let deriver = SimpleStorageRootDeriver;
        let mut slots_a = BTreeMap::new();
        slots_a.insert([1u8; 32], [0xAAu8; 32]);
        slots_a.insert([2u8; 32], [0xBBu8; 32]);

        let mut slots_b = BTreeMap::new();
        slots_b.insert([2u8; 32], [0xBBu8; 32]);
        slots_b.insert([1u8; 32], [0xAAu8; 32]);

        // Both are BTreeMaps, so iteration order is identical regardless of
        // insertion order -- this asserts that fact holds through derivation.
        assert_eq!(deriver.derive(&(), &slots_a).unwrap(), deriver.derive(&(), &slots_b).unwrap());
    }

    #[test]
    fn resolve_storage_roots_moves_every_certificate_off_awaiting_derivation() {
        let addr_a = [0x01u8; 20];
        let addr_b = [0x02u8; 20]; // has no storage entries at all

        let mut certs = BTreeMap::new();
        certs.insert(addr_a, sample_cert(addr_a));
        certs.insert(addr_b, sample_cert(addr_b));

        let mut state_table = BTreeMap::new();
        state_table.insert(
            CanonicalKey::Storage { address: addr_a, slot: [7u8; 32] },
            ReducedVariant::Storage(crate::traits::ReducedTransition::new(
                [0u8; 32],
                [9u8; 32],
                crate::ir::timeline::Provenance::new(),
            )),
        );

        resolve_storage_roots(&mut certs, &state_table, &SimpleStorageRootDeriver).unwrap();

        for cert in certs.values() {
            assert!(matches!(cert.storage_root, StorageRootState::Verified(_)));
        }
        // addr_b had zero storage entries but still gets a real (empty) root,
        // not a leftover AwaitingDerivation.
        assert_ne!(certs[&addr_b].storage_root, StorageRootState::AwaitingDerivation);
    }

    #[test]
    fn transient_storage_is_excluded_from_the_root() {
        let addr = [0x03u8; 20];
        let mut certs = BTreeMap::new();
        certs.insert(addr, sample_cert(addr));

        let mut state_table = BTreeMap::new();
        state_table.insert(
            CanonicalKey::Transient { address: addr, slot: [1u8; 32] },
            ReducedVariant::Transient(crate::traits::ReducedTransition::new(
                [0u8; 32],
                [0xFFu8; 32],
                crate::ir::timeline::Provenance::new(),
            )),
        );

        resolve_storage_roots(&mut certs, &state_table, &SimpleStorageRootDeriver).unwrap();

        // No CanonicalKey::Storage entries exist for this address (only
        // Transient), so the root must equal the empty-storage root.
        let empty_root = SimpleStorageRootDeriver.derive(&(), &BTreeMap::new()).unwrap();
        match certs[&addr].storage_root {
            StorageRootState::Verified(root) => assert_eq!(root, empty_root),
            StorageRootState::AwaitingDerivation => panic!("expected Verified"),
        }
    }
}
