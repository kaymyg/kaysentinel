use crate::event::{CanonicalSemanticEvent, CsePayload};
use crate::errors::ValidationError;
use crate::traits::Validate;

pub struct CseValidator {
    expected_sequence: u64,
    in_transaction: bool,
}

impl CseValidator {
    pub fn new() -> Self {
        Self {
            expected_sequence: 0,
            in_transaction: false,
        }
    }

    pub fn validate_and_advance(&mut self, event: &CanonicalSemanticEvent) -> Result<(), ValidationError> {
        // 1. Syntactic structural compliance checks
        event.validate()?;

        // 2. Continuous sequence assertion
        if event.context.sequence_number != self.expected_sequence {
            return Err(ValidationError::SequenceBreakage {
                expected: self.expected_sequence,
                found: event.context.sequence_number,
            });
        }

        // 3. Envelope safety checks
        match &event.payload {
            CsePayload::BeginTransaction => {
                if self.in_transaction {
                    return Err(ValidationError::InvalidBoundaryCondition("Nested transaction boundary initiated"));
                }
                self.in_transaction = true;
            }
            CsePayload::EndTransaction => {
                if !self.in_transaction {
                    return Err(ValidationError::InvalidBoundaryCondition("Termination signature without active envelope"));
                }
                self.in_transaction = false;
            }
            _ => {
                if !self.in_transaction {
                    return Err(ValidationError::InvalidBoundaryCondition("Semantic event occurring outside active envelope"));
                }
            }
        }

        self.expected_sequence += 1;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.expected_sequence = 0;
        self.in_transaction = false;
    }
}
