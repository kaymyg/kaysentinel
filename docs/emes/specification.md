# EMES v1.0 Canonical Specification

> **Status**: Provisional Core / Active Reconciliation

This document is the single normative contract defining the semantics, types, and lifecycle rules of the Execution Model Event Stream.

## 1. Core Structural Layout
EMES partitions data into two isolated namespaces to guarantee deterministic execution:

1.  **Semantic Payload**: Consensus-relevant, deterministic, hashable data.
2.  **Diagnostic Context**: Telemetry, profiling, and debugging metadata. Must be stripped before hashing or cross-client comparison.

## 2. Canonical Types

### Storage Identifiers
*   **storage_key**: Formally defined as a fixed-size **32-byte storage slot identifier**.
*   *Informative Note*: Local runtimes may represent this using native types based on language constraints, but the boundary serialization layer must expose exactly 32 bytes.

## 3. Normative Requirements
*   Implementations MUST isolate operational telemetry fields from block identity algorithms (INV-001).
*   EVM storage slot identifiers SHALL be processed strictly as raw 32-byte boundaries (ADR-002).

---

## Appendix A: Unverified Candidate Materials

The entries in this section are currently under active reconciliation. No normative requirements exist for these fields, and they are omitted from the protocol specification until cross-client (Go + Rust) evidence is introduced and an ADR is approved. Being backed by real, working code (Medium authority) is sufficient to keep a field as an active Candidate rather than Unknown, but is explicitly **not** sufficient to promote it to Verified — that requires either cross-client agreement or an approved ADR per governance.md Rule A/C.

### Proposed Account Reference
*   **account_address**:
    *   *Status*: Candidate
    *   *Proposed Type*: Fixed 20-byte array (`bytes20`).
    *   *Evidence*: Every payload struct in `runtime/cse/src/payloads.rs` (`BalanceChanged`, `StorageSlotUpdated`, `NonceUpdated`, etc.) uses `address: [u8; 20]`.
    *   *Authority*: Medium — single implementation (Rust CSE only; Go tracer not yet inspected for this field).
    *   *Awaiting*: Confirmation that the Go tracer's equivalent field is also a fixed 20-byte layout before this can move to Verified.

### Proposed Mutation Value Fields
*   **previous_value** / **current_value**:
    *   *Status*: Candidate
    *   *Proposed Type*: Fixed 32-byte array (`bytes32`), not dynamically sized.
    *   *Evidence*: `StorageSlotUpdated` in `runtime/cse/src/payloads.rs` defines `previous_value: [u8; 32]` and `current_value: [u8; 32]`. Note the real field name is `current_value`, not `new_value` — an earlier draft of this matrix used the wrong name.
    *   *Authority*: Medium — single implementation (Rust CSE only).
    *   *Awaiting*: Go-side confirmation before promotion to Verified.

### Sequence Numbering Scope
*   *Status*: Candidate
*   *Proposed Rule*: A single global, monotonically increasing counter across the entire event stream — not scoped per-transaction or per-block.
*   *Evidence*: `runtime/builder/src/ir/timeline.rs`, `TraceProvenance` doc comment (Theorem 0.1, "Temporal Uniqueness"): every semantic mutation is assigned exactly one unique `trace_ordinal`, with no two events sharing a value, sourced directly from CSE's `NormativeContext.sequence_number`.
*   *Authority*: Medium — single implementation (Rust CSE/builder only).
*   *Awaiting*: Go-side confirmation. Also currently untested directly — no conformance test asserts global (vs. per-block) uniqueness; the invariant is structural/implied by the type, not exercised by `conformance.rs`.

### Nested Ordering
*   *Status*: Candidate (upgraded from Low authority in the prior draft)
*   *Proposed Rule*: Cross-frame call stacks serialize into one deterministic total order.
*   *Evidence*: Direct consequence of the Sequence Numbering Scope evidence above — a single global monotonic counter necessarily yields one total order across nested frames. Not a separately verified fact; it follows from the same source.
*   *Authority*: Medium.
