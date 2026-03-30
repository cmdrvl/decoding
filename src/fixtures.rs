//! Shared archaeology fixture helpers for tests and fixture-driven workflows.

use std::fs;
use std::path::{Path, PathBuf};

use crate::contracts::claim::{Claim, ClaimRefusal, parse_claim};
use crate::contracts::policy::{Policy, PolicyRefusal, load_policy};

const FIXTURES_DIR: &str = "tests/fixtures";
const CLAIMS_DIR: &str = "claims";
const POLICIES_DIR: &str = "policies";
const EXPECTED_DIR: &str = "expected";

/// Resolve the repository fixture root.
pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR)
}

/// Resolve a claim fixture path by filename.
pub fn claims_fixture_path(name: &str) -> PathBuf {
    fixtures_root().join(CLAIMS_DIR).join(name)
}

/// Resolve a policy fixture path by filename.
pub fn policy_fixture_path(name: &str) -> PathBuf {
    fixtures_root().join(POLICIES_DIR).join(name)
}

/// Resolve an expected-output fixture path by filename.
pub fn expected_output_fixture_path(name: &str) -> PathBuf {
    fixtures_root().join(EXPECTED_DIR).join(name)
}

/// Load non-empty raw JSONL claim lines from a named fixture file.
pub fn load_claim_fixture_lines(name: &str) -> Result<Vec<String>, FixtureLoadError> {
    let path = claims_fixture_path(name);
    let contents =
        fs::read_to_string(&path).map_err(|error| FixtureLoadError::Io { path, error })?;

    Ok(contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

/// Load and parse a valid claim fixture file into typed claims.
pub fn load_claim_fixture(name: &str) -> Result<Vec<Claim>, FixtureLoadError> {
    load_claim_fixture_lines(name)?
        .into_iter()
        .map(|line| {
            parse_claim(&line).map_err(|error| FixtureLoadError::Claim {
                name: name.into(),
                error,
            })
        })
        .collect()
}

/// Load and validate a policy fixture file into the typed Phase 1 policy contract.
pub fn load_policy_fixture(name: &str) -> Result<Policy, FixtureLoadError> {
    let path = policy_fixture_path(name);
    load_policy(&path).map_err(|error| FixtureLoadError::Policy { path, error })
}

/// Error type for shared fixture loading helpers.
#[derive(Debug)]
pub enum FixtureLoadError {
    Io {
        path: PathBuf,
        error: std::io::Error,
    },
    Claim {
        name: String,
        error: ClaimRefusal,
    },
    Policy {
        path: PathBuf,
        error: PolicyRefusal,
    },
}

impl std::fmt::Display for FixtureLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, error } => {
                write!(f, "failed to read fixture `{}`: {error}", path.display())
            }
            Self::Claim { name, error } => {
                write!(f, "failed to parse claim fixture `{name}`: {error}")
            }
            Self::Policy { path, error } => {
                write!(
                    f,
                    "failed to load policy fixture `{}`: {error}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for FixtureLoadError {}

#[cfg(test)]
mod tests {
    use super::{
        claims_fixture_path, expected_output_fixture_path, fixtures_root, load_claim_fixture,
        load_claim_fixture_lines, policy_fixture_path,
    };
    use crate::contracts::vocabulary::PropertyType;

    #[test]
    fn fixture_paths_resolve_from_repo_root() {
        assert_eq!(
            claims_fixture_path("mixed_source.jsonl"),
            fixtures_root().join("claims").join("mixed_source.jsonl")
        );
        assert_eq!(
            policy_fixture_path("legacy.decode.v0.json"),
            fixtures_root()
                .join("policies")
                .join("legacy.decode.v0.json")
        );
        assert_eq!(
            expected_output_fixture_path("canon-map.jsonl"),
            fixtures_root().join("expected").join("canon-map.jsonl")
        );
    }

    #[test]
    fn load_claim_fixture_lines_reads_non_empty_lines() {
        let lines = load_claim_fixture_lines("mixed_source.jsonl").unwrap();

        assert_eq!(lines.len(), 12);
        assert!(
            lines
                .iter()
                .all(|line| line.contains("\"event\":\"claim.v0\""))
        );
    }

    #[test]
    fn load_claim_fixture_parses_typed_claims() {
        let claims = load_claim_fixture("mixed_source.jsonl").unwrap();

        assert_eq!(claims.len(), 12);
        assert_eq!(claims[0].property_type, PropertyType::DependsOn);
        assert_eq!(claims[3].property_type, PropertyType::Liveness);
    }
}
