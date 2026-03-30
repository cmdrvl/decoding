//! `convergence.v0` summary report contract.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::vocabulary::{PropertyType, SourceKind};

/// Summary report of the archaeology convergence run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvergenceReport {
    pub event: String,
    pub policy_id: String,
    pub totals: ConvergenceTotals,
    pub by_property_type: IndexMap<PropertyType, usize>,
    pub by_source_kind: IndexMap<SourceKind, usize>,
    pub top_escalations: Vec<serde_json::Value>,
}

/// Aggregate bucket-state totals for a convergence run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvergenceTotals {
    pub buckets: usize,
    pub converged: usize,
    pub converging: usize,
    pub single_source: usize,
    pub conflicted: usize,
    pub escalated: usize,
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use serde_json::json;

    use super::{ConvergenceReport, ConvergenceTotals};
    use crate::contracts::vocabulary::{PropertyType, SourceKind};

    #[test]
    fn convergence_report_round_trips_with_frozen_minimal_shape() {
        let report = ConvergenceReport {
            event: "convergence.v0".to_string(),
            policy_id: "legacy.decode.v0".to_string(),
            totals: ConvergenceTotals {
                buckets: 42,
                converged: 31,
                converging: 0,
                single_source: 6,
                conflicted: 5,
                escalated: 5,
            },
            by_property_type: IndexMap::new(),
            by_source_kind: IndexMap::new(),
            top_escalations: Vec::new(),
        };

        let rendered = serde_json::to_value(&report).unwrap();
        assert_eq!(
            rendered,
            json!({
                "event": "convergence.v0",
                "policy_id": "legacy.decode.v0",
                "totals": {
                    "buckets": 42,
                    "converged": 31,
                    "converging": 0,
                    "single_source": 6,
                    "conflicted": 5,
                    "escalated": 5
                },
                "by_property_type": {},
                "by_source_kind": {},
                "top_escalations": []
            })
        );

        let reparsed: ConvergenceReport = serde_json::from_value(rendered).unwrap();
        assert_eq!(reparsed, report);
    }

    #[test]
    fn convergence_report_serializes_typed_breakdowns() {
        let report = ConvergenceReport {
            event: "convergence.v0".to_string(),
            policy_id: "legacy.decode.v0".to_string(),
            totals: ConvergenceTotals {
                buckets: 3,
                converged: 1,
                converging: 1,
                single_source: 0,
                conflicted: 1,
                escalated: 1,
            },
            by_property_type: IndexMap::from([
                (PropertyType::SemanticLabel, 2),
                (PropertyType::Liveness, 1),
            ]),
            by_source_kind: IndexMap::from([(SourceKind::RepoScan, 2), (SourceKind::DbScan, 1)]),
            top_escalations: vec![json!({
                "bucket_id": "sha256:3333333333333333333333333333333333333333333333333333333333333333",
                "subject": {
                    "kind": "mapping",
                    "id": "adj.ebitda.rule.family"
                },
                "property_type": "semantic_label"
            })],
        };

        let rendered = serde_json::to_value(&report).unwrap();
        assert_eq!(
            rendered["by_property_type"],
            json!({"semantic_label": 2, "liveness": 1})
        );
        assert_eq!(
            rendered["by_source_kind"],
            json!({"repo_scan": 2, "db_scan": 1})
        );

        let reparsed: ConvergenceReport = serde_json::from_value(rendered).unwrap();
        assert_eq!(reparsed, report);
    }
}
