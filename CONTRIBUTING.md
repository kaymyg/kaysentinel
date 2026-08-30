# Contributing to KAYSentinel

Thanks for your interest. This project's core claim is that its canonical
representation is faithful and client-agnostic — contributions that
strengthen that claim (closing a verification gap, adding a test vector that
exposes a real discrepancy) are especially valuable.

## Getting set up

### Rust runtime

```bash
cd runtime
cargo build --workspace
cargo test --workspace   # 76 tests across 8 crates
```

No special setup needed — standard `cargo`, stable toolchain.

### Go collection/validation layer

```bash
go build ./...
go vet ./...
go test ./... -v
```

Requires **Go 1.24+** (go-ethereum's `go.mod` uses a `tool` directive older
Go can't parse) and network access to the Go module proxy to fetch
`go-ethereum` and `holiman/uint256`. If `go build` fails with a 403 from
`proxy.golang.org`, that's a network policy blocking the proxy, not a code
problem — check your environment's egress settings before assuming
something's broken.

Pin to a stable go-ethereum release rather than `master` — see
`docs/emes_profile.md` §5.4 for why.

## Before opening a PR

CI runs `go build`, `go vet`, `go test ./... -v` on the Go side and
`cargo build --workspace` + `cargo test --workspace` on the Rust side, on
every push and PR. Please run both locally first if your change touches
that language.

If your change affects a documented test count, gap, or status claim
anywhere in `README.md` or `docs/`, update it in the same PR — this
codebase has a real history of docs drifting out of sync with what's
actually implemented (see the "Active Codebase & Implementation Status"
section, which was rewritten from a stale snapshot after being checked
against the actual passing test suites).

## What especially matters here

- **Precision over confidence.** The existing docs and code comments in this
  repo are unusually careful about distinguishing "implemented and tested,"
  "implemented but not yet validated against live data," and "not started."
  Match that register — don't claim something works because it compiles.
- **The `runtime/builder` storage-root deriver is intentionally not
  Ethereum-MPT-compatible.** Don't "fix" it to look more complete without
  reading the design note in `runtime/builder/src/lifecycle/storage_root.rs`
  first — real MPT compatibility means replacing the deriver behind the
  `StorageRootDeriver` trait, not patching this one.
- **The two Go event-collection paths** (`emes/`+`tracer/`+`validation/gate1.go`
  vs. `raw/transcript/`+`multiplexer/`) are intentionally both present right
  now — see `raw/transcript/README.md` for why. Don't unilaterally delete
  one; that's an open decision for the maintainer.
- **Gate 1 and Gate 2 are different layers.** Gate 1 (`validation/gate1.go`,
  Go) is structural verification, independent of what mutations mean
  semantically. Gate 2 (`runtime/verify`, Rust) is semantic replay
  verification. A PR fixing one should not conflate it with the other.

## Good places to start

- Wiring the tracer into a real running Geth node, per
  `docs/live_geth_validation_runbook.md` — the single most valuable
  contribution right now, since everything upstream of it is implemented
  and tested against simulated data
- Reth's `BundleState` transition-stage hooks (no Reth support exists yet)
- Deciding + implementing a merge of the two parallel Go event-collection
  paths, or documenting a clear reason to keep both
- More differential test vectors for the companion
  [`Sahek/kaysentinel-fixtures`](https://huggingface.co/datasets/Sahek/kaysentinel-fixtures)
  dataset

For anything larger, please open an issue first to discuss the approach.

## Commit style

Small, focused commits with a clear one-line summary. This repo's own
history is a good model — commits like "Add SSZ decoders with round-trip
tests; add live-Geth validation runbook" or "fix: emit BlockCommit from
OnBlockEnd; core/tracing has no OnStateUpdate hook" say exactly what changed
and why, not just "fix bug."
