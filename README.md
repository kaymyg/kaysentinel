---
license: mit
tags:
  - ethereum
  - formal-methods
  - evm
---

# Kaysentinel / PCAL + Sentinel

A post-execution authorization framework for state-machine systems (e.g. Ethereum-class runtimes). Kaysentinel lifts execution traces into a canonical **Structural Sufficient Representation (SSR)** and evaluates policy purely over that quotient space, so authorization decisions are provably independent of *which* client (Geth, Reth, ...) produced the trace.

> **Status:** formal specification, differential-test harness skeleton, and a working Rust reference implementation of the canonical event ABI, the builder pipeline (partition → reduce → lifecycle resolution → certificate assembly), and domain-separated cryptographic commitments. There is not yet a working Geth/Reth extractor, SSZ canonical encoding, or storage-trie derivation — see the roadmap below for exactly what's real vs. still open.

## Repo contents

- [`docs/framework.md`](docs/framework.md) — full formal spec: execution model, SSR canonical form, authorization/factorization theory, multi-client portability theorem, gating semantics, complexity model.
- [`docs/differential_testing.md`](docs/differential_testing.md) — the differential testing engine design used to check byte-level convergence between a Geth-side and Reth-side SSR extractor.
- [`docs/implementation_roadmap.md`](docs/implementation_roadmap.md) — concrete engineering tasks (Phase 1–3) to turn the spec into a working implementation.
- [`docs/semantic_architecture_spec.md`](docs/semantic_architecture_spec.md) — formal stage contracts (partition → projection → canonicalization → serialization → commitment) with real implementation cross-references.
- [`docs/hash_specification.md`](docs/hash_specification.md) — normative cryptographic commitment protocol (domain-separated BLAKE3), implemented in `runtime/hash`.
- [`runtime/`](runtime/) — the Rust reference implementation. See below.
- Curated test fixtures now live in a separate dataset repo: [`Sahek/kaysentinel-fixtures`](https://huggingface.co/datasets/Sahek/kaysentinel-fixtures).
- [`scripts/verify_ssr.py`](scripts/verify_ssr.py) — standalone verifier that hashes and byte-diffs two SSZ-encoded SSR outputs.
- [`tests/transient_storage_case.json`](tests/transient_storage_case.json) — example test vector (EIP-1153 transient storage + reentrancy rollback).

## `runtime/` — Rust reference implementation

A Cargo workspace. Status per crate, honestly:

| Crate | Status |
| --- | --- |
| `cse` | **Done.** Canonical Semantic Event ABI: frozen event/payload types, execution context, a sequence/transaction-boundary validator. |
| `builder` | **In progress, core pipeline works.** Partitions a CSE stream into flat relational tables (`partition.rs`), reduces per-key timelines to terminal transitions (`reduce.rs`), resolves account lifecycle generations (`lifecycle/resolve.rs`, `canonicalize.rs`), and assembles canonical account certificates from real state + generation data (`lifecycle/certificate.rs`, `hydration.rs`). 21 passing unit tests across the workspace. |
| `hash` | **Core primitive done.** Domain-separated BLAKE3 commitments (`derive_commitment`), per the RC1 spec. 8 passing tests, including real (not hand-typed) pinned vectors — see `runtime/hash/vectors/candidate.json`. Not yet cross-language-verified (no Python/Go implementation exists to check against), so nothing has been promoted to `normative.json`. |
| `ssr` | Empty stub. |
| `ssz` | Empty stub — canonical encoding doesn't exist yet, so nothing has real bytes to hash yet either. |
| `verify` | Empty stub. |

Known open gaps (tracked in `docs/semantic_architecture_spec.md`): no real `AccountSnapshotSource` (prior-block state) implementation; storage roots are always `AwaitingDerivation` (no Phase 5 trie construction); access-list touches aren't tracked; several `TraceProvenance`/generation-chain fields (`frame_ordinal`, `ProvenanceMetadataDivergence`, `MalformedProvenanceChain`) are defined but not yet wired to real detection logic.

## Roadmap

See [`docs/implementation_roadmap.md`](docs/implementation_roadmap.md) for the full phased task breakdown. Short version:

**Phase 1 — Canonical type system**
- [x] Canonical Semantic Event ABI (`runtime/cse`)
- [ ] SSZ schema + serialization (`runtime/ssz` — stub only)

**Phase 2 — Client extractors**
- [ ] Geth-side extractor (`StateDB` journal hook)
- [ ] Reth-side extractor (`BundleState` hook)

**Phase 3 — Differential testing infrastructure**
- [x] Verification harness (`scripts/verify_ssr.py`) — comparison logic only, done
- [ ] Test runner environment + GitHub Actions CI

**Phase 4 — Builder pipeline (Rust reference implementation)**
- [x] Partition + terminal projection (`runtime/builder/src/partition.rs`, `ir/timeline.rs`)
- [x] Lifecycle resolution + canonicalization (`runtime/builder/src/lifecycle/`)
- [x] Certificate hydration + assembly (`runtime/builder/src/lifecycle/{certificate,hydration}.rs`)
- [ ] Storage subtree/trie derivation (`StorageRootDeriver` trait exists, no implementation)

**Phase 5 — Cryptographic commitment**
- [x] Domain-separated BLAKE3 commitment primitive (`runtime/hash`)
- [ ] Cross-language (Python/Go) differential verification of vectors
- [ ] Wire real commitments into `VerifiedGeneration.state_table_proof_root` and `CanonicalAccountCertificate.storage_root`

## License

MIT — see [`LICENSE`](LICENSE).
