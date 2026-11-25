use std::{fs, path::Path};

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;

fn decoding() -> Result<Command, Box<dyn std::error::Error>> {
    Ok(Command::cargo_bin("decoding")?)
}

fn stdout_json(args: &[&str]) -> Result<Value, Box<dyn std::error::Error>> {
    let output = decoding()?
        .args(args)
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();

    Ok(serde_json::from_slice(&output)?)
}

#[test]
fn doctor_health_json_is_read_only() -> Result<(), Box<dyn std::error::Error>> {
    let report = stdout_json(&["doctor", "health", "--json"])?;

    assert_eq!(report["schema_version"], "cmdrvl.doctor.v1");
    assert_eq!(report["mode"], "health");
    assert_eq!(report["healthy"], true);
    assert_eq!(report["findings"].as_array().unwrap().len(), 4);
    assert!(report.get("contract").is_none());
    assert!(report.get("next_actions").is_none());
    assert!(
        !Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".doctor")
            .exists()
    );
    Ok(())
}

#[test]
fn doctor_capabilities_json_describes_contracts() -> Result<(), Box<dyn std::error::Error>> {
    let report = stdout_json(&["doctor", "capabilities", "--json"])?;

    assert_eq!(report["schema_version"], "cmdrvl.doctor.v1");
    assert_eq!(report["mode"], "capabilities");
    assert_eq!(report["contract"]["schema_version"], "operator.v1");
    assert_eq!(report["contract"]["name"], "decoding");
    Ok(())
}

#[test]
fn output_path_accepts_output_and_out_aliases() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let output = dir.path().join("canon-output.jsonl");
    let alias = dir.path().join("canon-out.jsonl");
    let claims = "tests/fixtures/claims/mixed_source.jsonl";
    let policy = "tests/fixtures/policies/legacy.decode.v0.json";

    let mut long_command = decoding()?;
    let long = long_command
        .args([
            "archaeology",
            claims,
            "--policy",
            policy,
            "--output",
            output.to_str().unwrap(),
            "--json",
        ])
        .output()?;
    let mut short_command = decoding()?;
    let short = short_command
        .args([
            "archaeology",
            claims,
            "--policy",
            policy,
            "--out",
            alias.to_str().unwrap(),
            "--json",
        ])
        .output()?;

    assert_eq!(long.status.code(), Some(1));
    assert_eq!(short.status.code(), Some(1));
    assert_eq!(long.stdout, short.stdout);
    assert_eq!(long.stderr, short.stderr);
    assert_eq!(fs::read(output)?, fs::read(alias)?);
    Ok(())
}

#[test]
fn doctor_robot_triage_json_lists_agent_actions() -> Result<(), Box<dyn std::error::Error>> {
    let report = stdout_json(&["doctor", "--robot-triage"])?;

    assert_eq!(report["schema_version"], "cmdrvl.doctor.v1");
    assert_eq!(report["mode"], "triage");
    assert_eq!(report["healthy"], true);
    assert!(report["next_actions"].as_array().unwrap().len() >= 3);
    Ok(())
}

#[test]
fn doctor_robot_docs_are_human_readable() -> Result<(), Box<dyn std::error::Error>> {
    decoding()?
        .args(["doctor", "robot-docs"])
        .assert()
        .success()
        .stderr("")
        .stdout(contains("decoding doctor robot-docs"))
        .stdout(contains("doctor --fix is intentionally absent"));

    Ok(())
}

#[test]
fn doctor_help_lists_read_only_commands() -> Result<(), Box<dyn std::error::Error>> {
    decoding()?
        .args(["doctor", "--help"])
        .assert()
        .success()
        .stdout(contains("health"))
        .stdout(contains("capabilities"))
        .stdout(contains("robot-docs"));

    Ok(())
}

#[test]
fn doctor_fix_is_rejected_by_clap() -> Result<(), Box<dyn std::error::Error>> {
    decoding()?
        .args(["doctor", "--fix"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("--fix"));

    Ok(())
}
