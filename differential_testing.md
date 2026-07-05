# Differential Testing Engine

To guarantee convergence between φ_geth(E_Geth) and φ_reth(E_Reth), this differential testing engine generates deterministic execution paths that maximize EVM edge-case entropy, runs them through both client boundary implementations, and checks bitwise identity over the resulting SSZ-serialized Δ_canonical byte streams.

## 1. Test Harness Format

Test vectors are defined as JSON (`KaysentinelTestCase`): a pre-state configuration, a transaction to execute, and the expected SSR root. See `tests/transient_storage_case.json` for a worked example.

## 2. Edge-Case Matrix

### Test Vector 1 — Transient Storage `TraceMid` Clean-Up (EIP-1153)

**Objective:** verify the extractor pulls transient storage values *before* VM teardown purges them, while ignoring keys written inside sub-calls that reverted.

**Execution path:**
1. `Tx.to` calls Contract A.
2. A executes `TSTORE(slot=0x01, value=0xAA)`.
3. A `DELEGATECALL`s into Contract B.
4. B executes `TSTORE(slot=0x01, value=0xBB)` and `TSTORE(slot=0x02, value=0xCC)`.
5. B hits `REVERT`, rolling back its stack frame.

**Assertion:** the final `TransientMutation` list for Contract A in Δ_canonical must contain exactly one entry — `[slot=0x01, value=0xAA]`. B's writes must be fully discarded, proving extraction happens at the TraceMid → Post boundary and doesn't pick up stale tree artifacts.

### Test Vector 2 — Post-Cancun `SELFDESTRUCT` Lifecycles (EIP-6780)

**Objective:** validate lifecycle statuses assigned by Geth's `core/state/journal.go` and Reth's `BundleState` align with Cancun semantics.

**Execution path:**
1. `Tx.to` deploys Contract C via `CREATE2`.
2. C calls a function executing `SELFDESTRUCT` targeting an external escrow vault, same transaction.
3. Contract D (100+ blocks old) is also forced to `SELFDESTRUCT` in the same block.

**Assertion:** C's account entry must show `lifecycle_status = 2` (`SELFDESTRUCT_PENDING`) with balance wiped. D's account entry must stay `lifecycle_status = 0` (`LIVE`), code intact, only the balance mutation recorded.

### Test Vector 3 — High-Entropy Multi-Account Modification (Sorting Validation)

**Objective:** stress-test map iteration ordering differences between Go's randomized map traversal and Rust's `BTreeMap`.

**Execution path:**
1. A proxy batching contract runs 20 sub-calls touching randomly generated addresses (A1...A20) via `BALANCE` modifications and storage initialization.
2. Addresses are touched out of sequential order at runtime (e.g. A15 → A3 → A19).

**Assertion:** Geth and Reth extractors must produce byte-identical SSZ output, proving `account_mutations` is sorted ascending lexicographically by address and `storage_diffs` by slot before serialization.

## 3. Verification Script

See `scripts/verify_ssr.py`. It takes the SSZ byte outputs from both runtimes, checks length equality, computes SHA-256 digests, and — on mismatch — walks the byte streams to report the first differing offset.
