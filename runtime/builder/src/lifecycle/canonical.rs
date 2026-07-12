use crate::ir::timeline::EventId;
use crate::lifecycle::keys::GenerationKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalPreState {
    pub key: GenerationKey, // generation_id pinned to 0 for initial snapshot identity
    pub existed_before_tx: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalGeneration {
    pub key: GenerationKey,
    pub begin: EventId,
    pub end: Option<EventId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalSemanticIr {
    pub pre_states: Vec<CanonicalPreState>,
    pub generations: Vec<CanonicalGeneration>,
}
