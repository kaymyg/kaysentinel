use std::collections::HashSet;

use crate::lifecycle::canonical::{CanonicalBalance, CanonicalGeneration, CanonicalPreState, CanonicalSemanticIr};
use crate::lifecycle::diagnostics::{Diagnostic, InvariantReport, LifecycleViolation, PassOutput, Severity};
use crate::lifecycle::keys::GenerationKey;

pub struct CanonicalizationPipeline;

impl CanonicalizationPipeline {
    /// Pure functional normal-form construction: maps unrefined pre-state, generation,
    /// and balance facts into their unique canonical form (sorted, deduplicated, keyed),
    /// then runs the full schema verification pass over the result.
    ///
    /// `balances` is accepted so a caller can supply generation-scoped balance facts if
    /// they have them; nothing in the pipeline currently derives these automatically —
    /// the real `state_table` (ir/timeline.rs, reduce.rs) is still flat and address-keyed,
    /// not generation-scoped, so there's no existing source feeding this parameter yet.
    pub fn process(
        pre_states: &[CanonicalPreState],
        generations: &[CanonicalGeneration],
        balances: &[CanonicalBalance],
    ) -> PassOutput<CanonicalSemanticIr> {
        let mut canonical_pre_states = pre_states.to_vec();
        canonical_pre_states.sort_by_key(|p| p.key);
        canonical_pre_states.dedup_by(|a, b| a.key == b.key);

        let mut canonical_generations = generations.to_vec();
        // Enforce Theorem 5: order strictly by address space, then chronologically.
        canonical_generations.sort_by(|a, b| {
            a.key
                .address
                .cmp(&b.key.address)
                .then(a.begin.trace_ordinal.cmp(&b.begin.trace_ordinal))
                .then(a.key.generation_id.cmp(&b.key.generation_id))
        });
        canonical_generations.dedup_by(|a, b| a.key == b.key);

        let mut canonical_balances = balances.to_vec();
        canonical_balances.sort_by_key(|b| b.key);
        canonical_balances.dedup_by(|a, b| a.key == b.key);

        let canonical_ir = CanonicalSemanticIr {
            pre_states: canonical_pre_states,
            generations: canonical_generations,
            balances: canonical_balances,
        };

        let (diagnostics, report) = Self::verify_global_schema(&canonical_ir);

        PassOutput {
            value: canonical_ir,
            diagnostics,
            invariant_report: report,
        }
    }

    /// Pure verification: asserts an already-canonical-shaped schema still satisfies
    /// every required invariant, without re-sorting it.
    pub fn verify_already_canonical(ir: &CanonicalSemanticIr) -> (Vec<Diagnostic>, InvariantReport) {
        Self::verify_global_schema(ir)
    }

    fn verify_global_schema(ir: &CanonicalSemanticIr) -> (Vec<Diagnostic>, InvariantReport) {
        let mut diagnostics = Vec::new();
        let mut report = InvariantReport {
            keys_unique: true,
            referential_integrity: true,
            deterministic_order: true,
            intervals_valid: true,
        };

        let mut seen_generation_keys: HashSet<GenerationKey> = HashSet::new();
        let mut active_generation_set: HashSet<GenerationKey> = HashSet::new();

        for gen in &ir.generations {
            if !seen_generation_keys.insert(gen.key) {
                report.keys_unique = false;
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    location: gen.begin,
                    violation: LifecycleViolation::DuplicateKey(gen.key),
                    explanation: format!("Key uniqueness violation: {:?}", gen.key),
                });
            }
            active_generation_set.insert(gen.key);

            // Theorem 4 (Temporal Well-Formedness)
            if let Some(end_coord) = gen.end {
                if gen.begin.trace_ordinal >= end_coord.trace_ordinal {
                    report.intervals_valid = false;
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        location: gen.begin,
                        violation: LifecycleViolation::IntervalInvalid(gen.key),
                        explanation: format!("Temporal well-formedness breach for {:?}", gen.key),
                    });
                }
            }
        }

        // Referential integrity: every balance fact must reference a generation that exists.
        for bal in &ir.balances {
            if !active_generation_set.contains(&bal.key) {
                report.referential_integrity = false;
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    location: bal.location,
                    violation: LifecycleViolation::IntegrityBreach(bal.key),
                    explanation: format!("Referential integrity breach for {:?}", bal.key),
                });
            }
        }

        // Physical arrangement + Theorem 5 (Chronological Adjacency) window checks.
        for window in ir.generations.windows(2) {
            let g1 = &window[0];
            let g2 = &window[1];

            let current_order = g1
                .key
                .address
                .cmp(&g2.key.address)
                .then(g1.begin.trace_ordinal.cmp(&g2.begin.trace_ordinal))
                .then(g1.key.generation_id.cmp(&g2.key.generation_id));

            if current_order == std::cmp::Ordering::Greater {
                report.deterministic_order = false;
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    location: g2.begin,
                    violation: LifecycleViolation::OrderingBreach,
                    explanation: "Deterministic order violation detected within generations".to_string(),
                });
            }

            if g1.key.address == g2.key.address {
                if let Some(end_coord) = g1.end {
                    if end_coord.trace_ordinal >= g2.begin.trace_ordinal {
                        report.intervals_valid = false;
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            location: g2.begin,
                            violation: LifecycleViolation::OverlapDetected(g2.key.address),
                            explanation: format!(
                                "Temporal overlap breach at trace ordinal {}",
                                g2.begin.trace_ordinal
                            ),
                        });
                    }
                }
            }
        }

        (diagnostics, report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::timeline::TraceProvenance;

    fn make_prov(ord: u64) -> TraceProvenance {
        TraceProvenance { trace_ordinal: ord, tx_index: 0, frame_id: 1, call_depth: 1, frame_ordinal: 0 }
    }

    #[test]
    fn theorem_1_sorting_normalization() {
        let addr = [2u8; 20];
        let g1 = CanonicalGeneration {
            key: GenerationKey { address: addr, generation_id: 1 },
            begin: make_prov(40),
            end: Some(make_prov(50)),
        };
        let g2 = CanonicalGeneration {
            key: GenerationKey { address: addr, generation_id: 0 },
            begin: make_prov(10),
            end: Some(make_prov(20)),
        };

        let out = CanonicalizationPipeline::process(&[], &[g1.clone(), g2.clone()], &[]);

        assert_eq!(out.value.generations[0], g2);
        assert_eq!(out.value.generations[1], g1);
        assert!(out.invariant_report.deterministic_order);
        assert!(out.invariant_report.intervals_valid);
    }

    #[test]
    fn theorem_2_projection_stability() {
        let addr = [3u8; 20];
        let gens = vec![CanonicalGeneration {
            key: GenerationKey { address: addr, generation_id: 0 },
            begin: make_prov(10),
            end: Some(make_prov(20)),
        }];
        let bals = vec![CanonicalBalance {
            key: GenerationKey { address: addr, generation_id: 0 },
            value: [7, 0, 0, 0],
            location: make_prov(10),
        }];

        let baseline_pass = CanonicalizationPipeline::process(&[], &gens, &bals);
        let extended_pass = CanonicalizationPipeline::process(&[], &gens, &bals);

        assert_eq!(baseline_pass.value.balances, extended_pass.value.balances);
        assert_eq!(baseline_pass.value.generations, extended_pass.value.generations);
        assert!(baseline_pass.invariant_report.referential_integrity);
    }

    #[test]
    fn canonicalization_is_idempotent() {
        let pass_1 = CanonicalizationPipeline::process(&[], &[], &[]);
        let pass_2 = CanonicalizationPipeline::process(
            &pass_1.value.pre_states,
            &pass_1.value.generations,
            &pass_1.value.balances,
        );
        assert_eq!(pass_1.value, pass_2.value);
    }
}
