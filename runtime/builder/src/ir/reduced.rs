use std::collections::BTreeMap;
use crate::ir::timeline::{TransactionMetadata, ObservedLifecycle, ObservedLog};
use crate::traits::{ReducedTransition, ReducibleTimeline};
use crate::errors::BuilderError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountNode {
    pub address: [u8; 20],
    pub lifecycle_history: Vec<ObservedLifecycle>,
    pub balance: Option<ReducedTransition<[u8; 32]>>,
    pub nonce: Option<ReducedTransition<u64>>,
    pub code_hash: Option<ReducedTransition<[u8; 32]>>,
    pub storage: BTreeMap<[u8; 32], ReducedTransition<[u8; 32]>>,
    pub transient_storage: BTreeMap<[u8; 32], ReducedTransition<[u8; 32]>>,
    pub logs: Vec<ObservedLog>,
}

impl AccountNode {
    /// Self-contained reduction method keeping domain structures isolated.
    pub fn reduce(raw: crate::ir::timeline::AccountNode) -> Result<Self, BuilderError> {
        let balance = raw.balance_timeline.map(|t| t.reduce()).transpose()?;
        let nonce = raw.nonce_timeline.map(|t| t.reduce()).transpose()?;
        let code_hash = raw.code_timeline.map(|t| t.reduce()).transpose()?;

        let mut storage = BTreeMap::new();
        for (slot, timeline) in raw.storage_timelines {
            storage.insert(slot, timeline.reduce()?);
        }

        let mut transient_storage = BTreeMap::new();
        for (slot, timeline) in raw.transient_timelines {
            transient_storage.insert(slot, timeline.reduce()?);
        }

        Ok(Self {
            address: raw.address,
            lifecycle_history: raw.lifecycle_history,
            balance,
            nonce,
            code_hash,
            storage,
            transient_storage,
            logs: raw.logs,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionBucket {
    pub metadata: TransactionMetadata,
    pub account_nodes: BTreeMap<[u8; 20], AccountNode>,
    pub gas_refund: Option<ReducedTransition<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedIr(pub Vec<TransactionBucket>);
