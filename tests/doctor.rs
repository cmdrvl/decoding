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
fn redacts_claim_values_by_default_and_explicit_reveals_them()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let claims_path = dir.path().join("canary-claims.jsonl");
    let redacted_path = dir.path().join("redacted-escalations.jsonl");
    let redacted_report_path = dir.path().join("redacted-convergence.json");
    let explicit_path = dir.path().join("explicit-escalations.jsonl");
    let claims = fs::read_to_string("tests/fixtures/claims/mixed_source.jsonl")?
        .replace("Adjusted EBITDA rule family", "CANARY-SENSITIVE-VALUE")
        .replace("EBITDA exception class", "OTHER-SENSITIVE-VALUE");
    fs::write(&claims_path, claims)?;

    let claims_arg = claims_path.to_str().unwrap();
    let redacted_arg = redacted_path.to_str().unwrap();
    let redacted_report_arg = redacted_report_path.to_str().unwrap();
    let policy = "tests/fixtures/policies/legacy.decode.v0.json";

    let mut redacted = decoding()?;
    let redacted_output = redacted
        .args([
            "archaeology",
            claims_arg,
            "--policy",
            policy,
            "--escalations",
            redacted_arg,
            "--convergence",
            redacted_report_arg,
        ])
        .output()?;
    assert_eq!(redacted_output.status.code(), Some(1));
    let redacted_escalations = fs::read_to_string(&redacted_path)?;
    let redacted_report = fs::read_to_string(&redacted_report_path)?;
    assert!(!redacted_escalations.contains("CANARY-SENSITIVE-VALUE"));
    assert!(!redacted_escalations.contains("OTHER-SENSITIVE-VALUE"));
    assert!(!redacted_report.contains("CANARY-SENSITIVE-VALUE"));
    assert!(redacted_escalations.contains("\"redacted\":true"));
    assert!(redacted_escalations.contains("\"sha256\":\"sha256:"));

    let explicit_arg = explicit_path.to_str().unwrap();
    let mut explicit = decoding()?;
    let explicit_output = explicit
        .args([
            "archaeology",
            claims_arg,
            "--policy",
            policy,
            "--escalations",
            explicit_arg,
            "--explicit",
        ])
        .output()?;
    assert_eq!(explicit_output.status.code(), Some(1));
    let explicit_escalations = fs::read_to_string(explicit_path)?;
    assert!(explicit_escalations.contains("CANARY-SENSITIVE-VALUE"));
    assert!(explicit_escalations.contains("OTHER-SENSITIVE-VALUE"));
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
fn unhealthy_doctor_state_returns_exit_one() -> Result<(), Box<dyn std::error::Error>> {
    let output = decoding()?
        .args(["doctor", "--json"])
        .env("DECODING_DOCTOR_FORCE_UNHEALTHY", "1")
        .output()?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["healthy"], false);
    assert_eq!(report["findings"][4]["severity"], "error");
    Ok(())
}

#[test]
fn no_color_and_non_tty_guard_forced_ansi() -> Result<(), Box<dyn std::error::Error>> {
    let forced = decoding()?
        .args(["doctor"])
        .env("CLICOLOR_FORCE", "1")
        .env_remove("NO_COLOR")
        .env_remove("TERM")
        .output()?;
    assert!(String::from_utf8_lossy(&forced.stdout).contains("\x1b["));

    let no_color = decoding()?
        .args(["doctor"])
        .env("CLICOLOR_FORCE", "1")
        .env("NO_COLOR", "1")
        .env_remove("TERM")
        .output()?;
    assert!(!String::from_utf8_lossy(&no_color.stdout).contains("\x1b["));

    let dumb = decoding()?
        .args(["doctor"])
        .env("CLICOLOR_FORCE", "1")
        .env_remove("NO_COLOR")
        .env("TERM", "dumb")
        .output()?;
    assert!(!String::from_utf8_lossy(&dumb.stdout).contains("\x1b["));

    let non_tty = decoding()?
        .args(["doctor"])
        .env_remove("CLICOLOR_FORCE")
        .env_remove("NO_COLOR")
        .env_remove("TERM")
        .output()?;
    assert!(!String::from_utf8_lossy(&non_tty.stdout).contains("\x1b["));
    Ok(())
}

#[test]
fn edit_distance_one_typo_has_a_clap_hint() -> Result<(), Box<dyn std::error::Error>> {
    decoding()?
        .args(["archaeolog"])
        .assert()
        .failure()
        .code(2)
        .stderr(contains("similar subcommand"))
        .stderr(contains("archaeology"));
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
