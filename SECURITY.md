# Security Policy

## Scope

KAYSentinel's core claim is the extraction map invariant:
`E(σ1) = E(σ2) ⟺ Post(σ1) = Post(σ2)` — two execution traces that produce
the same post-state must produce the same canonical representation and
commitment, and traces that produce *different* post-states must not
collide. A break of that invariant, or of Gate 1/Gate 2 verification, is a
genuine soundness bug in a project whose entire purpose is being a trust
anchor for downstream verification, forensics, and consensus auditing.

What **is** a genuine finding here:

- A **hash collision or false positive**: two structurally different SSRs
  (different post-states) that produce the same canonical commitment
- A **false negative**: a case where Gate 1 or Gate 2 should reject a stream
  (structurally invalid, or semantically inconsistent with the certificates)
  but doesn't
- A way to **forge or tamper with a `CanonicalAccountCertificate`** that
  `kaysentinel-verify`'s independent re-derivation and comparison doesn't
  catch
- A **non-determinism bug**: the same input producing different canonical
  output across runs (SSZ encoding, BLAKE3 domain separation, Merkle
  folding, or lifecycle/generation resolution)
- A case where the storage-root deriver's leaf ordering or padding isn't
  actually deterministic (it's supposed to be — `BTreeMap` iteration order
  + explicit zero-padding to a power of two)
- A **panic or crash on malformed input** anywhere in the decode path
  (`runtime/ssz/src/decode.rs`, `runtime/bridge`) that should instead return
  a typed error

What's a **known limitation, not a new finding** (see the README's "Current
Gaps" section before reporting):

- The storage root is a protocol-native BLAKE3 commitment, not a real
  Ethereum Merkle-Patricia Trie root — this is a documented design choice,
  not a bug
- Nothing here has been run against a live Geth node or real transaction
  trace yet — the whole pipeline is verified against simulated/synthetic
  input, and that gap is tracked, not hidden

## Reporting

Please **do not** open a public GitHub issue for a working hash collision,
verification bypass, or forgery — that publishes the technique before
there's a fix.

Instead, use GitHub's private vulnerability reporting for this repository
(Security tab → "Report a vulnerability"). Please include:

- The two inputs (traces, event streams, or certificates) that should be
  distinguishable/rejected but aren't, or vice versa
- Which stage catches it incorrectly: Gate 1 (Go, structural), Gate 2 (Rust,
  semantic replay), or the hash/SSZ layer directly
- A minimal reproduction, ideally as a new test in the style of the existing
  ones in `runtime/*/src/**/*.rs` or `validation/gate1_test.go`

## Response expectations

Independent research project — best-effort response, not a guaranteed SLA.
Genuine soundness findings will be prioritized and credited (with
permission) once fixed.
