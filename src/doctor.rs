//! Read-only diagnostics for headless agents and operators.

use std::io::{self, Write};

use serde_json::{Value, json};

use crate::cli::{DoctorArgs, DoctorCommand, Outcome};
use crate::paths::config_footprint;

const CONTRACT: &str = "cmdrvl.read_only_doctor.v1";
const HEALTH_SCHEMA: &str = "decoding.doctor.health.v1";
const CAPABILITIES_SCHEMA: &str = "decoding.doctor.capabilities.v1";
const TRIAGE_SCHEMA: &str = "decoding.doctor.triage.v1";

pub fn execute(args: &DoctorArgs) -> Result<Outcome, Box<dyn std::error::Error>> {
    if args.robot_triage {
        write_json(robot_triage_report())?;
        return Ok(Outcome::Clean);
    }

    match &args.command {
        Some(DoctorCommand::Health(output)) => {
            if output.json {
                write_json(health_report())?;
            } else {
                write_human_health()?;
            }
        }
        Some(DoctorCommand::Capabilities(output)) => {
            if output.json {
                write_json(capabilities_report())?;
            } else {
                write_human_capabilities()?;
            }
        }
        Some(DoctorCommand::RobotDocs) => write_robot_docs()?,
        None => write_human_health()?,
    }

    Ok(Outcome::Clean)
}

fn write_json(value: Value) -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    serde_json::to_writer_pretty(&mut handle, &value)?;
    handle.write_all(b"\n")?;
    Ok(())
}

fn write_human_health() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "decoding doctor health")?;
    writeln!(stdout, "status: healthy")?;
    writeln!(stdout, "contract: {CONTRACT}")?;
    writeln!(stdout, "version: {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(stdout, "read_only: true")?;
    writeln!(stdout, "config_root: {}", crate::paths::CANONICAL_ROOT)?;
    writeln!(stdout, "fix_available: false")?;
    writeln!(stdout, "claim_loaders_entered: false")?;
    writeln!(stdout, "policy_loaders_entered: false")?;
    writeln!(stdout, "output_writers_entered: false")?;
    Ok(())
}

fn write_human_capabilities() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "decoding doctor capabilities")?;
    writeln!(stdout, "contract: {CONTRACT}")?;
    writeln!(stdout, "config_root: {}", crate::paths::CANONICAL_ROOT)?;
    writeln!(stdout, "commands:")?;
    writeln!(stdout, "  decoding doctor health [--json]")?;
    writeln!(stdout, "  decoding doctor capabilities [--json]")?;
    writeln!(stdout, "  decoding doctor robot-docs")?;
    writeln!(stdout, "  decoding doctor --robot-triage")?;
    writeln!(stdout, "fix_mode: unavailable")?;
    writeln!(stdout, "online_probes: unavailable")?;
    Ok(())
}

fn write_robot_docs() -> Result<(), Box<dyn std::error::Error>> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "decoding doctor robot-docs")?;
    writeln!(stdout, "contract: {CONTRACT}")?;
    writeln!(stdout)?;
    writeln!(stdout, "Purpose")?;
    writeln!(
        stdout,
        "  Provide deterministic, read-only diagnostics for the decoding CLI."
    )?;
    writeln!(
        stdout,
        "  Doctor commands do not read claim files, policy files, output paths, or .doctor/."
    )?;
    writeln!(stdout)?;
    writeln!(stdout, "Commands")?;
    writeln!(stdout, "  decoding doctor health --json")?;
    writeln!(stdout, "  decoding doctor capabilities --json")?;
    writeln!(stdout, "  decoding doctor --robot-triage")?;
    writeln!(stdout)?;
    writeln!(stdout, "Safety")?;
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

fn health_report() -> Value {
    json!({
        "schema": HEALTH_SCHEMA,
        "contract": CONTRACT,
        "status": "healthy",
        "healthy": true,
        "tool": tool_identity(),
        "config_footprint": config_footprint(),
        "checks": [
            {
                "id": "cli_loaded",
                "status": "pass",
                "detail": "decoding CLI metadata is available"
            },
            {
                "id": "doctor_read_only",
                "status": "pass",
                "detail": "doctor dispatch returns before archaeology claim, policy, and output loaders"
            },
            {
                "id": "fix_mode_disabled",
                "status": "pass",
                "detail": "doctor --fix is not part of the clap surface"
            },
            {
                "id": "boundary_preserved",
                "status": "pass",
                "detail": "doctor does not parse catalog records or derived claim files"
            }
        ],
        "observed_inputs": {
            "claims": [],
            "policy": null,
            "output": null,
            "escalations": null,
            "convergence": null
        },
        "side_effects": side_effects(),
        "domain_boundaries": domain_boundaries()
    })
}

fn capabilities_report() -> Value {
    json!({
        "schema": CAPABILITIES_SCHEMA,
        "contract": CONTRACT,
        "tool": tool_identity(),
        "config_footprint": config_footprint(),
        "commands": [
            {
                "command": "decoding doctor health",
                "json": true,
                "human": true,
                "description": "read-only health summary"
            },
            {
                "command": "decoding doctor capabilities",
                "json": true,
                "human": true,
                "description": "machine-readable command and safety contract"
            },
            {
                "command": "decoding doctor robot-docs",
                "json": false,
                "human": true,
                "description": "deterministic operating notes for agents"
            },
            {
                "command": "decoding doctor --robot-triage",
                "json": true,
                "human": false,
                "description": "prioritized read-only triage guidance"
            }
        ],
        "core_command": {
            "command": "decoding archaeology <CLAIMS>... --policy <FILE>",
            "input_contract": "claim.v0",
            "policy_contract": "legacy.decode.v0",
            "output_contracts": ["canon_entry.v0", "escalation.v0", "convergence.v0"],
            "exit_codes": {
                "0": "all claims converged or resolved",
                "1": "escalations emitted",
                "2": "refusal, invalid policy, contract violation, or operational error"
            }
        },
        "fix_mode": {
            "available": false,
            "reason": "read-only first slice; mutation mode requires separate detector, backup, inverse, fixture, and undo work"
        },
        "side_effects": side_effects(),
        "domain_boundaries": domain_boundaries()
    })
}

fn robot_triage_report() -> Value {
    json!({
        "schema": TRIAGE_SCHEMA,
        "contract": CONTRACT,
        "status": "healthy",
        "healthy": true,
        "tool": tool_identity(),
        "config_footprint": config_footprint(),
        "recommended_actions": [
            {
                "priority": 1,
                "action": "run decoding doctor capabilities --json",
                "reason": "discover the supported read-only diagnostic contract"
            },
            {
                "priority": 2,
                "action": "run decoding archaeology --help",
                "reason": "inspect the domain command without loading claim data"
            },
            {
                "priority": 3,
                "action": "run decoding archaeology <claims.jsonl> --policy <policy.json> on a fixture or approved input",
                "reason": "execute convergence only after claim and policy paths are explicit"
            }
        ],
        "known_failure_modes": [
            {
                "id": "invalid_claim_contract",
                "exit_code": 2,
                "classification": "refusal",
                "operator_action": "fix claim.v0 producer or input fixture"
            },
            {
                "id": "unknown_policy_key",
                "exit_code": 2,
                "classification": "refusal",
                "operator_action": "fix policy JSON to match legacy.decode.v0"
            },
            {
                "id": "unresolved_or_conflicting_bucket",
                "exit_code": 1,
                "classification": "escalation",
                "operator_action": "review escalation.v0 or scan additional evidence"
            }
        ],
        "side_effects": side_effects()
    })
}

fn tool_identity() -> Value {
    json!({
        "name": "decoding",
        "version": env!("CARGO_PKG_VERSION"),
        "role": "deterministic convergence engine for legacy-system archaeology",
        "primary_input": "derived claim.v0",
        "primary_outputs": ["canon_entry.v0", "escalation.v0", "convergence.v0"]
    })
}

fn side_effects() -> Value {
    json!({
        "reads_stdin": false,
        "reads_claim_files": false,
        "reads_policy_files": false,
        "detects_claim_format": false,
        "parses_claim_contracts": false,
        "loads_decode_policy": false,
        "groups_buckets": false,
        "resolves_claims": false,
        "renders_canon_entries": false,
        "renders_escalations": false,
        "renders_convergence_report": false,
        "writes_output_files": false,
        "writes_doctor_artifacts": false,
        "uses_network": false,
        "changes_cwd": false
    })
}

fn domain_boundaries() -> Value {
    json!({
        "owns": [
            "derived claim.v0 convergence",
            "canon_entry.v0 rendering",
            "escalation.v0 rendering",
            "convergence.v0 summaries"
        ],
        "does_not_own": [
            "direct metadata catalog ingestion",
            "catalog record parsing",
            "scanner repair",
            "gold truth minting",
            "production database mutation"
        ]
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{capabilities_report, health_report, robot_triage_report};

    #[test]
    fn health_report_is_read_only() {
        let report = health_report();

        assert_eq!(
            report.pointer("/healthy").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            report
                .pointer("/observed_inputs/claims")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            report
                .pointer("/side_effects/reads_claim_files")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            report
                .pointer("/side_effects/writes_output_files")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            report
                .pointer("/side_effects/writes_doctor_artifacts")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            report
                .pointer("/config_footprint/canonical_root")
                .and_then(Value::as_str),
            Some("~/.cmdrvl")
        );
        assert_eq!(
            report
                .pointer("/config_footprint/legacy_migration_required")
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn capabilities_report_disables_fix_mode() {
        let report = capabilities_report();

        assert_eq!(
            report
                .pointer("/fix_mode/available")
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            report
                .pointer("/core_command/output_contracts/0")
                .and_then(Value::as_str),
            Some("canon_entry.v0")
        );
        assert_eq!(
            report
                .pointer("/config_footprint/managed_state_paths")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
    }

    #[test]
    fn robot_triage_mentions_refusal_and_escalation_paths() {
        let report = robot_triage_report();

        assert!(
            report
                .pointer("/known_failure_modes")
                .and_then(Value::as_array)
                .is_some_and(|modes| modes
                    .iter()
                    .any(|mode| mode.get("classification").and_then(Value::as_str)
                        == Some("refusal")))
        );
        assert!(
            report
                .pointer("/known_failure_modes")
                .and_then(Value::as_array)
                .is_some_and(|modes| modes
                    .iter()
                    .any(|mode| mode.get("classification").and_then(Value::as_str)
                        == Some("escalation")))
        );
        assert_eq!(
            report
                .pointer("/config_footprint/deprecation_notices")
                .and_then(Value::as_str),
            Some("~/.cmdrvl/notices/deprecated-paths.jsonl")
        );
    }
}
