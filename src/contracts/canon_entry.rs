//! `canon_entry.v0` output contract.

use serde::{Deserialize, Serialize};

use super::vocabulary::{PropertyType, SubjectRef};

/// A resolved canonical entry emitted for a converged bucket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonEntry {
    pub event: String,
    pub bucket_id: String,
    pub subject: SubjectRef,
    pub property_type: PropertyType,
    pub canonical_value: serde_json::Value,
    pub policy_id: String,
    pub convergence: ConvergenceState,
    pub explain: Explanation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConvergenceState {
    pub state: ConvergenceStateKind,
    pub source_count: usize,
    pub claim_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explanation {
    pub winner_claim_ids: Vec<String>,
    pub compatible_claim_ids: Vec<String>,
    pub resolution_kind: ResolutionKind,
}

/// Convergence state for a canonical entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConvergenceStateKind {
    SingleSource,
    Converging,
    Converged,
}

/// How the canonical value was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionKind {
    SingleSource,
    Corroborated,
    PriorityBreak,
    LivenessFold,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CanonEntry, ConvergenceState, ConvergenceStateKind, Explanation, ResolutionKind};
    use crate::contracts::vocabulary::{PropertyType, SubjectKind, SubjectRef};

    #[test]
    fn canon_entry_round_trips_with_frozen_wire_shape() {
        let entry = CanonEntry {
            event: "canon_entry.v0".to_string(),
            bucket_id: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Report,
                id: "hyperion.close_pack_ebitda".to_string(),
            },
            property_type: PropertyType::DependsOn,
            canonical_value: json!({"kind":"feed","id":"fdmee.actuals_load"}),
            policy_id: "legacy.decode.v0".to_string(),
            convergence: ConvergenceState {
                state: ConvergenceStateKind::Converged,
                source_count: 3,
                claim_count: 4,
            },
            explain: Explanation {
                winner_claim_ids: vec![
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                ],
                compatible_claim_ids: vec![
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                        .to_string(),
                ],
                resolution_kind: ResolutionKind::Corroborated,
            },
        };

        let rendered = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            rendered,
            json!({
                "event": "canon_entry.v0",
                "bucket_id": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                "subject": {
                    "kind": "report",
                    "id": "hyperion.close_pack_ebitda"
                },
                "property_type": "depends_on",
                "canonical_value": {
                    "kind": "feed",
                    "id": "fdmee.actuals_load"
                },
                "policy_id": "legacy.decode.v0",
                "convergence": {
                    "state": "converged",
                    "source_count": 3,
                    "claim_count": 4
                },
                "explain": {
                    "winner_claim_ids": [
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    ],
                    "compatible_claim_ids": [
                        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    ],
                    "resolution_kind": "corroborated"
                }
            })
        );

        let reparsed: CanonEntry = serde_json::from_value(rendered).unwrap();
        assert_eq!(reparsed, entry);
    }
}
