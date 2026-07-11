use crate::errors::TimelineError;
use crate::ir::timeline::Provenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedTransition<T: Clone + PartialEq> {
    pub initial: T,
    pub terminal: T,
    pub provenance: Provenance,
}

pub trait ReducibleTimeline {
    type Value: Clone + PartialEq;

    /// Pure functional reduction of an absolute event vector down into a stable
    /// single-transition object, guaranteeing O(M) verification complexity where M is history length.
    fn reduce(self) -> Result<ReducedTransition<Self::Value>, TimelineError>;
}
