#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimelineError {
    EmptyTimeline,
    DiscontinuousLineage,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BuilderError {
    EventOutsideTransaction,
    UnexpectedTransactionBoundary,
    DuplicateTransactionBoundary,
    InvalidSequence,
    InvalidLifecycle,
    TimelineFailure(TimelineError),
}

impl From<TimelineError> for BuilderError {
    fn from(err: TimelineError) -> Self {
        BuilderError::TimelineFailure(err)
    }
}
