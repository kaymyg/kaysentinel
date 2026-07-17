pub mod canonical;
pub mod canonicalize;
pub mod certificate;
pub mod diagnostics;
pub mod hydration;
pub mod keys;
pub mod resolve;
pub mod semantic;
pub mod storage_root;

use std::collections::BTreeMap;

use crate::ir::reduced::ReducedIr;
use canonical::{CanonicalGeneration, CanonicalPreState, CanonicalSemanticIr};
use canonicalize::CanonicalizationPipeline;
use diagnostics::PassOutput;
use keys::GenerationKey;

/// Runs lifecycle resolution (Phase 3) followed by canonicalization (Phase 4) over
/// every transaction bucket in a `ReducedIr`, in order. `pre_states` is an optional
/// externally-supplied "did this address exist before the transaction" map — see
/// `resolve::resolve` for why this isn't sourced from the pipeline itself yet.
///
/// Balance facts are not threaded through here yet — see `canonicalize.rs` for why.
pub fn process(
    ir: &ReducedIr,
    pre_states: &BTreeMap<[u8; 20], bool>,
) -> Vec<PassOutput<CanonicalSemanticIr>> {
    ir.0
        .iter()
        .map(|bucket| {
            let semantic_pass = resolve::resolve(&bucket.lifecycle_table, pre_states);

            let canonical_pre_states: Vec<CanonicalPreState> = semantic_pass
                .value
                .pre_states
                .iter()
                .map(|p| CanonicalPreState {
                    key: GenerationKey { address: p.address, generation_id: 0 },
                    existed_before_tx: p.existed_before_tx,
                })
                .collect();

            let canonical_generations: Vec<CanonicalGeneration> = semantic_pass
                .value
                .generations
                .iter()
                .map(|g| CanonicalGeneration {
                    key: GenerationKey { address: g.address, generation_id: g.generation_id },
                    begin: g.begin,
                    end: g.end,
                })
                .collect();

            let mut canonical_pass = CanonicalizationPipeline::process(
                &canonical_pre_states,
                &canonical_generations,
                &[],
            );
            canonical_pass.diagnostics.extend(semantic_pass.diagnostics);
            canonical_pass
        })
        .collect()
}
