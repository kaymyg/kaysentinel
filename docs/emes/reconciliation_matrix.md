# EMES Master Reconciliation Matrix

This file serves as the active tracker for the reconciliation process. Items marked `Unknown` or `Candidate` will be systematically updated as implementation code from the Go tracer and Rust CSE codebases is provided.

## 1. Behavioral & Lifecycle Matrix

| Operational Behavior | Status | Evidence | Authority | Architectural Notes / Open Questions |
| :--- | :--- | :--- | :--- | :--- |
| **Buffered before commit?** | **Unknown** | None | None | Is execution data emitted immediately to a stream, or buffered until a frame success block? |
| **Emits explicit revert event?** | **Unknown** | None | None | Does the engine signal downstream consumers via an explicit `REVERT` marker? Note: `CsePayload` (Rust) has no revert-specific variant today — reverted state simply isn't emitted as a committed mutation, per the EMES-V1 draft's "Isolation of No-ops" rule — but that rule itself is still Candidate/Low, not verified against either implementation's actual behavior. |
| **Removes reverted events?** | **Unknown** | None | None | Does EMES record chronological *Execution History* or strictly the final *Committed State*? |
| **Nested ordering guaranteed?** | **Candidate** | `runtime/builder/src/ir/timeline.rs` (`TraceProvenance` Theorem 0.1) | **Medium** | A single global `trace_ordinal` counter implies one deterministic total order across nested call frames. Upgraded from Low/Abstract-Draft evidence to Medium/real-code evidence. Still Candidate, not Verified — no cross-client confirmation, and not directly exercised by a conformance test. |
| **Sequence numbering scope?** | **Candidate** | `runtime/builder/src/ir/timeline.rs` (`TraceProvenance` Theorem 0.1); sourced from CSE `NormativeContext.sequence_number` | **Medium** | Evidence indicates a single global monotonic counter across the whole stream (not per-tx or per-block scoped). Rust-only; awaiting Go-side confirmation before Verified. |

## 2. Structural Vocabulary & Type Matrix

| Field Name | Type Mapping | Status | Namespace Assignment | Evidence / ADR | Resolution Note |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `storage_key` | `bytes32` | **Verified** | Semantic Payload | ADR-002 | Confirmed as a raw 32-byte slot value. |
| `account_address` | `bytes20` | **Candidate** | Semantic Payload | `runtime/cse/src/payloads.rs` (`address: [u8; 20]` on every payload struct) | **Medium** authority — real Rust code, but single-implementation; awaiting Go-side struct confirmation before Verified. |
| `previous_value` | `bytes32` | **Candidate** | Semantic Payload | `runtime/cse/src/payloads.rs`, `StorageSlotUpdated.previous_value: [u8; 32]` | **Medium** — fixed-size, not variable-length as an earlier draft speculated. |
| `current_value` | `bytes32` | **Candidate** | Semantic Payload | `runtime/cse/src/payloads.rs`, `StorageSlotUpdated.current_value: [u8; 32]` | **Medium**. Note: this field was previously listed as `new_value` in an earlier draft of this matrix — that name does not exist in the codebase; the real field is `current_value`. Corrected here. |
| `call_frame_id` | `u32` | **Verified** | Diagnostic Context | ADR-001 / RL-0001; `runtime/cse/src/context.rs`, `TraceContext.call_frame_id: u32` | Routed to Diagnostic Context (`TraceContext`); structurally excluded from equality/hashing via the PR1 split. |
| `call_depth` | `u32` | **Verified** | Diagnostic Context | ADR-001 / RL-0001; `runtime/cse/src/context.rs`, `TraceContext.call_depth: u32` | Routed to Diagnostic Context (`TraceContext`); structurally excluded from equality/hashing via the PR1 split. |

**Correction note**: an earlier draft of this matrix listed `trace_id: string` and `elapsed_ns: uint64` as the two fields verified under ADR-001/RL-0001. Neither field exists anywhere in the codebase — `TraceContext` in `runtime/cse/src/context.rs` has exactly two fields, `call_frame_id: u32` and `call_depth: u32`, which are what's actually replaced them in this revision. Per governance.md Rule D, no row should carry a Verified status without its evidence being checked against the real source file — this correction is the direct result of doing that check.
