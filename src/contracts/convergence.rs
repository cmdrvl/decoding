//! `convergence.v0` summary report contract.

use serde::{Deserialize, Serialize};

/// Summary report of the archaeology convergence run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceReport {
    pub event: String,
    pub policy_id: String,
    pub totals: ConvergenceTotals,
    pub by_property_type: serde_json::Value,
    pub by_source_kind: serde_json::Value,
    pub top_escalations: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceTotals {
    pub buckets: usize,
    pub converged: usize,
    pub converging: usize,
    pub single_source: usize,
    pub conflicted: usize,
    pub escalated: usize,
}
