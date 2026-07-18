# EMES-V1 Stream Validation & Gate 1 Invariants

This document establishes the normative structural validation rules enforced by `Gate 1` (`validation/gate1.go`) over an incoming EMES event stream, along with the historical record of retracted state-machine assumptions.

Per `governance.md`'s Evidence Authority Rubric: claims here are graded **Medium** (single implementation — Rust/Go source read directly, no live Geth trace run) unless marked otherwise. Nothing in this document has been empirically verified against a running node; everything is derived from static reading of `tracer/kaysentinel_tracer.go`, `validation/gate1.go`, and upstream `go-ethereum`'s `core/state_transition.go` / `core/vm/evm.go`.

---

## 1. Normative Invariants

### Invariant T1: Execution Lifecycle Consistency

Let $E$ be an ordered sequence of events bounded between a `TransactionStartEvent` and its matching `TransactionEndEvent`. Let $E_{\text{enter0}}$ be the subset of `FrameEnterEvent`s where $\text{Depth} == 0$.

- **Predicate:** $\text{RootFramePresent} \iff |E_{\text{enter0}}| > 0$
- **Invariant Constraint:** If $\text{RootFramePresent} == \text{true}$, then exactly one corresponding root frame exit event MUST exist ($|E_{\text{exit0}}| = 1$), and:

$$\text{TransactionEndEvent.Reverted} == \text{RootFrameExitEvent.Reverted}$$

- **Scope:** This invariant applies only to transaction streams containing a root `FrameEnterEvent`. Transaction streams without a root frame are outside the domain of this invariant and are validated according to the remaining structural rules together with the Layer 3 taxonomy defined in `004-bridge-buffering-spec.md`.
- **Implementation status:** **Not yet implemented in `validation/gate1.go`.** The current `VerifyGate1Invariants` function checks block encapsulation, sequence monotonicity, transaction encapsulation, and frame stack balance/topology — it does not read `.Reverted` on any event. Adding T1 is real, scoped, additive work against that function.

### Orthogonal Stack Integrity

Structural constraints regarding call-frame topology, already implemented in `validation/gate1.go` today, are evaluated independently of Invariant T1:

1. **Nesting Monotonicity:** `FrameEnterEvent` increments depth by exactly 1; `FrameExitEvent` decrements depth by exactly 1.
2. **Identifier Balance:** the `FrameID` on a `FrameExitEvent` must match the `FrameID` of the most recently pushed, unclosed `FrameEnterEvent`.
3. **Root Topology:** `FrameID` 0's `ParentFrameID` must be the `0xFFFFFFFFFFFFFFFF` sentinel (`noParentFrame`).

---

## 2. Retracted Invariants & Historical Lineage

### [RETRACTED] Invariant T0: Absolute Execution Presence Constraint

- **Original Formulation:** *"If `RootFramePresent == false`, then no state mutation events (such as `StorageMutationEvent`, `NonceMutationEvent`, or balance changes) are permitted to exist in the transaction event stream."*
- **Status:** **RETRACTED**
- **Falsifying Evidence (static source audit, `core/state_transition.go` and `core/vm/evm.go`):** the EVM state processor executes several protocol-level state transitions entirely outside the interpreter/call-frame loop, for every transaction:
  1. **Gas pre-payment debit** (`buyGas()` → `SubBalance(..., BalanceDecreaseGasBuy)`) — before any root frame opens.
  2. **Sender nonce increment for CALL transactions** (`executeCall()` → `SetNonce(..., NonceChangeEoACall)`) — before `evm.Call()` (and therefore before `OnEnter`) fires.
  3. **Gas refund credit and coinbase fee payment** (`settleGas()` / `execute()` → `AddBalance(..., BalanceIncreaseGasReturn / BalanceIncreaseRewardTransactionFee)`) — after the root frame has already closed.
  None of these three are gated on the root call's revert status (`vmerr`) anywhere in the source.
- **Superseding architecture:** Invariant T0 is replaced by the Layer 3 Frameless Mutation Taxonomy (`004-bridge-buffering-spec.md` §3) — a per-hook, reason-keyed classification table instead of a blanket structural ban.
