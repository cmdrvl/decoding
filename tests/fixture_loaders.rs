mod fixture_support;

use std::path::Path;

use decoding::contracts::vocabulary::{PropertyType, SubjectKind};

use crate::fixture_support::{
    claim_fixture_path, expected_output_path, load_claim_fixture, load_claim_fixture_lines,
    load_policy_fixture, policy_fixture_path,
};

#[test]
fn loads_shared_claim_fixture_lines() {
    let path = claim_fixture_path("mixed_source.jsonl");
    let lines = load_claim_fixture_lines("mixed_source.jsonl").unwrap();

    assert!(path.exists());
    assert!(path.ends_with(Path::new("tests/fixtures/claims/mixed_source.jsonl")));
    assert!(!lines.is_empty());
}

#[test]
fn parses_shared_claim_fixture_into_typed_claims() {
    let claims = load_claim_fixture("mixed_source.jsonl").unwrap();

    assert!(!claims.is_empty());
    assert_eq!(claims[0].event, "claim.v0");
    assert_eq!(claims[0].subject.kind, SubjectKind::Report);
    assert_eq!(claims[0].property_type, PropertyType::DependsOn);
}

#[test]
fn reports_line_numbers_for_invalid_claim_fixtures() {
    let error = load_claim_fixture("refusal_invalid.jsonl").unwrap_err();
    let rendered = error.to_string();

    assert!(rendered.contains("line 1"));
    assert!(rendered.contains("claim refusal"));
}

#[test]
fn loads_shared_policy_fixture() {
    let path = policy_fixture_path("legacy.decode.v0.json");
    let policy = load_policy_fixture("legacy.decode.v0.json").unwrap();

    assert!(path.exists());
    assert_eq!(policy.policy_id, "legacy.decode.v0");
    assert!(policy.auto_resolves(PropertyType::Exists));
    assert_eq!(
        policy.corroboration_threshold(PropertyType::SemanticLabel),
        Some(2)
    );
    assert_eq!(
        policy
            .source_priority_for(PropertyType::Liveness)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn expected_output_paths_are_located_under_shared_root() {
    let path = expected_output_path("archaeology/mixed_source.canon.jsonl");

    assert_eq!(
        path,
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("expected")
            .join("archaeology/mixed_source.canon.jsonl")
    );
}
