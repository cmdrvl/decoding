use std::{fs, process::Command};

use clap::CommandFactory;
use decoding::{OPERATOR_JSON, OUTPUT_SCHEMA, cli::Cli};
use serde_json::Value;
use spine_rules::contract::OperatorContract;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_decoding"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn describe_is_verbatim_and_uses_shared_operator_types() {
    let output = run(&["--describe"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, OPERATOR_JSON.as_bytes());
    let manifest: OperatorContract = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(manifest.schema_version, "operator.v1");
    assert_eq!(manifest.name, "decoding");
    assert_eq!(manifest.tool_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest.exit_codes["1"].domain, "negative");
    assert_eq!(
        manifest.extra["capabilities"]["witness"]["ambient_recording"],
        "not_applicable"
    );
}

#[test]
fn manifest_subcommands_and_options_exist_in_clap() {
    let manifest: OperatorContract = serde_json::from_str(OPERATOR_JSON).unwrap();
    let cli = Cli::command();
    for subcommand in manifest.commands {
        let mut command = &cli;
        for name in subcommand.name.split_whitespace() {
            let next = command.find_subcommand(name);
            assert!(
                next.is_some(),
                "manifest command {} missing in clap",
                subcommand.name
            );
            command = next.unwrap_or(command);
        }
        for option in subcommand.extra["options"].as_array().unwrap() {
            let flag = option["flag"].as_str().unwrap().trim_start_matches("--");
            assert!(
                command
                    .get_arguments()
                    .any(|arg| arg.get_long() == Some(flag))
            );
        }
        for argument in subcommand.extra["arguments"].as_array().unwrap() {
            let name = argument["name"].as_str().unwrap();
            assert!(command.get_arguments().any(|arg| arg.get_id() == name));
        }
    }
}

#[test]
fn schema_is_embedded_and_describes_the_stdout_contract() {
    let output = run(&["--schema"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, OUTPUT_SCHEMA.as_bytes());
    let schema: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(schema["properties"]["event"]["const"], "canon_entry.v0");
    assert_eq!(schema["additionalProperties"], false);
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&Value::from("explain"))
    );
}

#[test]
fn discovery_aliases_are_identical_and_need_no_domain_inputs() {
    let top = run(&["capabilities", "--json"]);
    let nested = run(&["doctor", "capabilities", "--json"]);
    assert_eq!(top.status.code(), Some(0));
    assert!(top.stderr.is_empty());
    assert_eq!(top.stdout, OPERATOR_JSON.as_bytes());
    assert_eq!(nested.status.code(), Some(0));
    let capabilities: Value = serde_json::from_slice(&top.stdout).unwrap();
    let doctor: Value = serde_json::from_slice(&nested.stdout).unwrap();
    assert_eq!(doctor["schema_version"], "cmdrvl.doctor.v1");
    assert_eq!(doctor["mode"], "capabilities");
    assert_eq!(doctor["contract"], capabilities);

    let top = run(&["robot-docs", "guide"]);
    assert_eq!(top.status.code(), Some(0));
    assert!(top.stderr.is_empty());
    assert_eq!(top.stdout, run(&["doctor", "robot-docs", "guide"]).stdout);
    assert_eq!(top.stdout, run(&["doctor", "robot-docs"]).stdout);
    let guide = String::from_utf8(top.stdout).unwrap();
    let manifest: Value = serde_json::from_str(OPERATOR_JSON).unwrap();
    for refusal in manifest["refusals"].as_array().unwrap() {
        assert!(guide.contains(refusal["code"].as_str().unwrap()));
    }
}

#[test]
fn doctor_reports_use_v1_and_legacy_triage_alias_matches() {
    let bare_health = run(&["doctor", "--json"]);
    let nested_health = run(&["doctor", "health", "--json"]);
    assert_eq!(bare_health.status.code(), Some(0));
    assert_eq!(bare_health.stdout, nested_health.stdout);

    let canonical = run(&["doctor", "triage", "--json"]);
    assert_eq!(canonical.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&canonical.stdout).unwrap();
    assert_eq!(report["schema_version"], "cmdrvl.doctor.v1");
    assert_eq!(report["mode"], "triage");
    assert!(!report["next_actions"].as_array().unwrap().is_empty());
    assert!(
        report.get("schema").is_none(),
        "legacy doctor schema leaked"
    );

    let legacy = run(&["--robot-triage"]);
    let nested_legacy = run(&["doctor", "--robot-triage"]);
    assert_eq!(legacy.status.code(), Some(0));
    assert_eq!(legacy.stdout, canonical.stdout);
    assert_eq!(nested_legacy.stdout, canonical.stdout);
}

#[test]
fn invalid_claims_and_policies_have_stable_stderr_codes_before_writes() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("canon.jsonl");
    let invalid_policy = dir.path().join("policy.json");
    fs::write(&invalid_policy, "{\"unknown_policy_key\":true}").unwrap();
    for (claims, policy, code) in [
        (
            "tests/fixtures/claims/refusal_invalid.jsonl",
            "tests/fixtures/policies/legacy.decode.v0.json",
            "E_CLAIM_INVALID",
        ),
        (
            "tests/fixtures/claims/mixed_source.jsonl",
            invalid_policy.to_str().unwrap(),
            "E_POLICY_INVALID",
        ),
    ] {
        let output = run(&[
            "archaeology",
            claims,
            "--policy",
            policy,
            "--output",
            destination.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let status: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(status["status"], "refusal");
        assert_eq!(status["refusal"]["code"], code);
        assert!(status["refusal"]["detail"].as_str().is_some());
        assert_eq!(status["version"], env!("CARGO_PKG_VERSION"));
        assert!(!destination.exists());
    }
}

#[test]
fn operational_errors_are_distinct_from_refusals() {
    let output = run(&[
        "archaeology",
        "missing-claims.jsonl",
        "--policy",
        "tests/fixtures/policies/legacy.decode.v0.json",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let status: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(status["status"], "error");
    assert_eq!(status["error"]["code"], "E_IO");
    assert!(status.get("refusal").is_none());
}

#[test]
fn json_status_does_not_replace_canonical_jsonl() {
    let output = run(&[
        "archaeology",
        "tests/fixtures/claims/mixed_source.jsonl",
        "--policy",
        "tests/fixtures/policies/legacy.decode.v0.json",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.is_empty());
    for line in stdout.lines() {
        let entry: Value = serde_json::from_str(line).unwrap();
        assert_eq!(entry["event"], "canon_entry.v0");
    }
    let status: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(status["status"], "escalations");
}
