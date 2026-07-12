use std::collections::{BTreeMap, BTreeSet};
use kaysentinel_cse::payloads::*;
use crate::errors::TimelineError;
use crate::traits::{ReducedTransition, ReducibleTimeline};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub u64);

/// A richer execution coordinate for lifecycle events than a bare `EventId`.
///
/// THEOREM 0.1 (Temporal Uniqueness): every semantic mutation emitted by the tracer
/// is assigned exactly one unique `trace_ordinal`, and no two events share the same
/// value. Equality and ordering are therefore defined solely in terms of `trace_ordinal`;
/// the remaining fields are descriptive structural metadata only.
#[derive(Debug, Clone, Copy, Hash, Default)]
pub struct TraceProvenance {
    pub trace_ordinal: u64,
    pub tx_index: u64,
    pub frame_id: usize,
    pub call_depth: usize,
    pub frame_ordinal: u32,
}

impl PartialEq for TraceProvenance {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.trace_ordinal == other.trace_ordinal
    }
}

impl Eq for TraceProvenance {}

impl Ord for TraceProvenance {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.trace_ordinal.cmp(&other.trace_ordinal)
    }
}

impl PartialOrd for TraceProvenance {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance(pub BTreeSet<EventId>);

impl Provenance {
    pub fn new() -> Self { Self(BTreeSet::new()) }
    pub fn record(&mut self, id: EventId) { self.0.insert(id); }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalKey {
    Balance { address: [u8; 20] },
    Nonce { address: [u8; 20] },
    Code { address: [u8; 20] },
    Storage { address: [u8; 20], slot: [u8; 32] },
    Transient { address: [u8; 20], slot: [u8; 32] },
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
    pub trace_provenance: TraceProvenance,
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
    pub fn new() -> Self { Self { history: Vec::new() } }
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
        let mut last_event_id = self.history.first().unwrap().event_id;
        
        provenance.record(last_event_id);

        for observation in self.history.into_iter().skip(1) {
            if observation.before != current_after {
                return Err(TimelineError::DiscontinuousLineage {
                    previous_event: last_event_id,
                    current_event: observation.event_id,
                });
            }
            current_after = observation.after;
            last_event_id = observation.event_id;
            provenance.record(observation.event_id);
        }

        Ok(ReducedTransition::new(initial, current_after, provenance))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionMetadata {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: [u8; 32],
    pub transaction_hash: [u8; 32],
    pub transaction_index: u32,
}

/// RawIr represents completely flat, relational tables grouped by transaction boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionBucket {
    pub metadata: TransactionMetadata,
    pub state_tables: BTreeMap<CanonicalKey, TimelineVariant>,
    pub lifecycle_table: BTreeMap<[u8; 20], Vec<ObservedLifecycle>>,
    pub log_table: BTreeMap<[u8; 20], Vec<ObservedLog>>,
    pub gas_refund_timeline: Option<Timeline<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineVariant {
    Balance(Timeline<[u8; 32]>),
    Nonce(Timeline<u64>),
    Code(Timeline<[u8; 32]>),
    Storage(Timeline<[u8; 32]>),
    Transient(Timeline<[u8; 32]>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawIr(pub Vec<TransactionBucket>);
