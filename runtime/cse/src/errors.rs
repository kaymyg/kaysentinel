#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// The event carries a version tag not supported by this runtime version.
    UnsupportedVersion { expected: u32, found: u32 },
    /// The stream sequence counter broke monotonicity or skipped updates.
    SequenceBreakage { expected: u64, found: u64 },
    /// The payload contains zero mutations or structural contradictions (e.g. self-canceling logs).
    InvalidPayloadState(&'static str),
    /// Emitted boundaries violate protocol rules (e.g. operations before a transaction starts).
    InvalidBoundaryCondition(&'static str),
}
