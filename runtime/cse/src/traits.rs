use crate::event::CseVersion;
pub use crate::errors::ValidationError;

pub trait Versioned {
    /// Returns the explicit semantic schema version enforced by the object.
    fn version(&self) -> CseVersion;
}

pub trait Validate {
    /// Evaluates the syntactic and isolated structural correctness of the type.
    fn validate(&self) -> Result<(), ValidationError>;
}

pub trait Identifiable {
    /// Returns the targeted Ethereum address account context for the mutation event.
    fn target_address(&self) -> [u8; 20];
}
