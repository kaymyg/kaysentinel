# KAY Sentinel: Validation Vector Specification (v1.0.0)

> **Status:** normative test-vector specification. This supersedes the
> informal vector blueprint sketched in `semantic_contract.md` with a strict
> JSON Schema (`../schemas/validation_vector_schema.v1.json`) and a fixed
> vocabulary of journal mutation events. Like the rest of this repo, this is
> a design/test-authoring artifact, not a working implementation: no
> extractor exists yet to produce these mutation streams or hashes from a
> real client, so every vector here is hand-authored against the spec, not
> machine-verified against Geth/Reth.

## 1. Core Semantic Invariants

Every conforming extractor **SHALL** satisfy:

- **Minimality Invariant:** no identity deltas survive normalization. If an
  account property or storage slot returns to its exact pre-state value at
  the transaction boundary, it MUST be culled from the output.
- **Determinism Invariant:** identical raw state inputs under equivalent
  environment contexts SHALL yield identical normalized SSRs, regardless of
  client-internal architecture.
- **Ordering Invariant:** account arrays and storage updates MUST follow
  strict ascending unsigned byte-wise lexicographical order (§2.2).
- **Rollback Invariant:** mutations accumulated in a sub-context frame that
  later hits `FRAME_REVERT` SHALL become entirely non-observable in the
  finalized SSR.
- **Isolation Invariant:** `TRANSIENT_MUTATION` events MUST NOT bleed into
  the finalized persistent storage update array.
- **Fork Invariant:** lifecycle mutation and destruction tracking SHALL
  adapt to the behavior defined by the active `fork_identifier` (e.g.
  EIP-6780 SELFDESTRUCT semantics only apply from CANCUN onward).

## 2. Layer 1 — Mutation Typings & Semantics

### 2.1 Normative Event Definitions

A test harness ingests a `journal_mutation_sequence`: a stream of explicit,
observable state transitions. **The ordering of elements in that array is
illustrative/input-only** — extractors MUST be invariant to the relative
interleaving of orthogonal client mutations; only the final per-address,
per-slot values at the transaction boundary matter for the output SSR.

| Action | Meaning |
|---|---|
| `NONCE_MUTATION` | account's nonce counter changes; carries `new_nonce` |
| `BALANCE_MUTATION` | native balance changes; carries `new_balance` |
| `CODE_MUTATION` | bytecode deployed/wiped; carries `new_code` |
| `STORAGE_MUTATION` | persistent storage slot write; carries `slot`, `new_value` |
| `TRANSIENT_MUTATION` | ephemeral (transient-storage) write; carries `slot`, `new_value`; excluded from the SSR per the Isolation Invariant |
| `FRAME_REVERT` | the current `context_depth`'s uncommitted mutations are discarded |

Note: there is no dedicated `CREATE`/`SELFDESTRUCT` action. Account
lifecycle (creation, destruction, resurrection) is inferred from the
combination of `NONCE_MUTATION` / `CODE_MUTATION` / `BALANCE_MUTATION`
events on the same address, in the same way a real client journal would
expose it. See `KS-LC-001` in the fixtures repo for a worked example and the
gap this leaves in the create-then-destroy omission rule.

### 2.2 Canonical Byte-Wise Ordering Rules

- **Account deltas:** compared as fixed 20-byte sequences, ascending.
- **Storage updates:** compared as fixed 32-byte sequences, ascending.

Ordering is computed on raw bytes, not on the hex string representation, to
avoid divergence between implementations that differ in case-folding or
locale-aware string comparison.

## 3. Layer 1 Schema

The full JSON Schema is at
[`../schemas/validation_vector_schema.v1.json`](../schemas/validation_vector_schema.v1.json).
Every vector under `validation_vectors/` in the fixtures repo validates
against it — this is checked mechanically, not just asserted (see
`scripts/validate_vectors.py`, TODO in the implementation roadmap).

`vector_id` follows `KS-(ST|RB|LC|TS|OR)-NNN`, where the category code maps
to the `category` field: **ST**=Storage, **RB**=Rollback, **LC**=Lifecycle,
**TS**=Transient, **OR**=Ordering.

A note on the account-existence representation: since this schema has no
explicit null/"account does not exist" type, a nonexistent account is
represented with `nonce: 0`, `balance: "0"`, and `code_hash` equal to
`EMPTY_CODE_HASH` (`keccak256("")` =
`0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470`). This
matches Ethereum's own state-clearing convention (EIP-161): an account with
those exact values is indistinguishable from one that was never created.

## 4. Primary Conformance Vectors

Vectors live in the fixtures repo under `validation_vectors/`:

- `KS-ST-001` — storage identity-loop pruning (Minimality Invariant)
- `KS-LC-001` — same-tx create → destroy → recreate ("resurrection"); tests
  the gap in the create-then-destroy omission rule
- `KS-OR-001` — cross-account and cross-slot byte-wise ordering with
  deliberately out-of-order input

## 5. Layer 2 — Serialization Profile (`SSZ-V1`)

- **Serialization spec:** SSZ (fixed-width elements sequential; variable-width
  lists via offsets).
- **Hash engine:** SHA-256 (standard SSZ `hash_tree_root`).

**Status: not yet computable.** A Layer 2 conformance manifest needs a real
SSZ encoder to produce `serialized_length` and `expected_root_hash` for each
vector. No such encoder exists yet (see the implementation roadmap, Phase
1). Any hash or length value asserted here before that encoder exists would
be fabricated and indistinguishable from a real, verified value to a reader
— so this repo does not publish placeholder hashes. Once the SSZ encoder
lands, run it over each vector in `validation_vectors/` and record the real
output in `schemas/ssz_v1_conformance_manifest.json` (template below).

```json
{
  "profile_id": "SSZ-V1",
  "profile_target_version": "1.0.0",
  "vector_targets": [
    { "vector_id": "KS-ST-001", "serialized_length": null, "expected_root_hash": null },
    { "vector_id": "KS-LC-001", "serialized_length": null, "expected_root_hash": null },
    { "vector_id": "KS-OR-001", "serialized_length": null, "expected_root_hash": null }
  ]
}
```
