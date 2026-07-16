//! Conformance tests exercising the actual pipeline entry points end-to-end:
//! `partition::process` -> `reduce::process` -> `lifecycle::process` ->
//! `lifecycle::certificate::build_canonical_certificates`.
//!
//! Verifies two properties from the governance baseline:
//!   - Diagnostic Isolation: TraceContext (call_frame_id/call_depth) must not
//!     affect the resulting certificates.
//!   - Semantic Sensitivity: a change to a normative field (e.g. a balance) MUST
//!     change the resulting certificates.

use std::collections::BTreeMap;

use kaysentinel_builder::{
    build_canonical_certificates, partition_events, reduce_timelines, resolve_lifecycles,
};
use kaysentinel_builder::lifecycle::certificate::{
    AccountSnapshotSource, GenerationResolutionError, ResolvedAccountSnapshot, VerifiedGeneration,
};
use kaysentinel_builder::lifecycle::keys::GenerationKey;
use kaysentinel_cse::context::{ExecutionContext, NormativeContext, ProvisionalContext, TraceContext};
use kaysentinel_cse::event::{CanonicalSemanticEvent, CseVersion, CsePayload};
use kaysentinel_cse::payloads::BalanceChanged;

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

fn balance_event(
    seq: u64,
    address: [u8; 20],
    previous_balance: [u8; 32],
    current_balance: [u8; 32],
    trace: TraceContext,
) -> CanonicalSemanticEvent {
    CanonicalSemanticEvent {
        version: CseVersion::V1,
        context: ExecutionContext {
            normative: NormativeContext { sequence_number: seq },
            provisional: ProvisionalContext {
                chain_id: 1,
                block_hash: [0u8; 32],
                block_number: 100,
                transaction_hash: [0u8; 32],
                transaction_index: 0,
            },
            trace,
        },
        payload: CsePayload::BalanceChanged(BalanceChanged { address, previous_balance, current_balance }),
    }
}

fn boundary_event(seq: u64, payload: CsePayload, trace: TraceContext) -> CanonicalSemanticEvent {
    CanonicalSemanticEvent {
        version: CseVersion::V1,
        context: ExecutionContext {
            normative: NormativeContext { sequence_number: seq },
            provisional: ProvisionalContext {
                chain_id: 1,
                block_hash: [0u8; 32],
                block_number: 100,
                transaction_hash: [0u8; 32],
                transaction_index: 0,
            },
            trace,
        },
        payload,
    }
}

/// Runs the real pipeline entry points end-to-end and returns the resulting
/// certificate set (an owned, comparable value) for a given event stream.
fn run_pipeline(events: Vec<CanonicalSemanticEvent>) -> BTreeMap<[u8; 20], String> {
    let raw = partition_events(events).expect("partition failed");
    let reduced = reduce_timelines(raw).expect("reduce failed");

    let pre_states = BTreeMap::new();
    let lifecycle_pass = resolve_lifecycles(&reduced, &pre_states);

    // Single-transaction fixtures only, for this test.
    let bucket = &reduced.0[0];
    let canonical = &lifecycle_pass[0].value;

    let certs = build_canonical_certificates(&bucket.state_table, &canonical.generations, &NullSnapshotSource)
        .expect("certificate build failed");

    certs.into_iter().map(|(addr, cert)| (addr, format!("{:?}", cert))).collect()
}

#[test]
fn test_property_diagnostic_isolation() {
    let address = [0xABu8; 20];
    let mut balance_after = [0u8; 32];
    balance_after[31] = 200;

    let shallow_trace = TraceContext { call_frame_id: 1, call_depth: 1 };
    let deep_trace = TraceContext { call_frame_id: 999, call_depth: 12 };

    let stream_a = vec![
        boundary_event(0, CsePayload::BeginTransaction, shallow_trace),
        balance_event(1, address, [0u8; 32], balance_after, shallow_trace),
        boundary_event(2, CsePayload::EndTransaction, shallow_trace),
    ];

    let stream_b = vec![
        boundary_event(0, CsePayload::BeginTransaction, deep_trace),
        balance_event(1, address, [0u8; 32], balance_after, deep_trace),
        boundary_event(2, CsePayload::EndTransaction, deep_trace),
    ];

    let certs_a = run_pipeline(stream_a);
    let certs_b = run_pipeline(stream_b);

    assert_eq!(
        certs_a, certs_b,
        "Diagnostic Isolation failed: differing TraceContext altered the resulting certificates"
    );
}

#[test]
fn test_property_semantic_sensitivity() {
    let address = [0xCDu8; 20];
    let trace = TraceContext { call_frame_id: 1, call_depth: 1 };

    let mut balance_200 = [0u8; 32];
    balance_200[31] = 200;
    let mut balance_201 = [0u8; 32];
    balance_201[31] = 201;

    let stream_a = vec![
        boundary_event(0, CsePayload::BeginTransaction, trace),
        balance_event(1, address, [0u8; 32], balance_200, trace),
        boundary_event(2, CsePayload::EndTransaction, trace),
    ];

    let stream_b = vec![
        boundary_event(0, CsePayload::BeginTransaction, trace),
        balance_event(1, address, [0u8; 32], balance_201, trace),
        boundary_event(2, CsePayload::EndTransaction, trace),
    ];

    let certs_a = run_pipeline(stream_a);
    let certs_b = run_pipeline(stream_b);

    assert_ne!(
        certs_a, certs_b,
        "Semantic Sensitivity failed: a normative balance change did not alter the resulting certificates"
    );
}
