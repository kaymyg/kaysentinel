---
name: Soundness report (non-sensitive)
about: A case where the faithfulness invariant, Gate 1, or Gate 2 should catch something and doesn't, and it's safe to discuss publicly
title: "[Soundness] "
labels: soundness
---

> If this is a working hash collision, verification bypass, or certificate
> forgery you'd rather not publish before there's a fix, please use private
> vulnerability reporting instead (Security tab → "Report a vulnerability")
> per [SECURITY.md](../../SECURITY.md).

Use this template for issues you're comfortable discussing openly, or that
overlap with an already-documented limitation (see the README's "Current
Gaps" section).

**What should be caught, and isn't (or vice versa)?**

**Which layer?**
Gate 1 (Go, structural) / Gate 2 (Rust, semantic replay) / hash / SSZ
encode-decode round trip / storage-root derivation.

**Minimal reproduction**
Ideally as a new test, in the style of the existing ones in
`runtime/*/src/**/*.rs` or `validation/gate1_test.go`.

**Is this the documented storage-root / live-Geth limitation, or something new?**
