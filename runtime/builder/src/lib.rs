//! # Kaysentinel Builder
//!
//! Compiles a validated stream of Canonical Semantic Events (CSE) into the
//! Structural Sufficient Representation (SSR) via a lossless intermediate
//! representation (IR), timeline reduction, and canonicalization pipeline.

pub mod errors;
pub mod ir;
pub mod traits;
pub mod partition;
pub mod reduce;

pub use errors::{BuilderError, TimelineError};
pub use ir::timeline::{TimelineIr, EventId, Provenance, Timeline};
pub use ir::reduced::ReducedIr;
pub use traits::{ReducedTransition, ReducibleTimeline};
pub use partition::process as partition_events;
pub use reduce::process as reduce_timelines;
