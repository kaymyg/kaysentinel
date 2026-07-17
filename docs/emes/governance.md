# EMES Protocol Governance Framework

This document defines the process, lifecycle states, authority metrics, and conflict resolution rules used to establish and evolve the Canonical Execution Model Event Stream (EMES) specification.

## 1. Glossary

*   **Semantic Payload**: The set of consensus-relevant, deterministic, and hashable fields required for replay equivalence.
*   **Diagnostic Context**: Telemetry, profiling, or operational metadata that is non-deterministic and stripped before serialization or hashing.
*   **Invariant**: A system-wide structural or behavioral truth that must be universally satisfied by all compliant implementations.
*   **Requirement**: A normative specification rule denoted by RFC-2119 keywords (MUST, SHALL, etc.).
*   **Candidate**: A proposed specification rule or structural mapping under active review but lacking high-authority evidence.
*   **Decision Pending**: A state indicating that all necessary implementation evidence has been gathered, but a formal architectural choice has not yet been approved.
*   **Authority**: The graded hierarchy of reliability assigned to a given piece of protocol evidence.
*   **Conformance**: The state of verifying an implementation against an invariant using an executable, non-malleable test suite.

## 2. Requirement Lifecycle State Machine

Every event kind, data field, behavioral rule, or protocol invariant moves through an explicit state machine based on empirical evidence:

*   **Unknown**: No empirical evidence or formal decision has been introduced yet.
*   **Candidate**: A plausible proposal derived from a draft or single-client implementation under active review.
*   **Decision Pending**: Structural evidence has been gathered, but a formal architectural decision is required to resolve a divergence.
*   **Verified**: Permanently adopted as part of the normative specification, backed by cross-client agreement or an approved ADR.
*   **Rejected**: Explicitly evaluated and excluded from the protocol. Preserved to prevent re-litigation.

## 3. Evidence Authority Rubric

| Authority Level | Source / Provenance | Description |
| :--- | :--- | :--- |
| **Normative** | Approved Governance Decision | Explicit, signed-off architectural direction (e.g., ADR-xxx). |
| **High** | Conformance Test / Multi-Client | Verified by executable tests or matching Go + Rust behaviors. |
| **Medium** | Single Implementation | Present only in the Go Tracer or only in the Rust CSE. |
| **Low** | Draft Document | Abstract drafts, legacy specifications, or repository outlines. |
| **Informational**| Design Discussion | Developer chat logs, informal PR threads, or whiteboards. |

## 4. Conflict Resolution Rules

*   **Rule A (Agreement)**: If independent implementations agree, but the old abstract draft differs, the draft is updated to match the implementations (**Verified**).
*   **Rule B (Divergence)**: If implementations disagree and no ADR exists, the item shifts to **Decision Pending** and downstream development is frozen on that component.
*   **Rule C (Precedence)**: If an approved ADR exists, implementations must converge to match it (**Verified**).
*   **Rule D (Traceability Enforcement)**: A reconciliation item MUST NOT transition to Verified unless its explicit evidence and authority level are permanently recorded, and that evidence MUST be checked against the actual source file at the time of recording — not reconstructed from memory or a prior draft.
