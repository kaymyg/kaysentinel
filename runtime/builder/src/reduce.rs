use std::collections::BTreeMap;
use crate::ir::timeline::TimelineIr;
use crate::ir::reduced::{ReducedIr, TransactionBucket, AccountNode};
use crate::traits::ReducibleTimeline;
use crate::errors::BuilderError;

/// Transforms a TimelineIr into a ReducedIr by delegating reduction to the nodes.
pub fn process(ir: TimelineIr) -> Result<ReducedIr, BuilderError> {
    let mut reduced_buckets = Vec::with_capacity(ir.0.len());

    for bucket in ir.0 {
        let mut reduced_nodes = BTreeMap::new();

        for (address, node) in bucket.account_nodes {
            let reduced_node = AccountNode::reduce(node)?;
            reduced_nodes.insert(address, reduced_node);
        }

        let gas_refund = bucket.gas_refund_timeline.map(|t| t.reduce()).transpose()?;

        reduced_buckets.push(TransactionBucket {
            metadata: bucket.metadata,
            account_nodes: reduced_nodes,
            gas_refund,
        });
    }

    Ok(ReducedIr(reduced_buckets))
}
