//! # Kaysentinel Builder
//!
//! Compiles a validated stream of Canonical Semantic Events (CSE) into the
//! Structural Sufficient Representation (SSR) via a lossless, flat relational
//! intermediate representation (IR), timeline reduction, and canonicalization.

pub mod errors;
pub mod ir;
pub mod traits;
pub mod partition;
pub mod reduce;
pub mod lifecycle;

pub use errors::{BuilderError, TimelineError, LifecycleState, LifecycleEvent};
pub use ir::timeline::{RawIr, EventId, Provenance, TraceProvenance, Timeline, CanonicalKey, TimelineVariant};
pub use ir::reduced::{ReducedIr, ReducedTransactionBucket, ReducedVariant};
pub use traits::{ReducedTransition, ReducibleTimeline};
pub use partition::process as partition_events;
pub use reduce::process as reduce_timelines;
pub use lifecycle::process as resolve_lifecycles;
pub use lifecycle::certificate::{
    AccountSnapshotSource, CanonicalAccountCertificate, GenerationResolutionError,
    ResolvedAccountSnapshot, StorageRootState, VerifiedGeneration,
    build_canonical_certificates, resolve_generation_for_address,
};
