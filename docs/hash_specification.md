# KAYSentinel Hash Specification (RC1)

Normative cryptographic protocol for `kaysentinel-hash`.

**Status:** Release Candidate 1 (RC1). The Rust reference implementation exists and
is self-consistent (deterministic, domain-separated, regression-pinned by unit
tests). Cross-language verification (Python/Go) has **not** happened — no
implementations exist in those languages yet — so vectors in `vectors/candidate.json`
remain `"status": "candidate"`, not `"normative"`, per the promotion criteria below.

## 1. Terminology

The key words "MUST", "MUST NOT", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", and
"MAY" in this document are to be interpreted as described in RFC 2119 and updated by
RFC 8174.

## 2. Cryptographic Primitive & Configuration

* **Core Algorithm:** The protocol SHALL use the BLAKE3 algorithm. All commitments
  MUST use the standard unkeyed BLAKE3 hash function. Keyed hashing, `derive_key`
  mode, and extendable-output mode (XOF) are prohibited.
* **Commitment Layout:** The resulting digest is an immutable sequence of exactly 32
  bytes (256 bits). No integer interpretation, byte-swapping, or endianness applies
  to the final digest sequence.
* **Security Constraints:** The security architecture relies strictly on the
  collision resistance and preimage resistance properties of the underlying BLAKE3
  hash function.
* **Portability & Determinism:** Given identical `DomainBytes` and `CanonicalBytes`,
  all conforming implementations SHALL produce identical digest bytes on every
  platform, host architecture, and execution environment.

## 3. Domain Separation Architecture

```
HashInput := DomainBytes || CanonicalBytes
Digest    := BLAKE3(HashInput)
```

* **DomainBytes**: the raw, fixed byte sequence representing the unique protocol
  constant.
* **CanonicalBytes**: the deterministic, byte-stable serialization of the underlying
  data structure.
* **||**: strict byte concatenation.

Core invariants:

1. **Structural Ordering:** domain separation SHALL precede all canonical bytes. No
   protocol structure may insert additional bytes, padding, version flags,
   delimiters, framing bytes, or metadata between the domain identifier and the
   canonical serialization.
2. **Infallibility by Contract:** commitment derivation is total and cannot fail once
   valid canonical bytes have been produced.
3. **No Normalization:** domain identifiers SHALL be matched strictly byte-for-byte.
   Unicode normalization, case folding, locale conversion, whitespace trimming, or
   character encoding transformations are prohibited.

## 4. Canonical Encoding Separation

The canonical encoding of every protocol structure is defined independently of the
hashing function. Canonical encoding SHALL be deterministic, injective (no two
distinct logical objects serialize to identical byte sequences), and entirely
independent of the runtime environment. The hashing function operates exclusively on
raw, opaque byte sequences and is not responsible for serialization semantics.

**Status:** no canonical encoding (SSZ) exists yet — `runtime/ssz` is still an empty
stub crate. Everything hashed so far in tests/vectors is raw test bytes, not real
canonical-encoded protocol structures.

## 5. Hash Domain Registry (Version: 1)

Future protocol versions MAY append new domain tags to this registry. Existing tags
SHALL remain permanently immutable, SHALL NOT change their underlying semantic
meaning, and SHALL NEVER be reused or recycled upon feature retirement. No two
protocol operations SHALL share the same domain byte sequence.

| Protocol Constant | Normative Value (ASCII) | Context / Usage |
| --- | --- | --- |
| `KAY_TRIE_LEAF` | `"KAY_TRIE_LEAF"` | Storage trie leaf nodes |
| `KAY_TRIE_BRANCH` | `"KAY_TRIE_BRANCH"` | Storage trie branch nodes |
| `KAY_LIFE_CERT` | `"KAY_LIFE_CERT"` | Verification certificates |
| `KAY_SSR_ROOT` | `"KAY_SSR_ROOT"` | Serialization Record roots |

## 6. Reference Implementation

`runtime/hash/src/lib.rs` implements this specification: `Digest`, `Domain`,
`derive_commitment(domain, canonical_bytes) -> Digest`. 8 unit tests cover
determinism, domain separation, length validation, and pin real (not hand-typed)
BLAKE3 output for one vector per registered domain — see
`runtime/hash/vectors/candidate.json`.

## 7. Vector Status & Promotion Criteria

Per the execution lifecycle, a vector is `"candidate"` once a reference
implementation computes and regression-pins it, and becomes `"normative"` only after
independent implementations in multiple languages are shown to converge on the same
digest via automated differential verification. `runtime/hash/vectors/candidate.json`
holds real, computed, Rust-self-consistent values — but since no Python or Go
implementation exists yet to check against, none have been promoted to
`vectors/normative.json`, and that file doesn't exist yet.
