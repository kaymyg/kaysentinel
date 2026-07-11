use std::collections::BTreeMap;
use crate::ir::timeline::{RawIr, TimelineVariant};
use crate::ir::reduced::{ReducedIr, ReducedTransactionBucket, ReducedVariant};
use crate::traits::ReducibleTimeline;
use crate::errors::BuilderError;

pub fn process(raw_ir: RawIr) -> Result<ReducedIr, BuilderError> {
    let mut reduced_buckets = Vec::with_capacity(raw_ir.0.len());

    for bucket in raw_ir.0 {
        let mut reduced_table = BTreeMap::new();

        for (key, timeline_var) in bucket.state_tables {
            let reduced_var = match timeline_var {
                TimelineVariant::Balance(tl) => ReducedVariant::Balance(tl.reduce()?),
                TimelineVariant::Nonce(tl) => ReducedVariant::Nonce(tl.reduce()?),
                TimelineVariant::Code(tl) => ReducedVariant::Code(tl.reduce()?),
                TimelineVariant::Storage(tl) => ReducedVariant::Storage(tl.reduce()?),
                TimelineVariant::Transient(tl) => ReducedVariant::Transient(tl.reduce()?),
            };
            reduced_table.insert(key, reduced_var);
        }

        let gas_refund = bucket.gas_refund_timeline.map(|t| t.reduce()).transpose()?;

        reduced_buckets.push(ReducedTransactionBucket {
            metadata: bucket.metadata,
            state_table: reduced_table,
            lifecycle_table: bucket.lifecycle_table,
            log_table: bucket.log_table,
            gas_refund,
        });
    }

    Ok(ReducedIr(reduced_buckets))
}
