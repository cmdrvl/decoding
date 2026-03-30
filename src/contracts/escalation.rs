//! `escalation.v0` output contract.

use serde::{Deserialize, Serialize};

use super::vocabulary::{PropertyType, SubjectRef, ValueRef};

/// An escalation emitted for an unresolved or conflicting bucket.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Escalation {
    pub event: String,
    pub bucket_id: String,
    pub subject: SubjectRef,
    pub property_type: PropertyType,
    pub reason: EscalationReason,
    pub claim_ids: Vec<String>,
    pub candidate_values: Vec<CandidateValue>,
    pub recommended_action: RecommendedAction,
    pub summary: String,
}

/// Candidate values under escalation review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CandidateValue {
    Scalar(ScalarCandidateValue),
    Ref(ValueRef),
}

/// Tagged scalar candidate value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarCandidateValue {
    pub kind: ScalarCandidateKind,
    pub value: serde_json::Value,
}

/// Scalar candidate discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarCandidateKind {
    Scalar,
}

/// Why the bucket was escalated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationReason {
    Conflicted,
    MissingCorroboration,
    NoResolutionPath,
}

/// What the operator should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedAction {
    Review,
    ScanMore,
    FixScanner,
    FixPolicy,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        CandidateValue, Escalation, EscalationReason, RecommendedAction, ScalarCandidateKind,
        ScalarCandidateValue,
    };
    use crate::contracts::vocabulary::{PropertyType, SubjectKind, SubjectRef, ValueRef};

    #[test]
    fn escalation_round_trips_with_scalar_and_ref_candidates() {
        let escalation = Escalation {
            event: "escalation.v0".to_string(),
            bucket_id: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Mapping,
                id: "adj.ebitda.rule.family".to_string(),
            },
            property_type: PropertyType::SemanticLabel,
            reason: EscalationReason::Conflicted,
            claim_ids: vec![
                "sha256:7777777777777777777777777777777777777777777777777777777777777777"
                    .to_string(),
                "sha256:8888888888888888888888888888888888888888888888888888888888888888"
                    .to_string(),
            ],
            candidate_values: vec![
                CandidateValue::Scalar(ScalarCandidateValue {
                    kind: ScalarCandidateKind::Scalar,
                    value: json!("Adjusted EBITDA rule family"),
                }),
                CandidateValue::Ref(ValueRef {
                    kind: SubjectKind::Feed,
                    id: "fdmee.actuals_load".to_string(),
                }),
            ],
            recommended_action: RecommendedAction::Review,
            summary: "two incompatible semantic interpretations remain".to_string(),
        };

        let rendered = serde_json::to_value(&escalation).unwrap();
        assert_eq!(
            rendered,
            json!({
                "event": "escalation.v0",
                "bucket_id": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                "subject": {
                    "kind": "mapping",
                    "id": "adj.ebitda.rule.family"
                },
                "property_type": "semantic_label",
                "reason": "conflicted",
                "claim_ids": [
                    "sha256:7777777777777777777777777777777777777777777777777777777777777777",
                    "sha256:8888888888888888888888888888888888888888888888888888888888888888"
                ],
                "candidate_values": [
                    {
                        "kind": "scalar",
                        "value": "Adjusted EBITDA rule family"
                    },
                    {
                        "kind": "feed",
                        "id": "fdmee.actuals_load"
                    }
                ],
                "recommended_action": "review",
                "summary": "two incompatible semantic interpretations remain"
            })
        );

        let reparsed: Escalation = serde_json::from_value(rendered).unwrap();
        assert_eq!(reparsed, escalation);
    }

    #[test]
    fn escalation_rejects_unknown_fields() {
        let error = serde_json::from_value::<Escalation>(json!({
            "event": "escalation.v0",
            "bucket_id": "sha256:2222222222222222222222222222222222222222222222222222222222222222",
            "subject": {
                "kind": "mapping",
                "id": "adj.ebitda.rule.family"
            },
            "property_type": "semantic_label",
            "reason": "conflicted",
            "claim_ids": [
                "sha256:7777777777777777777777777777777777777777777777777777777777777777"
            ],
            "candidate_values": [
                {
                    "kind": "scalar",
                    "value": "Adjusted EBITDA rule family"
                }
            ],
            "recommended_action": "review",
            "summary": "two incompatible semantic interpretations remain",
            "unexpected": true
        }))
        .unwrap_err();

        assert!(error.to_string().contains("unknown field"));
    }
}
