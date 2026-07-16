/// Normative execution metadata. `sequence_number` is, today, the *only* field
/// anywhere in `runtime/builder` that any invariant-verification logic actually
/// reads (see `TraceProvenance`'s `trace_ordinal` in the builder, which is sourced
/// directly from this field and used for all temporal/ordering checks).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NormativeContext {
    pub sequence_number: u64,
}

/// Metadata retained for domain-separation/grouping purposes. Currently gathered
/// into `TransactionMetadata` by `runtime/builder`'s partition stage but not read
/// by any certificate-building or invariant-checking logic downstream of that —
/// kept normative-adjacent (participates in Eq/Hash) pending a decision on whether
/// it should ever gate consensus-relevant behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProvisionalContext {
    pub chain_id: u64,
    pub block_hash: [u8; 32],
    pub block_number: u64,
    pub transaction_hash: [u8; 32],
    pub transaction_index: u32,
}

/// Non-normative debugging/diagnostic telemetry. Deliberately does not derive
/// `PartialEq`/`Eq`/`Hash` so it cannot structurally participate in equality,
/// hashing, or (once implemented) SSZ/commitment routines — verified by
/// `test_property_diagnostic_isolation` in `runtime/builder`.
#[derive(Debug, Clone, Copy)]
pub struct TraceContext {
    pub call_frame_id: u32,
    pub call_depth: u32,
}

/// Environmental metadata surrounding a `CanonicalSemanticEvent`. Does not derive
/// `PartialEq`/`Eq`/`Hash` itself, since it carries a `TraceContext`; compare or
/// hash `.normative` / `.provisional` explicitly where equality is needed.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionContext {
    pub normative: NormativeContext,
    pub provisional: ProvisionalContext,
    pub trace: TraceContext,
}
