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

/// The non-mutable evidentiary artifact for a fully resolved account's canonical
/// state.
///
/// NOTE: assembling one of these for real requires nonce/balance/storage-root/
/// code-hash data cross-referenced to a specific generation — and nothing in the
/// pipeline does that cross-referencing yet (the same gap flagged since the
/// relational-IR and provenance phases: `state_table` is still flat and
/// address-keyed, not generation-scoped). This type and its fields exist so a
/// caller with that data can assemble one; the pipeline doesn't produce one
/// automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalAccountCertificate {
    pub address: [u8; 20],
    pub generation: VerifiedGeneration,
    pub nonce: u64,
    pub balance: [u64; 4],
    pub storage_root: [u8; 32],
    pub code_hash: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_prov(ord: u64) -> crate::ir::timeline::TraceProvenance {
        crate::ir::timeline::TraceProvenance {
            trace_ordinal: ord,
            tx_index: 0,
            frame_id: 0,
            call_depth: 0,
            frame_ordinal: 0,
        }
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
        // Two entries sharing the same (highest) generation_id — shouldn't occur if
        // canonicalize.rs's key-uniqueness invariant holds, but resolution must still
        // fail safely rather than silently pick one.
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
}
