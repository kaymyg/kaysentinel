# KAY Sentinel Execution Mutation Event Specification (`EMES-V1`)

> **Status:** §1–3 (the wire event taxonomy) are unchanged and stable — they
> describe a client-agnostic telemetry format, not Geth internals, and
> nothing in them turned out to be wrong. §5 (the Geth collection mechanism)
> has been rewritten: the original scaffold targeted `vm.EVMLogger`, an
> interface that **no longer exists** in go-ethereum. The replacement in
> [`tracer/kaysentinel_tracer.go`](../tracer/kaysentinel_tracer.go) is
> compiled and `go vet`-clean against the real go-ethereum source (see
> §5.4) — not just written to look plausible.

## 1. Core Mandate & Replay Invariant

> **The EMES Replay Invariant:** Given an identical initial state and an
> identical `EMES-V1` event stream, any conforming semantic consolidation
> engine MUST reconstruct an identical canonical mutation set, entirely
> independent of the originating execution client architecture.

State access optimization metrics (e.g. EIP-2929 warm/cold transitions)
SHALL NOT be emitted. All 256-bit quantities in this telemetry layer MUST be
recorded big-endian (canonical Ethereum execution format); little-endian is
strictly deferred to the downstream SSZ layer (`ssz_profile.md`).

## 2. Global Stream Context & Call Frame Topology

- **Sequence:** every event carries a strictly increasing `uint64` counter
  starting at 0 per block. Sequence order is canonical; transport delivery
  order MUST be ignored. Overflow MUST abort and flag the block
  non-conforming.
- **FrameID:** unique only within a single transaction; resets to 0 at each
  new transaction. `FrameID = 0` is the top-level frame, with
  `ParentFrameID = 0xFFFFFFFFFFFFFFFF`. Sub-calls get sequentially
  incrementing FrameIDs.
- **FrameType:** `Call | CallCode | DelegateCall | StaticCall | Create |
  Create2`. Unmapped frame types are non-conforming.

## 3. Structural Event Taxonomy

Unchanged from the frozen draft — `BlockStartEvent`, `TransactionStartEvent`,
`FrameEnterEvent`, `FrameExitEvent`, `TransactionEndEvent`,
`BlockCommitEvent`, `BalanceMutationEvent`, `NonceMutationEvent`,
`CodeMutationEvent`, `StorageMutationEvent`, `AccountCreatedEvent`,
`SelfDestructEvent`. See `tracer/kaysentinel_tracer.go` for the canonical Go
struct definitions (kept identical to the original draft).

## 4. Operational Invariants

- **Block/Transaction Encapsulation:** exactly one `BlockStartEvent`
  (Sequence 0) and one terminal `BlockCommitEvent` per block; every
  `TransactionStartEvent` MUST be followed by exactly one matching
  `TransactionEndEvent` before another transaction begins.
- **State Application Bounds:** emit only on logical state mutation, never on
  internal client cache/DB-flush churn that doesn't change execution state.
- **Stream Immutability:** the event stream is append-only. Rollback on a
  reverted `FrameExitEvent`/`TransactionEndEvent` applies only to the
  downstream engine's ephemeral reconstruction cache, using the stream's
  `Before` values — the raw stream itself is never mutated or purged.

## 5. Geth Collection Mechanism

### 5.1 What was wrong with the original scaffold

The original design targeted `vm.EVMLogger` with `Capture*`-named methods
(`CaptureStart`, `CaptureEnter`, `CaptureExit`, `CaptureState`, `CaptureEnd`,
`CaptureFault`). Checked against the real go-ethereum source:

1. **The interface has been removed entirely.** Per go-ethereum's own
   `core/tracing/CHANGELOG.md`, `vm.EVMLogger` was deleted as part of the
   "live tracing" overhaul and replaced by a struct of function pointers,
   `tracing.Hooks`, with `On*`-named fields.
2. Even against the last version where `vm.EVMLogger` existed, the original
   scaffold was **missing two required methods** (`CaptureTxStart`,
   `CaptureTxEnd`) — a Go interface requires every method to be implemented,
   so this wouldn't have satisfied the interface even historically.
3. `CaptureState`'s scaffold signature had the wrong type for one parameter
   (`rval *vm.ReturnStack`, which isn't a real type in that position; the
   real parameter is `rData []byte`).

### 5.2 The corrected design

[`tracer/kaysentinel_tracer.go`](../tracer/kaysentinel_tracer.go) targets
`core/tracing.Hooks` instead. This is a net simplification, not just a
rename:

- `OnBalanceChange`, `OnNonceChangeV2`, `OnCodeChangeV2`, `OnStorageChange`
  hand the tracer `(prev, new)` pairs directly. The original design's plan
  to manually diff via `StateDB.GetState` inside `CaptureState` is no longer
  needed — Geth already computes Before/After for us.
- `OnEnter`/`OnExit` give a `depth int` per call, not a stable FrameID —
  EMES's FrameID/ParentFrameID bookkeeping still has to live in the tracer
  (a small stack, same idea as the original design, just wired to the real
  hook names).
- There is **no dedicated SELFDESTRUCT hook.** Self-destruction is exposed
  as *reason codes* on the ordinary mutation hooks: `CodeChangeSelfDestruct`
  on the destructing account's `OnCodeChangeV2`, paired with
  `BalanceIncreaseSelfdestruct` on the beneficiary's `OnBalanceChange`. The
  tracer correlates these two (keyed by `FrameID`) to synthesize
  `SelfDestructEvent`. This is arguably a better fit for EMES's own stated
  design principle elsewhere ("lifecycle is inferred from combinations of
  mutation events, not a dedicated action type") than the original plan to
  catch a raw `SELFDESTRUCT` opcode in `CaptureState`.
- `BlockCommitEvent.StateRoot` is populated from the `OnStateUpdate` hook
  (`update.Root`), not from `OnBlockEnd` — `OnBlockEnd` only carries an
  `error`, no root.

### 5.3 Open item: `AccountCreatedEvent` correlation

Not yet wired up. `OnNonceChangeV2` with reason `NonceChangeContractCreator`
tells you *someone* created a contract, but not the new contract's address;
that comes from the paired `FrameEnterEvent` (a `Create`/`Create2` frame,
whose `To` *is* the new address). Correlating these to emit
`AccountCreatedEvent` is a straightforward next step but wasn't done here to
avoid growing the scope further before this gets used against a real trace.

### 5.4 Verification performed

This was actually compiled, not just written to look right:

```bash
# go-ethereum requires Go 1.24+ (its go.mod uses a `tool` directive go1.22 can't parse)
go build ./tracer/...
go vet ./tracer/...
```

Both ran clean against a real checkout of `github.com/ethereum/go-ethereum`.
One practical note from doing this: go-ethereum's `master` branch (what a
plain `git clone` gives you) currently contains **unreleased, in-development
material** — specifically a multi-dimensional gas-accounting change
(`GasChangeHookV2`, tied to an "Amsterdam" fork / EIP-8037) that doesn't
appear in any tagged release. The latest **stable** release series as of
this writing is v1.16.x. If you build against this for real, **pin to a
stable tag** (e.g. `go get github.com/ethereum/go-ethereum@v1.16.9`) rather
than `master`, so the hook surface you're coding against doesn't shift under
you.

### 5.5 Second design pass: package split, structured errors, Gate 1

A later design pass proposed several genuinely good additions layered on
top of §5.2-5.4: an `EnvironmentDescriptor` for fork/client-aware
classification, structured `InternalTracerError` diagnostics instead of
silent no-ops, a self-describing `FixtureEnvelope` for provenance-stamped
fixture files, and a two-gate conformance architecture (Gate 1: structural
stream verification; Gate 2: semantic state-replay verification, not yet
implemented). All of these were adopted.

**What was not adopted as pasted:** the accompanying tracer refactor
targeted `CaptureEnter`/`CaptureExit` again, with a signature
(`CaptureEnter(typ vm.OpCode, from, to, input, gas, value [32]byte)`) that
doesn't match `tracing.EnterHook` (`func(depth int, typ byte, from, to
common.Address, input []byte, gas uint64, value *big.Int)`) on four counts:
missing `depth`, `typ` as `vm.OpCode` instead of `byte`, `value` as
`[32]byte` instead of `*big.Int`, and `Capture*`-named methods that aren't
`tracing.Hooks` fields at all. It also renamed `FrameID`/`ParentFrameID` to
`Fid`/`ParentFid`, which would have silently diverged from the struct names
already committed here. The additive ideas above were merged into the
already-compiling `tracing.Hooks`-based tracer from §5.2 instead of
replacing it.

**What changed as a result:**

- The EMES-V1 event structs moved to their own package, `emes/`, separate
  from the Geth-specific collector in `tracer/`. This is a real improvement:
  a future Reth or Besu adapter can produce `emes.Event` values without
  importing anything Geth-specific.
- Every address/hash/256-bit-value field changed from a raw `[20]byte` /
  `[32]byte` to `emes.Address` / `emes.Hash`, which implement
  `MarshalJSON`/`UnmarshalJSON` as `"0x..."` hex strings. Raw fixed-size byte
  arrays serialize as JSON arrays of small integers by default in Go, which
  would have made every fixture file this project produces unreadable and
  inconsistent with the hex-string convention already used everywhere else
  in this repo (`semantic_contract.md`, `validation_vectors/*.json`).
  Verified: see §5.6.
- Every emitted event now carries an explicit `"type"` tag in its JSON
  encoding (`emes.Event.Kind()`), since a bare `[]Event` slice gives a JSON
  reader no way to tell a `BlockStartEvent` from a `FrameEnterEvent` other
  than by guessing from its field set.
- Internal tracer anomalies (an `OnExit` with no matching `OnEnter`, an
  unclosed frame at transaction end) are now logged as structured
  `emes.InternalTracerError` values instead of being silently dropped.
- **Gate 1 is a real, working implementation**
  (`validation/gate1.go`), not a stub: it checks block/tx encapsulation,
  strict sequence monotonicity, and frame stack balance/topology. It was
  tested against both a valid stream and two deliberately broken ones (see
  §5.6) to confirm it actually rejects violations rather than rubber-stamping
  everything.
- The harness (`harness/harness.go`) runs Gate 1, then writes a
  `FixtureEnvelope` to `<base>/<network>/<fork>/<client>-<version>/<scenario
  ID>.json`, matching the proposed directory layout.
- **Gate 2 (semantic replay)** is still unimplemented -- it needs an actual
  state-reconstruction engine that can replay an EMES stream against a
  pre-state and compare resulting roots across two clients. That's
  materially more work than Gate 1 and hasn't been started.

### 5.6 Verification performed (second pass)

All of this was compiled and run, not just written:

```bash
go build ./emes/... ./tracer/... ./validation/... ./harness/...
go vet   ./emes/... ./tracer/... ./validation/... ./harness/...
```

Both clean. Beyond compiling, three things were actually executed:

1. A synthetic transaction (CALL, then nonce/balance mutations, then
   SELFDESTRUCT to a beneficiary, then frame exit, tx end, block commit) was
   driven through the real tracer by calling its hook methods directly. The
   resulting stream passed Gate 1, and the SELFDESTRUCT correlation logic
   (§5.2) correctly synthesized a `SelfDestructEvent` from the paired
   `CodeChangeSelfDestruct` / `BalanceIncreaseSelfdestruct` reason codes.
2. Gate 1 was fed two deliberately broken streams -- one with an unclosed
   frame at transaction end, one with a non-increasing sequence number --
   and correctly rejected both with the specific rule that was violated,
   confirming it isn't a no-op.
3. The harness was run against that same tracer output and actually wrote a
   fixture file to
   `ethereum-mainnet/cancun/geth-1.16.9/smoketest-selfdestruct-01.json`,
   with readable hex addresses/hashes and `"type"` tags on every event, as
   designed.

This still isn't run against a real go-ethereum node -- the "transaction"
above was hand-constructed hook calls, not a real EVM execution. That's the
same gap noted in §5.4.

### 5.7 What's still not done

This is a compiling event-collection tracer, not a working extractor:

- It hasn't been run against a real block/transaction yet (no `vm.EVM`
  wiring, no live-tracer registration via `eth/tracers.LiveDirectory`).
- It doesn't yet feed into the Layer 1 normalization engine
  (`validation_vector_spec.md`) that turns an EMES stream into an
  `expected_normalized_ssr`. That consolidation engine doesn't exist yet.
- `AccountCreatedEvent` correlation (§5.3) is unimplemented.

Registering it as a live tracer and running it against a real Geth dev-mode
chain, then diffing its output against one of the hand-authored
`validation_vectors/` fixtures, is the natural next concrete step.
