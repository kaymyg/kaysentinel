# KAYSentinel SSZ Specification (SSR-RC1 / SSZ-RC1 / MP1)

**Status:** implemented and tested in `runtime/ssz`. Two corrections were made to
the source spec during implementation — see §5 below before trusting anything
copied from an earlier draft of this document.

## 1. Architectural Layering

```
[LAYER 1: SSR DATA MODEL]
        │  (canonical ordering, modulo 2ⁿ)
        ▼
[LAYER 2: SSZ WIRE FORMAT]
        │  (measure → plan → write)
        ▼
[LAYER 3: MERKLE PROFILE MP1]
        │  (chunkify → deterministic tree)
        ▼
[FINAL CANONICAL ROOT]
```

### Constitutional invariants

1. **Referential Transparency:** `encode: SSR -> Bytes` is a pure function of the
   object's semantic data — no dependence on host endianness, memory addresses,
   thread scheduling, or compiler optimization state.
2. **Serialization Locality:** changing one field's value never disturbs a sibling
   field's byte sequence. Downstream effects are bounded to the mutated field and
   its own offset pointer.
3. **Canonical Ordering:** any semantically-ordered collection is canonicalized by
   the validation layer *before* serialization — the serialization engine itself
   never sorts, reorders, deduplicates, or normalizes.
4. **Byte-Level Unambiguity:** fixed-width primitives (`uint32` = 4 bytes, `uint64`
   = 8 bytes), little-endian, modulo 2ⁿ. No floats, no signed integers.

## 2. Wire Format (SSZ-RC1)

Three-phase pipeline: **Measure** (compute fixed header + variable footprint size),
**Plan** (assign fixed-field offsets; variable fields get a 4-byte offset slot),
**Write** (fixed section + offsets, then variable payloads concatenated).

### `CanonicalEventSummary` — fixed-size container, 68 bytes (8+8+20+32)

| Field | Type | Bytes |
|---|---|---|
| `block_timestamp` | `uint64` | 0-7 |
| `sequence_nonce` | `uint64` | 8-15 |
| `actor_address` | `bytes20` | 16-35 |
| `payload_digest` | `bytes32` | 36-67 |

### `EpochStateSnapshot` — variable-size container

| Field | Type | Notes |
|---|---|---|
| `epoch_index` | `uint64` | fixed, bytes 0-7 |
| `event_summaries` | `List[CanonicalEventSummary, 16384]` | 4-byte offset at bytes 8-11 |

Implemented in `runtime/ssz/src/lib.rs::{CanonicalEventSummary, EpochStateSnapshot}`.

## 3. Merkle Profile MP1

```
                [Merkle Root]
                    /  \
             [Node 0]    [Node 1]
               /  \        /  \
             C0    C1    C2    C3   <- 32-byte padded chunks
```

1. **Chunkification:** slice the canonical byte stream into 32-byte chunks;
   right-pad the final chunk with zero octets if short.
2. **Tree construction:** arrange chunks as leaves of a deterministic binary tree.
3. **Hash profile:** internal nodes use domain-separated BLAKE3.

Implemented in `runtime/ssz/src/lib.rs::{chunkify, merkleize, ssr_root}`, built on
top of `kaysentinel-hash`'s `derive_commitment`.

## 4. Test Vector: VEC-SSR-RC1-001

Input: `epoch_index = 5`, one `CanonicalEventSummary` with
`block_timestamp = 171717`, `sequence_nonce = 1`,
`actor_address = 0xaaaa...aaaa` (20 bytes), `payload_digest = 0xffff...ffff` (32 bytes).

**Correctly computed and verified** (independently in Python, then in Rust, then
pinned as a regression test):

```
serialized_bytes_hex = 05000000000000000c000000c59e0200000000000100000000000000
                        aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
                        ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
total_length_bytes   = 80
ssr_root_hex         = 84fc188deeb381f7d1ac8454af2d21e37c8891112f6c37b4977e7d1c9b80820d
```

## 5. Corrections made to the source spec during implementation

An earlier draft of this document included a claimed `serialized_bytes_hex` and
`blake3_integrity_hash` for VEC-SSR-RC1-001 that were **wrong, not just
unverified**: decoding the claimed hex's `block_timestamp` field back out gives
`171711`, not the `171717` stated in the same vector's own input object — a
6-unit discrepancy that can't be explained by encoding convention. This was caught
by independently computing the encoding (Python, then this Rust implementation)
rather than transcribing the claimed value, and the correct hex/root above replace
it.

Two things in the source spec's prose were also ambiguous and had to be resolved
explicitly rather than guessed silently:

1. **Odd leaf-count handling:** the source text said the final leaf is "duplicated
   or handled strictly according to the deterministic tree packing standard" —
   offering two uncommitted-to alternatives. This implementation pads with zero
   chunks to the next power of two (the standard SSZ approach), not duplication,
   since duplication of the final leaf creates second-preimage weaknesses that
   fixed zero-padding avoids.
2. **Domain separation in the Merkle hash formula:** the source text captioned
   `H = BLAKE3(Left || Right)` as "domain-separated" but the formula itself shows
   no domain prefix. This implementation actually applies one — hashing
   `Domain::SsrRoot bytes || Left || Right` via `kaysentinel_hash::derive_commitment`
   — both to make the "domain-separated" claim true and to stay consistent with
   `kaysentinel-hash`'s own domain-separation invariant.

One more inconsistency, **not yet relevant to any implemented type** but worth
flagging for whoever adds a `bool` field next: the source spec states `bool` is
encoded as `0x00` (false) / `0x02` (true). The universal SSZ convention (and every
other implementation you'll interoperate with) uses `0x01` for true. Neither
`CanonicalEventSummary` nor `EpochStateSnapshot` has a `bool` field, so this
doesn't affect anything implemented so far — but it should be corrected to `0x01`
before any type that does use `bool` gets built, or cross-implementation vectors
will silently disagree.
