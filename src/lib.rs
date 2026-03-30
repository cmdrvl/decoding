#![forbid(unsafe_code)]

pub mod bucket;
pub mod cli;
pub mod compare;
pub mod contracts;
pub mod fixtures;
pub mod normalize;
pub mod render;
pub mod report;
pub mod resolve;

use cli::Outcome;

/// Top-level entry point. Parses CLI args, loads inputs, runs the pipeline.
pub fn run() -> Result<Outcome, Box<dyn std::error::Error>> {
    let args = cli::parse();
    cli::execute(args)
}
