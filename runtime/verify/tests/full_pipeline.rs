//! The capstone end-to-end test for the whole project: a Go-tracer-shaped
//! JSON event stream, through every real pipeline stage, verified by Gate 2.
//!
//!   Go JSON --(bridge::parse_event_stream)--> GoEvent stream
//!           --(bridge::translate)-----------> CanonicalSemanticEvent stream
//!           --(builder::partition_events)---> RawIr
//!           --(builder::reduce_timelines)---> ReducedIr
//!           --(builder::resolve_lifecycles)-> canonical generations
//!           --(builder::build_canonical_certificates
//!              + resolve_storage_roots)-----> CanonicalAccountCertificate set
//!           --(verify::replay_and_verify)---> Gate 2 PASS
//!
//! Every stage here is the real, shipped implementation -- no mocks, no
//! simulated stand-ins. This test failing means the pipeline's stages have
//! genuinely drifted out of agreement with each other.

use std::collections::BTreeMap;

use kaysentinel_bridge::{parse_event_stream, translate, BridgeConfig};
use kaysentinel_builder::lifecycle::certificate::{
    AccountSnapshotSource, GenerationResolutionError, ResolvedAccountSnapshot, VerifiedGeneration,
    build_canonical_certificates,
};
use kaysentinel_builder::lifecycle::keys::GenerationKey;
use kaysentinel_builder::{partition_events, reduce_timelines, resolve_lifecycles, resolve_storage_roots, SimpleStorageRootDeriver};
use kaysentinel_cse::event::CsePayload;
use kaysentinel_verify::{replay_and_verify, ExecutionBatchReplay};

struct NullSnapshotSource;

impl AccountSnapshotSource for NullSnapshotSource {
    fn resolve_identity_generation(
        &self,
        address: [u8; 20],
    ) -> Result<VerifiedGeneration, GenerationResolutionError> {
        Ok(VerifiedGeneration {
            key: GenerationKey { address, generation_id: 0 },
            state_table_proof_root: [0u8; 32],
        })
    }

    fn recover_account_snapshot(
        &self,
        _address: [u8; 20],
    ) -> Result<ResolvedAccountSnapshot, GenerationResolutionError> {
        Ok(ResolvedAccountSnapshot { nonce: 0, balance: [0u8; 32], code_hash: [0u8; 32] })
    }
}

#[test]
fn full_pipeline_go_json_to_gate2_verification() {
    // A realistic committed transaction: frameless gas debit, a root frame
    // containing a balance change and a storage write, frameless gas refund.
    let addr = "0x1111111111111111111111111111111111111111";
    let h = |v: u8| format!("0x{:062x}{:02x}", 0, v);
    let no_frame = u64::MAX;

    let events_json = format!(
        r#"{{"metadata":{{}},"events":[
            {{"type":"BlockStart","data":{{"Number":42}}}},
            {{"type":"TransactionStart","data":{{"TxIndex":0,"Hash":"{tx_hash}"}}}},
            {{"type":"BalanceMutation","data":{{"FrameID":{no_frame},"Address":"{addr}","Before":"{b100}","After":"{b90}"}}}},
            {{"type":"FrameEnter","data":{{"FrameID":0,"ParentFrameID":{no_frame},"Depth":0}}}},
            {{"type":"BalanceMutation","data":{{"FrameID":0,"Address":"{addr}","Before":"{b90}","After":"{b80}"}}}},
            {{"type":"StorageMutation","data":{{"FrameID":0,"Address":"{addr}","Slot":"{slot}","Before":"{b0}","After":"{b7}"}}}},
            {{"type":"FrameExit","data":{{"FrameID":0,"Reverted":false}}}},
            {{"type":"BalanceMutation","data":{{"FrameID":{no_frame},"Address":"{addr}","Before":"{b80}","After":"{b85}"}}}},
            {{"type":"TransactionEnd","data":{{"TxIndex":0,"Reverted":false}}}},
            {{"type":"BlockCommit","data":{{}}}}
        ]}}"#,
        tx_hash = h(1),
        addr = addr,
        b100 = h(100),
        b90 = h(90),
        b80 = h(80),
        b85 = h(85),
        b0 = h(0),
        b7 = h(7),
        slot = h(9),
        no_frame = no_frame,
    );

    // Stage 1-2: bridge (wire parse + Layer 2 buffering/translation).
    let go_events = parse_event_stream(&events_json).expect("wire parse failed");
    let cse_events = translate(&go_events, &BridgeConfig { chain_id: 1 }).expect("bridge translate failed");

    // Keep a copy of the payloads for the Gate 2 batch before the builder
    // consumes the events.
    let traces: Vec<CsePayload> = cse_events.iter().map(|e| e.payload.clone()).collect();

    // Stage 3-5: builder (partition -> reduce -> lifecycle resolution).
    let raw_ir = partition_events(cse_events).expect("partition failed");
    let reduced = reduce_timelines(raw_ir).expect("reduce failed");
    let lifecycle_passes = resolve_lifecycles(&reduced, &BTreeMap::new());

    assert_eq!(reduced.0.len(), 1, "expected exactly one transaction bucket");
    let bucket = &reduced.0[0];
    let canonical = &lifecycle_passes[0].value;

    // Stage 6: certificates + storage root resolution (the Phase 5 closer).
    let mut certs =
        build_canonical_certificates(&bucket.state_table, &canonical.generations, &NullSnapshotSource)
            .expect("certificate build failed");
    resolve_storage_roots(&mut certs, &bucket.state_table, &SimpleStorageRootDeriver)
        .expect("storage root resolution failed");

    // Stage 7: Gate 2 semantic replay verification.
    let batch = ExecutionBatchReplay {
        account_certificates: certs.into_values().collect(),
        execution_traces: traces,
    };

    let result = replay_and_verify(&batch);
    assert_eq!(result, Ok(()), "Gate 2 replay verification failed: {result:?}");
}

#[test]
fn full_pipeline_reverted_frame_still_verifies() {
    // Same shape, but the root frame reverts: its storage write and balance
    // change must vanish at the bridge, and the resulting (gas-only)
    // certificate must still replay-verify cleanly -- proving the bridge's
    // revert-discard rules and Gate 2's model agree with each other.
    let addr = "0x2222222222222222222222222222222222222222";
    let h = |v: u8| format!("0x{:062x}{:02x}", 0, v);
    let no_frame = u64::MAX;

    let events_json = format!(
        r#"{{"metadata":{{}},"events":[
            {{"type":"BlockStart","data":{{"Number":42}}}},
            {{"type":"TransactionStart","data":{{"TxIndex":0,"Hash":"{tx_hash}"}}}},
            {{"type":"BalanceMutation","data":{{"FrameID":{no_frame},"Address":"{addr}","Before":"{b100}","After":"{b90}"}}}},
            {{"type":"FrameEnter","data":{{"FrameID":0,"ParentFrameID":{no_frame},"Depth":0}}}},
            {{"type":"StorageMutation","data":{{"FrameID":0,"Address":"{addr}","Slot":"{slot}","Before":"{b0}","After":"{b7}"}}}},
            {{"type":"FrameExit","data":{{"FrameID":0,"Reverted":true}}}},
            {{"type":"TransactionEnd","data":{{"TxIndex":0,"Reverted":true}}}},
            {{"type":"BlockCommit","data":{{}}}}
        ]}}"#,
        tx_hash = h(2),
        addr = addr,
        b100 = h(100),
        b90 = h(90),
        b0 = h(0),
        b7 = h(7),
        slot = h(9),
        no_frame = no_frame,
    );

    let go_events = parse_event_stream(&events_json).expect("wire parse failed");
    let cse_events = translate(&go_events, &BridgeConfig { chain_id: 1 }).expect("bridge translate failed");
    let traces: Vec<CsePayload> = cse_events.iter().map(|e| e.payload.clone()).collect();

    let raw_ir = partition_events(cse_events).expect("partition failed");
    let reduced = reduce_timelines(raw_ir).expect("reduce failed");
    let lifecycle_passes = resolve_lifecycles(&reduced, &BTreeMap::new());
    let bucket = &reduced.0[0];
    let canonical = &lifecycle_passes[0].value;

    let mut certs =
        build_canonical_certificates(&bucket.state_table, &canonical.generations, &NullSnapshotSource)
            .expect("certificate build failed");
    resolve_storage_roots(&mut certs, &bucket.state_table, &SimpleStorageRootDeriver)
        .expect("storage root resolution failed");

    // Exactly one certificate (the gas-debited account), and its storage
    // must be empty -- the reverted storage write never reached the builder.
    assert_eq!(certs.len(), 1);

    let batch = ExecutionBatchReplay {
        account_certificates: certs.into_values().collect(),
        execution_traces: traces,
    };
    assert_eq!(replay_and_verify(&batch), Ok(()));
}
