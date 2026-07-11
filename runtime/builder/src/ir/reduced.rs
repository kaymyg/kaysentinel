use std::collections::BTreeMap;
use crate::ir::timeline::{TransactionMetadata, CanonicalKey, ObservedLifecycle, ObservedLog};
use crate::traits::ReducedTransition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReducedVariant {
    Balance(ReducedTransition<[u8; 32]>),
    Nonce(ReducedTransition<u64>),
    Code(ReducedTransition<[u8; 32]>),
    Storage(ReducedTransition<[u8; 32]>),
    Transient(ReducedTransition<[u8; 32]>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedTransactionBucket {
    pub metadata: TransactionMetadata,
    pub state_table: BTreeMap<CanonicalKey, ReducedVariant>,
    pub lifecycle_table: BTreeMap<[u8; 20], Vec<ObservedLifecycle>>,
    pub log_table: BTreeMap<[u8; 20], Vec<ObservedLog>>,
    pub gas_refund: Option<ReducedTransition<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedIr(pub Vec<ReducedTransactionBucket>);
