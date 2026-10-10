# KAYSentinel (PCAL + Sentinel)

[![CI](https://github.com/kaymyg/kaysentinel/actions/workflows/ci.yml/badge.svg)](https://github.com/kaymyg/kaysentinel/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust tests](https://img.shields.io/badge/rust%20tests-77%20passing-brightgreen)](runtime/)
[![CodeQL](https://github.com/kaymyg/kaysentinel/actions/workflows/codeql.yml/badge.svg)](https://github.com/kaymyg/kaysentinel/actions/workflows/codeql.yml)

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
│   ├── framework.md                    # Formal mathematical & semantic specification
│   ├── semantic_architecture_spec.md   # Stage-by-stage contract details & engineering gaps
│   ├── semantic_contract.md            # Normative semantic contract
│   ├── implementation_roadmap.md       # Phase 1–3 development execution plan
│   ├── emes_profile.md                 # EMES-V1 wire format, corrected against real go-ethereum source
│   ├── emes/                           # EMES v1.0 governance: spec, conformance, ADR register, reconciliation log/matrix
│   ├── ssz_specification.md / ssz_profile.md   # SSZ RC1 schema & serialization strategy
│   ├── hash_specification.md           # Domain-separated BLAKE3 commitment scheme
│   ├── validation_vector_spec.md       # Frozen v1.0.0 validation vector schema
│   ├── differential_testing.md         # Differential validation design
│   └── live_geth_validation_runbook.md # Runbook for the one phase that needs a real Geth node
├── runtime/                            # Rust workspace (8 crates)
│   ├── cse/                            # Canonical Semantic Event ABI definitions
│   ├── builder/                        # Timeline reduction, lifecycle/generation resolution, storage-root derivation
│   ├── protocol/                       # Canonical protocol definitions & invariant checkers
│   ├── ssr/                            # Structural Sufficient Representation types
│   ├── hash/                           # BLAKE3 domain-separated hashing (kaysentinel-hash)
│   ├── ssz/                            # SSZ RC1 encode + decode (kaysentinel-ssz)
│   ├── bridge/                         # Go-tracer-shaped JSON → CanonicalSemanticEvent translation
│   └── verify/                         # Gate 2: semantic replay + certificate verification
├── emes/                               # Go: EMES-V1 wire event types (no Geth dependency)
├── tracer/                             # Go: Geth-specific collector implementing core/tracing.Hooks
├── validation/                         # Go: Gate 1 structural verifier + differential-testing engine
├── harness/                            # Go: runs Gate 1, writes FixtureEnvelope files to disk
├── raw/transcript/                     # Go: flat RawEvent capture (separate, not-yet-merged event pipeline — see below)
├── multiplexer/                        # Go: fans one tracing.Hooks callback out to multiple sinks, panic-isolated
├── schemas/                            # JSON schemas: validation vectors, SSZ conformance manifest
├── scripts/                            # Python reference implementations (SSZ, SSR verification) for cross-checking
└── tests/
    └── transient_storage_case.json     # Reference EIP-1153 validation test vector
```

There are currently **two separate, non-integrated Go event-collection paths**
into the pipeline: the EMES-V1 path (`emes/` + `tracer/` + `validation/gate1.go`
+ `harness/`), and a newer flat-`RawEvent` path (`raw/transcript/` +
`multiplexer/` + `validation/normalizer.go` + `validation/engine.go`). Both
compile and are tested independently; whether to merge them or keep both for
different purposes is an open decision, documented as such in
`raw/transcript/README.md`.

---

## Active Codebase & Implementation Status

KAYSentinel is a functional, actively tested implementation spanning a Rust
runtime and a Go collection/validation layer. Below is the verified current
status (test counts confirmed by running the suites directly, not carried
over from an earlier snapshot).

### Rust runtime — 77 passing tests across the workspace

* **`runtime/cse` & `runtime/ssr`:** Canonical Semantic Event ABI and SSR type
  definitions consumed by every other crate.
* **`runtime/builder`:** The state-reduction engine — partitions and
  sequences raw incoming events, reduces timelines to terminal transitions,
  resolves lifecycles (contract creation, self-destruction) into immutable
  "generations," and derives storage roots via `SimpleStorageRootDeriver`.
  * *Status:* **21 passing tests** (19 unit + 2 conformance).
  * The storage-root deriver produces a deterministic, protocol-native
    BLAKE3 commitment over touched slots — **not** an Ethereum-compatible
    Merkle-Patricia Trie root (that would require Keccak256 + RLP, neither
    of which this codebase implements; see the design note in
    `runtime/builder/src/lifecycle/storage_root.rs`). If real MPT
    compatibility is ever required, this deriver gets replaced, not
    extended — that's exactly what the `StorageRootDeriver` trait boundary
    is for.
* **`runtime/protocol`:** Canonical protocol definitions, error-handling
  bounds, core state invariants. **2 passing tests.**
* **`runtime/hash` (`kaysentinel-hash`):** Domain-separated BLAKE3 commitment
  primitive. **8 passing tests with pinned vectors.**
* **`runtime/ssz` (`kaysentinel-ssz`):** SSZ RC1 encode **and decode**, with
  round-trip tests (`decode(encode(x)) == x`) plus explicit rejection tests
  for truncated, oversized, and trailing-garbage input, including precise
  off-by-one/nil/overflow offset boundary cases modeled on
  [`ethereum/consensus-spec-tests`](https://github.com/ethereum/consensus-spec-tests)'
  `ssz_generic/containers/invalid/` category. **26 passing tests.**
* **`runtime/bridge` (`kaysentinel-bridge`):** Parses Go-tracer-shaped JSON
  event streams and translates them into `CanonicalSemanticEvent`s — the
  actual Go→Rust boundary. **7 passing integration tests.**
* **`runtime/verify` (`kaysentinel-verify`):** Gate 2 — semantic replay and
  certificate verification (`replay_and_verify`, `ExecutionBatchReplay`).
  Independently re-derives and checks storage roots, catches missing or
  extra certificates, wrong-domain commitments, and tampered bytes.
  **11 passing tests.**
* **Capstone integration test** (`runtime/tests/full_pipeline.rs`): a
  Go-tracer-shaped JSON stream through every real stage — parse → translate
  → partition → reduce → resolve lifecycles → build certificates + derive
  storage roots → Gate 2 verify — with no mocks or simulated stand-ins.
  **2 passing tests** (including a reverted-frame case).

### Go collection & validation layer

* **`emes/` + `tracer/` + `validation/gate1.go` + `harness/`:** The Geth
  tracer implements `core/tracing.Hooks` against real `go-ethereum@v1.16.9`
  types, compiles and vets clean, and was run end-to-end against a
  hand-constructed synthetic transaction — it collected a full EMES-V1
  stream (including a correctly-correlated `SelfDestructEvent`), Gate 1
  passed it, and the harness wrote a real fixture file to disk. Gate 1 was
  also fed two deliberately broken streams and correctly rejected both.
  **13 Go tests** covering Gate 1, including duplicate and mid-stream block
  markers.
* **`raw/transcript/` + `multiplexer/` + `validation/normalizer.go` +
  `validation/engine.go`:** A separate flat-`RawEvent` capture path with its
  own differential validator, also compiling/vetting clean and exercised
  end-to-end (panic isolation across multiple hook sinks, confirmed eviction,
  a deliberately injected field mismatch correctly caught).
* **Not yet done:** registration as a live tracer against a real running
  Geth node, a real (non-synthetic) EVM transaction trace, and Reth
  integration. `docs/live_geth_validation_runbook.md` is the runbook for
  exactly this remaining phase — it's infrastructure/hardware work, not
  unwritten code.

> **Note on running the Go tests yourself:** this repo's `go.mod` requires
> Go 1.24+ and real dependencies (`go-ethereum`, `holiman/uint256`) fetched
> from the module proxy. If your environment blocks that proxy, `go build`
> will fail with a 403 rather than a code error — that's a network policy
> issue, not a broken checkout.

### Current Gaps & Near-Term Roadmap

1. **Live Geth integration.** The tracer, Gate 1, and the full Rust pipeline
   (through Gate 2) are all real and tested against synthetic/simulated
   input. Running the actual tracer against a live `geth --dev` node and
   feeding a real transaction trace through the whole pipeline has not been
   done yet — see `docs/live_geth_validation_runbook.md`.
2. **Reth integration.** No `BundleState` transition-stage hooks exist yet;
   Geth is currently the only supported client.
3. **Storage root is protocol-native, not Ethereum-MPT-compatible.** See the
   design note above — this is a deliberate interim choice, not an oversight,
   but it means KAYSentinel's storage commitments can't currently be compared
   directly against a client's real state root.
4. **The two Go event-collection paths aren't merged.** See "Repository &
   Runtime Architecture" above.
5. **Cross-language verification exists for synthetic/simulated JSON, not
   yet for a live-captured Geth trace.** `full_pipeline.rs` proves the Rust
   side of the Go→Rust boundary works end-to-end; closing gap #1 above is
   what would let that same pipeline run on real captured data.

---

## Related Work

KAYSentinel's cross-client conformance goal has real precedent worth being
explicit about:

- **[`ethereum/hive`](https://github.com/ethereum/hive)** is the Ethereum
  Foundation's own cross-client integration test harness — it verifies that
  independent execution clients agree at the RPC / Engine API / consensus
  level. KAYSentinel targets a different, lower layer (canonical state
  *mutations*, not RPC responses), but the goal — independent
  implementations provably agreeing — is the same. A KAYSentinel simulator
  plugged into Hive is a plausible path to real multi-client trace data
  instead of synthetic vectors, once live-node integration (see "Current
  Gaps") is further along.
- **Official EVM test vectors.** The modern source of official,
  client-generated state-transition test fixtures (formerly
  `ethereum/execution-spec-tests` / EEST) was archived and folded into
  [`ethereum/execution-specs`](https://github.com/ethereum/execution-specs),
  which now publishes fixture releases directly. Worth pointing anyone
  contributing test vectors at the current location rather than the
  archived repo.
- **SSZ.** `runtime/ssz` implements "SSZ RC1" — a deliberate variant, not
  the real Ethereum consensus SSZ spec (domain-separated BLAKE3 instead of
  SHA256 hash-tree-root; see the design note in
  `runtime/builder/src/lifecycle/storage_root.rs` for why). The official
  [`ethereum/consensus-spec-tests`](https://github.com/ethereum/consensus-spec-tests)
  suite's hash outputs therefore don't apply here, but its documented
  *structural* edge-case taxonomy (off-by-one and nil offsets, oversized
  lists, trailing bytes) informed additional boundary tests in
  `runtime/ssz/src/decode.rs`.

---

## Testing & Fixtures

Our reference test vectors and multi-client execution fixtures (evaluating boundary cases like EIP-1153 transient storage, reentrancy rollbacks, and nested account destructions) are managed in the companion dataset repository: [`Sahek/kaysentinel-fixtures`](https://huggingface.co/datasets/Sahek/kaysentinel-fixtures).

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## Contributing

Bug reports, new test vectors, and PRs are welcome — see
[`CONTRIBUTING.md`](CONTRIBUTING.md) for how to build both sides of the
codebase and what CI checks.

## Security

Found a case where two clients' SSRs should differ but the canonical hash
matches (or vice versa), or a way to defeat Gate 1/Gate 2 verification?
Please see [`SECURITY.md`](SECURITY.md) for how to report it responsibly
rather than opening a public issue.

## Changelog

See [`CHANGELOG.md`](CHANGELOG.md).
