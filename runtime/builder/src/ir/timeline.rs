use std::collections::{BTreeMap, BTreeSet};
use kaysentinel_cse::payloads::*;
use crate::errors::TimelineError;
use crate::traits::{ReducedTransition, ReducibleTimeline};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedTransition<T: Clone + PartialEq> {
    pub before: T,
    pub after: T,
    pub event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CseLifecyclePayload {
    Created(ContractCreated),
    Destroyed(ContractDestroyed),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedLifecycle {
    pub payload: CseLifecyclePayload,
    pub event_id: EventId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedLog {
    pub payload: LogEmitted,
    pub event_id: EventId,
}

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

impl<T: Clone + PartialEq + Copy> ReducibleTimeline for Timeline<T> {
    type Value = T;

    fn reduce(self) -> Result<ReducedTransition<Self::Value>, TimelineError> {
        if self.history.is_empty() {
            return Err(TimelineError::EmptyTimeline);
        }

        let mut provenance = Provenance::new();
        let initial = self.history.first().unwrap().before;
        let mut current_after = self.history.first().unwrap().after;
        
        provenance.record(self.history.first().unwrap().event_id);

        for observation in self.history.into_iter().skip(1) {
            if observation.before != current_after {
                return Err(TimelineError::DiscontinuousLineage);
            }
            current_after = observation.after;
            provenance.record(observation.event_id);
        }

        Ok(ReducedTransition {
            initial,
            terminal: current_after,
            provenance,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountNode {
    pub address: [u8; 20],
    pub lifecycle_history: Vec<ObservedLifecycle>,
    pub balance_timeline: Option<Timeline<[u8; 32]>>,
    pub nonce_timeline: Option<Timeline<u64>>,
    pub code_timeline: Option<Timeline<[u8; 32]>>,
    pub storage_timelines: BTreeMap<[u8; 32], Timeline<[u8; 32]>>,
    pub transient_timelines: BTreeMap<[u8; 32], Timeline<[u8; 32]>>,
    pub logs: Vec<ObservedLog>,
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
    pub gas_refund_timeline: Option<Timeline<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineIr(pub Vec<TransactionBucket>);
