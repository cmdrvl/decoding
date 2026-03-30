//! Resolver engine: drives bucket state transitions and produces decision records.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::bucket::Bucket;
use crate::compare::{Compatibility, compare};
use crate::contracts::canon_entry::{
    CanonEntry, ConvergenceState, ConvergenceStateKind, Explanation, ResolutionKind,
};
use crate::contracts::escalation::{
    CandidateValue, Escalation, EscalationReason, RecommendedAction, ScalarCandidateKind,
    ScalarCandidateValue,
};
use crate::contracts::policy::Policy;
use crate::contracts::vocabulary::{PropertyType, SourceKind, ValueRef};
use crate::normalize::{canonical_json, normalize_string, sorted_set};
use serde::Deserialize;
use serde_json::json;

/// The decision produced for a single bucket after resolution.
#[derive(Debug)]
pub enum Decision {
    /// Bucket resolved to a canonical entry.
    Resolved(CanonEntry),
    /// Bucket escalated for human review.
    Escalated(Escalation),
}

/// Resolve a single bucket against the policy. Returns a decision record.
pub fn resolve_bucket(bucket: &Bucket, policy: &Policy) -> Decision {
    if bucket.claims.is_empty() {
        return Decision::Escalated(build_escalation(
            bucket,
            EscalationReason::NoResolutionPath,
            Vec::new(),
            RecommendedAction::FixPolicy,
            "bucket contained no surviving claims".to_string(),
        ));
    }

    let property_type = bucket.key.property_type();
    let groups = candidate_groups(bucket, property_type);

    if has_incompatible_claims(bucket, property_type) {
        return Decision::Escalated(build_escalation(
            bucket,
            EscalationReason::Conflicted,
            build_candidate_values(property_type, &groups),
            recommended_action_for_conflict(policy, property_type),
            format!("{} incompatible candidate values remain", groups.len()),
        ));
    }

    if property_type == PropertyType::Liveness {
        return resolve_liveness(bucket, policy, &groups);
    }

    resolve_non_liveness(bucket, policy, property_type, &groups)
}

#[derive(Debug, Clone)]
struct CandidateGroup {
    canonical_key: String,
    canonical_value: serde_json::Value,
    display_value: serde_json::Value,
    display_key: String,
    claim_ids: Vec<String>,
    source_artifact_ids: BTreeSet<String>,
    source_kinds: Vec<SourceKind>,
}

impl CandidateGroup {
    fn source_count(&self) -> usize {
        self.source_artifact_ids.len()
    }
}

fn resolve_liveness(bucket: &Bucket, policy: &Policy, groups: &[CandidateGroup]) -> Decision {
    if groups.is_empty() {
        return Decision::Escalated(build_escalation(
            bucket,
            EscalationReason::NoResolutionPath,
            Vec::new(),
            RecommendedAction::FixPolicy,
            "policy does not define a resolution path for liveness".to_string(),
        ));
    }

    if groups.len() == 1 {
        let chosen = &groups[0];
        if is_dead_value(&chosen.canonical_value) && bucket.source_artifact_count() < 2 {
            return Decision::Escalated(build_escalation(
                bucket,
                EscalationReason::MissingCorroboration,
                build_candidate_values(PropertyType::Liveness, groups),
                RecommendedAction::ScanMore,
                format!(
                    "need at least 2 corroborating sources, found {}",
                    bucket.source_artifact_count()
                ),
            ));
        }

        let (state, resolution_kind) = if bucket.claim_count() == 1 {
            (
                ConvergenceStateKind::SingleSource,
                ResolutionKind::SingleSource,
            )
        } else {
            (
                ConvergenceStateKind::Converged,
                ResolutionKind::Corroborated,
            )
        };

        return Decision::Resolved(build_canon_entry(
            bucket,
            policy,
            chosen.canonical_value.clone(),
            state,
            chosen.claim_ids.clone(),
            claim_ids(bucket),
            resolution_kind,
        ));
    }

    let Some(source_priority) = policy.source_priority_for(PropertyType::Liveness) else {
        return Decision::Escalated(build_escalation(
            bucket,
            EscalationReason::NoResolutionPath,
            build_candidate_values(PropertyType::Liveness, groups),
            RecommendedAction::FixPolicy,
            "policy does not define a resolution path for liveness".to_string(),
        ));
    };

    let chosen = select_group_by_source_priority(groups, source_priority);

    Decision::Resolved(build_canon_entry(
        bucket,
        policy,
        chosen.canonical_value.clone(),
        ConvergenceStateKind::Converging,
        chosen.claim_ids.clone(),
        claim_ids(bucket),
        ResolutionKind::LivenessFold,
    ))
}

fn resolve_non_liveness(
    bucket: &Bucket,
    policy: &Policy,
    property_type: PropertyType,
    groups: &[CandidateGroup],
) -> Decision {
    if groups.len() != 1 {
        return Decision::Escalated(build_escalation(
            bucket,
            EscalationReason::NoResolutionPath,
            build_candidate_values(property_type, groups),
            RecommendedAction::FixPolicy,
            format!(
                "policy does not define a resolution path for {}",
                property_type_name(property_type)
            ),
        ));
    }

    let chosen = &groups[0];

    if policy.auto_resolves(property_type) {
        let (state, resolution_kind) = if bucket.claim_count() == 1 {
            (
                ConvergenceStateKind::SingleSource,
                ResolutionKind::SingleSource,
            )
        } else {
            (
                ConvergenceStateKind::Converged,
                ResolutionKind::Corroborated,
            )
        };

        return Decision::Resolved(build_canon_entry(
            bucket,
            policy,
            chosen.canonical_value.clone(),
            state,
            chosen.claim_ids.clone(),
            claim_ids(bucket),
            resolution_kind,
        ));
    }

    let Some(threshold) = policy.corroboration_threshold(property_type) else {
        return Decision::Escalated(build_escalation(
            bucket,
            EscalationReason::NoResolutionPath,
            build_candidate_values(property_type, groups),
            RecommendedAction::FixPolicy,
            format!(
                "policy does not define a resolution path for {}",
                property_type_name(property_type)
            ),
        ));
    };

    if bucket.source_artifact_count() < threshold {
        return Decision::Escalated(build_escalation(
            bucket,
            EscalationReason::MissingCorroboration,
            build_candidate_values(property_type, groups),
            RecommendedAction::ScanMore,
            format!(
                "need at least {threshold} corroborating sources, found {}",
                bucket.source_artifact_count()
            ),
        ));
    }

    let (state, resolution_kind) = if bucket.claim_count() == 1 {
        (
            ConvergenceStateKind::SingleSource,
            ResolutionKind::SingleSource,
        )
    } else {
        (
            ConvergenceStateKind::Converged,
            ResolutionKind::Corroborated,
        )
    };

    Decision::Resolved(build_canon_entry(
        bucket,
        policy,
        chosen.canonical_value.clone(),
        state,
        chosen.claim_ids.clone(),
        claim_ids(bucket),
        resolution_kind,
    ))
}

fn build_canon_entry(
    bucket: &Bucket,
    policy: &Policy,
    canonical_value: serde_json::Value,
    state: ConvergenceStateKind,
    winner_claim_ids: Vec<String>,
    compatible_claim_ids: Vec<String>,
    resolution_kind: ResolutionKind,
) -> CanonEntry {
    CanonEntry {
        event: "canon_entry.v0".to_string(),
        bucket_id: bucket.bucket_id.clone(),
        subject: bucket.key.subject().clone(),
        property_type: bucket.key.property_type(),
        canonical_value,
        policy_id: policy.policy_id.clone(),
        convergence: ConvergenceState {
            state,
            source_count: bucket.source_artifact_count(),
            claim_count: bucket.claim_count(),
        },
        explain: Explanation {
            winner_claim_ids,
            compatible_claim_ids,
            resolution_kind,
        },
    }
}

fn build_escalation(
    bucket: &Bucket,
    reason: EscalationReason,
    candidate_values: Vec<CandidateValue>,
    recommended_action: RecommendedAction,
    summary: String,
) -> Escalation {
    Escalation {
        event: "escalation.v0".to_string(),
        bucket_id: bucket.bucket_id.clone(),
        subject: bucket.key.subject().clone(),
        property_type: bucket.key.property_type(),
        reason,
        claim_ids: claim_ids(bucket),
        candidate_values,
        recommended_action,
        summary,
    }
}

fn claim_ids(bucket: &Bucket) -> Vec<String> {
    bucket
        .claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect()
}

fn candidate_groups(bucket: &Bucket, property_type: PropertyType) -> Vec<CandidateGroup> {
    let mut groups = BTreeMap::<String, CandidateGroup>::new();

    for claim in &bucket.claims {
        let canonical_value = canonical_output_value(property_type, &claim.value)
            .unwrap_or_else(|| canonicalize_json_value(&claim.value));
        let display_value = display_output_value(property_type, &claim.value)
            .unwrap_or_else(|| canonical_value.clone());
        let canonical_key = canonical_json(&canonical_value);
        let display_key = canonical_json(&display_value);

        match groups.get_mut(&canonical_key) {
            Some(group) => {
                group.claim_ids.push(claim.claim_id.clone());
                group
                    .source_artifact_ids
                    .insert(claim.source.artifact_id.clone());
                group.source_kinds.push(claim.source.kind);
                if display_key < group.display_key {
                    group.display_key = display_key;
                    group.display_value = display_value;
                }
            }
            None => {
                let mut source_artifact_ids = BTreeSet::new();
                source_artifact_ids.insert(claim.source.artifact_id.clone());
                groups.insert(
                    canonical_key.clone(),
                    CandidateGroup {
                        canonical_key,
                        canonical_value,
                        display_value,
                        display_key,
                        claim_ids: vec![claim.claim_id.clone()],
                        source_artifact_ids,
                        source_kinds: vec![claim.source.kind],
                    },
                );
            }
        }
    }

    let mut groups = groups.into_values().collect::<Vec<_>>();
    for group in &mut groups {
        group.claim_ids.sort_unstable();
    }
    groups
}

fn build_candidate_values(
    property_type: PropertyType,
    groups: &[CandidateGroup],
) -> Vec<CandidateValue> {
    groups
        .iter()
        .map(|group| candidate_value(property_type, &group.display_value))
        .collect()
}

fn candidate_value(property_type: PropertyType, value: &serde_json::Value) -> CandidateValue {
    if property_type.is_edge() {
        match serde_json::from_value::<ValueRef>(value.clone()) {
            Ok(value_ref) => CandidateValue::Ref(value_ref),
            Err(_) => CandidateValue::Scalar(ScalarCandidateValue {
                kind: ScalarCandidateKind::Scalar,
                value: value.clone(),
            }),
        }
    } else {
        CandidateValue::Scalar(ScalarCandidateValue {
            kind: ScalarCandidateKind::Scalar,
            value: value.clone(),
        })
    }
}

fn has_incompatible_claims(bucket: &Bucket, property_type: PropertyType) -> bool {
    for (index, left) in bucket.claims.iter().enumerate() {
        for right in bucket.claims.iter().skip(index + 1) {
            if compare(property_type, &left.value, &right.value) == Compatibility::Incompatible {
                return true;
            }
        }
    }

    false
}

fn select_group_by_source_priority<'a>(
    groups: &'a [CandidateGroup],
    source_priority: &[SourceKind],
) -> &'a CandidateGroup {
    let mut best = &groups[0];

    for group in groups.iter().skip(1) {
        if compare_group_priority(group, best, source_priority) == Ordering::Less {
            best = group;
        }
    }

    best
}

fn compare_group_priority(
    left: &CandidateGroup,
    right: &CandidateGroup,
    source_priority: &[SourceKind],
) -> Ordering {
    let left_rank = best_source_rank(left, source_priority);
    let right_rank = best_source_rank(right, source_priority);

    match left_rank.cmp(&right_rank) {
        Ordering::Equal => match right.source_count().cmp(&left.source_count()) {
            Ordering::Equal => match right.claim_ids.len().cmp(&left.claim_ids.len()) {
                Ordering::Equal => left.canonical_key.cmp(&right.canonical_key),
                other => other,
            },
            other => other,
        },
        other => other,
    }
}

fn best_source_rank(group: &CandidateGroup, source_priority: &[SourceKind]) -> usize {
    group
        .source_kinds
        .iter()
        .map(|kind| {
            source_priority
                .iter()
                .position(|candidate| candidate == kind)
                .unwrap_or(source_priority.len() + 1)
        })
        .min()
        .unwrap_or(source_priority.len() + 1)
}

fn canonical_output_value(
    property_type: PropertyType,
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    match property_type {
        PropertyType::Exists => value.as_bool().map(serde_json::Value::Bool),
        PropertyType::Schema | PropertyType::Constraint | PropertyType::Schedule => {
            Some(canonicalize_json_value(value))
        }
        PropertyType::Reads
        | PropertyType::Writes
        | PropertyType::DependsOn
        | PropertyType::UsedBy
        | PropertyType::AuthoritativeFor => serde_json::from_value::<ValueRef>(value.clone())
            .ok()
            .and_then(|value_ref| serde_json::to_value(value_ref).ok()),
        PropertyType::ValidValues => parse_string_set(value).map(|values| json!(values)),
        PropertyType::SemanticLabel | PropertyType::Liveness => {
            parse_scalar_string(value).map(|scalar| json!(normalize_string(&scalar)))
        }
    }
}

fn display_output_value(
    property_type: PropertyType,
    value: &serde_json::Value,
) -> Option<serde_json::Value> {
    match property_type {
        PropertyType::Exists => value.as_bool().map(serde_json::Value::Bool),
        PropertyType::Schema | PropertyType::Constraint | PropertyType::Schedule => {
            Some(canonicalize_json_value(value))
        }
        PropertyType::Reads
        | PropertyType::Writes
        | PropertyType::DependsOn
        | PropertyType::UsedBy
        | PropertyType::AuthoritativeFor => serde_json::from_value::<ValueRef>(value.clone())
            .ok()
            .and_then(|value_ref| serde_json::to_value(value_ref).ok()),
        PropertyType::ValidValues => parse_string_set(value).map(|values| json!(values)),
        PropertyType::SemanticLabel => parse_scalar_string(value)
            .map(|scalar| serde_json::Value::String(scalar.trim().to_string())),
        PropertyType::Liveness => {
            parse_scalar_string(value).map(|scalar| json!(normalize_string(&scalar)))
        }
    }
}

fn canonicalize_json_value(value: &serde_json::Value) -> serde_json::Value {
    match serde_json::from_str(&canonical_json(value)) {
        Ok(canonical) => canonical,
        Err(_) => value.clone(),
    }
}

fn parse_scalar_string(value: &serde_json::Value) -> Option<String> {
    let scalar: ScalarStringValue = serde_json::from_value(value.clone()).ok()?;
    if scalar.kind != "scalar" {
        return None;
    }

    Some(scalar.value)
}

fn parse_string_set(value: &serde_json::Value) -> Option<Vec<String>> {
    let string_set: StringSetValue = serde_json::from_value(value.clone()).ok()?;
    if string_set.kind != "string_set" {
        return None;
    }

    Some(sorted_set(&string_set.values))
}

fn is_dead_value(value: &serde_json::Value) -> bool {
    matches!(value, serde_json::Value::String(state) if state == "dead")
}

fn recommended_action_for_conflict(
    policy: &Policy,
    property_type: PropertyType,
) -> RecommendedAction {
    if policy.auto_resolves(property_type) {
        RecommendedAction::FixScanner
    } else if property_type == PropertyType::Liveness {
        RecommendedAction::ScanMore
    } else {
        RecommendedAction::Review
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScalarStringValue {
    kind: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StringSetValue {
    kind: String,
    values: Vec<String>,
}

#[cfg(test)]
mod tests {
    use indexmap::IndexMap;
    use serde_json::json;

    use super::Decision;
    use crate::bucket::BucketStore;
    use crate::contracts::canon_entry::{ConvergenceStateKind, ResolutionKind};
    use crate::contracts::claim::parse_claim;
    use crate::contracts::escalation::{
        CandidateValue, EscalationReason, RecommendedAction, ScalarCandidateKind,
        ScalarCandidateValue,
    };
    use crate::contracts::policy::Policy;
    use crate::contracts::vocabulary::{PropertyType, SubjectKind, SubjectRef};
    use crate::fixtures::{load_claim_fixture, load_policy_fixture};
    use crate::resolve::resolve_bucket;

    #[test]
    fn resolves_single_source_auto_resolve_bucket() {
        let mut store = BucketStore::default();
        store.insert(
            parse_claim(r#"{"event":"claim.v0","claim_id":"sha256:abababababababababababababababababababababababababababababababab","source":{"kind":"db_scan","scanner":"crucible.scan.db@0.1.0","artifact_id":"sha256:1616161616161616161616161616161616161616161616161616161616161616","locator":{"kind":"table_row","value":"ops.report_catalog#hyperion.close_pack_ebitda"}},"subject":{"kind":"report","id":"hyperion.close_pack_ebitda"},"property_type":"exists","value":true,"confidence":0.99}"#).unwrap(),
        );

        let policy = load_policy_fixture("legacy.decode.v0.json").unwrap();
        let bucket = store.buckets.values().next().unwrap();

        let decision = resolve_bucket(bucket, &policy);
        assert!(matches!(&decision, Decision::Resolved(_)));

        if let Decision::Resolved(entry) = decision {
            assert_eq!(entry.property_type, PropertyType::Exists);
            assert_eq!(entry.canonical_value, json!(true));
            assert_eq!(entry.convergence.state, ConvergenceStateKind::SingleSource);
            assert_eq!(entry.explain.resolution_kind, ResolutionKind::SingleSource);
            assert_eq!(
                entry.explain.winner_claim_ids,
                vec![
                    "sha256:abababababababababababababababababababababababababababababababab"
                        .to_string(),
                ]
            );
            assert_eq!(
                entry.explain.compatible_claim_ids,
                entry.explain.winner_claim_ids
            );
        }
    }

    #[test]
    fn resolves_corroborated_edge_bucket_from_fixture() {
        let store = mixed_source_store();
        let policy = load_policy_fixture("legacy.decode.v0.json").unwrap();
        let bucket = fixture_bucket(
            &store,
            "hyperion.close_pack_ebitda",
            PropertyType::DependsOn,
        );

        let decision = resolve_bucket(bucket, &policy);
        assert!(matches!(&decision, Decision::Resolved(_)));

        if let Decision::Resolved(entry) = decision {
            assert_eq!(entry.subject.kind, SubjectKind::Report);
            assert_eq!(
                entry.canonical_value,
                json!({"kind":"feed","id":"fdmee.actuals_load"})
            );
            assert_eq!(entry.convergence.state, ConvergenceStateKind::Converged);
            assert_eq!(entry.convergence.source_count, 3);
            assert_eq!(entry.convergence.claim_count, 3);
            assert_eq!(entry.explain.resolution_kind, ResolutionKind::Corroborated);
            assert_eq!(
                entry.explain.winner_claim_ids,
                vec![
                    "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                        .to_string(),
                    "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                        .to_string(),
                    "sha256:3333333333333333333333333333333333333333333333333333333333333333"
                        .to_string(),
                ]
            );
            assert_eq!(
                entry.explain.compatible_claim_ids,
                entry.explain.winner_claim_ids
            );
        }
    }

    #[test]
    fn resolves_compatible_liveness_with_fold() {
        let store = mixed_source_store();
        let policy = load_policy_fixture("legacy.decode.v0.json").unwrap();
        let bucket = fixture_bucket(&store, "fdmee.actuals_load", PropertyType::Liveness);

        let decision = resolve_bucket(bucket, &policy);
        assert!(matches!(&decision, Decision::Resolved(_)));

        if let Decision::Resolved(entry) = decision {
            assert_eq!(entry.subject.kind, SubjectKind::Feed);
            assert_eq!(entry.canonical_value, json!("alive"));
            assert_eq!(entry.convergence.state, ConvergenceStateKind::Converging);
            assert_eq!(entry.convergence.source_count, 3);
            assert_eq!(entry.convergence.claim_count, 3);
            assert_eq!(entry.explain.resolution_kind, ResolutionKind::LivenessFold);
            assert_eq!(
                entry.explain.winner_claim_ids,
                vec![
                    "sha256:4444444444444444444444444444444444444444444444444444444444444444"
                        .to_string(),
                    "sha256:6666666666666666666666666666666666666666666666666666666666666666"
                        .to_string(),
                ]
            );
            assert_eq!(
                entry.explain.compatible_claim_ids,
                vec![
                    "sha256:4444444444444444444444444444444444444444444444444444444444444444"
                        .to_string(),
                    "sha256:5555555555555555555555555555555555555555555555555555555555555555"
                        .to_string(),
                    "sha256:6666666666666666666666666666666666666666666666666666666666666666"
                        .to_string(),
                ]
            );
        }
    }

    #[test]
    fn escalates_conflicted_semantic_label_bucket() {
        let store = mixed_source_store();
        let policy = load_policy_fixture("legacy.decode.v0.json").unwrap();
        let bucket = fixture_bucket(
            &store,
            "adj.ebitda.rule.family",
            PropertyType::SemanticLabel,
        );

        let decision = resolve_bucket(bucket, &policy);
        assert!(matches!(&decision, Decision::Escalated(_)));

        if let Decision::Escalated(escalation) = decision {
            assert_eq!(escalation.reason, EscalationReason::Conflicted);
            assert_eq!(escalation.recommended_action, RecommendedAction::Review);
            assert_eq!(
                escalation.claim_ids,
                vec![
                    "sha256:7777777777777777777777777777777777777777777777777777777777777777"
                        .to_string(),
                    "sha256:8888888888888888888888888888888888888888888888888888888888888888"
                        .to_string(),
                ]
            );
            assert_eq!(
                escalation.candidate_values,
                vec![
                    CandidateValue::Scalar(ScalarCandidateValue {
                        kind: ScalarCandidateKind::Scalar,
                        value: json!("Adjusted EBITDA rule family"),
                    }),
                    CandidateValue::Scalar(ScalarCandidateValue {
                        kind: ScalarCandidateKind::Scalar,
                        value: json!("EBITDA exception class"),
                    }),
                ]
            );
            assert_eq!(escalation.summary, "2 incompatible candidate values remain");
        }
    }

    #[test]
    fn escalates_missing_corroboration_for_single_source_valid_values() {
        let store = mixed_source_store();
        let policy = load_policy_fixture("legacy.decode.v0.json").unwrap();
        let bucket = fixture_bucket(&store, "adj.status_code_map", PropertyType::ValidValues);

        let decision = resolve_bucket(bucket, &policy);
        assert!(matches!(&decision, Decision::Escalated(_)));

        if let Decision::Escalated(escalation) = decision {
            assert_eq!(escalation.reason, EscalationReason::MissingCorroboration);
            assert_eq!(escalation.recommended_action, RecommendedAction::ScanMore);
            assert_eq!(
                escalation.candidate_values,
                vec![CandidateValue::Scalar(ScalarCandidateValue {
                    kind: ScalarCandidateKind::Scalar,
                    value: json!(["CLOSED", "HOLD", "OPEN"]),
                })]
            );
            assert_eq!(
                escalation.summary,
                "need at least 2 corroborating sources, found 1"
            );
        }
    }

    #[test]
    fn escalates_when_policy_has_no_resolution_path() {
        let store = mixed_source_store();
        let bucket = fixture_bucket(&store, "adj.status_code_map", PropertyType::ValidValues);
        let policy = Policy {
            policy_id: "legacy.decode.v0".to_string(),
            auto_resolve: vec![],
            min_corroboration: IndexMap::new(),
            source_priority: IndexMap::new(),
        };

        let decision = resolve_bucket(bucket, &policy);
        assert!(matches!(&decision, Decision::Escalated(_)));

        if let Decision::Escalated(escalation) = decision {
            assert_eq!(escalation.reason, EscalationReason::NoResolutionPath);
            assert_eq!(escalation.recommended_action, RecommendedAction::FixPolicy);
            assert_eq!(
                escalation.summary,
                "policy does not define a resolution path for valid_values"
            );
        }
    }

    fn mixed_source_store() -> BucketStore {
        let mut store = BucketStore::default();
        for claim in load_claim_fixture("mixed_source.jsonl").unwrap() {
            store.insert(claim);
        }
        store
    }

    fn fixture_bucket<'a>(
        store: &'a BucketStore,
        subject_id: &str,
        property_type: PropertyType,
    ) -> &'a crate::bucket::Bucket {
        store
            .buckets
            .values()
            .find(|bucket| {
                bucket.key.subject()
                    == &SubjectRef {
                        kind: bucket.key.subject().kind,
                        id: subject_id.to_string(),
                    }
                    && bucket.key.property_type() == property_type
            })
            .unwrap()
    }
}
