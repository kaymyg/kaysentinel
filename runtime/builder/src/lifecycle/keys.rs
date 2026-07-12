/// Identifies a single identity "generation" of an account: the span between
/// one creation and the next destruction (or the end of execution).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GenerationKey {
    pub address: [u8; 20],
    pub generation_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AspectKey {
    pub generation: GenerationKey,
}
