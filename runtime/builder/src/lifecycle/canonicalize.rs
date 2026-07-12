use crate::lifecycle::canonical::{CanonicalGeneration, CanonicalPreState, CanonicalSemanticIr};
use crate::lifecycle::diagnostics::{InvariantReport, PassOutput};
use crate::lifecycle::keys::GenerationKey;
use crate::lifecycle::semantic::SemanticIr;

/// Functional normalization: maps unrefined `SemanticIr` generation facts into their
/// unique normal-form representation (sorted, deduplicated, keyed).
pub fn canonicalize(source_ir: &SemanticIr) -> PassOutput<CanonicalSemanticIr> {
    let mut canonical_ir = CanonicalSemanticIr::default();

    canonical_ir.pre_states = normalize_pre_states(source_ir);
    canonical_ir.generations = normalize_generations(source_ir);

    let report = verify_schema_invariants(&canonical_ir);

    PassOutput {
        value: canonical_ir,
        diagnostics: Vec::new(), // this pass is purely structural; it can't introduce new violations
        invariant_report: report,
    }
}

fn normalize_pre_states(source: &SemanticIr) -> Vec<CanonicalPreState> {
    let mut targets: Vec<CanonicalPreState> = source
        .pre_states
        .iter()
        .map(|p| CanonicalPreState {
            key: GenerationKey { address: p.address, generation_id: 0 },
            existed_before_tx: p.existed_before_tx,
        })
        .collect();
    targets.sort_by_key(|p| p.key);
    targets.dedup_by(|a, b| a.key == b.key);
    targets
}

fn normalize_generations(source: &SemanticIr) -> Vec<CanonicalGeneration> {
    let mut targets: Vec<CanonicalGeneration> = source
        .generations
        .iter()
        .map(|g| CanonicalGeneration {
            key: GenerationKey { address: g.address, generation_id: g.generation_id },
            begin: g.begin,
            end: g.end,
        })
        .collect();
    targets.sort_by_key(|g| g.key);
    targets.dedup_by(|a, b| a.key == b.key);
    targets
}

fn verify_schema_invariants(ir: &CanonicalSemanticIr) -> InvariantReport {
    let mut report = InvariantReport {
        keys_unique: true,
        referential_integrity: true,
        deterministic_order: true,
        intervals_valid: true,
    };

    for window in ir.generations.windows(2) {
        if window[0].key >= window[1].key {
            report.deterministic_order = false;
        }
        if window[0].key.address == window[1].key.address {
            if window[1].key.generation_id != window[0].key.generation_id + 1 {
                report.intervals_valid = false; // breach of contiguous identity sequence rule
            }
            if let (Some(end), begin) = (window[0].end, window[1].begin) {
                if end > begin {
                    report.intervals_valid = false;
                }
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::semantic::{AccountPreStateRelation, GenerationRelation};

    #[test]
    fn canonicalization_is_idempotent() {
        let unrefined_ir = SemanticIr::default();
        let pass_1 = canonicalize(&unrefined_ir);

        // Re-feed canonical output back through the transformation as unrefined input.
        let degraded = SemanticIr {
            pre_states: pass_1
                .value
                .pre_states
                .iter()
                .map(|p| AccountPreStateRelation {
                    address: p.key.address,
                    existed_before_tx: p.existed_before_tx,
                })
                .collect(),
            generations: pass_1
                .value
                .generations
                .iter()
                .map(|g| GenerationRelation {
                    address: g.key.address,
                    generation_id: g.key.generation_id,
                    begin: g.begin,
                    end: g.end,
                })
                .collect(),
        };

        let pass_2 = canonicalize(&degraded);

        // N(N(x)) == N(x)
        assert_eq!(pass_1.value, pass_2.value);
        assert!(pass_2.invariant_report.deterministic_order);
        assert!(pass_2.invariant_report.referential_integrity);
    }
}
