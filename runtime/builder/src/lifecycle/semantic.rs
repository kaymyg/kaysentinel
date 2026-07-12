use crate::ir::timeline::TraceProvenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountPreStateRelation {
    pub address: [u8; 20],
    pub existed_before_tx: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationRelation {
    pub address: [u8; 20],
    pub generation_id: u32,
    pub begin: TraceProvenance,
    pub end: Option<TraceProvenance>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticIr {
    pub pre_states: Vec<AccountPreStateRelation>,
    pub generations: Vec<GenerationRelation>,
}
