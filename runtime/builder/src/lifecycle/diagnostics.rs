use crate::ir::timeline::TraceProvenance;
use crate::lifecycle::keys::GenerationKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleViolation {
    /// A creation was observed on an address while a prior generation was still open.
    Collision { address: [u8; 20], generation_id: u32 },
    DuplicateKey(GenerationKey),
    IntegrityBreach(GenerationKey),
    IntervalInvalid(GenerationKey),
    OverlapDetected([u8; 20]),
    OrderingBreach,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub location: TraceProvenance,
    pub violation: LifecycleViolation,
    pub explanation: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantReport {
    pub keys_unique: bool,
    pub referential_integrity: bool,
    pub deterministic_order: bool,
    pub intervals_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassOutput<T> {
    pub value: T,
    pub diagnostics: Vec<Diagnostic>,
    pub invariant_report: InvariantReport,
}

impl<T> PassOutput<T> {
    pub fn is_valid(&self) -> bool {
        !self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}
