## What does this change do?

<!-- Brief description -->

## Checklist

- [ ] `cargo test --workspace` passes (in `runtime/`)
- [ ] `go build ./... && go vet ./... && go test ./... -v` passes, if this
      touches the Go side
- [ ] If this changes a documented test count, gap, or status claim, the
      README's "Active Codebase & Implementation Status" section is updated
      to match (this repo has a real history of that drifting — see
      CONTRIBUTING.md)
- [ ] If this touches `runtime/builder/src/lifecycle/storage_root.rs`, the
      design note explaining it's protocol-native (not Ethereum-MPT) is
      still accurate, or has been deliberately updated
- [ ] Added/updated tests for the change

## Related issue

<!-- Closes #123, if applicable -->
