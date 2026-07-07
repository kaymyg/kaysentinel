# KAY Sentinel Geth Tracer + Conformance Pipeline

This directory (together with `../emes/`, `../validation/`, and
`../harness/`) implements EMES-V1 collection and Gate 1 structural
verification. See [`../docs/emes_profile.md`](../docs/emes_profile.md) for
the full design writeup, including two rounds of corrections against the
real go-ethereum source.

- `../emes/` -- wire event types (`Event`/`MutableEvent`, the 12 EMES-V1
  event structs, hex-marshaling `Address`/`Hash` types, `FixtureEnvelope`,
  `EnvironmentDescriptor`, structured `InternalTracerError`). No Geth
  dependency -- a future Reth/Besu adapter could import just this package.
- `kaysentinel_tracer.go` -- the Geth-specific collector, implementing
  `core/tracing.Hooks`.
- `../validation/gate1.go` -- real structural verifier (block/tx
  encapsulation, sequence monotonicity, frame balance).
- `../harness/harness.go` -- runs Gate 1, then writes a `FixtureEnvelope` to
  `<base>/<network>/<fork>/<client>-<version>/<scenario>.json`.

## Verifying it compiles and runs

Requires **Go 1.24+** (go-ethereum's `go.mod` uses a `tool` directive older
Go can't parse). Pin to a stable go-ethereum release rather than `master`
-- see `docs/emes_profile.md` §5.4 for why.

```bash
go build ./emes/... ./tracer/... ./validation/... ./harness/...
go vet   ./emes/... ./tracer/... ./validation/... ./harness/...
```

Both are clean against `github.com/ethereum/go-ethereum@v1.16.9`.

## Status

- Compiles and vets clean.
- Actually run end-to-end against a hand-constructed synthetic transaction
  (not a real EVM execution): the tracer collected a full EMES-V1 stream
  including a correctly-correlated `SelfDestructEvent`, Gate 1 passed it,
  and the harness wrote a real fixture file to disk with readable hex
  fields and `"type"`-tagged events.
- Gate 1 was also fed two deliberately broken streams (unclosed frame,
  non-increasing sequence) and correctly rejected both -- it's a real
  check, not a stub that always passes.
- **Not yet done:** registration as a live tracer against a real Geth node,
  a real transaction trace, `AccountCreatedEvent` correlation, and Gate 2
  (semantic state-replay verification across clients) -- see
  `docs/emes_profile.md` §5.7 for the full list.
