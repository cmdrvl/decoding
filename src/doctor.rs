//! Read-only diagnostics for headless agents and operators.

use std::io::{self, IsTerminal, Write};

use serde_json::Value;
use spine_rules::contract::OperatorContract;
use spine_rules::doctor::{DoctorMode, DoctorReport, Finding, Severity};

use crate::cli::{DoctorArgs, DoctorCommand, Outcome};

const FORCE_UNHEALTHY_ENV: &str = "DECODING_DOCTOR_FORCE_UNHEALTHY";

pub fn execute(args: &DoctorArgs) -> Result<Outcome, Box<dyn std::error::Error>> {
    if args.robot_triage {
        return write_report(triage_report());
    }

    match &args.command {
        Some(DoctorCommand::Health(output)) => {
            if output.json {
                return write_report(health_report());
            } else {
                return write_human_health();
            }
        }
        Some(DoctorCommand::Capabilities(output)) => {
            if output.json {
                return write_report(capabilities_report());
            } else {
                write_human_capabilities()?;
            }
        }
        Some(DoctorCommand::Triage(output)) => {
            if output.json {
                return write_report(triage_report());
            }
            write_human_triage()?;
        }
        Some(DoctorCommand::RobotDocs(_)) => write_robot_docs()?,
        None => {
            if args.json {
                return write_report(health_report());
            }
            return write_human_health();
        }
    }

    Ok(Outcome::Clean)
}

fn write_report(report: DoctorReport) -> Result<Outcome, Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, &report)?;
    handle.write_all(b"\n")?;
    Ok(if report.healthy {
        Outcome::Clean
    } else {
        Outcome::Escalations
    })
}

fn write_human_health() -> Result<Outcome, Box<dyn std::error::Error>> {
    let report = health_report();
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "decoding doctor health")?;
    let status = if report.healthy {
        color_status("healthy", true)
    } else {
        color_status("unhealthy", false)
    };
    writeln!(stdout, "status: {status}")?;
    writeln!(stdout, "contract: cmdrvl.doctor.v1")?;
    writeln!(stdout, "version: {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(stdout, "read_only: true")?;
    writeln!(stdout, "config_root: {}", crate::paths::CANONICAL_ROOT)?;
    writeln!(stdout, "fix_available: false")?;
    writeln!(stdout, "claim_loaders_entered: false")?;
    writeln!(stdout, "policy_loaders_entered: false")?;
    writeln!(stdout, "output_writers_entered: false")?;
    writeln!(
        stdout,
        "redaction_key_configured: {}",
        std::env::var_os("DECODING_REDACTION_KEY").is_some()
    )?;
    Ok(if report.healthy {
        Outcome::Clean
    } else {
        Outcome::Escalations
    })
}

fn color_status(status: &str, healthy: bool) -> String {
    if !color_enabled() {
        return status.to_owned();
    }
    let color = if healthy { "32" } else { "31" };
    format!("\x1b[{color}m{status}\x1b[0m")
}

fn color_enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some()
        || std::env::var("TERM").is_ok_and(|term| term.eq_ignore_ascii_case("dumb"))
    {
        return false;
    }

    let force = std::env::var("CLICOLOR_FORCE")
        .map(|value| !value.is_empty() && value != "0")
        .unwrap_or(false);
    force || io::stdout().is_terminal()
}

fn write_human_capabilities() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "decoding doctor capabilities")?;
    writeln!(stdout, "contract: operator.v1")?;
    writeln!(stdout, "config_root: {}", crate::paths::CANONICAL_ROOT)?;
    writeln!(stdout, "commands:")?;
    writeln!(stdout, "  decoding --describe")?;
    writeln!(stdout, "  decoding --schema")?;
    writeln!(stdout, "  decoding capabilities --json")?;
    writeln!(stdout, "  decoding robot-docs guide")?;
    writeln!(stdout, "  decoding doctor health [--json]")?;
    writeln!(stdout, "  decoding doctor capabilities [--json]")?;
    writeln!(stdout, "  decoding doctor triage --json")?;
    writeln!(stdout, "  decoding doctor robot-docs")?;
    writeln!(stdout, "  decoding doctor triage --json")?;
    writeln!(stdout, "  decoding --robot-triage (legacy alias)")?;
    writeln!(stdout, "fix_mode: unavailable")?;
    writeln!(stdout, "online_probes: unavailable")?;
    Ok(())
}

fn write_human_triage() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "decoding doctor triage")?;
    writeln!(stdout, "contract: cmdrvl.doctor.v1")?;
    writeln!(stdout, "status: healthy")?;
    writeln!(stdout, "next_action: decoding capabilities --json")?;
    Ok(())
}

fn write_robot_docs() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "# decoding robot-docs guide")?;
    writeln!(stdout, "Alias: decoding doctor robot-docs [guide]")?;
    writeln!(stdout, "contract: cmdrvl.doctor.v1")?;
    writeln!(stdout)?;
    writeln!(stdout, "Purpose")?;
    writeln!(
        stdout,
        "  Converge derived claim.v0 JSONL into canon_entry.v0, escalation.v0 and convergence.v0."
    )?;
    writeln!(
        stdout,
        "  Doctor commands do not read claim files, policy files, output paths, or .doctor/."
    )?;
    writeln!(stdout)?;
    writeln!(stdout, "Commands")?;
    writeln!(stdout, "  decoding --describe")?;
    writeln!(stdout, "  decoding --schema")?;
    writeln!(stdout, "  decoding capabilities --json")?;
    writeln!(stdout, "  decoding robot-docs guide")?;
    writeln!(stdout, "  decoding doctor health --json")?;
    writeln!(stdout, "  decoding doctor capabilities --json")?;
    writeln!(stdout, "  decoding doctor triage --json")?;
    writeln!(stdout, "  decoding --robot-triage (legacy alias)")?;
    writeln!(
        stdout,
        "  decoding archaeology <CLAIMS>... --policy <FILE> [--out <FILE>] [--escalations <FILE>] [--convergence <FILE>] [--json]"
    )?;
    writeln!(
        stdout,
        "  add --explicit to archaeology for raw escalation candidate values; default output uses ordinal refs or keyed HMAC markers"
    )?;
    writeln!(
        stdout,
        "  without DECODING_REDACTION_KEY, refs are deterministic v1, v2, ... by first appearance within one escalation output set; they reveal equality patterns and distinct-value count, not content"
    )?;
    writeln!(
        stdout,
        "  with DECODING_REDACTION_KEY (at least 16 bytes), markers are stable HMAC-SHA256 values; the key is never emitted"
    )?;
    writeln!(stdout)?;
    writeln!(stdout, "Routing and exits")?;
    writeln!(
        stdout,
        "  stdout: canon_entry.v0 JSONL unless --out is explicit. --schema describes each record."
    )?;
    writeln!(
        stdout,
        "  --json: status on stderr; refusal code/detail at /refusal/code and /refusal/detail."
    )?;
    writeln!(
        stdout,
        "  0: all buckets resolved or discovery completed; 1: escalations; 2: refusal, usage or I/O error."
    )?;
    writeln!(stdout, "Refusal and operational codes")?;
    let manifest: Value = serde_json::from_str(crate::OPERATOR_JSON)?;
    if let Some(refusals) = manifest["refusals"].as_array() {
        for refusal in refusals {
            writeln!(
                stdout,
                "  {}: {} (action: {})",
                refusal["code"].as_str().unwrap_or_default(),
                refusal["message"].as_str().unwrap_or_default(),
                refusal["action"].as_str().unwrap_or_default()
            )?;
        }
    }
    writeln!(stdout)?;
    writeln!(stdout, "Safety")?;
    writeln!(
        stdout,
        "  Explicit output paths replace existing files; keep them distinct from inputs and each other."
    )?;
    writeln!(
        stdout,
        "  No stdin, network, ambient witness recording or implicit config files. No --no-witness flag is needed."
    )?;
    writeln!(
        stdout,
        "  Directly observed catalog metadata bypasses decoding. Preserve exact decimal JSON numbers."
    )?;
    writeln!(
        stdout,
        "  doctor --fix is intentionally absent in this release."
    )?;
    writeln!(
        stdout,
        "  Any future fix mode needs detector, backup, inverse, fixture, and undo coverage."
    )?;
    Ok(())
}

fn health_report() -> DoctorReport {
    let forced_unhealthy = std::env::var_os(FORCE_UNHEALTHY_ENV).is_some();
    let mut findings = vec![
        finding("cli_loaded", "decoding CLI metadata is available"),
        finding(
            "doctor_read_only",
            "doctor dispatch returns before archaeology claim, policy, and output loaders",
        ),
        finding(
            "fix_mode_disabled",
            "doctor --fix is not part of the clap surface",
        ),
        finding(
            "boundary_preserved",
            "doctor does not parse catalog records or derived claim files",
        ),
        Finding {
            id: "redaction_key_configured".to_owned(),
            severity: Severity::Info,
            message: if std::env::var_os("DECODING_REDACTION_KEY").is_some() {
                "DECODING_REDACTION_KEY is configured; key material is never reported".to_owned()
            } else {
                "DECODING_REDACTION_KEY is not configured".to_owned()
            },
            fix: None,
        },
    ];
    if forced_unhealthy {
        findings.push(Finding {
            id: "forced_unhealthy".to_owned(),
            severity: Severity::Error,
            message: format!("{FORCE_UNHEALTHY_ENV} requested an unhealthy doctor fixture"),
            fix: Some(format!("unset {FORCE_UNHEALTHY_ENV}")),
        });
    }

    DoctorReport {
        schema_version: spine_rules::doctor::DOCTOR_SCHEMA_VERSION.to_owned(),
        mode: DoctorMode::Health,
        tool: "decoding".to_owned(),
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        healthy: !forced_unhealthy,
        findings,
        contract: None,
        next_actions: None,
    }
}

fn capabilities_report() -> DoctorReport {
    DoctorReport {
        schema_version: spine_rules::doctor::DOCTOR_SCHEMA_VERSION.to_owned(),
        mode: DoctorMode::Capabilities,
        tool: "decoding".to_owned(),
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        healthy: true,
        findings: vec![finding(
            "operator_contract_loaded",
            "operator.v1 contract is available",
        )],
        contract: Some(operator_contract()),
        next_actions: None,
    }
}

fn triage_report() -> DoctorReport {
    DoctorReport {
        schema_version: spine_rules::doctor::DOCTOR_SCHEMA_VERSION.to_owned(),
        mode: DoctorMode::Triage,
        tool: "decoding".to_owned(),
        tool_version: env!("CARGO_PKG_VERSION").to_owned(),
        healthy: true,
        findings: vec![finding(
            "triage_available",
            "read-only triage guidance is available",
        )],
        contract: None,
        next_actions: Some(vec![
            "run decoding capabilities --json".to_owned(),
            "run decoding archaeology --help".to_owned(),
            "run decoding archaeology <claims.jsonl> --policy <policy.json> on an approved input"
                .to_owned(),
        ]),
    }
}

fn finding(id: &str, message: &str) -> Finding {
    Finding {
        id: id.to_owned(),
        severity: Severity::Info,
        message: message.to_owned(),
        fix: None,
    }
}

fn operator_contract() -> OperatorContract {
    OperatorContract::parse(crate::OPERATOR_JSON)
        .expect("embedded operator.json must parse as operator.v1")
}

#[cfg(test)]
mod tests {
    use super::{capabilities_report, health_report, triage_report};

    #[test]
    fn health_report_is_v1_and_healthy() {
        let report = health_report();
        assert_eq!(report.schema_version, "cmdrvl.doctor.v1");
        assert_eq!(report.mode, spine_rules::doctor::DoctorMode::Health);
        assert!(report.healthy);
        assert!(report.validate().is_empty(), "{:?}", report.validate());
    }

    #[test]
    fn capabilities_report_embeds_valid_operator_contract() {
        let report = capabilities_report();
        assert_eq!(report.mode, spine_rules::doctor::DoctorMode::Capabilities);
        let contract = report.contract.as_ref().unwrap();
        assert!(contract.validate().is_empty(), "{:?}", contract.validate());
        assert_eq!(contract.name, "decoding");
    }

    #[test]
    fn triage_report_is_v1_with_actions() {
        let report = triage_report();
        assert_eq!(report.mode, spine_rules::doctor::DoctorMode::Triage);
        assert!(report.healthy);
        assert!(!report.next_actions.as_ref().unwrap().is_empty());
        assert!(report.validate().is_empty(), "{:?}", report.validate());
    }
}
