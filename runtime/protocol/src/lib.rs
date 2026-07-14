//! # Kaysentinel Protocol
//!
//! Shared, implementation-neutral protocol-level types — starting with a common
//! error interface (`ProtocolError`) that isn't coupled to any single crate's
//! internal enum naming or module layout.

pub mod errors;

pub use errors::ProtocolError;
