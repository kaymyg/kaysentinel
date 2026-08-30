---
name: Bug report
about: Report unexpected behavior, a failing test, or a broken build
title: "[Bug] "
labels: bug
---

**Describe the bug**
A clear, concise description of what's wrong.

**Which side does this affect?**
Rust runtime (`runtime/<crate>`) / Go layer (`emes`, `tracer`, `validation`,
`harness`, `raw/transcript`, `multiplexer`) / docs.

**To reproduce**
Command(s) or a minimal test case that triggers it, e.g.:
```
cd runtime && cargo test -p kaysentinel-ssz
```
or
```
go test ./validation/... -run TestGate1
```

**Expected behavior**
What you expected instead.

**Environment**
- OS:
- Rust version / Go version:
- Commit/tag:

**Additional context**
Relevant test output.
