use crate::ir::timeline::EventId;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimelineError {
    EmptyTimeline,
    DiscontinuousLineage { previous_event: EventId, current_event: EventId },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LifecycleState {
    Existed,
    Created,
    Destroyed,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    CreateAction,
    DestroyAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuilderError {
    EventOutsideTransaction,
    UnexpectedTransactionBoundary,
    DuplicateTransactionBoundary,
    TimelineFailure(TimelineError),
    InvalidLifecycle {
        address: [u8; 20],
        previous_state: LifecycleState,
        attempted: LifecycleEvent,
    },
}

impl From<TimelineError> for BuilderError {
    fn from(err: TimelineError) -> Self {
        BuilderError::TimelineFailure(err)
    }
}
