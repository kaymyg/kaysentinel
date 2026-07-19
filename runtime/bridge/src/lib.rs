//! # Kaysentinel Bridge
//!
//! Layer 2 of the Go -> Rust bridge architecture
//! (docs/emes/004-bridge-buffering-spec.md): parses the Go tracer's tagged
//! JSON event stream (`wire`) and translates it into
//! `Vec<kaysentinel_cse::CanonicalSemanticEvent>`, ready for
//! `kaysentinel_builder::partition_events` (`translate`).
//!
//! Assumes its input has already passed Gate 1 structural validation
//! (`validation/gate1.go`) -- see `translate::BridgeError` for the
//! defensive (not primary) checks this crate still performs.

pub mod translate;
pub mod wire;

pub use translate::{translate, BridgeConfig, BridgeError};
pub use wire::{parse_event_stream, GoEvent};
