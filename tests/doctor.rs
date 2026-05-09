use std::path::Path;

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

    assert_eq!(report["schema"], "decoding.doctor.health.v1");
    assert_eq!(report["healthy"], true);
    assert_eq!(
        report["observed_inputs"]["claims"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(report["side_effects"]["reads_claim_files"], false);
    assert_eq!(report["side_effects"]["reads_policy_files"], false);
    assert_eq!(report["side_effects"]["writes_output_files"], false);
    assert_eq!(report["side_effects"]["writes_doctor_artifacts"], false);
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

    assert_eq!(report["schema"], "decoding.doctor.capabilities.v1");
    assert_eq!(report["fix_mode"]["available"], false);
    assert_eq!(report["core_command"]["input_contract"], "claim.v0");
    assert_eq!(
        report["core_command"]["output_contracts"][2],
        "convergence.v0"
    );
    Ok(())
}

#[test]
fn doctor_robot_triage_json_lists_agent_actions() -> Result<(), Box<dyn std::error::Error>> {
    let report = stdout_json(&["doctor", "--robot-triage"])?;

    assert_eq!(report["schema"], "decoding.doctor.triage.v1");
    assert_eq!(report["status"], "healthy");
    assert!(report["recommended_actions"].as_array().unwrap().len() >= 3);
    assert_eq!(report["side_effects"]["resolves_claims"], false);
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
