use std::collections::{BTreeMap, BTreeSet};
use kaysentinel_cse::payloads::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance(pub BTreeSet<EventId>);

impl Provenance {
    pub fn new() -> Self {
        Self(BTreeSet::new())
    }
    pub fn record(&mut self, id: EventId) {
        self.0.insert(id);
    }
}

// --- The Core Abstraction: Pure Fact Recording ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTransition<T: Clone + PartialEq> {
    pub before: T,
    pub after: T,
    pub event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedLifecycle {
    pub payload: CseLifecyclePayload,
    pub event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CseLifecyclePayload {
    Created(ContractCreated),
    Destroyed(ContractDestroyed),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedLog {
    pub payload: LogEmitted,
    pub event_id: EventId,
}

// --- Timelines (Lossless Ingestion Targets) ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Timeline<T: Clone + PartialEq> {
    pub history: Vec<ObservedTransition<T>>,
}

impl<T: Clone + PartialEq> Timeline<T> {
    pub fn new() -> Self {
        Self { history: Vec::new() }
    }
    pub fn push(&mut self, before: T, after: T, event_id: EventId) {
        self.history.push(ObservedTransition { before, after, event_id });
    }
}

// --- The Structured Unoptimized Account Node ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountNode {
    pub address: [u8; 20],
    pub lifecycle_history: Vec<ObservedLifecycle>,
    pub balance_timeline: Timeline<[u8; 32]>,
    pub nonce_timeline: Timeline<u64>,
    pub code_timeline: Timeline<[u8; 32]>,
    pub storage_timelines: BTreeMap<[u8; 32], Timeline<[u8; 32]>>,
    pub transient_timelines: BTreeMap<[u8; 32], Timeline<[u8; 32]>>,
    pub logs: Vec<ObservedLog>,
    pub access_list_timeline: Timeline<Option<[u8; 32]>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionMetadata {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub transaction_hash: [u8; 32],
    pub transaction_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionBucket {
    pub metadata: TransactionMetadata,
    pub account_nodes: BTreeMap<[u8; 20], AccountNode>,
    pub gas_refund_timeline: Timeline<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuilderIr(pub Vec<TransactionBucket>);
