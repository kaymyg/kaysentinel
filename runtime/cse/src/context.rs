#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutionContext {
    pub chain_id: u64,
    pub block_hash: [u8; 32],
    pub block_number: u64,
    pub transaction_hash: [u8; 32],
    pub transaction_index: u32,
    pub call_frame_id: u32,
    pub call_depth: u32,
    pub sequence_number: u64,
}
