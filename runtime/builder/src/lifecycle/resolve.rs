use std::collections::BTreeMap;
use crate::ir::timeline::{CseLifecyclePayload, EventId, ObservedLifecycle};
use crate::lifecycle::diagnostics::{Diagnostic, InvariantReport, LifecycleViolation, PassOutput, Severity};
use crate::lifecycle::semantic::{AccountPreStateRelation, GenerationRelation, SemanticIr};

/// Resolves a transaction bucket's `lifecycle_table` (already partitioned and keyed by
/// address upstream in `partition.rs` / `ir/timeline.rs`) into identity "generations":
/// the span an address holds a single deployed-contract identity between one creation
/// and the next destruction (or the end of execution, if never destroyed).
///
/// `pre_states` lets a caller supply externally-known "did this address already have
/// code before this transaction ran" facts (e.g. from a prior state snapshot). Nothing
/// in the current CSE ABI/pipeline carries this information yet, so addresses absent
/// from the map are conservatively treated as not having existed beforehand.
pub fn resolve(
    lifecycle_table: &BTreeMap<[u8; 20], Vec<ObservedLifecycle>>,
    pre_states: &BTreeMap<[u8; 20], bool>,
) -> PassOutput<SemanticIr> {
    let mut semantic_ir = SemanticIr::default();
    let mut diagnostics = Vec::new();

    for (address, observations) in lifecycle_table {
        let existed_before_tx = pre_states.get(address).copied().unwrap_or(false);
        semantic_ir.pre_states.push(AccountPreStateRelation {
            address: *address,
            existed_before_tx,
        });

        let mut current_gen_id: u32 = 0;
        let mut begin = observations
            .first()
            .map(|o| o.event_id)
            .unwrap_or(EventId(0));
        let mut gen_open = true;

        for obs in observations {
            match &obs.payload {
                CseLifecyclePayload::Created(_) => {
                    let existed_before = if current_gen_id == 0 {
                        existed_before_tx
                    } else {
                        false
                    };
                    if existed_before {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            location: obs.event_id,
                            violation: LifecycleViolation::Collision {
                                address: *address,
                                generation_id: current_gen_id,
                            },
                            explanation: format!(
                                "Contract creation observed on account {:?} while generation {} was already active",
                                address, current_gen_id
                            ),
                        });
                    }
                    begin = obs.event_id;
                    gen_open = true;
                }
                CseLifecyclePayload::Destroyed(_) => {
                    semantic_ir.generations.push(GenerationRelation {
                        address: *address,
                        generation_id: current_gen_id,
                        begin,
                        end: Some(obs.event_id),
                    });
                    current_gen_id += 1;
                    begin = obs.event_id;
                    gen_open = false;
                }
            }
        }

        if gen_open {
            semantic_ir.generations.push(GenerationRelation {
                address: *address,
                generation_id: current_gen_id,
                begin,
                end: None,
            });
        }
    }

    PassOutput {
        value: semantic_ir,
        diagnostics,
        invariant_report: InvariantReport {
            keys_unique: true,
            referential_integrity: true,
            deterministic_order: false, // not guaranteed until canonicalize
            intervals_valid: true,
        },
    }
}
