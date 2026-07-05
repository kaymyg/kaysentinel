"""
verify_ssr.py

Validates byte-level convergence between two client-side SSR (Structural
Sufficient Representation) extractions, as produced by a Geth-side and a
Reth-side Kaysentinel extractor implementation.

Usage:
    from verify_ssr import verify_differential_ssr, SSRVerificationError

    try:
        verify_differential_ssr(geth_bytes, reth_bytes)
    except SSRVerificationError:
        # This branch is the ONLY correct way to handle a failed check.
        # Deny / block here. Do NOT catch a broader exception type
        # (e.g. `except Exception`) elsewhere in the call chain and
        # treat it as success -- that turns this authorization check
        # fail-open.
        deny()

Note: this script only checks the *comparison* logic. It does not itself
implement the Geth or Reth extractors -- those still need to be built
(see docs/framework.md, section 8).
"""

import hashlib
import hmac

# Defensive upper bound on input size. SSR payloads are bounded by protocol
# design; anything wildly larger than that is either a bug or an attempt to
# force excessive hashing/memory work. Adjust to the real max SSR size once
# the SSZ schema (Phase 1) is finalized.
MAX_SSR_BYTES = 16 * 1024 * 1024  # 16 MiB


class SSRVerificationError(Exception):
    """Base class for all SSR verification failures.

    Deliberately a dedicated exception hierarchy (not ValueError /
    AssertionError) so that callers cannot accidentally swallow a failed
    authorization check inside a generic `except (ValueError, ...)` block
    written for unrelated input-validation errors elsewhere in the
    pipeline. Any code that catches this exception is expected to deny
    the action being authorized, not to continue as if verification had
    succeeded.
    """


class SSRLengthMismatch(SSRVerificationError):
    """Raised when the two SSR payloads differ in length."""


class SSRDivergenceError(SSRVerificationError):
    """Raised when the two SSR payloads differ in content."""


def _validate_input(name: str, data: bytes) -> None:
    if not isinstance(data, (bytes, bytearray)):
        raise TypeError(f"{name} must be bytes, got {type(data).__name__}")
    if len(data) == 0:
        raise ValueError(f"{name} must not be empty")
    if len(data) > MAX_SSR_BYTES:
        raise ValueError(
            f"{name} exceeds MAX_SSR_BYTES ({len(data)} > {MAX_SSR_BYTES})"
        )


def verify_differential_ssr(geth_ssz_bytes: bytes, reth_ssz_bytes: bytes) -> bool:
    """
    Validates absolute byte-level convergence between client SSR extractions.
    Ensures that client-internal structures produce zero semantic drift.

    Returns True only on successful verification. On any mismatch, raises
    a subclass of SSRVerificationError -- it never returns False. Fail
    closed: if this function is called and its exception is not
    explicitly handled by denying the associated authorization, the
    surrounding program will halt rather than silently proceed.
    """
    _validate_input("geth_ssz_bytes", geth_ssz_bytes)
    _validate_input("reth_ssz_bytes", reth_ssz_bytes)

    # 1. Assert identical payload sizing
    if len(geth_ssz_bytes) != len(reth_ssz_bytes):
        raise SSRLengthMismatch(
            f"Consensus Divergence Detected: Byte length mismatch. "
            f"Geth: {len(geth_ssz_bytes)} bytes, Reth: {len(reth_ssz_bytes)} bytes."
        )

    # 2. Compute SHA-256 root hashes
    geth_root = hashlib.sha256(geth_ssz_bytes).digest()
    reth_root = hashlib.sha256(reth_ssz_bytes).digest()

    # Constant-time comparison. Not strictly required here since neither
    # value is secret, but it costs nothing and removes any need to
    # reason about timing behavior if this function is ever reused in a
    # context where one side's hash is confidential.
    if not hmac.compare_digest(geth_root, reth_root):
        # Trace byte variance to identify the first layout or alignment
        # fault. This is diagnostic only -- the authorization decision
        # was already made above by the hash comparison.
        first_diff = None
        for idx, (b_g, b_r) in enumerate(zip(geth_ssz_bytes, reth_ssz_bytes)):
            if b_g != b_r:
                first_diff = idx
                break

        detail = (
            f"first differing byte at offset {hex(first_diff)}"
            if first_diff is not None
            else "hash mismatch with no byte-level difference found "
            "(unexpected; investigate encoding/comparison logic)"
        )
        raise SSRDivergenceError(
            f"Structural divergence detected. Geth root: 0x{geth_root.hex()}, "
            f"Reth root: 0x{reth_root.hex()}. {detail}."
        )

    print(f"SSR Verification Success. Canonical Root: 0x{geth_root.hex()}")
    return True


if __name__ == "__main__":
    # Minimal smoke test with matching dummy payloads.
    dummy = b"\x00" * 32
    verify_differential_ssr(dummy, dummy)

    # Smoke test for the failure path.
    try:
        verify_differential_ssr(dummy, b"\x01" * 32)
    except SSRDivergenceError as e:
        print(f"Expected failure raised correctly: {e}")
