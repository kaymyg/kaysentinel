# Implementation Roadmap

Concrete engineering tasks to turn the [framework spec](framework.md) and [differential testing design](differential_testing.md) into working code. Nothing here is implemented yet — see status per phase below.

## Phase 1 — Canonical Type System & Serialization

The serialization layer needs to be settled before client logic begins, to prevent format drift.

- [ ] **1.1 Define the strict SSZ schema.** Draft the official SSZ definitions for `CanonicalSSR` and its sub-containers. Enforce little-endian byte ordering for integers and explicit padding rules for `Uint256` (32-byte arrays).
- [ ] **1.2 Implement serialization libraries.**
  - **Go:** integrate an SSZ library (e.g. [`fastssz`](https://github.com/ferranbt/fastssz)) to generate static encoder/decoder methods for the Go-side SSR types.
  - **Rust:** configure an SSZ crate (e.g. `ssz` or `lighthouse_types`) to derive encoding logic on the Rust structs.

## Phase 2 — Client Extractor Modules

### Geth extractor (Go)

- [ ] **2.1 Locate lifecycle hooks.** Isolate the injection point in `core/state/state_processor.go`, immediately following the `ApplyTransaction` execution loop.
- [ ] **2.2 Extract state journal elements.** Iterate the internal dirty-account map (`sdb.GetDirtyAddresses()`); sort `AccountMutations` by address (`sort.Slice`) before invoking the SSZ encoder.
- [ ] **2.3 Access transient memory state.** Read `sdb.TransientStorage()` immediately prior to execution-state finalization.

### Reth extractor (Rust)

- [ ] **3.1 Intercept execution outputs.** Inject the extraction module where `BlockExecutionResult` yields its finalized in-memory `BundleState`.
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

## Current status summary

| Phase | Item | Status |
|---|---|---|
| 1 | SSZ schema definition | Not started |
| 1 | Go/Rust serialization libs | Not started |
| 2 | Geth extractor | Not started |
| 2 | Reth extractor | Not started |
| 3 | Verification harness (`verify_ssr.py`) | Done (comparison logic only — see script docstring) |
| 3 | Test runner environment | Not started |
| 3 | CI automation | Not started |
