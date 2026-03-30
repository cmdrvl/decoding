use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

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
pub enum Outcome {
    /// All claims converged, no escalations.
    Clean,
    /// Escalations were emitted.
    Escalations,
}

impl Outcome {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Outcome::Clean => ExitCode::from(0),
            Outcome::Escalations => ExitCode::from(1),
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
        Command::Archaeology(_args) => {
            todo!("archaeology pipeline execution")
        }
    }
}
