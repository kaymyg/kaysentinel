# KAYSentinel Semantic Architecture & Formal Specification

**Draft Version 0.1**

This specification defines the intended semantic contracts, invariants, and structural
transformations for the KAYSentinel pipeline. Formal proofs and implementation
validation are ongoing.

---

## Status

* **[Core Semantic Architecture]** Defined.
* **[Stage Contracts]** Specified.
* **[Determinism Model]** Specified.
* **[Formal Proofs]** Under development.
* **[Reference Implementation]** In progress — see `runtime/builder/src/lifecycle/certificate.rs`
  for the concrete Stage 3 (Generation Resolution) implementation.
* **[Cryptographic Commitment Layer]** Not started (`runtime/hash` is still an empty
  stub crate).

---

## 1. Formal Preliminaries & Definitions

### Definition 1: The Total Canonical Execution Order (≺)

Let M be the set of all semantic mutations emitted during a partition runtime. We
define a strict total ordering relation, denoted by ≺, where m_i ≺ m_j if and only if
mutation m_i was systematically appended to the execution journal chronologically
prior to mutation m_j within the sequence defined by `partition.rs`.

### Definition 2: StateTable Coherence

A `StateTable` is coherent if and only if it satisfies the following four structural
invariants simultaneously:

* **Uniqueness:** Every registered identity resolves to at most one active generation
  identifier.
* **Acyclicity:** Provenance chains linking generations to ancestral epochs are
  strictly directed and acyclic (G_n → G_(n-1)).
* **Non-Divergence:** Generation identifiers are globally unique across the state
  domain.
* **Proof Consistency:** Structural state proofs correspond bijectively to recorded
  generation states.

---

## 2. System Axioms & Assumptions

To evaluate the pipeline's transformations independently of specific environment
variables, the system relies on the following operational assumptions:

* **Assumption A1 (Total Input Ordering):** The Partition stage emits an execution
  sequence that is totally ordered under ≺.
* **Assumption A2 (StateTable Integrity):** The underlying `StateTable` continuously
  preserves the invariants outlined in Definition 2 (Coherence).
* **Assumption A3 (Deterministic Serialization):** The serialization engine maps
  identical inputs to identical byte layouts without introducing runtime-dependent
  padding or variations.
* **Assumption A4 (Deterministic Cryptography):** The cryptographic commitment and
  hashing primitives behave as pure mathematical functions (identical byte streams
  yield identical commitment outputs).
* **Assumption A5 (Compositional Compatibility):** For any two sequential stages S_n
  and S_(n+1), the postconditions guaranteed by S_n satisfy the complete set of
  preconditions required by S_(n+1).

---

## 3. The Compositional Stage Contracts

```
[Journal Input]
       │
       ▼ (Partition Stage) — Pre: Journal ordered. Post: Total Order (≺) established [A1].
[Ordered Semantic Mutations]
       │
       ▼ (Contract 1: Projection) — Pre: Mutation order total. Post: Single survivor per key.
[TerminalProjection]
       │
       ▼ (Contract 2: Canonicalization) — Pre: Coherent StateTable [A2]. Post: Resolved Certificates.
[CanonicalAccountCertificate]
       │
       ▼ (Contract 3: Serialization) — Pre: BTreeMap sorting. Post: Byte-identical layout [A3].
[Canonical Byte Stream]
       │
       ▼ (Contract 4: Commitment) — Pre: Deterministic hash [A4]. Post: Deterministic root.
[Cryptographic Commitment]
```

### Stage 1: Partition

* **Precondition:** Journal entries are internally ordered.
* **Guarantee:** The resulting semantic mutation stream possesses a strict total
  ordering under ≺.
* **Status:** Implemented — `runtime/builder/src/partition.rs`.

### Stage 2: Terminal Projection

* **Precondition:** Ingested assignments adhere to Assumption A1 (total ordering).
* **Guarantee (Terminal Projection Uniqueness):** The terminal projection contains
  exactly one assignment for every distinct key appearing in the sequence, and that
  assignment equals the final occurrence of that key under ≺.
* **Status:** Implemented — `Timeline::reduce()` in `runtime/builder/src/ir/timeline.rs`,
  operating over the flat, `CanonicalKey`-keyed `state_tables` built by `partition.rs`
  and consumed by `runtime/builder/src/reduce.rs`.

### Stage 3: Canonicalization

* **Precondition:** The input terminal projection satisfies Terminal Projection
  Uniqueness, and the `StateTable` satisfies Assumption A2.
* **Guarantee (Generation Resolution Soundness):** Every survivor key either maps to
  exactly one verified generation certificate or the transformation terminates
  deterministically with a `GenerationResolutionError`.
* **Status:** Partially implemented — `runtime/builder/src/lifecycle/certificate.rs`
  implements `resolve_generation_for_address`, which resolves a flat address identity
  to its unique terminal `VerifiedGeneration`, or fails deterministically with
  `GenerationResolutionError::{IdentityAbsent, IdentityAmbiguous}`.
  `ProvenanceMetadataDivergence` and `MalformedProvenanceChain` are defined but not
  yet raised anywhere, since no cross-generation ancestry linkage is tracked yet.
  Full `CanonicalAccountCertificate` assembly (nonce/balance/storage_root/code_hash)
  is not yet wired, because the pipeline's state facts (`state_table`) are still flat
  and address-keyed rather than generation-scoped — there is currently no logic
  anywhere that cross-references a balance/storage/nonce fact to the specific
  generation that was active when it was last written.

### Stage 4: Serialization

* **Precondition:** Elements are stored within deterministically ordered collections
  (`BTreeMap`, `BTreeSet`) to fix internal layout topology.
* **Guarantee (Canonical Serialization Determinism):** Under Assumption A3, equal
  canonical structures serialize to identical byte sequences.
* **Status:** Not started (`runtime/ssr`, `runtime/ssz` are still empty stub crates).

### Stage 5: Commitment

* **Precondition:** Ingested byte sequences are generated via stages compliant with
  the serialization contract.
* **Guarantee (Commitment Determinism):** Under Assumption A4, identical canonical
  byte streams produce identical commitment outputs.
* **Status:** Not started (`runtime/hash` is still an empty stub crate).

---

## 4. Derived Structural Propositions

Based on the explicit axioms and stage boundaries, the pipeline establishes the
following properties:

### Proposition 1: Provenance Derivability

Every state object emitted by stage n is derivable from a well-defined set of
antecedent objects in stage n − 1 according to its specific transformation contract.
No emitted object lacks a traceable historical baseline within the pipeline.

### Proposition 2: Compositional Determinism

If all foundational assumptions (A1–A5) hold and every component stage successfully
satisfies its individual transformation contract, then the composed pipeline is
deterministic: given an identical sequence of initial journal events and a coherent
starting state, the terminal cryptographic commitment output is invariant across all
execution runtimes.

---

## 5. Note on the Reference EVM Instantiation

An earlier draft of this specification illustrated the model with a standalone code
sketch using `primitive_types::{H160, H256, U256}` and a fresh `EvmEntityKey` /
`Assignment<V>` / `TerminalProjection<V>` type family. That sketch is **not** part of
the reference implementation: it would have introduced a new external dependency and
a second, parallel key/value type system duplicating what `CanonicalKey`,
`TimelineVariant`, and `Timeline::reduce()` already implement (and have test coverage
for) in `runtime/builder/src/ir/timeline.rs`.

The two genuinely new concepts from that draft — `GenerationResolutionError` and
`CanonicalAccountCertificate` — were adapted directly onto the project's existing
types (`[u8; 20]` addresses, the existing `GenerationKey`, `CanonicalGeneration`) and
implemented in `runtime/builder/src/lifecycle/certificate.rs` instead, per the status
notes in Stage 3 above.
