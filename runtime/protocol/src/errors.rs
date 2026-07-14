use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    /// Returned when the block or epoch index violates genesis state constraints.
    InvalidGenesisContext,
    /// Returned when a list or vector length violates specified maximum protocol boundaries.
    LengthLimitExceeded,
    /// Returned when input data collections fail the required pre-sorted validation check.
    NonCanonicalInputOrder,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::InvalidGenesisContext => write!(f, "invalid genesis context"),
            ProtocolError::LengthLimitExceeded => write!(f, "length limit exceeded"),
            ProtocolError::NonCanonicalInputOrder => write!(f, "input is not in canonical order"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages_are_implementation_neutral() {
        // The public error message text should read naturally without leaking
        // Rust-specific enum variant naming (CamelCase, no "Error::" prefix, etc.)
        // — that's the whole point of having a Display impl instead of exposing
        // Debug output to callers.
        assert_eq!(ProtocolError::InvalidGenesisContext.to_string(), "invalid genesis context");
        assert_eq!(ProtocolError::LengthLimitExceeded.to_string(), "length limit exceeded");
        assert_eq!(ProtocolError::NonCanonicalInputOrder.to_string(), "input is not in canonical order");
    }

    #[test]
    fn implements_std_error() {
        fn assert_is_std_error<E: std::error::Error>(_: E) {}
        assert_is_std_error(ProtocolError::LengthLimitExceeded);
    }
}
