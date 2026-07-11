//! # Kaysentinel CSE (Canonical Semantic Event) ABI Crate
//! 
//! This crate implements the frozen system ABI layer defined in the `CSE-V1` protocol
//! specification. It contains no execution logic or builder awareness, operating
//! purely as a client-agnostic semantic interface.

pub mod version;
pub mod errors;
pub mod traits;
pub mod context;
pub mod payloads;
pub mod event;
pub mod validator;

pub use version::ABI_VERSION;
pub use errors::ValidationError;
pub use traits::{Versioned, Validate, Identifiable};
pub use context::ExecutionContext;
pub use event::{CanonicalSemanticEvent, CsePayload, CseVersion};
pub use validator::CseValidator;
