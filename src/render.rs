//! Render canon_entry.v0 and escalation.v0 records to JSONL output.

use std::io::Write;

use crate::contracts::canon_entry::CanonEntry;
use crate::contracts::escalation::{CandidateValue, Escalation};
use crate::normalize::canonical_json;

/// Write a canon entry as a single JSONL line.
pub fn write_canon_entry(
    writer: &mut dyn Write,
    entry: &CanonEntry,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entry = entry.clone();
    entry.explain.winner_claim_ids.sort_unstable();
    entry.explain.compatible_claim_ids.sort_unstable();

    serde_json::to_writer(&mut *writer, &entry)?;
    writer.write_all(b"\n")?;
    Ok(())
}

/// Write an escalation as a single JSONL line.
pub fn write_escalation(
    writer: &mut dyn Write,
    escalation: &Escalation,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut escalation = escalation.clone();
    escalation.claim_ids.sort_unstable();
    escalation.candidate_values.sort_by_key(candidate_sort_key);

    serde_json::to_writer(&mut *writer, &escalation)?;
    writer.write_all(b"\n")?;
    Ok(())
}

fn candidate_sort_key(candidate: &CandidateValue) -> String {
    match serde_json::to_value(candidate) {
        Ok(value) => canonical_json(&value),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{write_canon_entry, write_escalation};
    use crate::contracts::canon_entry::{
        CanonEntry, ConvergenceState, ConvergenceStateKind, Explanation, ResolutionKind,
    };
    use crate::contracts::escalation::{
        CandidateValue, Escalation, EscalationReason, RecommendedAction, ScalarCandidateKind,
        ScalarCandidateValue,
    };
    use crate::contracts::vocabulary::{PropertyType, SubjectKind, SubjectRef, ValueRef};

    #[test]
    fn writes_canon_entry_jsonl_with_sorted_explain_ids() {
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
                claim_count: 3,
            },
            explain: Explanation {
                winner_claim_ids: vec![
                    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                        .to_string(),
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                ],
                compatible_claim_ids: vec![
                    "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                        .to_string(),
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                ],
                resolution_kind: ResolutionKind::Corroborated,
            },
        };

        let mut output = Vec::new();
        write_canon_entry(&mut output, &entry).unwrap();

        let rendered = String::from_utf8(output).unwrap();
        let expected = serde_json::to_string(&CanonEntry {
            explain: Explanation {
                winner_claim_ids: vec![
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                        .to_string(),
                ],
                compatible_claim_ids: vec![
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                    "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                        .to_string(),
                ],
                ..entry.explain.clone()
            },
            ..entry.clone()
        })
        .unwrap()
            + "\n";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn writes_escalation_jsonl_with_sorted_claim_ids_and_candidate_values() {
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
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
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
            summary: "2 incompatible candidate values remain".to_string(),
        };

        let mut output = Vec::new();
        write_escalation(&mut output, &escalation).unwrap();

        let rendered = String::from_utf8(output).unwrap();
        let expected = serde_json::to_string(&Escalation {
            claim_ids: vec![
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
            ],
            candidate_values: vec![
                CandidateValue::Ref(ValueRef {
                    kind: SubjectKind::Feed,
                    id: "fdmee.actuals_load".to_string(),
                }),
                CandidateValue::Scalar(ScalarCandidateValue {
                    kind: ScalarCandidateKind::Scalar,
                    value: json!("Adjusted EBITDA rule family"),
                }),
            ],
            ..escalation.clone()
        })
        .unwrap()
            + "\n";

        assert_eq!(rendered, expected);
    }
}
