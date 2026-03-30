//! Convergence report generation from resolver output.

use std::collections::BTreeMap;

use indexmap::IndexMap;
use serde_json::json;

use crate::contracts::canon_entry::ConvergenceStateKind;
use crate::contracts::convergence::ConvergenceReport;
use crate::contracts::convergence::ConvergenceTotals;
use crate::contracts::escalation::EscalationReason;
use crate::contracts::vocabulary::{PropertyType, SourceKind};
use crate::normalize::canonical_json;
use crate::resolve::Decision;

/// Generate a convergence.v0 report from the set of resolver decisions.
pub fn generate_report(policy_id: &str, decisions: &[Decision]) -> ConvergenceReport {
    let mut totals = ConvergenceTotals {
        buckets: decisions.len(),
        converged: 0,
        converging: 0,
        single_source: 0,
        conflicted: 0,
        escalated: 0,
    };
    let mut by_property_type = BTreeMap::<String, (PropertyType, usize)>::new();
    let mut by_source_kind = BTreeMap::<String, (SourceKind, usize)>::new();
    let mut top_escalations = Vec::new();

    for decision in decisions {
        match decision {
            Decision::Resolved(decision) => {
                increment_property_count(&mut by_property_type, decision.entry.property_type);
                increment_source_counts(&mut by_source_kind, &decision.source_kinds);
                match decision.entry.convergence.state {
                    ConvergenceStateKind::Converged => totals.converged += 1,
                    ConvergenceStateKind::Converging => totals.converging += 1,
                    ConvergenceStateKind::SingleSource => totals.single_source += 1,
                }
            }
            Decision::Escalated(decision) => {
                increment_property_count(&mut by_property_type, decision.escalation.property_type);
                increment_source_counts(&mut by_source_kind, &decision.source_kinds);
                totals.escalated += 1;
                if decision.escalation.reason == EscalationReason::Conflicted {
                    totals.conflicted += 1;
                }

                top_escalations.push(json!({
                    "bucket_id": decision.escalation.bucket_id,
                    "subject": decision.escalation.subject,
                    "property_type": decision.escalation.property_type,
                    "reason": decision.escalation.reason,
                    "recommended_action": decision.escalation.recommended_action,
                    "summary": decision.escalation.summary
                }));
            }
        }
    }

    top_escalations.sort_by_key(canonical_json);

    ConvergenceReport {
        event: "convergence.v0".to_string(),
        policy_id: policy_id.to_string(),
        totals,
        by_property_type: by_property_type.into_values().collect::<IndexMap<_, _>>(),
        by_source_kind: by_source_kind.into_values().collect::<IndexMap<_, _>>(),
        top_escalations,
    }
}

fn increment_property_count(
    counts: &mut BTreeMap<String, (PropertyType, usize)>,
    property_type: PropertyType,
) {
    let key = property_type_name(property_type).to_string();
    match counts.get_mut(&key) {
        Some((_, count)) => *count += 1,
        None => {
            counts.insert(key, (property_type, 1));
        }
    }
}

fn increment_source_counts(
    counts: &mut BTreeMap<String, (SourceKind, usize)>,
    source_kinds: &[SourceKind],
) {
    for source_kind in source_kinds {
        let key = source_kind_name(*source_kind).to_string();
        match counts.get_mut(&key) {
            Some((_, count)) => *count += 1,
            None => {
                counts.insert(key, (*source_kind, 1));
            }
        }
    }
}

fn property_type_name(property_type: PropertyType) -> &'static str {
    match property_type {
        PropertyType::Exists => "exists",
        PropertyType::Schema => "schema",
        PropertyType::Constraint => "constraint",
        PropertyType::Reads => "reads",
        PropertyType::Writes => "writes",
        PropertyType::DependsOn => "depends_on",
        PropertyType::UsedBy => "used_by",
        PropertyType::Schedule => "schedule",
        PropertyType::ValidValues => "valid_values",
        PropertyType::SemanticLabel => "semantic_label",
        PropertyType::Liveness => "liveness",
        PropertyType::AuthoritativeFor => "authoritative_for",
    }
}

fn source_kind_name(source_kind: SourceKind) -> &'static str {
    match source_kind {
        SourceKind::RepoScan => "repo_scan",
        SourceKind::DbScan => "db_scan",
        SourceKind::FileScan => "file_scan",
    }
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use serde_json::json;

    use super::generate_report;
    use crate::contracts::canon_entry::{
        CanonEntry, ConvergenceState, ConvergenceStateKind, Explanation, ResolutionKind,
    };
    use crate::contracts::escalation::{Escalation, EscalationReason, RecommendedAction};
    use crate::contracts::vocabulary::{PropertyType, SourceKind, SubjectKind, SubjectRef};
    use crate::resolve::{Decision, EscalatedDecision, ResolvedDecision};

    #[test]
    fn generates_typed_totals_and_breakdowns() {
        let report = generate_report("legacy.decode.v0", &sample_decisions());

        assert_eq!(report.event, "convergence.v0");
        assert_eq!(report.policy_id, "legacy.decode.v0");
        assert_eq!(report.totals.buckets, 5);
        assert_eq!(report.totals.converged, 1);
        assert_eq!(report.totals.converging, 1);
        assert_eq!(report.totals.single_source, 1);
        assert_eq!(report.totals.conflicted, 1);
        assert_eq!(report.totals.escalated, 2);
        assert_eq!(
            report.by_property_type,
            IndexMap::from([
                (PropertyType::AuthoritativeFor, 1),
                (PropertyType::DependsOn, 1),
                (PropertyType::Exists, 1),
                (PropertyType::Liveness, 1),
                (PropertyType::SemanticLabel, 1),
            ])
        );
        assert_eq!(
            report.by_source_kind,
            IndexMap::from([
                (SourceKind::DbScan, 3),
                (SourceKind::FileScan, 3),
                (SourceKind::RepoScan, 3),
            ])
        );
        assert_eq!(report.top_escalations.len(), 2);
    }

    #[test]
    fn orders_top_escalations_deterministically() {
        let report = generate_report(
            "legacy.decode.v0",
            &[
                Decision::Escalated(EscalatedDecision {
                    escalation: sample_escalation(
                        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                        "adj.zeta",
                        PropertyType::SemanticLabel,
                        EscalationReason::Conflicted,
                    ),
                    source_kinds: vec![SourceKind::RepoScan],
                }),
                Decision::Escalated(EscalatedDecision {
                    escalation: sample_escalation(
                        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                        "adj.alpha",
                        PropertyType::ValidValues,
                        EscalationReason::MissingCorroboration,
                    ),
                    source_kinds: vec![SourceKind::FileScan],
                }),
            ],
        );

        assert_eq!(
            report.top_escalations,
            vec![
                json!({
                    "bucket_id": "sha256:1111111111111111111111111111111111111111111111111111111111111111",
                    "subject": {
                        "kind": "mapping",
                        "id": "adj.alpha"
                    },
                    "property_type": "valid_values",
                    "reason": "missing_corroboration",
                    "recommended_action": "scan_more",
                    "summary": "summary for adj.alpha"
                }),
                json!({
                    "bucket_id": "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                    "subject": {
                        "kind": "mapping",
                        "id": "adj.zeta"
                    },
                    "property_type": "semantic_label",
                    "reason": "conflicted",
                    "recommended_action": "review",
                    "summary": "summary for adj.zeta"
                }),
            ]
        );
    }

    fn sample_decisions() -> Vec<Decision> {
        vec![
            Decision::Resolved(ResolvedDecision {
                entry: sample_entry(
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                    PropertyType::Exists,
                    ConvergenceStateKind::SingleSource,
                ),
                source_kinds: vec![SourceKind::DbScan],
            }),
            Decision::Resolved(ResolvedDecision {
                entry: sample_entry(
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                    PropertyType::DependsOn,
                    ConvergenceStateKind::Converged,
                ),
                source_kinds: vec![SourceKind::DbScan, SourceKind::RepoScan],
            }),
            Decision::Resolved(ResolvedDecision {
                entry: sample_entry(
                    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                    PropertyType::Liveness,
                    ConvergenceStateKind::Converging,
                ),
                source_kinds: vec![
                    SourceKind::DbScan,
                    SourceKind::FileScan,
                    SourceKind::RepoScan,
                ],
            }),
            Decision::Escalated(EscalatedDecision {
                escalation: sample_escalation(
                    "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                    "adj.ebitda.rule.family",
                    PropertyType::SemanticLabel,
                    EscalationReason::Conflicted,
                ),
                source_kinds: vec![SourceKind::FileScan, SourceKind::RepoScan],
            }),
            Decision::Escalated(EscalatedDecision {
                escalation: sample_escalation(
                    "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                    "exec.close_pack_board",
                    PropertyType::AuthoritativeFor,
                    EscalationReason::MissingCorroboration,
                ),
                source_kinds: vec![SourceKind::FileScan],
            }),
        ]
    }

    fn sample_entry(
        bucket_id: &str,
        property_type: PropertyType,
        state: ConvergenceStateKind,
    ) -> CanonEntry {
        CanonEntry {
            event: "canon_entry.v0".to_string(),
            bucket_id: bucket_id.to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Report,
                id: "hyperion.close_pack_ebitda".to_string(),
            },
            property_type,
            canonical_value: json!(true),
            policy_id: "legacy.decode.v0".to_string(),
            convergence: ConvergenceState {
                state,
                source_count: 1,
                claim_count: 1,
            },
            explain: Explanation {
                winner_claim_ids: vec![
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                ],
                compatible_claim_ids: vec![
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                ],
                resolution_kind: ResolutionKind::SingleSource,
            },
        }
    }

    fn sample_escalation(
        bucket_id: &str,
        subject_id: &str,
        property_type: PropertyType,
        reason: EscalationReason,
    ) -> Escalation {
        let recommended_action = match reason {
            EscalationReason::Conflicted => RecommendedAction::Review,
            EscalationReason::MissingCorroboration => RecommendedAction::ScanMore,
            EscalationReason::NoResolutionPath => RecommendedAction::FixPolicy,
        };

        Escalation {
            event: "escalation.v0".to_string(),
            bucket_id: bucket_id.to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Mapping,
                id: subject_id.to_string(),
            },
            property_type,
            reason,
            claim_ids: vec![
                "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
                    .to_string(),
            ],
            candidate_values: Vec::new(),
            recommended_action,
            summary: format!("summary for {subject_id}"),
        }
    }
}
