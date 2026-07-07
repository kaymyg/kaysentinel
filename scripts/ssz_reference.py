"""
Reference SSZ-V1 encoder/merkleizer for KAY Sentinel's SemanticStateRecord.

This is a REFERENCE implementation of the Layer 2 serialization + merkleization
rules in docs/ssz_profile.md. It is NOT a Geth/Reth extractor -- it takes the
already-normalized `expected_normalized_ssr` block from a validation_vectors/
fixture and encodes it, so it validates the encoding/merkleization layer only.
Client-fidelity (does a real trace actually normalize to this SSR) is still
untested; that's Phase 2/3 of the implementation roadmap.

Depends on `remerkleable` (https://pypi.org/project/remerkleable/), an
existing, independently-maintained SSZ implementation used in Ethereum
consensus-layer tooling -- this script does not reimplement SSZ from scratch.

Usage:
    pip install remerkleable
    python ssz_reference.py path/to/validation_vectors/KS-ST-001.json
    python ssz_reference.py path/to/validation_vectors/*.json --write-manifest schemas/ssz_v1_conformance_manifest.json
"""
import argparse
import glob
import json
import sys

from remerkleable.basic import uint8, uint64, uint256
from remerkleable.byte_arrays import ByteVector, Bytes32
from remerkleable.complex import Container, List

Bytes20 = ByteVector[20]

MAX_STORAGE_UPDATES = 65536  # 2^16
MAX_ACCOUNTS = 16384         # 2^14


class StorageUpdate(Container):
    slot: Bytes32
    original: Bytes32
    current: Bytes32


class AccountDelta(Container):
    address: Bytes20
    original_nonce: uint64
    current_nonce: uint64
    original_balance: uint256
    current_balance: uint256
    original_code_hash: Bytes32
    current_code_hash: Bytes32
    storage_updates: List[StorageUpdate, MAX_STORAGE_UPDATES]


class SemanticStateRecord(Container):
    consensus_outcome: uint8   # 0 = Included, 1 = Rejected
    execution_outcome: uint8   # 0 = Success, 1 = Revert, 2 = ExceptionalHalt
    account_deltas: List[AccountDelta, MAX_ACCOUNTS]


CONSENSUS_MAP = {"Included": 0, "Rejected": 1}
EXECUTION_MAP = {"Success": 0, "Revert": 1, "ExceptionalHalt": 2}


def build_ssr(vector: dict) -> SemanticStateRecord:
    ssr_data = vector["expected_normalized_ssr"]
    account_views = []
    for acc in ssr_data["account_deltas"]:
        su_views = [
            StorageUpdate(
                slot=Bytes32(bytes.fromhex(su["slot"][2:])),
                original=Bytes32(bytes.fromhex(su["original"][2:])),
                current=Bytes32(bytes.fromhex(su["current"][2:])),
            )
            for su in acc["storage_updates"]
        ]
        account_views.append(
            AccountDelta(
                address=Bytes20(bytes.fromhex(acc["address"][2:])),
                original_nonce=uint64(acc["nonce"]["original"]),
                current_nonce=uint64(acc["nonce"]["current"]),
                original_balance=uint256(int(acc["balance"]["original"])),
                current_balance=uint256(int(acc["balance"]["current"])),
                original_code_hash=Bytes32(bytes.fromhex(acc["code_hash"]["original"][2:])),
                current_code_hash=Bytes32(bytes.fromhex(acc["code_hash"]["current"][2:])),
                storage_updates=List[StorageUpdate, MAX_STORAGE_UPDATES](*su_views),
            )
        )
    return SemanticStateRecord(
        consensus_outcome=uint8(CONSENSUS_MAP[ssr_data["consensus_outcome"]]),
        execution_outcome=uint8(EXECUTION_MAP[ssr_data["execution_outcome"]]),
        account_deltas=List[AccountDelta, MAX_ACCOUNTS](*account_views),
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("vector_files", nargs="+", help="validation_vectors/*.json files")
    parser.add_argument("--write-manifest", help="path to write a Layer 2 conformance manifest JSON")
    args = parser.parse_args()

    files = []
    for pattern in args.vector_files:
        files.extend(sorted(glob.glob(pattern)) or [pattern])

    manifest_targets = []
    for path in files:
        vector = json.load(open(path))
        ssr = build_ssr(vector)
        encoded = ssr.encode_bytes()
        root = "0x" + ssr.hash_tree_root().hex()
        length = len(encoded)
        vid = vector["vector_id"]
        print(f"{vid}: serialized_length={length}  expected_root_hash={root}")
        manifest_targets.append({
            "vector_id": vid,
            "serialized_length": length,
            "expected_root_hash": root,
        })

    if args.write_manifest:
        manifest = {
            "profile_id": "SSZ-V1",
            "profile_target_version": "1.0.0",
            "status": "computed by scripts/ssz_reference.py against remerkleable; "
                      "validates encoding/merkleization only, not client fidelity",
            "vector_targets": manifest_targets,
        }
        with open(args.write_manifest, "w") as f:
            json.dump(manifest, f, indent=2)
            f.write("\n")
        print(f"\nWrote manifest to {args.write_manifest}")


if __name__ == "__main__":
    sys.exit(main())
