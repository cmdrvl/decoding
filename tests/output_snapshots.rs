mod fixture_support;

use decoding::bucket::BucketStore;
use decoding::render::{write_canon_entry, write_escalation};
use decoding::report::generate_report;
use decoding::resolve::{Decision, resolve_bucket};
use insta::{assert_json_snapshot, assert_snapshot};

fn mixed_source_outputs() -> (String, String, serde_json::Value) {
    let _ = fixture_support::fixtures_root();
    let _ = fixture_support::claim_fixture_path("mixed_source.jsonl");
    let _ = fixture_support::policy_fixture_path("legacy.decode.v0.json");
    let _ = fixture_support::expected_output_path("archaeology");

    let claims = fixture_support::load_claim_fixture("mixed_source.jsonl").unwrap();
    let policy = fixture_support::load_policy_fixture("legacy.decode.v0.json").unwrap();

    let mut store = BucketStore::default();
    for claim in claims {
        store.insert(claim);
    }

    let decisions = store
        .buckets
        .values()
        .map(|bucket| resolve_bucket(bucket, &policy))
        .collect::<Vec<_>>();

    let mut canon_entries = Vec::new();
    let mut escalations = Vec::new();

    for decision in &decisions {
        match decision {
            Decision::Resolved(resolved) => {
                write_canon_entry(&mut canon_entries, &resolved.entry).unwrap();
            }
            Decision::Escalated(escalated) => {
                write_escalation(&mut escalations, &escalated.escalation).unwrap();
            }
        }
    }

    let report = generate_report(&policy.policy_id, &decisions);

    (
        String::from_utf8(canon_entries).unwrap(),
        String::from_utf8(escalations).unwrap(),
        serde_json::to_value(report).unwrap(),
    )
}

#[test]
fn canon_entry_output_matches_snapshot() {
    let (canon_entries, _, _) = mixed_source_outputs();
    assert_snapshot!("canon_entry_v0_mixed_source", canon_entries);
}

#[test]
fn escalation_output_matches_snapshot() {
    let (_, escalations, _) = mixed_source_outputs();
    assert_snapshot!("escalation_v0_mixed_source", escalations);
}

#[test]
fn convergence_output_matches_snapshot() {
    let (_, _, report) = mixed_source_outputs();
    assert_json_snapshot!("convergence_v0_mixed_source", report);
}
