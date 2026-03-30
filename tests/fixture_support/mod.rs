use std::path::{Path, PathBuf};

use decoding::contracts::claim::{Claim, parse_claim};
use decoding::contracts::policy::Policy;
use decoding::fixtures;

pub fn fixtures_root() -> PathBuf {
    fixtures::fixtures_root()
}

pub fn claim_fixture_path(file_name: &str) -> PathBuf {
    fixtures::claims_fixture_path(file_name)
}

pub fn policy_fixture_path(file_name: &str) -> PathBuf {
    fixtures::policy_fixture_path(file_name)
}

pub fn expected_output_path(relative_path: impl AsRef<Path>) -> PathBuf {
    fixtures_root().join("expected").join(relative_path)
}

pub fn load_claim_fixture_lines(
    file_name: &str,
) -> Result<Vec<String>, fixtures::FixtureLoadError> {
    fixtures::load_claim_fixture_lines(file_name)
}

pub fn load_claim_fixture(file_name: &str) -> Result<Vec<Claim>, ClaimFixtureLoadError> {
    load_claim_fixture_lines(file_name)?
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            parse_claim(&line).map_err(|source| ClaimFixtureLoadError::Claim {
                line_number: index + 1,
                source,
            })
        })
        .collect()
}

pub fn load_policy_fixture(file_name: &str) -> Result<Policy, fixtures::FixtureLoadError> {
    fixtures::load_policy_fixture(file_name)
}

#[derive(Debug)]
pub enum ClaimFixtureLoadError {
    Io(fixtures::FixtureLoadError),
    Claim {
        line_number: usize,
        source: decoding::contracts::claim::ClaimRefusal,
    },
}

impl std::fmt::Display for ClaimFixtureLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "failed to read claim fixture: {error}"),
            Self::Claim {
                line_number,
                source,
            } => {
                write!(
                    f,
                    "failed to parse claim fixture line {line_number}: {source}"
                )
            }
        }
    }
}

impl std::error::Error for ClaimFixtureLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Claim { source, .. } => Some(source),
        }
    }
}

impl From<fixtures::FixtureLoadError> for ClaimFixtureLoadError {
    fn from(error: fixtures::FixtureLoadError) -> Self {
        Self::Io(error)
    }
}
