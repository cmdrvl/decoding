use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use crate::bucket::BucketStore;
use crate::contracts::claim::{Claim, parse_claim};
use crate::contracts::policy::load_policy;
use crate::render::{write_canon_entry, write_escalation};
use crate::report::generate_report;
use crate::resolve::{Decision, resolve_bucket};

/// Deterministic convergence engine for legacy-system archaeology.
#[derive(Parser, Debug)]
#[command(name = "decoding", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Converge archaeology claims into canonical entries and escalations.
    Archaeology(ArchaeologyArgs),
}

#[derive(Parser, Debug)]
pub struct ArchaeologyArgs {
    /// Claim JSONL files.
    #[arg(required = true)]
    pub claims: Vec<PathBuf>,

    /// Archaeology decode policy file.
    #[arg(long)]
    pub policy: PathBuf,

    /// Canon entry JSONL output (default: stdout).
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Escalation JSONL output.
    #[arg(long)]
    pub escalations: Option<PathBuf>,

    /// Convergence report JSON output.
    #[arg(long)]
    pub convergence: Option<PathBuf>,

    /// Emit JSON status messages on stderr.
    #[arg(long)]
    pub json: bool,
}

/// Pipeline outcome determining exit code.
#[derive(Debug)]
pub enum Outcome {
    /// All claims converged, no escalations.
    Clean,
    /// Escalations were emitted.
    Escalations,
    /// Refusal due to invalid input, policy, or contract violation.
    Refusal,
    /// Operational error outside the refusal boundary.
    Error,
}

impl Outcome {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Outcome::Clean => ExitCode::from(0),
            Outcome::Escalations => ExitCode::from(1),
            Outcome::Refusal | Outcome::Error => ExitCode::from(2),
        }
    }
}

/// Parse CLI arguments.
pub fn parse() -> Cli {
    Cli::parse()
}

/// Execute the parsed CLI command.
pub fn execute(cli: Cli) -> Result<Outcome, Box<dyn std::error::Error>> {
    match cli.command {
        Command::Archaeology(args) => match execute_archaeology(&args) {
            Ok(summary) => {
                emit_success_status(&args, &summary)?;
                Ok(summary.outcome)
            }
            Err(error) => match error.kind {
                CliFailureKind::Refusal => {
                    emit_refusal_status(&args, &error)?;
                    Ok(Outcome::Refusal)
                }
                CliFailureKind::Error => {
                    emit_error_status(&args, &error)?;
                    Ok(Outcome::Error)
                }
            },
        },
    }
}

#[derive(Debug)]
struct ExecutionSummary {
    outcome: Outcome,
    canon_entry_count: usize,
    escalation_count: usize,
}

#[derive(Debug)]
struct CliFailure {
    kind: CliFailureKind,
    reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliFailureKind {
    Refusal,
    Error,
}

impl CliFailure {
    fn refusal(reason: impl Into<String>) -> Self {
        Self {
            kind: CliFailureKind::Refusal,
            reason: reason.into(),
        }
    }

    fn error(reason: impl Into<String>) -> Self {
        Self {
            kind: CliFailureKind::Error,
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for CliFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for CliFailure {}

fn execute_archaeology(args: &ArchaeologyArgs) -> Result<ExecutionSummary, CliFailure> {
    let policy = load_policy(&args.policy).map_err(|error| {
        let message = format!("failed to load policy `{}`: {error}", args.policy.display());
        if error.reason.starts_with("failed to read policy `") {
            CliFailure::error(message)
        } else {
            CliFailure::refusal(message)
        }
    })?;
    let claims = load_claims(&args.claims)?;

    let mut store = BucketStore::default();
    for claim in claims {
        store.insert(claim);
    }

    let decisions = store
        .buckets
        .values()
        .map(|bucket| resolve_bucket(bucket, &policy))
        .collect::<Vec<_>>();

    let canon_entry_count = decisions
        .iter()
        .filter(|decision| matches!(decision, Decision::Resolved(_)))
        .count();
    let escalation_count = decisions
        .iter()
        .filter(|decision| matches!(decision, Decision::Escalated(_)))
        .count();

    write_canon_entries(args.output.as_ref(), &decisions)?;
    write_escalations(args.escalations.as_ref(), &decisions)?;
    write_convergence_report(args.convergence.as_ref(), &policy.policy_id, &decisions)?;

    Ok(ExecutionSummary {
        outcome: if escalation_count == 0 {
            Outcome::Clean
        } else {
            Outcome::Escalations
        },
        canon_entry_count,
        escalation_count,
    })
}

fn load_claims(paths: &[PathBuf]) -> Result<Vec<Claim>, CliFailure> {
    let mut claims = Vec::new();

    for path in paths {
        let file = File::open(path).map_err(|error| {
            CliFailure::error(format!(
                "failed to open claims file `{}`: {error}",
                path.display()
            ))
        })?;
        let reader = BufReader::new(file);

        for (line_index, line_result) in reader.lines().enumerate() {
            let line = line_result.map_err(|error| {
                CliFailure::error(format!(
                    "failed to read claims file `{}` line {}: {error}",
                    path.display(),
                    line_index + 1
                ))
            })?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let claim = parse_claim(trimmed).map_err(|error| {
                CliFailure::refusal(format!(
                    "failed to parse claims file `{}` line {}: {error}",
                    path.display(),
                    line_index + 1
                ))
            })?;
            claims.push(claim);
        }
    }

    Ok(claims)
}

fn write_canon_entries(
    output_path: Option<&PathBuf>,
    decisions: &[Decision],
) -> Result<(), CliFailure> {
    let mut writer = create_writer(output_path)?;

    for decision in decisions {
        if let Decision::Resolved(resolved) = decision {
            write_canon_entry(&mut writer, &resolved.entry).map_err(|error| {
                CliFailure::error(format!("failed to write canon entries: {error}"))
            })?;
        }
    }

    writer
        .flush()
        .map_err(|error| CliFailure::error(format!("failed to flush canon output: {error}")))?;

    Ok(())
}

fn write_escalations(
    output_path: Option<&PathBuf>,
    decisions: &[Decision],
) -> Result<(), CliFailure> {
    let Some(path) = output_path else {
        return Ok(());
    };
    let mut writer = create_writer(Some(path))?;

    for decision in decisions {
        if let Decision::Escalated(escalated) = decision {
            write_escalation(&mut writer, &escalated.escalation).map_err(|error| {
                CliFailure::error(format!("failed to write escalations: {error}"))
            })?;
        }
    }

    writer.flush().map_err(|error| {
        CliFailure::error(format!("failed to flush escalation output: {error}"))
    })?;

    Ok(())
}

fn write_convergence_report(
    output_path: Option<&PathBuf>,
    policy_id: &str,
    decisions: &[Decision],
) -> Result<(), CliFailure> {
    let Some(path) = output_path else {
        return Ok(());
    };
    let mut writer = create_writer(Some(path))?;
    let report = generate_report(policy_id, decisions);

    serde_json::to_writer(&mut writer, &report).map_err(|error| {
        CliFailure::error(format!("failed to write convergence report: {error}"))
    })?;
    writer.write_all(b"\n").map_err(|error| {
        CliFailure::error(format!(
            "failed to finalize convergence report output: {error}"
        ))
    })?;
    writer.flush().map_err(|error| {
        CliFailure::error(format!("failed to flush convergence output: {error}"))
    })?;

    Ok(())
}

fn create_writer(path: Option<&PathBuf>) -> Result<Box<dyn Write>, CliFailure> {
    match path {
        Some(path) => File::create(path)
            .map(|file| Box::new(file) as Box<dyn Write>)
            .map_err(|error| {
                CliFailure::error(format!(
                    "failed to create output `{}`: {error}",
                    path.display()
                ))
            }),
        None => Ok(Box::new(io::stdout())),
    }
}

fn emit_success_status(
    args: &ArchaeologyArgs,
    summary: &ExecutionSummary,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.json {
        let status = match summary.outcome {
            Outcome::Clean => "clean",
            Outcome::Escalations => "escalations",
            Outcome::Refusal => "refusal",
            Outcome::Error => "error",
        };
        writeln!(
            io::stderr(),
            "{}",
            serde_json::json!({
                "status": status,
                "canon_entry_count": summary.canon_entry_count,
                "escalation_count": summary.escalation_count,
            })
        )?;
    } else {
        writeln!(
            io::stderr(),
            "wrote {} canon entries and {} escalations",
            summary.canon_entry_count,
            summary.escalation_count
        )?;
    }

    Ok(())
}

fn emit_refusal_status(
    args: &ArchaeologyArgs,
    error: &CliFailure,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.json {
        writeln!(
            io::stderr(),
            "{}",
            serde_json::json!({
                "status": "refusal",
                "reason": error.reason,
            })
        )?;
    } else {
        writeln!(io::stderr(), "refusal: {}", error.reason)?;
    }

    Ok(())
}

fn emit_error_status(
    args: &ArchaeologyArgs,
    error: &CliFailure,
) -> Result<(), Box<dyn std::error::Error>> {
    if args.json {
        writeln!(
            io::stderr(),
            "{}",
            serde_json::json!({
                "status": "error",
                "reason": error.reason,
            })
        )?;
    } else {
        writeln!(io::stderr(), "error: {}", error.reason)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{ArchaeologyArgs, Cli, Command, Outcome, execute};
    use crate::fixtures::{claims_fixture_path, policy_fixture_path};

    #[test]
    fn executes_archaeology_pipeline_and_writes_outputs() {
        let temp_dir = TempDir::new().unwrap();
        let output = temp_dir.path().join("canon.jsonl");
        let escalations = temp_dir.path().join("escalations.jsonl");
        let convergence = temp_dir.path().join("convergence.json");

        let outcome = execute(Cli {
            command: Command::Archaeology(ArchaeologyArgs {
                claims: vec![claims_fixture_path("mixed_source.jsonl")],
                policy: policy_fixture_path("legacy.decode.v0.json"),
                output: Some(output.clone()),
                escalations: Some(escalations.clone()),
                convergence: Some(convergence.clone()),
                json: false,
            }),
        })
        .unwrap();

        assert!(matches!(outcome, Outcome::Escalations));
        assert!(!fs::read_to_string(output).unwrap().trim().is_empty());
        assert!(!fs::read_to_string(escalations).unwrap().trim().is_empty());
        assert!(
            fs::read_to_string(convergence)
                .unwrap()
                .contains("\"event\":\"convergence.v0\"")
        );
    }

    #[test]
    fn refusal_inputs_map_to_refusal_outcome() {
        let temp_dir = TempDir::new().unwrap();
        let output = temp_dir.path().join("canon.jsonl");

        let outcome = execute(Cli {
            command: Command::Archaeology(ArchaeologyArgs {
                claims: vec![claims_fixture_path("refusal_invalid.jsonl")],
                policy: policy_fixture_path("legacy.decode.v0.json"),
                output: Some(output),
                escalations: None,
                convergence: None,
                json: true,
            }),
        })
        .unwrap();

        assert!(matches!(outcome, Outcome::Refusal));
    }

    #[test]
    fn output_write_failures_map_to_error_outcome() {
        let temp_dir = TempDir::new().unwrap();
        let missing_parent = temp_dir.path().join("missing").join("canon.jsonl");

        let outcome = execute(Cli {
            command: Command::Archaeology(ArchaeologyArgs {
                claims: vec![claims_fixture_path("mixed_source.jsonl")],
                policy: policy_fixture_path("legacy.decode.v0.json"),
                output: Some(missing_parent),
                escalations: None,
                convergence: None,
                json: true,
            }),
        })
        .unwrap();

        assert!(matches!(outcome, Outcome::Error));
    }
}
