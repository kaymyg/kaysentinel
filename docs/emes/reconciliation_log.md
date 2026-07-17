# EMES Reconciliation History Log

This log records every empirical observation and subsequent architectural decision made while reconciling implementation deltas.

### [RL-0001] Diagnostic Isolation of Execution Context
*   **Observation**: During architectural reconciliation, diagnostic metadata and semantic identity were identified as requiring strict structural separation. The Rust builder implementation introduced an explicit `ExecutionContext` split, and conformance tests were added to verify diagnostic isolation.
*   **Decision**: Adopt the strict separation of namespaces as a protocol-wide rule.
*   **Rationale**: Telemetry fields must never influence consensus tracking or block identity algorithms.
*   **Impact**:
    *   Rust CSE/Builder: Refactored in PR1 (commit `8ba1a4e`). `ExecutionContext` split into `NormativeContext { sequence_number }`, `ProvisionalContext { chain_id, block_hash, block_number, transaction_hash, transaction_index }`, and `TraceContext { call_frame_id, call_depth }`. `TraceContext` deliberately does not derive `PartialEq`/`Eq`/`Hash`, so it cannot structurally participate in equality or (once implemented) hashing/SSZ routines.
    *   Go Tracer: **Untouched.** `emes/types.go` still mixes frame-depth and gas telemetry (`FrameID`, `Depth`, `GasLimit`, `GasPrice`) directly into its event structs. No diagnostic isolation exists on the Go side yet — this is open work, not completed work.
    *   Test Validation: Covered under `INV-001`, `INV-002` — both passing, Rust side only. See `conformance.md` for the authority caveat: "High" here means test-verified on one implementation, not cross-client verified.

### [RL-0002] Storage Key Type Normalization
*   **Observation**: The protocol architecture identified that language-specific representations of EVM storage slots may differ across execution clients.
*   **Decision**: ADR-002 standardizes the protocol representation as a fixed 32-byte storage slot identifier.
*   **Rationale**: Decouples the protocol definition from individual language memory alignments, providing an unambiguous serialization target.
*   **Impact**: Downstream boundary and serialization layers must read and write raw 32-byte buffers.
