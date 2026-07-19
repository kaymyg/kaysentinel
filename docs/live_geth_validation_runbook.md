# KAYSentinel Live-Geth Validation Runbook

This runbook covers the one remaining phase that cannot be done by
source-reading and sandboxed testing: running the real tracer inside a real
Geth node and feeding actual traces through the full pipeline. Every open
question in the project's docs that is marked "requires runtime validation"
is resolved by one of the capture steps below.

## 0. Hardware Reality Check (read this first)

This entire runbook is designed for a modest laptop (tested assumptions:
~8 GB RAM, integrated graphics, ~100 GB free disk). Key facts:

- **`geth --dev` is NOT mainnet syncing.** It creates a private, single-node,
  in-memory chain. No multi-hundred-GB download, no days of syncing, no
  sustained disk thrash. RAM usage is a few hundred MB; CPU is near-idle
  except in the moment a transaction executes.
- The heaviest single operation in this runbook is `cargo build` of the Rust
  workspace: a few minutes of CPU, roughly 2-3 GB of disk in `target/`.
  It happens once; subsequent builds are incremental and fast.
- `go build` of the tracer takes seconds.
- **Cleanup is built in** (§6) so nothing lingers eating disk afterward.
- Nothing here uses the GPU at all.

If the laptop gets warm during `cargo build`, that is normal compilation
load, not damage. If you want to be extra gentle, run
`cargo build -j 2` to cap it at 2 parallel jobs.

## 1. One-Time Setup

### 1.1 Install Go (if not present)
Download from https://go.dev/dl/ (Windows x64 installer, ~250 MB installed).
Verify: `go version` (need 1.22+; the repo's go.mod says 1.24 — install the
latest stable).

### 1.2 Install Geth
Download the Windows binary from https://geth.ethereum.org/downloads
(Geth-only archive is enough, ~50 MB). Unzip somewhere convenient, e.g.
`C:\geth\`. Verify: `C:\geth\geth.exe version`.

### 1.3 Install Rust (if not present)
https://rustup.rs — default settings. Verify: `cargo --version`.

### 1.4 Get the repo locally
You already have `kaysentinel_clone`. Pull latest:
```powershell
cd "C:\Users\karan\Downloads\KAYSENTINEL FULL REPO\kaysentinel_clone"
git pull
```

### 1.5 Sanity-build both sides once
```powershell
# Go side (seconds):
go build ./...

# Rust side (few minutes, once; use -j 2 to be gentle on the machine):
cd runtime
cargo test -j 2
```
Expected: `go build` silent success; `cargo test` reports 74 passing, 0
failed. If either fails BEFORE you've touched Geth, stop and fix that first
— it means the local checkout is broken, not the tracer.

## 2. Wiring the Tracer into Geth

The tracer (`tracer/kaysentinel_tracer.go`) implements go-ethereum's
`tracing.Hooks` interface, designed for Geth's live-tracing plugin mechanism.
Geth's release binaries don't load arbitrary plugins, so the practical path
is building a tiny harness binary that embeds both Geth's dev-node machinery
and the tracer. **Simpler alternative for a first pass:** use the existing
`harness/harness.go` in the repo, which already drives the tracer against
`go-ethereum`'s in-process simulated backend — no external geth.exe needed
at all. Check what it currently supports:

```powershell
go run ./harness --help
```

If the harness already executes transactions against a simulated backend,
Captures A-C below can be done entirely in-process — even lighter on the
machine than running geth.exe. Only fall back to a real `geth --dev` node if
the harness turns out to be fixture-generation-only.

### 2.1 Real geth --dev fallback (if needed)
```powershell
C:\geth\geth.exe --dev --http --http.api eth,web3,personal --datadir C:\geth\devchain
```
Leave this running in one terminal. It pre-funds a developer account and
mines instantly on demand. RAM: ~300-500 MB.

## 3. The Captures (what to run, and which question each answers)

For each capture: run the transaction, save the tracer's emitted
FixtureEnvelope JSON to `captures/<name>.json`.

### Capture A — Plain EOA-to-EOA transfer
Send ETH from the dev account to a fresh address. No contract involved.

**Resolves:**
- The frameless-mutation taxonomy's core prediction (docs/emes/004 §3):
  expect `BalanceMutation` events with `FrameID == 0xFFFF...FFFF` (sentinel)
  for gas debit / refund / coinbase fee, and possibly zero `FrameEnter`
  events at all. If mutation events appear with the sentinel FrameID, the
  T0 retraction is empirically confirmed (upgrading it from static-audit
  authority to empirical/High).
- Whether a plain transfer produces a root frame or not (open question from
  the T0 discussion — the answer is currently unknown even after source
  reading).

### Capture B — Contract deployment + a state-writing call
Deploy any trivial storage contract (a `set(uint)` / `get()` pair is enough,
compiled with solc or Remix), then call `set(7)`.

**Resolves:**
- `StorageMutationEvent` exhaustiveness (docs/emes/004 §3, the one row
  marked "high confidence, not exhaustive"): every StorageMutation in the
  capture should carry a real (non-sentinel) FrameID. A single
  sentinel-FrameID StorageMutation would falsify the "Always Frame-Scoped"
  classification — that's exactly what this capture exists to check.
- Whether `NonceChangeContractCreator` fires inside the root frame as the
  static audit of evm.go predicted (real FrameID, not sentinel).

### Capture C — A deliberate revert
Call a contract function that always reverts (add `function boom() { revert(); }`),
with enough gas that execution genuinely enters the EVM.

**Resolves:**
- Invariant T1 against reality: `TransactionEndEvent.Reverted` must equal
  the root `FrameExitEvent.Reverted` (both true). Run the capture through
  Gate 1 (§4) — it should pass. If it errors on `t1-consistency`, either
  the tracer or the invariant is wrong in a way source-reading missed.
- The bridge's revert-discard behavior on real data (§5).

### Capture D (optional, stretch) — Nested call with caught child revert
A contract that calls another contract which reverts, catches the failure,
and continues. Resolves the child-revert/parent-commit case (currently only
covered by hand-written fixtures in gate1_test.go and the bridge tests).

## 4. Validate Every Capture Through Gate 1

Write a tiny Go main (or extend the harness) that reads a capture JSON,
reconstructs the event stream, and calls
`validation.VerifyGate1Invariants(stream)`.

Note: `emes.FixtureEnvelope` deliberately has no `UnmarshalJSON` — the
consumer reconstructing events from the `"type"` tag was left unwritten
(the comment in types.go says so explicitly). Writing that ~50-line tagged
decoder in Go is the one small piece of new code this runbook requires.
Model it on the Rust side's `runtime/bridge/src/wire.rs`, which already
implements exactly this tagged-decode logic for the same wire format.

**Expected:** all captures pass. Any failure is a genuine finding — record
which rule fired and at which event index (Gate1Error carries both).

## 5. Run Every Capture Through the Full Rust Pipeline

The full path already exists and is tested end-to-end on synthetic fixtures
(`runtime/verify/tests/full_pipeline.rs`). Adapt that test into a small
binary or test that reads a capture file:

```rust
let json = std::fs::read_to_string("captures/capture_a.json")?;
let go_events = kaysentinel_bridge::parse_event_stream(&json)?;
let cse = kaysentinel_bridge::translate(&go_events, &BridgeConfig { chain_id: 1337 })?;
// ...then the exact same stage sequence as full_pipeline.rs:
// partition -> reduce -> resolve -> certificates -> storage roots ->
// ExecutionBatchReplay -> replay_and_verify
```
(`--dev` chains use chain_id 1337.)

**Expected:** `replay_and_verify` returns `Ok(())` for every capture. Any
`Err` is a real cross-language disagreement — the exact class of bug this
whole project exists to detect. Record the full error value; the error
types (`TraceInconsistency`, `CertificateMismatch { mismatches }`, etc.)
are designed to localize the divergence to a specific address and field.

## 6. Cleanup (keep the small disk happy)

```powershell
# Rust build artifacts (~2-3 GB) — safe to delete, rebuilt on demand:
cd runtime && cargo clean

# Geth dev chain data (only if you used the real-geth fallback):
Remove-Item -Recurse -Force C:\geth\devchain

# Go build cache if you want the space back (~500 MB - 1 GB):
go clean -cache
```

## 7. Feeding Results Back Into the Docs

Every capture upgrades specific authority grades in the committed docs:

| Finding confirmed | Doc row to upgrade | From -> To |
|---|---|---|
| Sentinel-FrameID balance mutations observed (Capture A) | docs/emes/004 §3, BalanceMutationEvent frameless rows | Medium (static audit) -> High (empirical) |
| Zero sentinel-FrameID StorageMutations across all captures | docs/emes/004 §3, StorageMutationEvent row | "high confidence, not exhaustive" -> Verified (empirical) |
| T1 holds on the revert capture (Capture C) | docs/emes/003 §1, Invariant T1 | Medium -> High |
| Bridge + Gate 2 agree on real data (§5, all captures) | conformance.md INV-001/INV-002 authority note | "Rust side only" caveat can note real-trace validation |

If any capture instead **falsifies** a classification, that's a bigger and
more valuable finding: open a reconciliation-log entry (RL-xxxx) per
docs/emes/reconciliation_log.md's format, record the falsifying capture
file, and re-derive the affected taxonomy row before touching any code.

## 8. Known Limitations That This Runbook Does NOT Resolve

Stated so nothing silently disappears:
- **EIP-7702 multi-authorization mid-batch gas exhaustion** — the accepted
  V1 information-loss boundary (docs/emes/004 §3). A dev-mode capture of
  this exact path is impractical; it stays a documented limitation.
- **`block_hash`** — still absent from the Go event model; bridge output
  will still carry the documented zero placeholder.
- **Reth-side extractor** — multi-client portability remains demonstrated
  on one client only until a Reth adapter emitting the same emes.Event
  types exists.
