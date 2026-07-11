#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuilderError {
    /// A timeline's chronological chain of before/after values did not link up.
    InvalidSequence,
    /// A `BeginTransaction` event arrived while a transaction bucket was already open.
    DuplicateTransactionBoundary,
    /// An `EndTransaction` event arrived with no open transaction bucket.
    UnexpectedTransactionBoundary,
    /// A non-boundary event arrived with no open transaction bucket.
    EventOutsideTransaction,
}
