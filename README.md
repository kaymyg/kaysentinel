---
license: mit
tags:
  - ethereum
  - formal-methods
  - evm
---

# KAYSentinel (PCAL + Sentinel)

A high-performance, client-agnostic **Protocol Integrity Runtime** that produces deterministic, client-independent representations of state execution for validation, security auditing, and forensic analysis.

Existing blockchain security tools analyze raw, client-specific execution traces, log structures, or protocol-specific events which differ between implementations. **KAYSentinel** instead captures canonical state mutations directly at the execution layer. It extracts these mutations into an invariant, minimal representation called the **Structural Sufficient Representation (SSR)**, ensuring that downstream systems always analyze identical execution states regardless of whether the node is running Geth, Reth, or another client.

---

## Technical Architecture & Core Pipeline

The core lifecycle of a state transition in KAYSentinel bypasses client-specific nuances by extracting state mutations into a canonical, verifiable timeline:

```text
       Execution Runtime (e.g., Geth / Reth)
                         │
                         ▼
      EMES (Execution Mutation Events Specification)
                         │
                         ▼
        Canonical Timeline Builder (runtime/builder)
                         │
                         ▼
     SSR (Structural Sufficient Representation - Δ)
                         │
                         ▼
           SSZ RC1 Serialization (runtime/ssz)
                         │
                         ▼
             Domain-Separated Canonical Hash
                         │
      ┌──────────────────┴──────────────────┐
      ▼                  ▼                  ▼
Verification         Forensics        Consensus Audits

```

### The Quotient-Space Model

Formally, rather than attempting to analyze raw, noisy execution traces $\sigma \in \Sigma$ which contain client-specific database side-effects and ephemeral memory states, KAYSentinel maps traces to a canonical space $\Delta$ (the SSR) via an extraction map $E$:

$$E: \Sigma \to \Delta$$

This extraction map enforces the foundational mathematical invariant of **Faithfulness + Abstraction**:

$$E(\sigma_1) = E(\sigma_2) \iff \text{Post}(\sigma_1) = \text{Post}(\sigma_2)$$

By factoring downstream evaluations purely through $\Delta$, **all downstream verification, policy enforcement, and consensus-checking are decoupled from client-specific execution details.** If two clients produce equivalent state outputs, their SSRs ($\Delta$) and resulting canonical cryptographic commitments are guaranteed to be identical.

---

## Downstream Applications

By moving beyond simple "post-execution authorization," the KAYSentinel runtime acts as an infrastructure layer for several distinct security and consensus applications:

* **Consensus & Client Validation:** Verifying that independent clients (e.g., Geth vs. Reth) agree on execution state transitions down to the mutation level.
* **Forensic Reconstruction:** Rebuilding step-by-step transaction lifecycles from standardized mutation streams.
* **Downstream Policy Evaluation:** Running complex out-of-band security rules over deterministic state outcomes without trusting client-specific database structures.
* **Anomaly Detection & Auditing:** Isolating unexpected runtime side-effects (such as transient storage leaks or unexpected reentrancy footprints) at the protocol layer.

---

## Repository & Runtime Architecture

```text
├── docs/
│   ├── framework.md                  # Formal mathematical & semantic specification
│   ├── semantic_architecture_spec.md # Stage-by-stage contract details & engineering gaps
│   └── implementation_roadmap.md     # Detailed Phase 1–3 development execution plan
├── runtime/
│   ├── emes/                         # Execution Mutation Event Specification schemas
│   ├── cse/                          # Canonical Semantic Event ABI definitions
│   ├── builder/                      # Timeline reduction, generations, and SSR hydration
│   ├── protocol/                     # Canonical protocol definitions & invariant checkers
│   ├── hash/                         # BLAKE3 domain-separated hashing workspace
│   └── ssz/                          # SSZ RC1 canonical schema & serialization strategy
└── tests/
    └── transient_storage_case.json   # Reference EIP-1153 validation test vector

```

---

## Active Codebase & Implementation Status

KAYSentinel is a functional, actively tested Rust implementation. Below is the current implementation status of our core runtime modules:

### What is Built & Fully Operational

* **`runtime/emes` & `runtime/cse`:** Complete specifications and ABI mappings that capture and validate execution-layer state mutations uniformly.
* **`runtime/builder`:** The state-reduction engine. It partitions and sequences raw incoming EMES events, reduces complex state timelines down to their terminal transitions, and resolves lifecycles (such as contract creation and self-destruction) into immutable "generations."
  * *Status:* **21 passing unit tests.**
* **`runtime/protocol`:** House of canonical protocol definitions, baseline error-handling bounds, and core state invariants.
* **`runtime/hash` (`kaysentinel-hash`):** A domain-separated, deterministic commitment primitive powered by **BLAKE3**, generating invariant hashes for SSR structures.
  * *Status:* **8 passing unit tests with pinned vectors.**

### Current Gaps & Near-Term Roadmap

1. **SSZ Serialization Incompleteness (`runtime/ssz`):** While the **SSZ RC1** schemas and serialization strategy are designed, the complete serialization/deserialization implementation remains incomplete.
2. **Storage Trie Derivation:** The trait interfaces for state trie derivation are defined, but the actual trie-generation code is pending. Currently, `storage_root` calculations default to `AwaitingDerivation`.
3. **Client-Side Extractors (Geth & Reth Hooks):** The out-of-process hooks that actually generate the raw EMES event stream from live execution clients are not yet written. The builder pipeline is currently tested and validated using high-fidelity simulated trace vectors.
   * *Target Geth integration:* `StateDB` journal hooks.
   * *Target Reth integration:* `BundleState` transition stage hooks.
4. **Cross-Language Verification:** Tooling to verify and test hash vector equivalence across Rust and Go boundary environments is planned but not yet implemented.

---

## Testing & Fixtures

Our reference test vectors and multi-client execution fixtures (evaluating boundary cases like EIP-1153 transient storage, reentrancy rollbacks, and nested account destructions) are managed in the companion dataset repository: [`Sahek/kaysentinel-fixtures`](https://huggingface.co/datasets/Sahek/kaysentinel-fixtures).

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.
