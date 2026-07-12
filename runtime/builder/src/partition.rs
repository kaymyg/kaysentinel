use std::collections::BTreeMap;
use kaysentinel_cse::event::{CanonicalSemanticEvent as CseEvent, CsePayload};
use crate::errors::BuilderError;
use crate::ir::timeline::*;

pub fn process(events: Vec<CseEvent>) -> Result<RawIr, BuilderError> {
    let mut buckets = Vec::new();
    let mut current_bucket: Option<TransactionBucket> = None;

    for event in events {
        let event_id = EventId(event.context.sequence_number);

        match event.payload {
            CsePayload::BeginTransaction => {
                if current_bucket.is_some() {
                    return Err(BuilderError::DuplicateTransactionBoundary);
                }
                current_bucket = Some(TransactionBucket {
                    metadata: TransactionMetadata {
                        chain_id: event.context.chain_id,
                        block_number: event.context.block_number,
                        block_hash: event.context.block_hash,
                        transaction_hash: event.context.transaction_hash,
                        transaction_index: event.context.transaction_index,
                    },
                    state_tables: BTreeMap::new(),
                    lifecycle_table: BTreeMap::new(),
                    log_table: BTreeMap::new(),
                    gas_refund_timeline: None,
                });
            }

            CsePayload::EndTransaction => {
                let bucket = current_bucket.take().ok_or(BuilderError::UnexpectedTransactionBoundary)?;
                buckets.push(bucket);
            }

            other_payload => {
                let bucket = current_bucket.as_mut().ok_or(BuilderError::EventOutsideTransaction)?;

                match other_payload {
                    CsePayload::BalanceChanged(p) => {
                        let key = CanonicalKey::Balance { address: p.address };
                        let entry = bucket.state_tables.entry(key)
                            .or_insert_with(|| TimelineVariant::Balance(Timeline::new()));
                        if let TimelineVariant::Balance(tl) = entry {
                            tl.push(p.previous_balance, p.current_balance, event_id);
                        }
                    }
                    CsePayload::NonceUpdated(p) => {
                        let key = CanonicalKey::Nonce { address: p.address };
                        let entry = bucket.state_tables.entry(key)
                            .or_insert_with(|| TimelineVariant::Nonce(Timeline::new()));
                        if let TimelineVariant::Nonce(tl) = entry {
                            tl.push(p.previous_nonce, p.current_nonce, event_id);
                        }
                    }
                    CsePayload::CodeUpdated(p) => {
                        let key = CanonicalKey::Code { address: p.address };
                        let entry = bucket.state_tables.entry(key)
                            .or_insert_with(|| TimelineVariant::Code(Timeline::new()));
                        if let TimelineVariant::Code(tl) = entry {
                            tl.push(p.previous_code_hash, p.current_code_hash, event_id);
                        }
                    }
                    CsePayload::StorageSlotUpdated(p) => {
                        let key = CanonicalKey::Storage { address: p.address, slot: p.slot };
                        let entry = bucket.state_tables.entry(key)
                            .or_insert_with(|| TimelineVariant::Storage(Timeline::new()));
                        if let TimelineVariant::Storage(tl) = entry {
                            tl.push(p.previous_value, p.current_value, event_id);
                        }
                    }
                    CsePayload::TransientStorageUpdated(p) => {
                        let key = CanonicalKey::Transient { address: p.address, slot: p.slot };
                        let entry = bucket.state_tables.entry(key)
                            .or_insert_with(|| TimelineVariant::Transient(Timeline::new()));
                        if let TimelineVariant::Transient(tl) = entry {
                            tl.push(p.previous_value, p.current_value, event_id);
                        }
                    }
                    CsePayload::ContractCreated(p) => {
                        bucket.lifecycle_table.entry(p.address).or_insert_with(Vec::new)
                            .push(ObservedLifecycle {
                                payload: CseLifecyclePayload::Created(p),
                                event_id,
                                trace_provenance: trace_provenance_from(&event.context),
                            });
                    }
                    CsePayload::ContractDestroyed(p) => {
                        bucket.lifecycle_table.entry(p.address).or_insert_with(Vec::new)
                            .push(ObservedLifecycle {
                                payload: CseLifecyclePayload::Destroyed(p),
                                event_id,
                                trace_provenance: trace_provenance_from(&event.context),
                            });
                    }
                    CsePayload::LogEmitted(p) => {
                        bucket.log_table.entry(p.address).or_insert_with(Vec::new)
                            .push(ObservedLog { payload: p, event_id });
                    }
                    CsePayload::GasRefundChanged(p) => {
                        bucket.gas_refund_timeline
                            .get_or_insert_with(Timeline::new)
                            .push(p.previous_refund, p.current_refund, event_id);
                    }
                    // NOTE: AccessListTouched remains untracked, unchanged from the prior phase.
                    CsePayload::AccessListTouched(_) => {}
                    CsePayload::BeginTransaction | CsePayload::EndTransaction => unreachable!(),
                }
            }
        }
    }

    if current_bucket.is_some() {
        return Err(BuilderError::UnexpectedTransactionBoundary);
    }

    Ok(RawIr(buckets))
}

/// Maps a CSE `ExecutionContext` onto the richer lifecycle `TraceProvenance` coordinate.
///
/// NOTE: `frame_ordinal` (an ordinal position *within* a call frame) has no source in
/// the current CSE ABI's `ExecutionContext` — it's set to 0 as an honest placeholder,
/// not a real value, until the ABI is extended to carry one.
fn trace_provenance_from(ctx: &kaysentinel_cse::context::ExecutionContext) -> TraceProvenance {
    TraceProvenance {
        trace_ordinal: ctx.sequence_number,
        tx_index: ctx.transaction_index as u64,
        frame_id: ctx.call_frame_id as usize,
        call_depth: ctx.call_depth as usize,
        frame_ordinal: 0,
    }
}
