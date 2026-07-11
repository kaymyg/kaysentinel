use std::collections::BTreeMap;
use kaysentinel_cse::event::{CanonicalSemanticEvent as CseEvent, CsePayload};
use crate::errors::BuilderError;
use crate::ir::timeline::*;

pub fn process(events: Vec<CseEvent>) -> Result<TimelineIr, BuilderError> {
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
                    account_nodes: BTreeMap::new(),
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
                        let node = get_or_create_node(&mut bucket.account_nodes, p.address);
                        node.balance_timeline
                            .get_or_insert_with(Timeline::new)
                            .push(p.previous_balance, p.current_balance, event_id);
                    }
                    CsePayload::NonceUpdated(p) => {
                        let node = get_or_create_node(&mut bucket.account_nodes, p.address);
                        node.nonce_timeline
                            .get_or_insert_with(Timeline::new)
                            .push(p.previous_nonce, p.current_nonce, event_id);
                    }
                    CsePayload::StorageSlotUpdated(p) => {
                        let node = get_or_create_node(&mut bucket.account_nodes, p.address);
                        let tl = node.storage_timelines.entry(p.slot).or_insert_with(Timeline::new);
                        tl.push(p.previous_value, p.current_value, event_id);
                    }
                    CsePayload::TransientStorageUpdated(p) => {
                        let node = get_or_create_node(&mut bucket.account_nodes, p.address);
                        let tl = node.transient_timelines.entry(p.slot).or_insert_with(Timeline::new);
                        tl.push(p.previous_value, p.current_value, event_id);
                    }
                    CsePayload::CodeUpdated(p) => {
                        let node = get_or_create_node(&mut bucket.account_nodes, p.address);
                        node.code_timeline
                            .get_or_insert_with(Timeline::new)
                            .push(p.previous_code_hash, p.current_code_hash, event_id);
                    }
                    CsePayload::ContractCreated(p) => {
                        let node = get_or_create_node(&mut bucket.account_nodes, p.address);
                        node.lifecycle_history.push(ObservedLifecycle {
                            payload: CseLifecyclePayload::Created(p),
                            event_id,
                        });
                    }
                    CsePayload::ContractDestroyed(p) => {
                        let node = get_or_create_node(&mut bucket.account_nodes, p.address);
                        node.lifecycle_history.push(ObservedLifecycle {
                            payload: CseLifecyclePayload::Destroyed(p),
                            event_id,
                        });
                    }
                    CsePayload::LogEmitted(p) => {
                        let node = get_or_create_node(&mut bucket.account_nodes, p.address);
                        node.logs.push(ObservedLog { payload: p, event_id });
                    }
                    CsePayload::GasRefundChanged(p) => {
                        bucket.gas_refund_timeline
                            .get_or_insert_with(Timeline::new)
                            .push(p.previous_refund, p.current_refund, event_id);
                    }
                    // NOTE: AccessListTouched is intentionally not recorded here — the refactored
                    // AccountNode (ir/timeline.rs) dropped the access_list_timeline field, and
                    // this phase's doc didn't specify a replacement home for it. Flagging this
                    // as an open gap rather than inventing a place to put it.
                    CsePayload::AccessListTouched(_) => {}
                    CsePayload::BeginTransaction | CsePayload::EndTransaction => unreachable!(),
                }
            }
        }
    }

    if current_bucket.is_some() {
        return Err(BuilderError::UnexpectedTransactionBoundary);
    }

    Ok(TimelineIr(buckets))
}

fn get_or_create_node(nodes: &mut BTreeMap<[u8; 20], AccountNode>, address: [u8; 20]) -> &mut AccountNode {
    nodes.entry(address).or_insert_with(|| AccountNode {
        address,
        lifecycle_history: Vec::new(),
        balance_timeline: None,
        nonce_timeline: None,
        code_timeline: None,
        storage_timelines: BTreeMap::new(),
        transient_timelines: BTreeMap::new(),
        logs: Vec::new(),
    })
}
