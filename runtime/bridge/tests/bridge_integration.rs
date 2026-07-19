use kaysentinel_bridge::{parse_event_stream, translate, BridgeConfig, BridgeError};
use kaysentinel_cse::event::CsePayload;

const CFG: BridgeConfig = BridgeConfig { chain_id: 1 };

fn envelope(events_json: &str) -> String {
    format!(r#"{{"metadata":{{}},"events":[{events_json}]}}"#)
}

fn tagged(kind: &str, data: &str) -> String {
    format!(r#"{{"type":"{kind}","data":{data}}}"#)
}

const ADDR_A: &str = "0x1111111111111111111111111111111111111111";
const ADDR_B: &str = "0x2222222222222222222222222222222222222222";
const HASH_ZERO: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";
const HASH_ONE: &str = "0x0000000000000000000000000000000000000000000000000000000000000001";
const HASH_TWO: &str = "0x0000000000000000000000000000000000000000000000000000000000000002";
const NO_FRAME_HEX: &str = "18446744073709551615"; // u64::MAX, matches Go's ^uint64(0)

#[test]
fn simple_committed_call_round_trips_correctly() {
    let events = [
        tagged("BlockStart", r#"{"Number":100}"#),
        tagged("TransactionStart", &format!(r#"{{"TxIndex":0,"Hash":"{HASH_ONE}"}}"#)),
        tagged("FrameEnter", &format!(r#"{{"FrameID":0,"ParentFrameID":{NO_FRAME_HEX},"Depth":0}}"#)),
        tagged(
            "BalanceMutation",
            &format!(r#"{{"FrameID":0,"Address":"{ADDR_A}","Before":"{HASH_ZERO}","After":"{HASH_ONE}"}}"#),
        ),
        tagged("FrameExit", r#"{"FrameID":0,"Reverted":false}"#),
        tagged("TransactionEnd", r#"{"TxIndex":0,"Reverted":false}"#),
        tagged("BlockCommit", "{}"),
    ]
    .join(",");

    let parsed = parse_event_stream(&envelope(&events)).expect("parse failed");
    let out = translate(&parsed, &CFG).expect("translate failed");

    // BeginTransaction, BalanceChanged, EndTransaction -- FrameEnter/Exit and
    // block markers are consumed internally, not forwarded as CSE events.
    assert_eq!(out.len(), 3);
    assert!(matches!(out[0].payload, CsePayload::BeginTransaction));
    assert!(matches!(out[1].payload, CsePayload::BalanceChanged(_)));
    assert!(matches!(out[2].payload, CsePayload::EndTransaction));

    // Sequence numbers assigned densely, strictly increasing.
    assert_eq!(out[0].context.normative.sequence_number, 0);
    assert_eq!(out[1].context.normative.sequence_number, 1);
    assert_eq!(out[2].context.normative.sequence_number, 2);
}

#[test]
fn root_revert_discards_frame_scoped_mutation() {
    let events = [
        tagged("BlockStart", r#"{"Number":100}"#),
        tagged("TransactionStart", &format!(r#"{{"TxIndex":0,"Hash":"{HASH_ONE}"}}"#)),
        tagged("FrameEnter", &format!(r#"{{"FrameID":0,"ParentFrameID":{NO_FRAME_HEX},"Depth":0}}"#)),
        tagged(
            "StorageMutation",
            &format!(
                r#"{{"FrameID":0,"Address":"{ADDR_A}","Slot":"{HASH_ZERO}","Before":"{HASH_ZERO}","After":"{HASH_ONE}"}}"#
            ),
        ),
        tagged("FrameExit", r#"{"FrameID":0,"Reverted":true}"#),
        tagged("TransactionEnd", r#"{"TxIndex":0,"Reverted":true}"#),
        tagged("BlockCommit", "{}"),
    ]
    .join(",");

    let parsed = parse_event_stream(&envelope(&events)).expect("parse failed");
    let out = translate(&parsed, &CFG).expect("translate failed");

    // Only the boundary markers survive -- the reverted StorageMutation must
    // not appear anywhere in the output.
    assert_eq!(out.len(), 2);
    assert!(matches!(out[0].payload, CsePayload::BeginTransaction));
    assert!(matches!(out[1].payload, CsePayload::EndTransaction));
}

#[test]
fn frameless_mutations_survive_root_revert() {
    // Models the real, universal case: gas debit (frameless, before root
    // opens) must survive even when the root call reverts.
    let events = [
        tagged("BlockStart", r#"{"Number":100}"#),
        tagged("TransactionStart", &format!(r#"{{"TxIndex":0,"Hash":"{HASH_ONE}"}}"#)),
        tagged(
            "BalanceMutation",
            &format!(r#"{{"FrameID":{NO_FRAME_HEX},"Address":"{ADDR_A}","Before":"{HASH_TWO}","After":"{HASH_ONE}"}}"#),
        ),
        tagged("FrameEnter", &format!(r#"{{"FrameID":0,"ParentFrameID":{NO_FRAME_HEX},"Depth":0}}"#)),
        tagged(
            "StorageMutation",
            &format!(
                r#"{{"FrameID":0,"Address":"{ADDR_B}","Slot":"{HASH_ZERO}","Before":"{HASH_ZERO}","After":"{HASH_ONE}"}}"#
            ),
        ),
        tagged("FrameExit", r#"{"FrameID":0,"Reverted":true}"#),
        tagged(
            "BalanceMutation",
            &format!(r#"{{"FrameID":{NO_FRAME_HEX},"Address":"{ADDR_A}","Before":"{HASH_ONE}","After":"{HASH_ZERO}"}}"#),
        ),
        tagged("TransactionEnd", r#"{"TxIndex":0,"Reverted":true}"#),
        tagged("BlockCommit", "{}"),
    ]
    .join(",");

    let parsed = parse_event_stream(&envelope(&events)).expect("parse failed");
    let out = translate(&parsed, &CFG).expect("translate failed");

    // Begin, BalanceChanged (gas debit, before), BalanceChanged (gas refund,
    // after), End -- the StorageMutation inside the reverted root frame must
    // be gone, but both frameless balance changes must survive.
    assert_eq!(out.len(), 4);
    assert!(matches!(out[0].payload, CsePayload::BeginTransaction));
    assert!(matches!(out[1].payload, CsePayload::BalanceChanged(_)));
    assert!(matches!(out[2].payload, CsePayload::BalanceChanged(_)));
    assert!(matches!(out[3].payload, CsePayload::EndTransaction));
}

#[test]
fn nested_child_revert_does_not_discard_committed_root() {
    let events = [
        tagged("BlockStart", r#"{"Number":100}"#),
        tagged("TransactionStart", &format!(r#"{{"TxIndex":0,"Hash":"{HASH_ONE}"}}"#)),
        tagged("FrameEnter", &format!(r#"{{"FrameID":0,"ParentFrameID":{NO_FRAME_HEX},"Depth":0}}"#)),
        tagged(
            "BalanceMutation",
            &format!(r#"{{"FrameID":0,"Address":"{ADDR_A}","Before":"{HASH_ZERO}","After":"{HASH_ONE}"}}"#),
        ),
        tagged("FrameEnter", r#"{"FrameID":1,"ParentFrameID":0,"Depth":1}"#),
        tagged(
            "StorageMutation",
            &format!(
                r#"{{"FrameID":1,"Address":"{ADDR_B}","Slot":"{HASH_ZERO}","Before":"{HASH_ZERO}","After":"{HASH_ONE}"}}"#
            ),
        ),
        tagged("FrameExit", r#"{"FrameID":1,"Reverted":true}"#), // child reverts...
        tagged("FrameExit", r#"{"FrameID":0,"Reverted":false}"#), // ...root still commits
        tagged("TransactionEnd", r#"{"TxIndex":0,"Reverted":false}"#),
        tagged("BlockCommit", "{}"),
    ]
    .join(",");

    let parsed = parse_event_stream(&envelope(&events)).expect("parse failed");
    let out = translate(&parsed, &CFG).expect("translate failed");

    // Begin, BalanceChanged (root's own, kept), End -- the child's
    // StorageMutation must be gone; the root's own mutation must survive.
    assert_eq!(out.len(), 3);
    assert!(matches!(out[0].payload, CsePayload::BeginTransaction));
    assert!(matches!(out[1].payload, CsePayload::BalanceChanged(_)));
    assert!(matches!(out[2].payload, CsePayload::EndTransaction));
}

#[test]
fn zero_frame_transaction_passes_frameless_mutations_through() {
    // Models a pre-execution validation failure (per core/state_transition.go):
    // TxStart, gas debit (frameless), TxEnd(Reverted=true) -- no frames at all.
    let events = [
        tagged("BlockStart", r#"{"Number":100}"#),
        tagged("TransactionStart", &format!(r#"{{"TxIndex":0,"Hash":"{HASH_ONE}"}}"#)),
        tagged(
            "BalanceMutation",
            &format!(r#"{{"FrameID":{NO_FRAME_HEX},"Address":"{ADDR_A}","Before":"{HASH_TWO}","After":"{HASH_ONE}"}}"#),
        ),
        tagged("TransactionEnd", r#"{"TxIndex":0,"Reverted":true}"#),
        tagged("BlockCommit", "{}"),
    ]
    .join(",");

    let parsed = parse_event_stream(&envelope(&events)).expect("parse failed");
    let out = translate(&parsed, &CFG).expect("translate failed");

    assert_eq!(out.len(), 3);
    assert!(matches!(out[0].payload, CsePayload::BeginTransaction));
    assert!(matches!(out[1].payload, CsePayload::BalanceChanged(_)));
    assert!(matches!(out[2].payload, CsePayload::EndTransaction));
}

#[test]
fn unclosed_frame_at_transaction_end_is_rejected() {
    let events = [
        tagged("BlockStart", r#"{"Number":100}"#),
        tagged("TransactionStart", &format!(r#"{{"TxIndex":0,"Hash":"{HASH_ONE}"}}"#)),
        tagged("FrameEnter", &format!(r#"{{"FrameID":0,"ParentFrameID":{NO_FRAME_HEX},"Depth":0}}"#)),
        tagged("TransactionEnd", r#"{"TxIndex":0,"Reverted":false}"#),
        tagged("BlockCommit", "{}"),
    ]
    .join(",");

    let parsed = parse_event_stream(&envelope(&events)).expect("parse failed");
    let err = translate(&parsed, &CFG).unwrap_err();
    assert!(matches!(err, BridgeError::UnbalancedFrames { .. }));
}

#[test]
fn hex_address_and_hash_decode_correctly() {
    // Sanity check on the hex-decoding path itself, independent of buffering.
    let events = [
        tagged("BlockStart", r#"{"Number":1}"#),
        tagged("TransactionStart", &format!(r#"{{"TxIndex":0,"Hash":"{HASH_ONE}"}}"#)),
        tagged(
            "BalanceMutation",
            &format!(r#"{{"FrameID":{NO_FRAME_HEX},"Address":"{ADDR_A}","Before":"{HASH_ZERO}","After":"{HASH_TWO}"}}"#),
        ),
        tagged("TransactionEnd", r#"{"TxIndex":0,"Reverted":false}"#),
        tagged("BlockCommit", "{}"),
    ]
    .join(",");

    let parsed = parse_event_stream(&envelope(&events)).expect("parse failed");
    let out = translate(&parsed, &CFG).expect("translate failed");

    let CsePayload::BalanceChanged(b) = &out[1].payload else {
        panic!("expected BalanceChanged");
    };
    assert_eq!(b.address, [0x11u8; 20]);
    assert_eq!(b.previous_balance, [0u8; 32]);
    let mut expected_after = [0u8; 32];
    expected_after[31] = 2;
    assert_eq!(b.current_balance, expected_after);
}
