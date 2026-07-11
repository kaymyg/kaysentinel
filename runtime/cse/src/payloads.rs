use crate::traits::Identifiable;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceChanged {
    pub address: [u8; 20],
    pub previous_balance: [u8; 32], // Big-endian U256 representation
    pub current_balance: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageSlotUpdated {
    pub address: [u8; 20],
    pub slot: [u8; 32],
    pub previous_value: [u8; 32],
    pub current_value: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransientStorageUpdated {
    pub address: [u8; 20],
    pub slot: [u8; 32],
    pub previous_value: [u8; 32],
    pub current_value: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonceUpdated {
    pub address: [u8; 20],
    pub previous_nonce: u64,
    pub current_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractCreated {
    pub address: [u8; 20],
    pub creator: [u8; 20],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeUpdated {
    pub address: [u8; 20],
    pub previous_code_hash: [u8; 32],
    pub current_code_hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractDestroyed {
    pub address: [u8; 20],
    pub refund_target: [u8; 20],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEmitted {
    pub address: [u8; 20],
    pub topics: Vec<[u8; 32]>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GasRefundChanged {
    pub previous_refund: u64,
    pub current_refund: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessListTouched {
    pub address: [u8; 20],
    pub slot: Option<[u8; 32]>,
}

// Implement Identifiable markers for targeted account scopes
impl Identifiable for BalanceChanged { fn target_address(&self) -> [u8; 20] { self.address } }
impl Identifiable for StorageSlotUpdated { fn target_address(&self) -> [u8; 20] { self.address } }
impl Identifiable for TransientStorageUpdated { fn target_address(&self) -> [u8; 20] { self.address } }
impl Identifiable for NonceUpdated { fn target_address(&self) -> [u8; 20] { self.address } }
impl Identifiable for ContractCreated { fn target_address(&self) -> [u8; 20] { self.address } }
impl Identifiable for CodeUpdated { fn target_address(&self) -> [u8; 20] { self.address } }
impl Identifiable for ContractDestroyed { fn target_address(&self) -> [u8; 20] { self.address } }
impl Identifiable for LogEmitted { fn target_address(&self) -> [u8; 20] { self.address } }
impl Identifiable for AccessListTouched { fn target_address(&self) -> [u8; 20] { self.address } }
