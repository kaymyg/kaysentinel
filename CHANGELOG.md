# Changelog

All notable changes to this project are documented here.
Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

This project has no prior version tag, so this file summarizes the existing
commit history (2026-07-05 through 2026-07-28) into milestones rather than
listing all ~50 commits individually, then marks today's additions as the
first tagged release.

## [0.1.0] - 2026-07-28

First tagged release. KAYSentinel is pre-1.0: the Rust pipeline and the
simulated/synthetic-input Go layer are real and tested, but live-Geth and
Reth integration are not done yet (see README's "Current Gaps").

### Milestones included in this release

- **Framework & specs** (Jul 5–6): initial formal framework, normative
  `semantic_contract.md` v1.0.0, frozen validation vector schema.
- **Go EMES-V1 collection layer** (Jul 7–8): real Gate 1 structural
  verifier, Geth tracer against `core/tracing.Hooks`, harness, and a
  separate flat-`RawEvent` transcript + multiplexer + differential
  validator path — tested end-to-end, including deliberate hook-signature
  bug fixes checked against real `go-ethereum` source rather than assumed.
- **Rust runtime workspace** (Jul 11–18): `cse`, `builder` (lifecycle
  resolution, generations, `SimpleStorageRootDeriver`), `protocol`,
  `kaysentinel-hash` (BLAKE3 domain separation), `kaysentinel-ssz` (SSZ RC1
  encoding + Merkle profile).
- **Gate 2 + cross-language bridge** (Jul 19): `runtime/bridge` (Go-JSON →
  `CanonicalSemanticEvent`), `runtime/verify` (semantic replay + certificate
  verification), and the capstone `full_pipeline` integration test taking a
  Go-tracer-shaped stream through every real stage to a Gate 2 pass.
- **SSZ decode + live-Geth runbook** (Jul 19): round-trip decode tests for
  every encoder, plus `docs/live_geth_validation_runbook.md` for the one
  remaining phase requiring a real node.
- **CI + Gate 1 hardening** (Jul 28): CI workflow (Go build/vet/test, Rust
  build/test), `uint256` dependency fix, block-encapsulation enforcement in
  Gate 1, `BlockCommit` emission fix, tests for duplicate/mid-stream block
  markers.

### Added (this release, on top of the above)

- LICENSE was already present and correct; verified.
- CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, issue templates
  (including a soundness-report template), PR template, Dependabot
  (cargo + gomod + github-actions).
- README: rewrote the "Repository & Runtime Architecture" tree and "Active
  Codebase & Implementation Status" sections, which had drifted
  significantly behind the actual codebase — see "Fixed" below.

### Fixed

- README's architecture diagram only listed 6 of the 8 Rust crates
  (missing `ssr` and `bridge`) and none of the Go-side directories
  (`emes/`, `tracer/`, `validation/`, `harness/`, `raw/transcript/`,
  `multiplexer/`, `schemas/`, `scripts/`) — updated to reflect the real
  repository layout, confirmed by directory listing.
- README's "Active Codebase" section only credited `hash` (8 tests) and
  `builder` (21 tests) — both counts were accurate, but `ssz` (25 tests),
  `verify` (11 tests), `bridge` (7 tests), and the `full_pipeline`
  integration test (2 tests) went unmentioned. Verified all 76 tests pass
  by running `cargo test --workspace` directly.
- README's "Current Gaps" claimed SSZ serialization was incomplete, storage
  root derivation was unimplemented (`AwaitingDerivation` only), Geth/Reth
  hooks were "not yet written," and cross-language verification was
  "planned but not implemented." All four were stale: SSZ decode exists
  with round-trip tests, `SimpleStorageRootDeriver` is real (though
  intentionally not Ethereum-MPT-compatible — a design choice, not a gap),
  the Geth tracer compiles/vets clean and passed an end-to-end synthetic
  run, and `full_pipeline.rs` is a real, passing Go-JSON-to-Gate-2 test.
  Rewrote the gaps list to reflect what's actually still open: live-node
  integration, Reth support, and merging the two parallel Go event paths.
- `docs/live_geth_validation_runbook.md` said "cargo test reports 74
  passing" — corrected to the verified current count of 76.
- Stripped Hugging Face Hub YAML frontmatter from README (GitHub renders it
  literally instead of hiding it).
- `runtime/Cargo.lock` was stale — missing `kaysentinel-bridge` and its
  dependency edges entirely (and a few transitive crates like `serde_json`,
  `memchr`), meaning it hadn't been regenerated since that crate was added.
  Regenerated via `cargo build --workspace`.
