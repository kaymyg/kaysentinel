//! Layer 2 of the bridge architecture (docs/emes/004-bridge-buffering-spec.md):
//! translates an already Gate-1-validated Go `emes.Event` stream into a
//! `Vec<CanonicalSemanticEvent>` ready for `kaysentinel_builder::partition_events`.
//!
//! This module assumes its input already passed Gate 1 (`validation/gate1.go`)
//! -- it does defensive structural checks, but is not a re-implementation of
//! that validator. Layers are meant to stay decoupled per the design doc.

use kaysentinel_cse::context::{ExecutionContext, NormativeContext, ProvisionalContext, TraceContext};
use kaysentinel_cse::event::{CanonicalSemanticEvent, CsePayload, CseVersion};
use kaysentinel_cse::payloads::{BalanceChanged, CodeUpdated, ContractCreated, ContractDestroyed, NonceUpdated, StorageSlotUpdated};
// Note: CsePayload::AccessListTouched and CsePayload::GasRefundChanged are
// never constructed anywhere in this module -- the Go tracer has no source
// event for either (see docs/emes/004-bridge-buffering-spec.md's taxonomy
// table), and both are already confirmed dead-ends in runtime/builder even
// when present.

use crate::wire::{GoEvent, NO_FRAME};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    /// A `FrameExit` was seen with no matching open frame, or `TransactionEnd`
    /// was seen with frames still open. Gate 1 should already have rejected
    /// this upstream -- this is a defensive backstop, not primary validation.
    UnbalancedFrames { at: usize },
    TransactionStartWhileOpen { at: usize },
    EventOutsideTransaction { at: usize },
    /// A Go `FrameID`/`ParentFrameID` didn't fit in the `u32` that
    /// `TraceContext` uses on the Rust side. Not expected in practice (see
    /// module docs), but checked explicitly rather than silently truncated.
    FrameIdOverflow { value: u64 },
}

/// Data the Go event stream doesn't carry anywhere and must be supplied
/// externally. `chain_id` has a source (`emes.FixtureMetadata.ChainID`, once
/// per fixture) but isn't wired into `wire::parse_event_stream` (which only
/// reads the `events` array) -- callers read it from the same JSON document
/// themselves and pass it here.
///
/// `block_hash` has **no source anywhere in the current Go event model** --
/// `emes.BlockStartEvent` carries only `Number`/`Timestamp`, and
/// `FixtureMetadata` has no block-hash field at all. Rather than fabricate a
/// value, every emitted event's `ProvisionalContext.block_hash` is `[0u8; 32]`
/// -- a known, documented gap, same pattern as `StorageRootState::AwaitingDerivation`
/// elsewhere in this repo, not a silently-wrong placeholder.
#[derive(Debug, Clone, Copy)]
pub struct BridgeConfig {
    pub chain_id: u64,
}

struct FrameBuffer {
    frame_id: u64,
    depth: u64,
    events: Vec<CanonicalSemanticEvent>,
}

/// Translates a Gate-1-validated Go event stream into CSE events, applying
/// the Layer 2 buffering rules:
///   - frame-scoped mutations are buffered per open frame; a committed child
///     frame's buffer merges into its parent's; a reverted frame's buffer
///     (at any depth, including root) is discarded outright.
///   - frameless mutations (`FrameID == NO_FRAME`) are forwarded immediately,
///     unconditionally -- per the verified frameless taxonomy
///     (docs/emes/004-bridge-buffering-spec.md §3), these always survive
///     regardless of the root frame's outcome.
///   - `BeginTransaction`/`EndTransaction` boundary markers are always
///     forwarded for every transaction, committed or not -- `runtime/builder`
///     needs well-formed (possibly empty) transaction buckets either way.
///
/// `NormativeContext.sequence_number` is assigned fresh, densely, in final
/// output order -- not copied from Go's raw `Sequence` field, since dropped
/// events (frame markers, block markers, reverted mutations) would otherwise
/// leave gaps. Gaps are permitted by `TraceProvenance`'s own invariant
/// (uniqueness + strict ordering, not contiguity), but a dense assignment is
/// simpler to reason about and costs nothing.
pub fn translate(events: &[GoEvent], config: &BridgeConfig) -> Result<Vec<CanonicalSemanticEvent>, BridgeError> {
    let mut final_output: Vec<CanonicalSemanticEvent> = Vec::new();

    let mut tx_open = false;
    let mut tx_output: Vec<CanonicalSemanticEvent> = Vec::new();
    let mut frame_stack: Vec<FrameBuffer> = Vec::new();

    let mut block_number: u64 = 0;
    let mut tx_index: u64 = 0;
    let mut tx_hash: [u8; 32] = [0u8; 32];

    for (i, event) in events.iter().enumerate() {
        match event {
            GoEvent::BlockStart(data) => {
                block_number = data.number;
            }
            GoEvent::BlockCommit => {
                // Dropped at the bridge boundary -- no Rust type receives
                // block-level data (see module docs and docs/emes/004).
            }

            GoEvent::TransactionStart(data) => {
                if tx_open {
                    return Err(BridgeError::TransactionStartWhileOpen { at: i });
                }
                tx_open = true;
                tx_index = data.tx_index;
                tx_hash = data.hash.0;
                tx_output.clear();
                frame_stack.clear();
                tx_output.push(boundary_event(CsePayload::BeginTransaction, config, block_number, tx_hash, tx_index));
            }

            GoEvent::TransactionEnd(data) => {
                if !tx_open {
                    return Err(BridgeError::EventOutsideTransaction { at: i });
                }
                if !frame_stack.is_empty() {
                    return Err(BridgeError::UnbalancedFrames { at: i });
                }
                let _ = data.reverted; // T1 consistency already checked by Gate 1; not re-derived here.
                tx_output.push(boundary_event(CsePayload::EndTransaction, config, block_number, tx_hash, tx_index));
                final_output.append(&mut tx_output);
                tx_open = false;
            }

            GoEvent::FrameEnter(data) => {
                if !tx_open {
                    return Err(BridgeError::EventOutsideTransaction { at: i });
                }
                frame_stack.push(FrameBuffer { frame_id: data.frame_id, depth: data.depth, events: Vec::new() });
            }

            GoEvent::FrameExit(data) => {
                if !tx_open {
                    return Err(BridgeError::EventOutsideTransaction { at: i });
                }
                let popped = frame_stack
                    .pop()
                    .ok_or(BridgeError::UnbalancedFrames { at: i })?;
                if popped.frame_id != data.frame_id {
                    return Err(BridgeError::UnbalancedFrames { at: i });
                }
                if data.reverted {
                    // Discarded outright, at any nesting depth -- child or root.
                } else if let Some(parent) = frame_stack.last_mut() {
                    parent.events.extend(popped.events);
                } else {
                    // This was the root frame's own exit: flush straight into
                    // tx_output, in place, preserving chronological order
                    // relative to any frameless events before/after it.
                    tx_output.extend(popped.events);
                }
            }

            // --- Mutation events -----------------------------------------
            GoEvent::BalanceMutation(data) => {
                let payload = CsePayload::BalanceChanged(BalanceChanged {
                    address: data.address.0,
                    previous_balance: data.before.0,
                    current_balance: data.after.0,
                });
                append_mutation(&mut tx_output, &mut frame_stack, data.frame_id, config, block_number, tx_hash, tx_index, payload)?;
            }
            GoEvent::NonceMutation(data) => {
                let payload = CsePayload::NonceUpdated(NonceUpdated {
                    address: data.address.0,
                    previous_nonce: data.before,
                    current_nonce: data.after,
                });
                append_mutation(&mut tx_output, &mut frame_stack, data.frame_id, config, block_number, tx_hash, tx_index, payload)?;
            }
            GoEvent::CodeMutation(data) => {
                let payload = CsePayload::CodeUpdated(CodeUpdated {
                    address: data.address.0,
                    previous_code_hash: data.before.0,
                    current_code_hash: data.after.0,
                });
                append_mutation(&mut tx_output, &mut frame_stack, data.frame_id, config, block_number, tx_hash, tx_index, payload)?;
            }
            GoEvent::StorageMutation(data) => {
                let payload = CsePayload::StorageSlotUpdated(StorageSlotUpdated {
                    address: data.address.0,
                    slot: data.slot.0,
                    previous_value: data.before.0,
                    current_value: data.after.0,
                });
                append_mutation(&mut tx_output, &mut frame_stack, data.frame_id, config, block_number, tx_hash, tx_index, payload)?;
            }
            GoEvent::AccountCreated(data) => {
                let payload = CsePayload::ContractCreated(ContractCreated {
                    address: data.address.0,
                    creator: data.creator.0,
                });
                append_mutation(&mut tx_output, &mut frame_stack, data.frame_id, config, block_number, tx_hash, tx_index, payload)?;
            }
            GoEvent::SelfDestruct(data) => {
                let payload = CsePayload::ContractDestroyed(ContractDestroyed {
                    address: data.address.0,
                    refund_target: data.beneficiary.0,
                });
                append_mutation(&mut tx_output, &mut frame_stack, data.frame_id, config, block_number, tx_hash, tx_index, payload)?;
            }
        }
    }

    if tx_open {
        return Err(BridgeError::EventOutsideTransaction { at: events.len() });
    }

    // Dense, monotonic sequence assignment over the final, already-filtered
    // output -- see the "why" in this function's doc comment above.
    for (seq, event) in final_output.iter_mut().enumerate() {
        event.context.normative.sequence_number = seq as u64;
    }

    Ok(final_output)
}

#[allow(clippy::too_many_arguments)]
fn append_mutation(
    tx_output: &mut Vec<CanonicalSemanticEvent>,
    frame_stack: &mut [FrameBuffer],
    frame_id: u64,
    config: &BridgeConfig,
    block_number: u64,
    tx_hash: [u8; 32],
    tx_index: u64,
    payload: CsePayload,
) -> Result<(), BridgeError> {
    let (call_frame_id, call_depth) = if frame_id == NO_FRAME {
        (u32::MAX, 0)
    } else if let Some(top) = frame_stack.last() {
        (
            u32::try_from(top.frame_id).map_err(|_| BridgeError::FrameIdOverflow { value: top.frame_id })?,
            u32::try_from(top.depth).map_err(|_| BridgeError::FrameIdOverflow { value: top.depth })?,
        )
    } else {
        (u32::MAX, 0)
    };

    let event = CanonicalSemanticEvent {
        version: CseVersion::V1,
        context: ExecutionContext {
            normative: NormativeContext { sequence_number: 0 }, // assigned densely at the end
            provisional: ProvisionalContext {
                chain_id: config.chain_id,
                block_hash: [0u8; 32], // documented gap -- see BridgeConfig doc comment
                block_number,
                transaction_hash: tx_hash,
                transaction_index: tx_index as u32,
            },
            trace: TraceContext { call_frame_id, call_depth },
        },
        payload,
    };

    if frame_id == NO_FRAME {
        tx_output.push(event);
    } else if let Some(top) = frame_stack.last_mut() {
        top.events.push(event);
    } else {
        // A non-sentinel FrameID with an empty frame stack means the input
        // didn't actually pass Gate 1 (frame-balance would have caught this).
        return Err(BridgeError::UnbalancedFrames { at: usize::MAX });
    }
    Ok(())
}

fn boundary_event(
    payload: CsePayload,
    config: &BridgeConfig,
    block_number: u64,
    tx_hash: [u8; 32],
    tx_index: u64,
) -> CanonicalSemanticEvent {
    CanonicalSemanticEvent {
        version: CseVersion::V1,
        context: ExecutionContext {
            normative: NormativeContext { sequence_number: 0 },
            provisional: ProvisionalContext {
                chain_id: config.chain_id,
                block_hash: [0u8; 32],
                block_number,
                transaction_hash: tx_hash,
                transaction_index: tx_index as u32,
            },
            trace: TraceContext { call_frame_id: u32::MAX, call_depth: 0 },
        },
        payload,
    }
}
