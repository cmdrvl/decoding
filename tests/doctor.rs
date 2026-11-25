use std::{fs, path::Path};

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::Value;
use sha2::{Digest, Sha256};

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
    assert_eq!(report["findings"].as_array().unwrap().len(), 5);
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
    let redacted_stdout = String::from_utf8_lossy(&redacted_output.stdout);
    let redacted_stderr = String::from_utf8_lossy(&redacted_output.stderr);
    assert!(!redacted_escalations.contains("CANARY-SENSITIVE-VALUE"));
    assert!(!redacted_escalations.contains("OTHER-SENSITIVE-VALUE"));
    assert!(!redacted_report.contains("CANARY-SENSITIVE-VALUE"));
    assert!(!redacted_report.contains("OTHER-SENSITIVE-VALUE"));
    assert!(!redacted_stdout.contains("CANARY-SENSITIVE-VALUE"));
    assert!(!redacted_stdout.contains("OTHER-SENSITIVE-VALUE"));
    assert!(!redacted_stderr.contains("CANARY-SENSITIVE-VALUE"));
    assert!(!redacted_stderr.contains("OTHER-SENSITIVE-VALUE"));
    assert!(redacted_escalations.contains("\"redacted\":true"));
    assert!(redacted_escalations.contains("\"scope\":\"ordinal\""));
    assert!(redacted_escalations.contains("\"ref\":\"v1\""));
    assert!(!redacted_escalations.contains("\"sha256\":\"sha256:"));
    let old_marker_hash = Sha256::digest(br#"{"kind":"scalar","value":"CANARY-SENSITIVE-VALUE"}"#);
    let old_marker_hex = old_marker_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    for channel in [
        redacted_escalations.as_str(),
        redacted_report.as_str(),
        redacted_stdout.as_ref(),
        redacted_stderr.as_ref(),
    ] {
        assert!(!channel.contains(&old_marker_hex));
        assert!(!channel.contains(&format!("sha256:{old_marker_hex}")));
    }

    let repeat_path = dir.path().join("repeat-escalations.jsonl");
    let mut repeat = decoding()?;
    let repeat_output = repeat
        .args([
            "archaeology",
            claims_arg,
            "--policy",
            policy,
            "--escalations",
            repeat_path.to_str().unwrap(),
        ])
        .output()?;
    assert_eq!(repeat_output.status.code(), Some(1));
    assert_eq!(redacted_output.stdout, repeat_output.stdout);
    assert_eq!(redacted_output.stderr, repeat_output.stderr);
    assert_eq!(
        fs::read(&redacted_path)?,
        fs::read(&repeat_path)?,
        "default redaction must be byte-identical across reruns"
    );

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
fn keyed_redaction_is_stable_and_keeps_key_out_of_every_channel()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let claims_path = dir.path().join("canary-claims.jsonl");
    fs::write(
        &claims_path,
        fs::read_to_string("tests/fixtures/claims/mixed_source.jsonl")?
            .replace("Adjusted EBITDA rule family", "CANARY-SENSITIVE-VALUE")
            .replace("EBITDA exception class", "OTHER-SENSITIVE-VALUE"),
    )?;
    let claims = claims_path.to_str().unwrap();
    let policy = "tests/fixtures/policies/legacy.decode.v0.json";
    let key = "0123456789abcdef";
    let first_escalations = dir.path().join("first-escalations.jsonl");
    let first_convergence = dir.path().join("first-convergence.json");
    let second_escalations = dir.path().join("second-escalations.jsonl");
    let second_convergence = dir.path().join("second-convergence.json");
    let other_escalations = dir.path().join("other-escalations.jsonl");

    let mut first = decoding()?;
    let first_output = first
        .args([
            "archaeology",
            claims,
            "--policy",
            policy,
            "--escalations",
            first_escalations.to_str().unwrap(),
            "--convergence",
            first_convergence.to_str().unwrap(),
            "--json",
        ])
        .env("DECODING_REDACTION_KEY", key)
        .output()?;
    assert_eq!(first_output.status.code(), Some(1));

    let first_escalation_text = fs::read_to_string(&first_escalations)?;
    assert!(
        first_escalation_text.contains(
            "hmac-sha256:4b46ceaec08be6577e32390c43e3b66e2d92a88d0ea5e4f4e542bc3b154e9672"
        )
    );
    assert!(first_escalation_text.contains("\"scope\":\"key\""));
    assert!(!first_escalation_text.contains(key));
    assert!(!first_escalation_text.contains("CANARY-SENSITIVE-VALUE"));
    assert!(!first_escalation_text.contains("OTHER-SENSITIVE-VALUE"));
    let first_stdout = String::from_utf8_lossy(&first_output.stdout);
    let first_stderr = String::from_utf8_lossy(&first_output.stderr);
    assert!(!first_stdout.contains(key));
    assert!(!first_stderr.contains(key));
    let first_convergence_text = fs::read_to_string(&first_convergence)?;
    assert!(!first_convergence_text.contains(key));
    for channel in [
        first_escalation_text.as_str(),
        first_convergence_text.as_str(),
        first_stdout.as_ref(),
        first_stderr.as_ref(),
    ] {
        assert!(!channel.contains("CANARY-SENSITIVE-VALUE"));
        assert!(!channel.contains("OTHER-SENSITIVE-VALUE"));
    }
    let old_marker_hash = Sha256::digest(br#"{"kind":"scalar","value":"CANARY-SENSITIVE-VALUE"}"#);
    let old_marker_hex = old_marker_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert!(!first_escalation_text.contains(&old_marker_hex));
    assert!(!first_convergence_text.contains(&old_marker_hex));

    let mut second = decoding()?;
    let second_output = second
        .args([
            "archaeology",
            claims,
            "--policy",
            policy,
            "--escalations",
            second_escalations.to_str().unwrap(),
            "--convergence",
            second_convergence.to_str().unwrap(),
            "--json",
        ])
        .env("DECODING_REDACTION_KEY", key)
        .output()?;
    assert_eq!(second_output.status.code(), Some(1));
    assert_eq!(
        fs::read(&first_escalations)?,
        fs::read(&second_escalations)?,
        "same key and input must produce the same markers"
    );

    let mut other = decoding()?;
    let other_output = other
        .args([
            "archaeology",
            claims,
            "--policy",
            policy,
            "--escalations",
            other_escalations.to_str().unwrap(),
        ])
        .env("DECODING_REDACTION_KEY", "fedcba9876543210")
        .output()?;
    assert_eq!(other_output.status.code(), Some(1));
    assert_ne!(
        fs::read(&first_escalations)?,
        fs::read(&other_escalations)?,
        "different keys must produce different markers"
    );

    let mut doctor = decoding()?;
    let doctor_output = doctor
        .args(["doctor", "health", "--json"])
        .env("DECODING_REDACTION_KEY", key)
        .output()?;
    assert_eq!(doctor_output.status.code(), Some(0));
    assert!(!String::from_utf8_lossy(&doctor_output.stdout).contains(key));
    assert!(!String::from_utf8_lossy(&doctor_output.stderr).contains(key));
    let doctor_report: Value = serde_json::from_slice(&doctor_output.stdout)?;
    assert_eq!(doctor_report["healthy"], true);
    assert!(
        doctor_report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["id"] == "redaction_key_configured")
    );
    Ok(())
}

#[test]
fn short_redaction_keys_are_typed_refusals_before_writes() -> Result<(), Box<dyn std::error::Error>>
{
    let policy = "tests/fixtures/policies/legacy.decode.v0.json";
    for key in ["", "123456789012345"] {
        let dir = tempfile::tempdir()?;
        let escalation_path = dir.path().join("refused-escalations.jsonl");
        let mut command = decoding()?;
        let output = command
            .args([
                "archaeology",
                "tests/fixtures/claims/mixed_source.jsonl",
                "--policy",
                policy,
                "--escalations",
                escalation_path.to_str().unwrap(),
                "--json",
            ])
            .env("DECODING_REDACTION_KEY", key)
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let status: Value = serde_json::from_slice(&output.stderr)?;
        assert_eq!(status["status"], "refusal");
        assert_eq!(status["refusal"]["code"], "E_REDACTION_KEY_INVALID");
        assert!(!escalation_path.exists());
        if !key.is_empty() {
            assert!(!String::from_utf8_lossy(&output.stderr).contains(key));
        }
    }
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
    let forced = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|finding| finding["id"] == "forced_unhealthy")
        .unwrap();
    assert_eq!(forced["severity"], "error");
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
