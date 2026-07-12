use crate::ir::timeline::TraceProvenance;
use crate::lifecycle::keys::GenerationKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPreState {
    pub key: GenerationKey, // generation_id pinned to 0 for initial snapshot identity
    pub existed_before_tx: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalGeneration {
    pub key: GenerationKey,
    pub begin: TraceProvenance,
    pub end: Option<TraceProvenance>,
}

/// A balance fact scoped to a specific identity generation, rather than to a bare
/// address. NOTE: nothing in the pipeline currently derives these from the real
/// `state_table` (which is still flat, address-keyed, generation-agnostic) — see
/// `canonicalize.rs` for how they're accepted, and the phase writeup for why the
/// address -> generation cross-referencing logic isn't wired up yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalBalance {
    pub key: GenerationKey,
    pub value: [u64; 4],
    pub location: TraceProvenance,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSemanticIr {
    pub pre_states: Vec<CanonicalPreState>,
    pub generations: Vec<CanonicalGeneration>,
    pub balances: Vec<CanonicalBalance>,
}
