//! Convergence report generation from resolver output.

use crate::contracts::convergence::ConvergenceReport;
use crate::resolve::Decision;

/// Generate a convergence.v0 report from the set of resolver decisions.
pub fn generate_report(_policy_id: &str, _decisions: &[Decision]) -> ConvergenceReport {
    todo!("convergence report generation")
}
