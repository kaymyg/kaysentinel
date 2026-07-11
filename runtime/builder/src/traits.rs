use crate::ir::Provenance;
use crate::errors::BuilderError;

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
    fn reduce(self) -> Result<ReducedTransition<Self::Value>, BuilderError>;
}

impl<T: Clone + PartialEq + Copy> ReducibleTimeline for crate::ir::Timeline<T> {
    type Value = T;

    fn reduce(self) -> Result<ReducedTransition<Self::Value>, BuilderError> {
        if self.history.is_empty() {
            return Err(BuilderError::InvalidSequence);
        }

        let mut provenance = Provenance::new();
        let initial = self.history.first().unwrap().before;
        let mut current_after = self.history.first().unwrap().after;
        
        provenance.record(self.history.first().unwrap().event_id);

        // Mechanically verify absolute chronological data lineage continuity
        for observation in self.history.into_iter().skip(1) {
            if observation.before != current_after {
                // Catches internal extraction discontinuities or state skips natively
                return Err(BuilderError::InvalidSequence);
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
