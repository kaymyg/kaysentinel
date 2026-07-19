//! Deserialization of the Go tracer's tagged JSON wire format.
//!
//! Matches `emes.FixtureEnvelope.MarshalJSON()` exactly: each event is
//! `{"type": "<Kind()>", "data": {...}}`, where `<Kind()>` is one of the 12
//! literal strings returned by each Go event type's `Kind()` method
//! (`emes/types.go`) -- e.g. `"BalanceMutation"`, not `"BalanceMutationEvent"`.
//!
//! Field names below match the real Go struct fields exactly (`FrameID`,
//! `ParentFrameID`, `Before`/`After`, etc.), including Go's capitalization,
//! since `serde(rename_all)` isn't used here -- Go's `encoding/json` produces
//! `PascalCase` keys by default (no `json` tags on most fields in
//! `emes/types.go`), and this must match on the wire exactly.

use serde::Deserialize;

pub const HASH_LEN: usize = 32;
pub const ADDRESS_LEN: usize = 20;

/// A 32-byte hash/256-bit value, hex-decoded from Go's `"0x..."` string
/// convention (`emes.Hash.MarshalJSON`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HexHash(pub [u8; HASH_LEN]);

/// A 20-byte address, hex-decoded the same way (`emes.Address.MarshalJSON`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HexAddress(pub [u8; ADDRESS_LEN]);

fn decode_fixed_hex<const N: usize>(s: &str) -> Result<[u8; N], String> {
    let s = s
        .strip_prefix("0x")
        .ok_or_else(|| format!("hex value {s:?} must be 0x-prefixed"))?;
    let bytes = hex_decode(s)?;
    if bytes.len() != N {
        return Err(format!("hex value has {} bytes, expected {}", bytes.len(), N));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err(format!("odd-length hex string {s:?}"));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

impl<'de> Deserialize<'de> for HexHash {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        decode_fixed_hex::<HASH_LEN>(&s).map(HexHash).map_err(serde::de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for HexAddress {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        decode_fixed_hex::<ADDRESS_LEN>(&s).map(HexAddress).map_err(serde::de::Error::custom)
    }
}

/// The sentinel `ParentFrameID`/`FrameID` value Go uses for "no frame" --
/// `^uint64(0)` in Go, i.e. `u64::MAX`. Verified against `kaysentinel_tracer.go`
/// (`noParentFrame`) and `validation/gate1.go`'s own sentinel check.
pub const NO_FRAME: u64 = u64::MAX;

#[derive(Debug, Clone, Deserialize)]
pub struct BlockStartData {
    #[serde(rename = "Number")]
    pub number: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionStartData {
    #[serde(rename = "TxIndex")]
    pub tx_index: u64,
    #[serde(rename = "Hash")]
    pub hash: HexHash,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionEndData {
    #[serde(rename = "TxIndex")]
    pub tx_index: u64,
    #[serde(rename = "Reverted")]
    pub reverted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrameEnterData {
    #[serde(rename = "FrameID")]
    pub frame_id: u64,
    #[serde(rename = "ParentFrameID")]
    pub parent_frame_id: u64,
    #[serde(rename = "Depth")]
    pub depth: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrameExitData {
    #[serde(rename = "FrameID")]
    pub frame_id: u64,
    #[serde(rename = "Reverted")]
    pub reverted: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BalanceMutationData {
    #[serde(rename = "FrameID")]
    pub frame_id: u64,
    #[serde(rename = "Address")]
    pub address: HexAddress,
    #[serde(rename = "Before")]
    pub before: HexHash,
    #[serde(rename = "After")]
    pub after: HexHash,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NonceMutationData {
    #[serde(rename = "FrameID")]
    pub frame_id: u64,
    #[serde(rename = "Address")]
    pub address: HexAddress,
    #[serde(rename = "Before")]
    pub before: u64,
    #[serde(rename = "After")]
    pub after: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodeMutationData {
    #[serde(rename = "FrameID")]
    pub frame_id: u64,
    #[serde(rename = "Address")]
    pub address: HexAddress,
    #[serde(rename = "Before")]
    pub before: HexHash,
    #[serde(rename = "After")]
    pub after: HexHash,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageMutationData {
    #[serde(rename = "FrameID")]
    pub frame_id: u64,
    #[serde(rename = "Address")]
    pub address: HexAddress,
    #[serde(rename = "Slot")]
    pub slot: HexHash,
    #[serde(rename = "Before")]
    pub before: HexHash,
    #[serde(rename = "After")]
    pub after: HexHash,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountCreatedData {
    #[serde(rename = "FrameID")]
    pub frame_id: u64,
    #[serde(rename = "Address")]
    pub address: HexAddress,
    #[serde(rename = "Creator")]
    pub creator: HexAddress,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SelfDestructData {
    #[serde(rename = "FrameID")]
    pub frame_id: u64,
    #[serde(rename = "Address")]
    pub address: HexAddress,
    #[serde(rename = "Beneficiary")]
    pub beneficiary: HexAddress,
}

/// One event in the Go tracer's stream, after stripping the `{"type", "data"}`
/// tagging wrapper. `BlockCommitEvent` carries a `StateRoot` field on the Go
/// side, but it's intentionally not modeled here -- per the established
/// design (docs/emes/004-bridge-buffering-spec.md), block-level events are
/// dropped at the bridge boundary entirely; there is no Rust type to receive
/// block data.
#[derive(Debug, Clone)]
pub enum GoEvent {
    BlockStart(BlockStartData),
    TransactionStart(TransactionStartData),
    FrameEnter(FrameEnterData),
    FrameExit(FrameExitData),
    TransactionEnd(TransactionEndData),
    BlockCommit,
    BalanceMutation(BalanceMutationData),
    NonceMutation(NonceMutationData),
    CodeMutation(CodeMutationData),
    StorageMutation(StorageMutationData),
    AccountCreated(AccountCreatedData),
    SelfDestruct(SelfDestructData),
}

#[derive(Debug, Deserialize)]
struct TaggedEvent {
    #[serde(rename = "type")]
    kind: String,
    data: serde_json::Value,
}

/// Parses a full Go `FixtureEnvelope`-shaped JSON document's `events` array
/// into `GoEvent`s. Ignores `metadata`/`errors` fields entirely -- callers
/// needing `chain_id` should read `metadata.chain_id` from the same JSON
/// document themselves (see the `chain_id` gap noted in
/// `translate::BridgeConfig`).
pub fn parse_event_stream(json: &str) -> Result<Vec<GoEvent>, String> {
    #[derive(Deserialize)]
    struct Envelope {
        events: Vec<TaggedEvent>,
    }

    let envelope: Envelope = serde_json::from_str(json).map_err(|e| e.to_string())?;

    envelope
        .events
        .into_iter()
        .map(|tagged| {
            fn parse<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Result<T, String> {
                serde_json::from_value(v).map_err(|e| e.to_string())
            }
            match tagged.kind.as_str() {
                "BlockStart" => Ok(GoEvent::BlockStart(parse(tagged.data)?)),
                "TransactionStart" => Ok(GoEvent::TransactionStart(parse(tagged.data)?)),
                "FrameEnter" => Ok(GoEvent::FrameEnter(parse(tagged.data)?)),
                "FrameExit" => Ok(GoEvent::FrameExit(parse(tagged.data)?)),
                "TransactionEnd" => Ok(GoEvent::TransactionEnd(parse(tagged.data)?)),
                "BlockCommit" => Ok(GoEvent::BlockCommit),
                "BalanceMutation" => Ok(GoEvent::BalanceMutation(parse(tagged.data)?)),
                "NonceMutation" => Ok(GoEvent::NonceMutation(parse(tagged.data)?)),
                "CodeMutation" => Ok(GoEvent::CodeMutation(parse(tagged.data)?)),
                "StorageMutation" => Ok(GoEvent::StorageMutation(parse(tagged.data)?)),
                "AccountCreated" => Ok(GoEvent::AccountCreated(parse(tagged.data)?)),
                "SelfDestruct" => Ok(GoEvent::SelfDestruct(parse(tagged.data)?)),
                other => Err(format!("unknown event kind {other:?}")),
            }
        })
        .collect()
}
