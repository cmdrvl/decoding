use serde_json::Value;
use std::{fs, process::Command};

fn run(
    left: &str,
    left_scale: &str,
    right: &str,
    right_scale: &str,
    tolerance: &str,
    inference: &str,
) -> (i32, String, String, String) {
    let dir = tempfile::tempdir().unwrap();
    let claims: String = [(1, left, left_scale), (2, right, right_scale)].into_iter().map(|(id, value, scale)| {
        format!(r#"{{"event":"claim.v0","claim_id":"sha256:{id:064x}","source":{{"kind":"parser_extraction","scanner":"numeric-test","artifact_id":"source-{id}","locator":{{"kind":"fixture","value":"{id}"}}}},"subject":{{"kind":"fund","id":"numeric-test"}},"property_type":"numeric_scalar","value":{{"kind":"numeric_scalar","value":{value},"scale":"{scale}"}},"confidence":0.9}}
"#)
    }).collect();
    fs::write(dir.path().join("claims.jsonl"), claims).unwrap();
    fs::write(dir.path().join("policy.json"), format!(r#"{{"policy_id":"legacy.decode.v0","auto_resolve":[],"min_corroboration":{{"numeric_scalar":2}},"source_priority":{{}},"numeric_tolerance":{{"numeric_scalar":{tolerance}}},"numeric_scale_inference":{inference}}}"#)).unwrap();
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

fn exact(left: &str, right: &str) -> (i32, String, String, String) {
    run(
        left,
        "dollars",
        right,
        "dollars",
        r#"{"absolute":0}"#,
        "null",
    )
}

#[test]
fn distinct_values_have_no_implicit_float_epsilon() {
    for (left, right) in [
        ("9007199254740992", "9007199254740993"),
        ("100", "100.0000000000000001"),
        ("0", "1e-100"),
        ("-9007199254740992", "-9007199254740993"),
    ] {
        let (code, canonical, escalation, diagnostic) = exact(left, right);
        assert_eq!(code, 1, "{left} vs {right}: {diagnostic}");
        assert!(canonical.is_empty());
        assert!(escalation.contains("conflicted"));
    }
}

#[test]
fn canonical_output_preserves_all_decimal_digits() {
    let amount = "9007199254740993.1234567890123456789";
    let (code, canonical, _, diagnostic) = exact(amount, amount);
    assert_eq!(code, 0, "{diagnostic}");
    let value: Value = serde_json::from_str(&canonical).unwrap();
    assert_eq!(value["canonical_value"]["value"].to_string(), amount);
}

#[test]
fn scale_normalization_is_exact_including_negative_holdings() {
    for (left, right) in [
        ("0.1", "100000"),
        ("-0.10000000000000000001", "-100000.00000000000001"),
    ] {
        let (code, _, _, diagnostic) = run(
            left,
            "millions",
            right,
            "dollars",
            r#"{"absolute":0}"#,
            "null",
        );
        assert_eq!(code, 0, "{diagnostic}");
    }
}

#[test]
fn absolute_tolerance_boundary_is_exact() {
    assert_eq!(
        run(
            "1",
            "dollars",
            "1.01",
            "dollars",
            r#"{"absolute":0.01}"#,
            "null"
        )
        .0,
        0
    );
    assert_eq!(
        run(
            "1",
            "dollars",
            "1.01000000000000000001",
            "dollars",
            r#"{"absolute":0.01}"#,
            "null"
        )
        .0,
        1
    );
    assert_eq!(
        run(
            "1",
            "dollars",
            "1.01000000000000000001",
            "dollars",
            r#"{"absolute":0.01000000000000000001}"#,
            "null"
        )
        .0,
        0
    );
}

#[test]
fn relative_tolerance_uses_exact_cross_multiplication() {
    assert_eq!(
        run(
            "99",
            "dollars",
            "100",
            "dollars",
            r#"{"relative_percent":1}"#,
            "null"
        )
        .0,
        0
    );
    assert_eq!(
        run(
            "98.99999999999999999999",
            "dollars",
            "100",
            "dollars",
            r#"{"relative_percent":1}"#,
            "null"
        )
        .0,
        1
    );
    assert_eq!(
        run(
            "0",
            "dollars",
            "0",
            "dollars",
            r#"{"relative_percent":0}"#,
            "null"
        )
        .0,
        0
    );
}

#[test]
fn declared_legacy_scale_inference_uses_exact_factors() {
    let policy = r#"{"factors":[1000,1000.0],"max_relative_after":0}"#;
    let (code, canonical, _, diagnostic) = run(
        "0.1",
        "unknown",
        "100",
        "dollars",
        r#"{"absolute":0}"#,
        policy,
    );
    assert_eq!(code, 0, "{diagnostic}");
    assert!(canonical.contains("scale_inferred"));
    assert_eq!(
        run(
            "0.10000000000000000001",
            "unknown",
            "100",
            "dollars",
            r#"{"absolute":0}"#,
            policy
        )
        .0,
        1
    );
    let bad = r#"{"factors":[1000.00000000000000000001],"max_relative_after":0}"#;
    assert_eq!(
        run("0.1", "unknown", "100", "dollars", r#"{"absolute":0}"#, bad).0,
        2
    );
}

#[test]
fn out_of_bounds_or_non_numeric_values_refuse_instead_of_rounding() {
    for value in ["1e1025", "1e-1025", "\"0.1\"", "1e99999999999999999999"] {
        let (code, canonical, escalation, _) = exact(value, value);
        assert_eq!(code, 2, "{value}");
        assert!(canonical.is_empty());
        assert!(escalation.is_empty());
    }
    assert_eq!(
        run(
            "1e1024",
            "millions",
            "1",
            "dollars",
            r#"{"absolute":0}"#,
            "null"
        )
        .0,
        2
    );
    assert_eq!(
        run(
            "1",
            "dollars",
            "1",
            "dollars",
            r#"{"absolute":1e1025}"#,
            "null"
        )
        .0,
        2
    );
}

#[test]
fn equivalent_decimal_spellings_replay_with_same_canonical_value() {
    let first = exact("1.2300", "123e-2");
    let second = exact("123e-2", "1.2300");
    assert_eq!(first.0, 0);
    assert_eq!(first.1, second.1);
}

#[test]
fn accepted_resource_boundaries_round_trip_without_precision_loss() {
    for value in ["1e1024".to_owned(), "1e-1024".to_owned(), "9".repeat(1024)] {
        let (code, canonical, _, diagnostic) = exact(&value, &value);
        assert_eq!(code, 0, "{diagnostic}");
        let report: Value = serde_json::from_str(&canonical).unwrap();
        let output = report["canonical_value"]["value"].to_string();
        assert_eq!(
            output.parse::<bigdecimal::BigDecimal>().unwrap(),
            value.parse::<bigdecimal::BigDecimal>().unwrap()
        );
    }
    assert_eq!(exact(&"9".repeat(1025), "1").0, 2);
}
