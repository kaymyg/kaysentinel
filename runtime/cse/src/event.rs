use crate::context::ExecutionContext;
use crate::payloads::*;
use crate::traits::{Versioned, Validate};
use crate::errors::ValidationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CseVersion {
    V1 = 1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CsePayload {
    BeginTransaction,
    EndTransaction,
    BalanceChanged(BalanceChanged),
    StorageSlotUpdated(StorageSlotUpdated),
    TransientStorageUpdated(TransientStorageUpdated),
    NonceUpdated(NonceUpdated),
    ContractCreated(ContractCreated),
    CodeUpdated(CodeUpdated),
    ContractDestroyed(ContractDestroyed),
    LogEmitted(LogEmitted),
    GasRefundChanged(GasRefundChanged),
    AccessListTouched(AccessListTouched),
}

#[derive(Debug, Clone)]
pub struct CanonicalSemanticEvent {
    pub version: CseVersion,
    pub context: ExecutionContext,
    pub payload: CsePayload,
}

impl Versioned for CanonicalSemanticEvent {
    fn version(&self) -> CseVersion {
        self.version
    }
}

impl Validate for CanonicalSemanticEvent {
    fn validate(&self) -> Result<(), ValidationError> {
        if (self.version as u32) != crate::version::ABI_VERSION {
            return Err(ValidationError::UnsupportedVersion { 
                expected: crate::version::ABI_VERSION, 
                found: self.version as u32 
            });
        }

        match &self.payload {
            CsePayload::BalanceChanged(p) => {
                if p.previous_balance == p.current_balance {
                    return Err(ValidationError::InvalidPayloadState("No-op balance change signature"));
                }
            }
            CsePayload::StorageSlotUpdated(p) => {
                if p.previous_value == p.current_value {
                    return Err(ValidationError::InvalidPayloadState("No-op storage mutation signature"));
                }
            }
            CsePayload::ContractCreated(p) => {
                if p.address == [0u8; 20] {
                    return Err(ValidationError::InvalidPayloadState("Deployment targeting null address"));
                }
            }
            _ => {}
        }
        Ok(())
    }
}
