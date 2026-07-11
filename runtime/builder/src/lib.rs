//! # Kaysentinel Builder
//!
//! Compiles a validated stream of Canonical Semantic Events (CSE) into the
//! Structural Sufficient Representation (SSR) via a lossless intermediate
//! representation (IR), timeline reduction, and canonicalization pipeline.

pub mod errors;
pub mod ir;
pub mod traits;
pub mod partition;

pub use errors::BuilderError;
pub use ir::{BuilderIr, EventId, Provenance, Timeline};
pub use traits::{ReducedTransition, ReducibleTimeline};
pub use partition::process;
