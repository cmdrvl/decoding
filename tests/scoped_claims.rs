use decoding::contracts::claim::parse_claim;
use serde_json::{Value, json};
use std::{fs, process::Command};

fn claim(id: usize, origins: &[&str]) -> Value {
    json!({
        "event":"claim.v0", "claim_id":format!("sha256:{id:064x}"),
        "source":{"kind":"parser_extraction","scanner":"scoped-fixture",
            "artifact_id":format!("sha256:{:064x}", id + 100),
            "locator":{"kind":"fixture","value":id.to_string()}},
        "subject":{"kind":"fund","id":"test-fund"},
        "property_type":"numeric_scalar",
        "value":{"kind":"numeric_scalar","value":100,"scale":"dollars"},
        "confidence":0.9,
        "context":{"accession":"filing-a","document":"document-a","amendment":"original",
            "asof":"2026-03-31","currency":"USD","unit":"dollars",
            "scope":"consolidated","grain":"position:row-1"},
        "ancestry":{"status":"known","origin_source_ids":origins}
    })
}

fn run(claims: &[Value]) -> (i32, String, String, String) {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("claims.jsonl"),
        claims.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    fs::write(
        dir.path().join("policy.json"),
        json!({
            "policy_id":"legacy.decode.v0","auto_resolve":[],
            "min_corroboration":{"numeric_scalar":2},"source_priority":{},
            "numeric_tolerance":{"numeric_scalar":{"absolute":0}}
        })
        .to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_decoding"))
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
        .output()
        .unwrap();
    (
        output.status.code().unwrap(),
        fs::read_to_string(dir.path().join("canon.jsonl")).unwrap_or_default(),
        fs::read_to_string(dir.path().join("escalations.jsonl")).unwrap_or_default(),
        String::from_utf8(output.stderr).unwrap(),
    )
}

#[test]
fn independent_origins_converge_and_retain_exact_context() {
    let first = claim(1, &["issuer-filing"]);
    let (code, canonical, escalations, _) =
        run(&[first.clone(), claim(2, &["independent-document"])]);
    assert_eq!(code, 0);
    assert!(escalations.is_empty());
    let value: Value = serde_json::from_str(&canonical).unwrap();
    assert_eq!(value["context"], first["context"]);
    assert_eq!(value["convergence"]["source_count"], 2);
}

#[test]
fn five_copies_of_one_source_do_not_earn_corroboration() {
    let claims: Vec<_> = (1..=5).map(|i| claim(i, &["filing-a"])).collect();
    let (code, canonical, escalations, _) = run(&claims);
    assert_eq!(code, 1);
    assert!(canonical.is_empty());
    let value: Value = serde_json::from_str(&escalations).unwrap();
    assert_eq!(value["reason"], "missing_corroboration");
    assert_eq!(value["claim_ids"].as_array().unwrap().len(), 5);
    assert_eq!(value["context"], claims[0]["context"]);
}

#[test]
fn overlapping_derivations_are_transitively_dependent() {
    let claims = [claim(1, &["a"]), claim(2, &["b"]), claim(3, &["a", "b"])];
    let (code, canonical, escalations, _) = run(&claims);
    assert_eq!(code, 1);
    assert!(canonical.is_empty());
    assert!(escalations.contains("missing_corroboration"));
    let (code, canonical, _, _) = run(&[
        claims[0].clone(),
        claims[1].clone(),
        claims[2].clone(),
        claim(4, &["c"]),
    ]);
    assert_eq!(code, 0);
    let value: Value = serde_json::from_str(&canonical).unwrap();
    assert_eq!(value["convergence"]["source_count"], 2);
}

#[test]
fn unknown_ancestry_earns_no_independent_credit() {
    let mut unknown = claim(2, &["b"]);
    unknown["ancestry"]["status"] = json!("unknown");
    let (code, canonical, escalations, _) = run(&[claim(1, &["a"]), unknown]);
    assert_eq!(code, 1);
    assert!(canonical.is_empty());
    assert!(escalations.contains("missing_corroboration"));
}

#[test]
fn every_context_dimension_separates_equal_values() {
    for field in [
        "accession",
        "document",
        "amendment",
        "asof",
        "currency",
        "unit",
        "scope",
        "grain",
    ] {
        let first = claim(1, &["a"]);
        let mut second = claim(2, &["b"]);
        second["context"][field] = json!("other");
        let (code, canonical, escalations, _) = run(&[first, second]);
        assert_eq!(code, 1, "{field}");
        assert!(canonical.is_empty(), "{field}");
        let rows: Vec<Value> = escalations
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(rows.len(), 2, "{field}");
        assert_ne!(rows[0]["bucket_id"], rows[1]["bucket_id"], "{field}");
    }
}

#[test]
fn scoped_and_legacy_claims_do_not_mix() {
    let mut legacy = claim(2, &["b"]);
    legacy.as_object_mut().unwrap().remove("context");
    legacy.as_object_mut().unwrap().remove("ancestry");
    let (code, canonical, escalations, _) = run(&[claim(1, &["a"]), legacy]);
    assert_eq!(code, 1);
    assert!(canonical.is_empty());
    assert_eq!(escalations.lines().count(), 2);
}

#[test]
fn conflict_cannot_be_outvoted_by_copies_or_independent_agreement() {
    let mut claims: Vec<_> = (1..=5).map(|i| claim(i, &["a"])).collect();
    claims.push(claim(6, &["b"]));
    let mut conflict = claim(7, &["c"]);
    conflict["value"]["value"] = json!(200);
    claims.push(conflict);
    let (code, canonical, escalations, _) = run(&claims);
    assert_eq!(code, 1);
    assert!(canonical.is_empty());
    assert!(escalations.contains("conflicted"));
}

#[test]
fn duplicate_and_reordered_claims_replay_identically() {
    let a = claim(1, &["a"]);
    let b = claim(2, &["b"]);
    let mut other = claim(3, &["c"]);
    other["context"]["asof"] = json!("2025-12-31");
    let mut fourth = claim(4, &["d"]);
    fourth["context"] = other["context"].clone();
    let first = run(&[a.clone(), b.clone(), other.clone(), fourth.clone()]);
    let second = run(&[fourth, other, b, a.clone(), a]);
    assert_eq!(first.0, second.0);
    assert_eq!(first.1, second.1);
    assert_eq!(first.2, second.2);
}

#[test]
fn absent_unit_anchor_is_not_inferred_from_agreement() {
    let mut first = claim(1, &["a"]);
    let mut second = claim(2, &["b"]);
    first["value"]["scale"] = json!("unknown");
    second["value"]["scale"] = json!("unknown");
    let (code, canonical, escalations, _) = run(&[first, second]);
    assert_eq!(code, 1);
    assert!(canonical.is_empty());
    assert!(escalations.contains("no_resolution_path"));
}

#[test]
fn malformed_scope_and_ancestry_are_refused() {
    let mut variants = Vec::new();
    let mut unknown = claim(1, &["a"]);
    unknown["ancestry"]["status"] = json!("guessed");
    variants.push(unknown);
    let mut empty = claim(1, &[]);
    variants.push(empty.clone());
    empty["ancestry"]["status"] = json!("unknown");
    assert!(parse_claim(&empty.to_string()).is_ok());
    let mut missing = claim(1, &["a"]);
    missing.as_object_mut().unwrap().remove("ancestry");
    variants.push(missing);
    let mut blank = claim(1, &["a"]);
    blank["context"]["asof"] = json!(" ");
    variants.push(blank);
    let mut extra = claim(1, &["a"]);
    extra["ancestry"]["trust_me"] = json!(true);
    variants.push(extra);
    variants.push(claim(1, &["a", "a"]));
    for value in variants {
        let (code, canonical, escalations, _) = run(&[value]);
        assert_eq!(code, 2);
        assert!(canonical.is_empty());
        assert!(escalations.is_empty());
    }
}
