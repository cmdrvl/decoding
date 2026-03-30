#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match decoding::run() {
        Ok(outcome) => outcome.exit_code(),
        Err(e) => {
            eprintln!("decoding: {e}");
            ExitCode::from(2)
        }
    }
}
