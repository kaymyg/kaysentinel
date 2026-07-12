pub mod canonical;
pub mod canonicalize;
pub mod diagnostics;
pub mod keys;
pub mod resolve;
pub mod semantic;

use std::collections::BTreeMap;

use crate::ir::reduced::ReducedIr;
use canonical::CanonicalSemanticIr;
use diagnostics::PassOutput;

/// Runs lifecycle resolution (Phase 3) followed by canonicalization (Phase 4) over
/// every transaction bucket in a `ReducedIr`, in order. `pre_states` is an optional
/// externally-supplied "did this address exist before the transaction" map — see
/// `resolve::resolve` for why this isn't sourced from the pipeline itself yet.
pub fn process(
    ir: &ReducedIr,
    pre_states: &BTreeMap<[u8; 20], bool>,
) -> Vec<PassOutput<CanonicalSemanticIr>> {
    ir.0
        .iter()
        .map(|bucket| {
            let semantic_pass = resolve::resolve(&bucket.lifecycle_table, pre_states);
            let mut canonical_pass = canonicalize::canonicalize(&semantic_pass.value);
            canonical_pass.diagnostics.extend(semantic_pass.diagnostics);
            canonical_pass
        })
        .collect()
}
