# Execution Transcript (`raw/transcript`)

An internal, in-process buffer for raw execution trace events, with a
type-safe projection layer (`StreamProjection`) and a deterministic binary
encoding + SHA-256 hash (`ComputeDeterministicBinaryHashV1`) over the
buffered stream. Sits below `emes`/`tracer` as a lower-level recording
surface.

## Bugs found and fixed

Two rounds of this file were checked by actually compiling them (not just
reading them), and both had real, confirmed compile failures:

**Round 1 (incomplete draft):**
1. `import "math/uint256"` -- not a real package. Confirmed:
   `package math/uint256 is not in std`. The real package is
   `github.com/holiman/uint256`, already a genuine transitive dependency of
   go-ethereum.
2. `AppendBalanceChange(addr common.Address, old, new *big.Int, ...)` --
   the parameter named `new` shadows Go's builtin `new()`, so
   `new(big.Int).Set(new)` tries to call a `*big.Int` value as a function.
   Confirmed: `cannot call non-function new (variable of type *big.Int)`.

**Round 2 (claimed "Compile Readiness 10/10", fixing round 1):** fixed both
of the above, but introduced a third, structurally identical bug that its
own audit didn't catch: `writeErrorSnapshotV1(buf *bytes.Buffer, err
*ErrorSnapshot)` -- inside the function, `err.Message` (using the
`*ErrorSnapshot` parameter) is evaluated on the same line as `_, err :=
buf.WriteString(...)`, which tries to redeclare `err` as `error` in the same
scope. Go requires same-type redeclaration for `:=` to reuse a variable in
an outer/parameter scope; `*ErrorSnapshot != error`, so this fails.
Confirmed with a minimal repro before fixing it here by renaming the
parameter to `snap`.

## Verification performed

Not just compiled -- actually run:

```bash
go build ./raw/transcript/...
go vet   ./raw/transcript/...
```

Both clean. Beyond that:

- Appended one event of each of the 6 kinds, ran `StreamProjection`, and
  confirmed all 6 came back as the correct `Projected*` type.
- Called `ComputeDeterministicBinaryHashV1` on two independently-constructed
  transcripts built from identical inputs -- hashes matched, confirming the
  encoding is actually deterministic, not just intended to be.
- **Value isolation:** appended a `uint256.Int`, a `*big.Int` pair, and a
  `[]byte`, then mutated all three *originals* after appending, then
  recomputed the hash. It was identical to a fresh transcript built from the
  original (pre-mutation) values -- confirming the clone/deep-copy claims in
  the file's own comments actually hold, not just that the code compiles.
  (Also confirmed `uint256.Int` is a plain `[4]uint64` array with no
  internal pointers, so the value-copy-via-dereference pattern used here is
  genuinely safe.)

## Status

Compiles, vets clean, and its core claims (determinism, value isolation)
were actually tested, not just asserted. Not yet wired into
`tracer.Tracer` -- it currently exists as a standalone package with no
caller in this repo.
