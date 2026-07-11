use crate::errors::TimelineError;
use crate::ir::timeline::Provenance;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReducedTransition<T: Clone + PartialEq> {
    initial: T,
    terminal: T,
    provenance: Provenance,
}

impl<T: Clone + PartialEq> ReducedTransition<T> {
    pub fn new(initial: T, terminal: T, provenance: Provenance) -> Self {
        Self { initial, terminal, provenance }
    }
    pub fn initial(&self) -> &T { &self.initial }
    pub fn terminal(&self) -> &T { &self.terminal }
    pub fn provenance(&self) -> &Provenance { &self.provenance }
}

pub trait ReducibleTimeline {
    type Value: Clone + PartialEq;
    fn reduce(self) -> Result<ReducedTransition<Self::Value>, TimelineError>;
}
