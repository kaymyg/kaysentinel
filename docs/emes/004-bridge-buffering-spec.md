# EMES-V1 Semantic Reconstruction & Layered Bridge Engine

This document specifies the buffering and emission architecture of the Go→Rust EMES-V1 bridge (not yet implemented — see status notes throughout), and the operational limits of the underlying stream data model.

---

## 1. Layered Architecture

Processing an incoming EMES event stream is cleanly separated into three layers, each with a single responsibility and no upward dependency on the layer above it:

```
┌─────────────────────────────────────────────────────────────────┐
│              LAYER 3: PROTOCOL COVERAGE & TAXONOMY               │
│  [Taxonomy Registry] ──► [Fork Semantics] ──► [EIP-7702 Bounds]  │
└───────────────────────────────┬───────────────────────────────── ┘
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│             LAYER 2: SEMANTIC RECONSTRUCTION (BRIDGE)            │
│  [Frame Buffering Tree] ──► [Merge / Discard] ──► [Emit Engine]  │
└───────────────────────────────┬───────────────────────────────── ┘
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│             LAYER 1: STRUCTURAL VALIDATION (GATE 1)              │
│  [Nesting Integrity] ──► [Topology Checks] ──► [T1 Consistency]  │
└─────────────────────────────────────────────────────────────────┘
```

**Implementation status: none of the three layers' bridge-specific logic exists yet.** Layer 1's stack-integrity checks are real and shipped (`validation/gate1.go`); T1 is not. Layer 2 (the buffering tree) is design-only — nothing in `runtime/builder` or elsewhere consumes a Go event stream today. Layer 3 is this document's taxonomy table plus the Rust-side `CsePayload` enum it must eventually map onto.

---

## 2. Layer 2: Authoritative Tree Buffering

The bridge stages and routes mutations using call-frame topology as the primary signal, with transaction-level metadata (`TransactionEndEvent.Reverted`) used only as an out-of-band consistency check (per Invariant T1), never as the routing trigger.

- **Child frame commit** (`FrameExitEvent.Reverted == false`): merge the child's buffered mutations upward into its immediate parent's buffer.
- **Child frame revert** (`FrameExitEvent.Reverted == true`): discard the child's buffered context entirely.
- **Root frame commit:** merge the root buffer into the canonical emission queue.
- **Root frame revert:** discard the entire root transaction buffer.
- **Frameless trace boundary:** for mutations occurring outside any frame (no root frame present, or occurring before the root opens / after it closes), the bridge defers to the Layer 3 taxonomy below — frame topology has no opinion on these by construction.

---

## 3. Layer 3: Frameless Mutation Taxonomy

Every EMES mutation hook is classified against the tracing `reason` value the upstream hook call carries (`tracing.BalanceChangeReason`, `tracing.NonceChangeReason`), not just its event type — because, as verified below, the same event *type* can be frame-scoped or frameless depending on *why* the mutation occurred.

| Mutation Hook | Discriminator | Classification | Evidence |
|---|---|---|---|
| `BalanceMutationEvent` | `reason ∈ {BalanceDecreaseGasBuy, BalanceIncreaseGasReturn, BalanceIncreaseRewardTransactionFee}` | **Always Frameless** | `core/state_transition.go` — verified (static audit) |
| `BalanceMutationEvent` | any other `reason` (ordinary value transfer, `BalanceIncreaseSelfdestruct`, etc.) | **Always Frame-Scoped** | `tracer/kaysentinel_tracer.go`'s `OnBalanceChange` tags every event via live `currentFrameID()` — verified |
| `NonceMutationEvent` | `reason ∈ {NonceChangeEoACall, NonceChangeAuthorization}` | **Always Frameless** | `core/state_transition.go` — verified |
| `NonceMutationEvent` | `reason == NonceChangeContractCreator`, Amsterdam rules, insufficient account-creation state gas | **Frameless (rare edge case)** | `core/state_transition.go`'s `executeCreate` early-return path — verified |
| `NonceMutationEvent` | `reason ∈ {NonceChangeContractCreator, NonceChangeNewContract}`, normal path | **Always Frame-Scoped** | `core/vm/evm.go`'s `create()` — verified; fires after `captureBegin` (i.e. after `OnEnter`) |
| `CodeMutationEvent` (EIP-7702 delegation set/clear) | paired with `NonceChangeAuthorization` | **Always Frameless** | `applyAuthorization` — verified frameless; see EIP-7702 Scope Boundary below for the separate revert-observability gap |
| `StorageMutationEvent` | — | **Always Frame-Scoped (high confidence, not exhaustive)** | No `SetState`-equivalent call site found in `core/state_transition.go`'s transaction setup/teardown paths (confirmed via direct search across all `SubBalance`/`AddBalance`/`SetBalance`/`SetNonce`/`SetCode`/`SetState`/`CreateAccount`/`SelfDestruct`/`CreateContract` call sites in that file). The interpreter loop and EIP-4762/Verkle witness-gas paths in `core/vm/evm.go` have **not** been checked for a stray exception — this row is "pending exhaustive verification," not "verified," despite the strong supporting evidence. |

**Verification note:** every row above comes from static source reading (`core/state_transition.go`, `core/vm/evm.go`), not from running the tracer against a live or synthetic Geth trace. Per `governance.md`'s authority rubric, this is Medium authority — real code, single source, no empirical confirmation. Closing that gap requires an actual Go-Ethereum dev environment capable of executing real transactions and inspecting the tracer's emitted stream, which does not exist in the environment this document was produced in.

### EIP-7702 / Amsterdam Scope Boundary

Under Amsterdam rules, `applyAuthorizations` processes a transaction's authorization list sequentially. If an authorization earlier in the list is applied successfully (its `SetNonce`/`SetCode` calls fire, and the corresponding tracer hooks fire with them) and a **later** authorization in the same list fails specifically with `ErrOutOfGasRuntime`, the caller (`executeCall`) rolls back the entire batch via `RevertToSnapshot`. Go-ethereum's tracing hooks are not re-invoked to signal this rollback — the emitted EMES stream contains `NonceMutationEvent`/`CodeMutationEvent` entries for the earlier authority that describe a mutation that did not, in the end, take effect.

Verified narrowing of this scenario: ordinary authorization validation failures (wrong chain ID, bad signature, nonce mismatch, destination has code) never trigger this path at all — `validateAuthorization` returns before any state mutation, so there is nothing to roll back. The `ErrOutOfGasRuntime` check inside `applyAuthorization` also occurs before that specific authorization's own mutations — so the authorization that triggers the rollback never mutates anything itself either. The gap only exists for **earlier, already-applied** authorizations in a **multi-authorization** list.

**Decision:** this is documented as a known, accepted limitation of the EMES-V1 stream model — deterministic reconstruction is not guaranteed for a transaction hitting this specific multi-authorization, mid-batch, `ErrOutOfGasRuntime` path. It is deferred to a future protocol revision rather than requiring a new tracer hook/event type now, given the narrow, low-frequency trigger condition (single-authorization transactions — almost certainly the overwhelming majority of real EIP-7702 usage — cannot hit this path at all).
