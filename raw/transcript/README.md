# Transcript rewrite + Multiplexer + Differential Validation

This replaces the earlier tagged-union `raw/transcript/transcript.go`
(nativeEvent/tracePayload + StreamProjection + deterministic hash) with a
flat, single-struct `RawEvent` design, per an explicit decision to trade
away the deterministic hash and immutable-projection guarantees for lower
per-event allocation cost. If you want the hash/projection version back,
it's recoverable from git history (commit `ed2bd8a`).

## What was extended beyond the pasted draft

The pasted `RawEvent`/`EventKind` only covered `TxStart/TxEnd/Enter/Exit/
Fault/BalanceChange`, with only `AppendEnter`/`AppendExit` actually
implemented. That's not enough to capture what EMES-V1
(`docs/emes_profile.md`) needs to reconstruct an SSR -- nonce, code, and
storage mutations, self-destruct, and account creation were missing
entirely. This version adds `KindNonceChange`, `KindCodeChange`,
`KindStorageChange`, `KindSelfDestruct`, `KindAccountCreated` and their
`Append*` methods, following the same flat-struct design.

One inconsistency kept rather than silently fixed: the original design's
stated goal was eliminating string allocations, but `RawEvent.ErrType` is
still a `string`. Noted in the source rather than either silently dropping
error type info or silently ignoring the stated goal.

## Bugs found and fixed (multiplexer/broadcast.go)

Checked against the real `core/tracing/hooks.go` (not assumed), three
signature mismatches:

1. `OnTxStart` was missing the `from common.Address` parameter
   `TxStartHook` requires, and used a value `tracing.VMContext` instead of
   the required pointer `*tracing.VMContext`.
2. `OnEnter`'s `value` parameter was typed `*uint256.Int`; `EnterHook`
   requires `*big.Int`.
3. `OnFault` didn't match `FaultHook` at all -- the real signature is
   `func(pc uint64, op byte, gas, cost uint64, scope OpContext, depth int,
   err error)`, missing the `scope OpContext` parameter entirely and in a
   different parameter order (pasted had `depth` first; the real hook has
   it second-to-last).

Also: the pasted `BroadcastSink.WiringToGethHooks()` only implemented
`OnEnter` and left the rest as a comment ("mirror this exact wrapper
structure"). All five remaining hooks (`OnTxStart`, `OnTxEnd`, `OnExit`,
`OnFault`, `OnBalanceChange`) are now actually implemented, not left as a
placeholder, and the method was renamed `Hooks()` for consistency with
`tracer.Tracer.Hooks()` elsewhere in this repo.

## Other fixes (validation/normalizer.go, validation/engine.go)

- `normalizer.go` imported `kay-sentinel/raw/transcript` (hyphenated
  module name); this repo's actual module is `kaysentinel` (see `go.mod`).
  Fixed to `kaysentinel/raw/transcript`.
- `engine.go`'s comparison method was unexported
  (`compareSemanticFields`), making it uncallable from outside package
  `validation` -- presumably an oversight for something meant to be a
  differential-testing API. Exported as `CompareSemanticFields`.
- Noted, not fixed: `SemanticEvent` only carries the fields shown
  (Sequence/Opcode/Depth/From/To/Value/GasUsed/Reverted/ErrType), so a
  differential check can't currently catch a mismatch in, say, a specific
  storage slot's before/after value -- only that *some* event of a given
  Kind/Sequence differs on the fields that do exist.

## Verification performed

```bash
go build ./raw/... ./validation/... ./multiplexer/...
go vet   ./raw/... ./validation/... ./multiplexer/...
```

Both clean. Beyond that, an actual end-to-end run:

- Registered two `HookSink`s on a `BroadcastSink`: one that records into an
  `ExecutionTranscript`, one that deliberately panics on `OnEnter`.
- Drove a fake transaction through `bus.Hooks()`. Confirmed: the panicking
  sink's panic was caught, recorded as a `PanicReport` naming the right
  sink and callback, and did **not** propagate (strictMode=false) --
  and confirmed it was evicted: a second round of hook calls produced no
  new panic and no new report, proving eviction actually removes the sink
  rather than just logging and continuing to call it.
- The recording sink's transcript was fed through `EventNormalizer` and
  `DifferentialValidator`: comparing a stream against itself matched on
  every event, and a deliberately injected field mismatch (`Depth: 99`) was
  correctly caught and reported with the expected tabular diff format.

## Status

Compiles, vets clean, and the panic-isolation + differential-validation
behavior was actually exercised, not just compiled. Not yet wired into
`tracer.Tracer` from the earlier EMES-V1 work -- this and that tracer are
currently two separate, non-integrated ways of collecting events from
Geth's `tracing.Hooks`. Deciding whether to merge them or keep both for
different purposes (e.g. this one for a legacy-vs-new differential period)
is an open decision, not resolved here.
