//! `canon_entry.v0` output contract.

use serde::{Deserialize, Serialize};

use super::vocabulary::{PropertyType, SubjectRef};

/// A resolved canonical entry emitted for a converged bucket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct ConvergenceState {
    pub state: ConvergenceStateKind,
    pub source_count: usize,
    pub claim_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Explanation {
    pub winner_claim_ids: Vec<String>,
    pub compatible_claim_ids: Vec<String>,
    pub resolution_kind: ResolutionKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale_inference: Option<ScaleInferenceExplanation>,
}

/// Structured provenance for numeric scale inference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScaleInferenceExplanation {
    pub canonical_scale: String,
    pub inferred_factors: Vec<InferredScaleFactor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InferredScaleFactor {
    pub claim_id: String,
    pub factor: f64,
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
    ScaleInferred,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        CanonEntry, ConvergenceState, ConvergenceStateKind, Explanation, InferredScaleFactor,
        ResolutionKind, ScaleInferenceExplanation,
    };
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
                scale_inference: None,
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

    #[test]
    fn canon_entry_serializes_scale_inference_explanation() {
        let entry = CanonEntry {
            event: "canon_entry.v0".to_string(),
            bucket_id: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Fund,
                id: "ares.2026_q1".to_string(),
            },
            property_type: PropertyType::NumericScalar,
            canonical_value: json!({
                "kind": "numeric_scalar",
                "scale": "dollars",
                "value": 29499300000.0
            }),
            policy_id: "legacy.decode.v0".to_string(),
            convergence: ConvergenceState {
                state: ConvergenceStateKind::Converged,
                source_count: 2,
                claim_count: 2,
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
                ],
                resolution_kind: ResolutionKind::ScaleInferred,
                scale_inference: Some(ScaleInferenceExplanation {
                    canonical_scale: "dollars".to_string(),
                    inferred_factors: vec![InferredScaleFactor {
                        claim_id:
                            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                                .to_string(),
                        factor: 1_000_000.0,
                    }],
                }),
            },
        };

        let rendered = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            rendered["explain"]["resolution_kind"],
            json!("scale_inferred")
        );
        assert_eq!(
            rendered["explain"]["scale_inference"],
            json!({
                "canonical_scale": "dollars",
                "inferred_factors": [{
                    "claim_id": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    "factor": 1000000.0
                }]
            })
        );
    }

    #[test]
    fn canon_entry_rejects_unknown_fields() {
        let error = serde_json::from_value::<CanonEntry>(json!({
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
                "claim_count": 4,
                "unexpected": true
            },
            "explain": {
                "winner_claim_ids": [
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                ],
                "compatible_claim_ids": [
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                ],
                "resolution_kind": "corroborated"
            }
        }))
        .unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }
}
