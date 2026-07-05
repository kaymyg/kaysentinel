---
license: mit
tags:
  - ethereum
  - formal-methods
  - evm
---

# Kaysentinel / PCAL + Sentinel

A post-execution authorization framework for state-machine systems (e.g. Ethereum-class runtimes). Kaysentinel lifts execution traces into a canonical **Structural Sufficient Representation (SSR)** and evaluates policy purely over that quotient space, so authorization decisions are provably independent of *which* client (Geth, Reth, ...) produced the trace.

> **Status:** conceptual specification + differential-test harness skeleton. There is not yet a working Geth/Reth extractor implementation — the theorems below describe the target invariants that any implementation must satisfy, they are not machine-checked proofs.

## Repo contents

- [`docs/framework.md`](docs/framework.md) — full formal spec: execution model, SSR canonical form, authorization/factorization theory, multi-client portability theorem, gating semantics, complexity model.
- [`docs/differential_testing.md`](docs/differential_testing.md) — the differential testing engine design used to check byte-level convergence between a Geth-side and Reth-side SSR extractor.
- [`scripts/verify_ssr.py`](scripts/verify_ssr.py) — standalone verifier that hashes and byte-diffs two SSZ-encoded SSR outputs.
- [`tests/transient_storage_case.json`](tests/transient_storage_case.json) — example test vector (EIP-1153 transient storage + reentrancy rollback).

## Roadmap (not yet implemented)

- [ ] Geth-side extractor (`StateDB` journal hook)
- [ ] Reth-side extractor (`BundleState` hook)
- [ ] SSZ canonical encoder for `Δ` (the SSR type)
- [ ] CI job running `scripts/verify_ssr.py` against both extractors per test vector

## License

MIT — see [`LICENSE`](LICENSE).
