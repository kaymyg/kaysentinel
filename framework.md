# Kaysentinel / PCAL + Sentinel — Formal + Engineering Framework

## 0. System Overview

Kaysentinel defines a post-execution authorization architecture for state-machine systems (e.g., Ethereum-class runtimes) by lifting execution traces into a canonical Structural Sufficient Representation (SSR) and evaluating policies purely over that quotient space.

**Core idea:** instead of authorizing *execution paths*, the system authorizes equivalence classes of state-impacting traces.

## 1. Formal Semantic Foundation

### 1.1 Execution Model

- Σ = space of execution traces
- s ∈ S = pre-state
- t ∈ T = transaction
- Π(s,t) ∈ Σ = deterministic execution trace

### 1.2 Observation Model

Post(σ) := (State(σ), Receipt(σ), Context(σ))

- **State(σ)** = (balances, storage, transient, nonces, lifecycles)
- **Receipt(σ)** = (logs, status, gas)
- **Context(σ)** = (block, tx_index, basefee)

### 1.3 SSR Extraction Map

E: Σ → Δ, where Δ is the canonical SSR space.

### 1.4 Fundamental Equivalence Property

- **Faithfulness:** E(σ1) = E(σ2) ⇒ Post(σ1) = Post(σ2)
- **Abstraction:** Post(σ1) = Post(σ2) ⇒ E(σ1) = E(σ2)
- **Result (quotient isomorphism):** Σ / ~Post ≅ Δ

## 2. SSR Canonical Form

### 2.1 Canonical Type System (SSZ-based)

Primitive types: Address = Bytes20, Hash = Bytes32, Uint64 / Uint256.

### 2.2 SSR Structure

Δ := CanonicalSSR, containing account mutations, storage diffs, transient diffs, logs, tx metadata — all with bounded lists.

### 2.3 Determinism Rules

1. Accounts sorted by address
2. Storage sorted by slot
3. Logs preserve execution order
4. Serialization = SSZ canonical encoding

### 2.4 Canonical Constraint

Ser(Δ1) = Ser(Δ2) ⟺ Δ1 = Δ2

## 3. Authorization Theory

### 3.1 Admissible Authorization Class

A ∈ 𝔄_obs iff Post(σ1) = Post(σ2) ⇒ A(σ1) = A(σ2)

### 3.2 Induced Policy (Factorization Core)

**Theorem (Factorization):** there exists a unique Ã: Δ → D such that A = Ã ∘ E.

All valid policies operate only over SSR space, never raw execution.

### 3.3 Policy Execution Pipeline

B(Δ, A_p ⋄ A_proto):
- FAIL if protocol rejects
- else policy evaluation on SSR

### 3.4 Null Policy Invariance

A_p^∅(Δ) = PASS, so valid execution ⇒ no behavioral divergence introduced.

## 4. Portability (Multi-Client Consensus Theorem)

**Theorem (SSR Portability):** let E1 = Geth extractor, E2 = Reth extractor. If both satisfy Faithfulness + Abstraction, then A = Ã∘E1 = Ã∘E2.

**Corollary:** all compliant clients compute identical policy outcomes — E1(Σ) ≡ E2(Σ) ⇒ no policy-induced forks.

## 5. Execution Model (Final Semantics)

### 5.1 Two-Phase Observation Model

- **Phase 1 — TraceMid:** captured during execution (transient storage snapshot, intermediate write-set)
- **Phase 2 — Post(σ):** captured after execution (state diff, receipt, logs)

### 5.2 Execution Identity

σ = Π(s,t); SSR extracted as E(σ).

## 6. Gating Semantics

### 6.1 Decision Output Space

{PASS, FAIL, QUARANTINE}

### 6.2 State Commitment Rules

| Result | Effect |
|---|---|
| PASS | Commit state |
| FAIL | Revert execution |
| QUARANTINE | Commit state + isolate metadata |

### 6.3 Non-Divergence Constraint

Policy layer must not alter consensus state root: StateRoot_native = StateRoot_sentinel.

## 7. Complexity Model

Let N = total state mutations.

- Extraction cost: T_extract = O(N)
- Canonicalization cost: worst-case O(N log N), bounded case O(N)
- Total cost: T_total = O(N log N), practical ≈ O(N)

## 8. Client Architecture Boundary Model

### 8.1 Geth Extraction Boundary

- Source: `StateDB` journal
- Hook: post-execution, pre-commit
- Extract: dirty accounts, storage diffs, logs, transient map snapshot

### 8.2 Reth Extraction Boundary

- Source: `BundleState`
- Hook: execution result stage
- Extract: state diffs, storage diffs, logs, transient snapshot

### 8.3 Canonicalization Layer

Both map into: φ_geth(E_geth) = φ_reth(E_reth)

## 9. Core Architectural Result

**System Closure Theorem:** the system defines a closed loop Σ →(E) Δ →(Ã) D satisfying determinism, client independence, post-execution isolation, and quotient-space completeness.

**Summary:** Kaysentinel is a client-agnostic post-execution authorization calculus over a canonical state quotient space.

## 10. Implementation Readiness

Specified across:
- Formal semantics (Σ → Δ quotient model)
- Canonical encoding (SSZ SSR)
- Multi-client extractors (Geth / Reth)
- Policy calculus (Ã factorization)
- Execution gating semantics (PASS/FAIL/QUARANTINE)
- Complexity guarantees (O(N) / O(N log N))

**Not yet done:** no reference implementation exists for the Geth/Reth extractors or the SSZ encoder. The theorems above are design invariants to build and test against, not proofs that have been mechanically verified.
