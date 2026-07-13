# Implementation Roadmap

Concrete engineering tasks to turn the [framework spec](framework.md) and [differential testing design](differential_testing.md) into working code.

> **Update:** the Rust reference implementation (`runtime/`) now has real, tested code for the canonical event ABI and most of the builder pipeline (Phase 1.2's Rust half, partially) plus the cryptographic commitment primitive (Phase 5). Status per item below has been updated accordingly — everything not explicitly marked done is still not started.

## Phase 1 — Canonical Type System & Serialization

The serialization layer needs to be settled before client logic begins, to prevent format drift.

- [ ] **1.1 Define the strict SSZ schema.** Draft the official SSZ definitions for `CanonicalSSR` and its sub-containers. Enforce little-endian byte ordering for integers and explicit padding rules for `Uint256` (32-byte arrays).
- [ ] **1.2 Implement serialization libraries.**
  - **Go:** integrate an SSZ library (e.g. [`fastssz`](https://github.com/ferranbt/fastssz)) to generate static encoder/decoder methods for the Go-side SSR types.
  - **Rust:** configure an SSZ crate (e.g. `ssz` or `lighthouse_types`) to derive encoding logic on the Rust structs. **Not started** — `runtime/ssz` is still an empty stub crate. The canonical event *types* that would eventually be serialized do exist (`runtime/cse`), but nothing encodes them to SSZ bytes yet.

## Phase 1.5 — Rust Builder Pipeline (new, not in the original phase breakdown)

The event-to-certificate compiler pipeline that consumes a validated `CanonicalSemanticEvent` stream (from `runtime/cse`) and produces canonical per-account certificates. This sits logically between Phase 1 (types) and Phase 2 (extractors) — it's what a real extractor's output would eventually flow into, but it doesn't extract from a real client itself (see Phase 2 status, unchanged).

- [x] **Canonical Semantic Event ABI** (`runtime/cse`) — frozen event/payload types, execution context, sequence/transaction-boundary validation.
- [x] **Partition + terminal projection** (`runtime/builder/src/partition.rs`, `ir/timeline.rs`) — flattens an event stream into relational, `CanonicalKey`-keyed tables; reduces per-key timelines to verified terminal transitions.
- [x] **Lifecycle resolution + canonicalization** (`runtime/builder/src/lifecycle/{resolve,canonicalize}.rs`) — resolves account creation/destruction into identity "generations," with formal invariant verification (uniqueness, temporal well-formedness, chronological adjacency).
- [x] **Certificate hydration + assembly** (`runtime/builder/src/lifecycle/{certificate,hydration}.rs`) — cross-references real state facts (balance/nonce/code) to resolved generations to build `CanonicalAccountCertificate`s, with a real (if unimplemented) `AccountSnapshotSource`/`StorageRootDeriver` extension point for prior-state and Phase-5 storage-trie data.
- [ ] Storage subtree/trie derivation — `StorageRootDeriver` trait exists, no implementation. Every certificate's `storage_root` is currently `AwaitingDerivation`.
- [ ] Access-list tracking — `CsePayload::AccessListTouched` events are parsed but not recorded anywhere in the pipeline.

21 passing unit tests across `runtime/cse` + `runtime/builder`. See `docs/semantic_architecture_spec.md` for the full stage-by-stage contract writeup and honest gap list.

## Phase 2 — Client Extractor Modules

### Geth extractor (Go)

- [ ] **2.1 Locate lifecycle hooks.** Isolate the injection point in `core/state/state_processor.go`, immediately following the `ApplyTransaction` execution loop.
- [ ] **2.2 Extract state journal elements.** Iterate the internal dirty-account map (`sdb.GetDirtyAddresses()`); sort `AccountMutations` by address (`sort.Slice`) before invoking the SSZ encoder.
- [ ] **2.3 Access transient memory state.** Read `sdb.TransientStorage()` immediately prior to execution-state finalization.

### Reth extractor (Rust)

- [ ] **3.1 Intercept execution outputs.** Inject the extraction module where `BlockExecutionResult` yields its finalized in-memory `BundleState`. **Not started** — nothing in `runtime/` reads from a real Reth `BundleState` yet; the builder pipeline (Phase 1.5) consumes already-abstracted `CanonicalSemanticEvent`s, which still need to be produced by a real extractor hook.
- [ ] **3.2 Map BundleState structural diffs.** Map state maps to `AccountMutation` vectors; use `sort_by_key` to enforce the same lexicographic ordering as the Go side over addresses and slots.

## Phase 3 — Infrastructure & Differential Testing CI

```
                        [ Pull Request Pipeline ]
                                    │
                                    ▼
                     ┌─────────────────────────────┐
                     │  Generate Test Vector JSON  │
                     └──────────────┬──────────────┘
                                    │
            ┌───────────────────────┴───────────────────────┐
            ▼                                                ▼
┌───────────────────────┐                        ┌───────────────────────┐
│ Run Geth Extractor    │                        │ Run Reth Extractor    │
│ Output: geth_ssr.bin  │                        │ Output: reth_ssr.bin  │
└───────────┬───────────┘                        └───────────┬───────────┘
            │                                                │
            └───────────────────────┬────────────────────────┘
                                     │
                                     ▼
                     ┌─────────────────────────────┐
                     │    scripts/verify_ssr.py    │
                     │  - Enforce Binary Identity  │
                     └─────────────────────────────┘
```

- [x] **4.1 Deploy the verification harness.** `scripts/verify_ssr.py` is already in the repo root's `scripts/` folder.
- [ ] **4.2 Build the test runner environment.** A script that spins up ephemeral, lightweight test harnesses for both Geth and Reth, feeds them the same raw transaction/pre-state input, and captures their respective `.bin` outputs.
- [ ] **4.3 Automate CI.** A GitHub Actions workflow that runs the full test matrix on every pull request, and fails the build on any binary drift or size mismatch between the two extractor outputs.

## Phase 4 — Fuzzing & Production Benchmarking (future, not started)

Depends entirely on Phase 1 and Phase 2 being complete — there's no extractor to compare or benchmark yet, so nothing here should be built before that.

- [ ] `fuzz_seeds/` — high-entropy multi-account fuzzing inputs (irregular address/slot access patterns) to stress-test that Go's map iteration and Rust's `BTreeMap` always converge on the same sorted SSZ tree.
- [ ] `production_traces/` — real historical block extractions (e.g. via a tool like `cryo` against an archive RPC endpoint), used to validate the O(N) complexity claim in `docs/framework.md` section 7 against real-world block sizes.

See the companion dataset repo: [`Sahek/kaysentinel-fixtures`](https://huggingface.co/datasets/Sahek/kaysentinel-fixtures) for the curated Phase 3 test fixtures that exist today.

## Phase 5 — Cryptographic Commitment (new, not in the original phase breakdown)

- [x] **Domain-separated BLAKE3 commitment primitive** (`runtime/hash`) — `derive_commitment(domain, canonical_bytes) -> Digest`, per `docs/hash_specification.md` (RC1). 8 passing tests, including real computed-and-pinned vectors per domain.
- [ ] Cross-language (Python/Go) differential verification of vectors — no Python/Go implementation exists yet, so nothing in `runtime/hash/vectors/candidate.json` has been promoted to `normative.json`.
- [ ] Wire real commitments into `VerifiedGeneration.state_table_proof_root` and `CanonicalAccountCertificate.storage_root` — both are still placeholder/`AwaitingDerivation` values in the builder pipeline.

## Current status summary

| Phase | Item | Status |
|---|---|---|
| 1 | SSZ schema definition | Not started |
| 1 | Go/Rust serialization libs | Not started |
| 1.5 | Canonical Semantic Event ABI (`runtime/cse`) | **Done** |
| 1.5 | Builder pipeline: partition + terminal projection | **Done** |
| 1.5 | Builder pipeline: lifecycle resolution + canonicalization | **Done** |
| 1.5 | Builder pipeline: certificate hydration + assembly | **Done** |
| 1.5 | Storage subtree/trie derivation | Not started (`StorageRootDeriver` trait only) |
| 2 | Geth extractor | Not started |
| 2 | Reth extractor | Not started |
| 3 | Verification harness (`verify_ssr.py`) | Done (comparison logic only — see script docstring) |
| 3 | Test runner environment | Not started |
| 3 | CI automation | Not started |
| 5 | BLAKE3 commitment primitive (`runtime/hash`) | **Done** |
| 5 | Cross-language vector verification | Not started |
| 5 | Real commitments wired into builder pipeline | Not started |
