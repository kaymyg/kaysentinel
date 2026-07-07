# KAY Sentinel Geth Tracer (EMES-V1 collection)

`kaysentinel_tracer.go` implements EMES-V1 event collection against
go-ethereum's current `core/tracing.Hooks` live-tracing API. See
[`../docs/emes_profile.md`](../docs/emes_profile.md) for what changed from
the original `vm.EVMLogger`-based design and why.

## Verifying it compiles

This package depends on `github.com/ethereum/go-ethereum`, which requires
**Go 1.24+** (its `go.mod` uses a `tool` directive older Go versions can't
parse). Pin to a stable release rather than `master` -- see
`docs/emes_profile.md` §5.4 for why.

```bash
mkdir -p kaysentinel_verify && cd kaysentinel_verify
go mod init kaysentinel/verify
go get github.com/ethereum/go-ethereum@v1.16.9
cp ../tracer/kaysentinel_tracer.go .
go build .
go vet .
```

If `go build`/`go vet` aren't clean, that's a real break against whatever
go-ethereum version you pinned -- please open an issue with the exact
version and error.

## Status

Compiles and vets clean (verified against a go-ethereum checkout during
development). Not yet wired into a running node or a live-tracer
registration, and not yet run against a real transaction. See
`docs/emes_profile.md` §5.5 for the concrete next steps.
