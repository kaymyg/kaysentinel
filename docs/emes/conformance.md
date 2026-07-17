# EMES Invariant & Conformance Traceability Matrix

This document maps protocol-wide semantic invariants directly to their executable validation tests within the project test suites.

| Invariant ID | Invariant Name | Status | Evidence / Authority | Conformance Test Target |
| :--- | :--- | :--- | :--- | :--- |
| **INV-001** | Diagnostic Isolation | Verified | PR1 (`ExecutionContext` split, commit `8ba1a4e`) / **High** | `runtime/builder/tests/conformance.rs::test_property_diagnostic_isolation` |
| **INV-002** | Semantic Sensitivity | Verified | PR2 (commit `8ba1a4e`) / **High** | `runtime/builder/tests/conformance.rs::test_property_semantic_sensitivity` |
| **INV-003** | Deterministic Ordering | Unknown | None / None | *Planned / Awaiting Unification* |
| **INV-004** | Replay Equivalence | Unknown | None / None | *Planned / Awaiting Unification* |

Note on authority: INV-001 and INV-002 are graded **High** because both are backed by executable, passing tests — but those tests currently exercise only the Rust `runtime/builder`/`runtime/cse` pipeline. Neither invariant has been checked against the Go tracer (`emes/types.go`), which is untouched. "High" here means "test-verified," not "cross-client verified" — see RL-0001.

---

## INV-001: Diagnostic Isolation

### Normative Definition
Diagnostic information MUST NOT influence semantic identity computation, block state extraction, or core execution flows.

### Rationale (Informative)
This prevents transient telemetry, platform-specific thread IDs, or timing variations from introducing non-determinism into the consensus boundary or changing state calculation outputs.

### Conformance
`runtime/builder/tests/conformance.rs::test_property_diagnostic_isolation`

---

## INV-002: Semantic Sensitivity

### Normative Definition
Any mutative change to the core protocol state space MUST immediately alter the output of the semantic identity state layer.

### Rationale (Informative)
This ensures that the identity layer functions as a cryptographically secure hash of the execution state, detecting structural state drifts immediately during processing.

### Conformance
`runtime/builder/tests/conformance.rs::test_property_semantic_sensitivity`
