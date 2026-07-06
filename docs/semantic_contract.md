# KAY Sentinel: Semantic Contract v1.0.0 (Normative)

> **Status:** normative specification. This document supersedes the informal
> Σ/Δ notation in `framework.md` §1–§2 with precise, testable rules. It is
> still a design document — no reference extractor exists yet (see
> `implementation_roadmap.md`). Nothing in this file has been machine-checked;
> "SHALL"/"MUST" describe target invariants that an implementation is required
> to satisfy, not proofs that have been verified.

## 1. Structural Definitions & System Boundaries

### 1.1 The World State

The **World State** is the mapping from Ethereum accounts to their persistent
consensus-defined state, as represented by the execution specification.

### 1.2 The State Transition Record (SSR)

The **State Transition Record (SSR)** is the net persistent consensus
world-state mutation resulting from execution of a single Ethereum
transaction. The SSR isolates state *effects* from execution *paths*.

- **Persistent Consensus State (INCLUDED):** account balance/nonce
  transitions, deployed bytecode hash updates, and persistent key-value
  storage slot mutations.
- **Consensus Artifacts (EXCLUDED):** transaction logs and receipts. These
  are part of blockchain history but are not part of the state trie, so
  they are out of scope for the SSR.
- **Ephemeral / Execution Metadata (EXCLUDED):** gas refunds, access lists,
  and transient storage exist only to govern execution mechanics and gas
  accounting. They do not persist in the world-state trie at the
  transaction boundary and SHALL NOT contribute to the canonical SSR.

## 2. Orthogonal Transaction Outcome Matrix

Extractors SHALL categorize every transaction along two independent axes.

### 2.1 Consensus Dimension

- **Included:** the transaction satisfied intrinsic consensus validity
  (valid signature, correct upfront gas payment, valid nonce) and was
  executed within a block. An SSR SHALL be generated for every Included
  transaction.
- **Rejected:** the transaction failed intrinsic consensus validation. No
  EVM execution occurred, no state mutation exists, and no SSR SHALL be
  generated.

### 2.2 Execution Dimension

- **Success:** execution terminated normally, with no halting exception and
  no explicit `REVERT`.
- **Revert:** execution hit an explicit `REVERT`. Speculative state changes
  are rolled back, but consensus-level billing (sender nonce increment, gas
  fee deduction) persists.
- **Exceptional Halt:** execution aborted on out-of-gas, invalid opcode, or
  stack under/overflow. All execution mutations roll back; only baseline
  consensus billing persists.

These two dimensions are orthogonal: a transaction is always exactly one of
{Included, Rejected} and, if Included, exactly one of {Success, Revert,
Exceptional Halt}.

## 3. Normative Normalization Rules

### 3.1 Identity Transition Pruning

Every state update is recorded as a delta `Δ = (Value_original, Value_current)`.
Implementations SHALL deep-copy mutable numeric values (e.g. balance
integers) during collection to prevent reference aliasing.

If an account or storage slot is modified arbitrarily during execution but
`Value_original == Value_current` at the transaction boundary, it is an
**identity transition** and SHALL be omitted from the finalized SSR delta
set.

### 3.2 Lifecycle Resolution

If an account is created within a transaction (via `CREATE`/`CREATE2`) and
subsequently `SELFDESTRUCT`s within the same transaction, its account object
SHALL NOT appear in the accounts update list — **provided no persistent
account remains at the transaction boundary**. Structural side effects it
caused (e.g. a value transfer to a beneficiary) SHALL still be captured
under that beneficiary's own independent account delta.

> Note: this rule governs the *create-then-destroy* case only. The
> resurrection case — an account that was previously destroyed and later
> becomes the beneficiary of a same-transaction transfer — is a distinct
> normative case; see the `ssr_normalization/account_resurrection` fixture
> for the worked example this rule does not yet cover explicitly.

### 3.3 Normalization Post-Condition

The normalization function 𝒩 acting on an unrefined SSR `S` SHALL be
idempotent: `𝒩(𝒩(S)) = 𝒩(S)`.

## 4. Formal Compliance Invariants

A conforming extractor MUST satisfy:

- **Determinism:** identical pre-state + identical transaction input SHALL
  produce byte-identical SSR encodings across independent client
  implementations.
- **Rollback Safety:** mutations from an execution frame that was later
  rolled back by the client's journal SHALL NOT appear in the finalized SSR.
- **Order Independence:** internal data-structure ordering (e.g.
  language-specific hash map iteration order) SHALL NOT affect the
  structural alignment or serialization of the finalized SSR.
- **Completeness:** every persistent world-state mutation SHALL appear
  exactly once in the normalized SSR, unless removed by an explicit
  normalization rule (§3).
- **Minimality:** no identity transitions SHALL survive normalization.

## 5. Serialization Integration Constraints

The serialization layer is decoupled from the semantic layer above; width,
byte-ordering, and structural grouping defer entirely to SSZ.

- Canonical sorting SHALL execute only *after* normalization has finalized
  the delta entries.
- **Account delta sorting:** ascending unsigned byte-wise comparison of
  20-byte addresses.
- **Storage delta sorting:** for each mutated account, ascending unsigned
  byte-wise comparison of 32-byte storage slot keys.

## 6. Security Considerations

- **Mutable Aliasing:** extractors sharing memory with the host client MUST
  deep-copy value types (e.g. pointers to big-integer objects) so that
  ongoing block processing cannot mutate historical delta records.
- **Rollback Desynchronization:** if an implementation hooks speculative
  append points rather than transaction-finality hooks, misalignment
  between internal EVM call frames and client journal snapshots can leak
  rolled-back mutations into the "verified" state trie.
- **State Bloat DoS:** transactions performing deep A→B→A storage thrashing,
  or excessive transient-storage writes, can force an unoptimized extractor
  past memory/CPU limits during collection, before normalization trims the
  footprint. Extractors SHOULD bound per-transaction working-set memory
  independent of normalization.

## 7. Versioning Matrix

| Field | Value |
|---|---|
| Semantic Contract Track Version | `1.0.0` |
| Serialization Schema Version | `SSZ-v1` |
| Target Ethereum Revision | Prague Execution Specification |

---

## Relationship to `framework.md`

This document is the normative source of truth for the extraction semantics
described informally in `framework.md` §1 (Formal Semantic Foundation) and
§2.3–2.4 (Determinism Rules / Canonical Constraint). Where the two disagree,
this document governs until `framework.md` is updated to match.
