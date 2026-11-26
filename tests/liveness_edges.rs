use serde_json::{Value, json};
use std::fs;
use std::panic::panic_any;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const SEED: u64 = 0x4445_434f_4449_4e47;

fn liveness_claim(
    id: usize,
    source_kind: &str,
    origins: &[&str],
    value: &str,
    ancestry_status: &str,
) -> Value {
    json!({
        "event": "claim.v0",
        "claim_id": format!("sha256:{id:064x}"),
        "source": {
            "kind": source_kind,
            "scanner": "liveness-edges",
            "artifact_id": format!("sha256:{:064x}", id + 10_000),
            "locator": {"kind": "fixture", "value": id.to_string()}
        },
        "subject": {"kind": "feed", "id": "seeded-feed"},
        "property_type": "liveness",
        "value": {"kind": "scalar", "value": value},
        "confidence": 0.9,
        "context": {
            "accession": "seeded-accession",
            "document": "seeded-document",
            "amendment": "original",
            "asof": "2026-03-31",
            "currency": "USD",
            "unit": "dollars",
            "scope": "consolidated",
            "grain": "feed"
        },
        "ancestry": {"status": ancestry_status, "origin_source_ids": origins}
    })
}

fn run_with_timeout(claims: &[Value], seed: u64) -> (i32, String, String, String) {
    let dir = tempfile::tempdir().expect("seeded liveness workspace should be creatable");
    fs::write(
        dir.path().join("claims.jsonl"),
        claims
            .iter()
            .map(|claim| format!("{claim}\n"))
            .collect::<String>(),
    )
    .expect("claims fixture should be writable");
    fs::write(
        dir.path().join("policy.json"),
        json!({
            "policy_id": "legacy.decode.v0",
            "auto_resolve": [],
            "min_corroboration": {},
            "source_priority": {"liveness": ["db_scan", "repo_scan"]},
            "numeric_tolerance": {}
        })
        .to_string(),
    )
    .expect("policy fixture should be writable");

    let mut child = Command::new(env!("CARGO_BIN_EXE_decoding"))
        .current_dir(dir.path())
        .args([
            "archaeology",
            "claims.jsonl",
            "--policy",
            "policy.json",
            "--output",
            "canon.jsonl",
            "--escalations",
            "escalations.jsonl",
            "--convergence",
            "convergence.json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| {
            panic_any(format!("seed {seed:#x}: failed to start decoding: {error}"))
        });
    let deadline = Instant::now() + Duration::from_secs(5);
    let output = loop {
        match child.try_wait() {
            Ok(Some(_)) => break child.wait_with_output().unwrap(),
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                child.kill().unwrap_or_else(|error| {
                    panic_any(format!("seed {seed:#x}: kill child: {error}"))
                });
                let _ = child.wait();
                panic_any(format!(
                    "seed {seed:#x}: decoding child exceeded five-second timeout"
                ));
            }
            Err(error) => panic_any(format!("seed {seed:#x}: poll decoding child: {error}")),
        }
    };
    let status = output.status.code().unwrap_or(-1);
    let canon = fs::read_to_string(dir.path().join("canon.jsonl")).unwrap_or_default();
    let escalations = fs::read_to_string(dir.path().join("escalations.jsonl")).unwrap_or_default();
    (
        status,
        canon,
        escalations,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn assert_case(
    claims: &[Value],
    expected_status: i32,
    expected_reason: Option<&str>,
    expected_sources: Option<usize>,
    expected_value: Option<&str>,
    expected_resolution: Option<&str>,
    seed: u64,
) -> (String, String) {
    let (status, canon, escalations, stderr) = run_with_timeout(claims, seed);
    assert_eq!(
        status, expected_status,
        "seed {seed:#x}: unexpected exit; canon={canon:?}, escalations={escalations:?}, stderr={stderr:?}"
    );
    if expected_status == 0 {
        assert!(
            escalations.is_empty(),
            "seed {seed:#x}: unexpected escalation"
        );
        let entry: Value = serde_json::from_str(canon.trim()).unwrap_or_else(|error| {
            panic_any(format!("seed {seed:#x}: invalid canon JSON: {error}"))
        });
        if let Some(source_count) = expected_sources {
            assert_eq!(entry["convergence"]["source_count"], source_count);
        }
        if let Some(value) = expected_value {
            assert_eq!(entry["canonical_value"], value);
        }
        if let Some(resolution) = expected_resolution {
            assert_eq!(entry["explain"]["resolution_kind"], resolution);
        }
    } else {
        assert!(
            canon.is_empty(),
            "seed {seed:#x}: refusal/escalation wrote canon"
        );
        let escalation: Value = serde_json::from_str(escalations.trim()).unwrap_or_else(|error| {
            panic_any(format!("seed {seed:#x}: invalid escalation JSON: {error}"))
        });
        assert_eq!(escalation["reason"], expected_reason.unwrap());
        if expected_reason == Some("missing_corroboration") {
            assert_eq!(
                escalation["summary"],
                "need at least 2 corroborating sources, found 1"
            );
        } else {
            assert_eq!(
                escalation["summary"],
                "2 incompatible candidate values remain"
            );
        }
    }
    (canon, escalations)
}

#[test]
fn seeded_liveness_channels_are_order_invariant_and_fail_closed() {
    // Invariant: ancestry origin components, not source kinds or artifact
    // counts, define independent corroboration. Oracle: disjoint origin sets
    // count as independent; overlapping sets and unknown ancestry count as one
    // component. A fixed seed identifies every generated permutation.
    for case in 0..32usize {
        let conflict = case % 3 != 0;
        let (first_origins, second_origins, second_status, source_count) = match case % 4 {
            0 => (&["origin-a"][..], &["origin-a"][..], "known", 1),
            1 => (&["origin-a"][..], &["origin-b"][..], "known", 2),
            2 => (&["origin-a", "origin-b"][..], &["origin-b"][..], "known", 1),
            _ => (&["origin-a"][..], &["origin-b"][..], "unknown", 1),
        };
        let first_value = "alive";
        let second_value = if conflict { "dead" } else { "alive" };
        let first = liveness_claim(case * 2 + 1, "db_scan", first_origins, first_value, "known");
        let second = liveness_claim(
            case * 2 + 2,
            "repo_scan",
            second_origins,
            second_value,
            second_status,
        );
        // Invariant: a true alive/dead conflict remains a conflict even when
        // origins are independent; source priority is never a conflict vote.
        let expected_status = if conflict { 1 } else { 0 };
        let expected_reason = conflict.then_some("conflicted");
        let expected_resolution = if source_count == 2 {
            Some("corroborated")
        } else {
            Some("single_source")
        };
        let expected_value = Some("alive");

        let original = assert_case(
            &[first.clone(), second.clone()],
            expected_status,
            expected_reason,
            (expected_status == 0).then_some(source_count),
            expected_value,
            expected_resolution,
            SEED ^ case as u64,
        );
        let reversed = assert_case(
            &[second, first],
            expected_status,
            expected_reason,
            (expected_status == 0).then_some(source_count),
            expected_value,
            expected_resolution,
            SEED ^ case as u64,
        );
        assert_eq!(
            original, reversed,
            "seed {SEED:#x}, case {case}: claim order changed the result"
        );
    }
}

#[test]
fn dead_liveness_requires_two_independent_origins() {
    // Invariant: a dead liveness verdict is never accepted from one source
    // channel, but identical dead claims from two disjoint origins may
    // converge. Oracle: the same source-component count controls exit 1 vs 0.
    let one = liveness_claim(10_001, "db_scan", &["only-origin"], "dead", "known");
    let two = liveness_claim(10_002, "repo_scan", &["second-origin"], "dead", "known");

    let (_, _, escalations, _) = run_with_timeout(std::slice::from_ref(&one), SEED ^ 0xdead);
    let escalation: Value =
        serde_json::from_str(escalations.trim()).expect("single dead claim should escalate");
    assert_eq!(escalation["reason"], "missing_corroboration");

    let (status, canon, escalations, stderr) = run_with_timeout(&[one, two], SEED ^ 0xbeef);
    assert_eq!(status, 0, "independent dead claims: {stderr}");
    assert!(escalations.is_empty());
    let entry: Value = serde_json::from_str(canon.trim()).expect("dead canon should be JSON");
    assert_eq!(entry["canonical_value"], "dead");
    assert_eq!(entry["convergence"]["source_count"], 2);
    assert_eq!(entry["explain"]["resolution_kind"], "corroborated");
}
