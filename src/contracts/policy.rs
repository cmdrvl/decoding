//! `legacy.decode.v0` policy loader and validator.

use std::fs;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::vocabulary::{PropertyType, SourceKind};

const PHASE_ONE_POLICY_ID: &str = "legacy.decode.v0";

/// The decoded archaeology policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub policy_id: String,
    pub auto_resolve: Vec<PropertyType>,
    pub min_corroboration: IndexMap<PropertyType, usize>,
    pub source_priority: IndexMap<PropertyType, Vec<SourceKind>>,
}

impl Policy {
    /// Whether the policy auto-resolves the given property type.
    pub fn auto_resolves(&self, property_type: PropertyType) -> bool {
        self.auto_resolve.contains(&property_type)
    }

    /// The corroboration threshold for a property type, when one is configured.
    pub fn corroboration_threshold(&self, property_type: PropertyType) -> Option<usize> {
        self.min_corroboration.get(&property_type).copied()
    }

    /// The source priority order for a property type, when one is configured.
    pub fn source_priority_for(&self, property_type: PropertyType) -> Option<&[SourceKind]> {
        self.source_priority.get(&property_type).map(Vec::as_slice)
    }

    fn validate(self) -> Result<Self, PolicyRefusal> {
        if self.policy_id != PHASE_ONE_POLICY_ID {
            return Err(PolicyRefusal::new(format!(
                "expected policy_id `{PHASE_ONE_POLICY_ID}`, found `{}`",
                self.policy_id
            )));
        }

        for property_type in &self.auto_resolve {
            if !matches!(
                property_type,
                PropertyType::Exists | PropertyType::Schema | PropertyType::Constraint
            ) {
                return Err(PolicyRefusal::new(format!(
                    "property `{property_type:?}` is not allowed in auto_resolve"
                )));
            }
        }

        for property_type in self.min_corroboration.keys() {
            if !matches!(
                property_type,
                PropertyType::Reads
                    | PropertyType::Writes
                    | PropertyType::DependsOn
                    | PropertyType::UsedBy
                    | PropertyType::Schedule
                    | PropertyType::ValidValues
                    | PropertyType::SemanticLabel
                    | PropertyType::AuthoritativeFor
            ) {
                return Err(PolicyRefusal::new(format!(
                    "property `{property_type:?}` is not allowed in min_corroboration"
                )));
            }
        }

        for (property_type, priorities) in &self.source_priority {
            if *property_type != PropertyType::Liveness {
                return Err(PolicyRefusal::new(format!(
                    "property `{property_type:?}` is not allowed in source_priority"
                )));
            }

            if priorities.is_empty() {
                return Err(PolicyRefusal::new(
                    "source_priority entries must provide at least one source kind",
                ));
            }
        }

        Ok(self)
    }
}

/// Load and validate a policy file. Refuses on unknown keys.
pub fn load_policy(path: &Path) -> Result<Policy, PolicyRefusal> {
    let raw = fs::read_to_string(path).map_err(|error| {
        PolicyRefusal::new(format!(
            "failed to read policy `{}`: {error}",
            path.display()
        ))
    })?;

    serde_json::from_str::<Policy>(&raw)
        .map_err(|error| {
            PolicyRefusal::new(format!(
                "failed to parse policy `{}`: {error}",
                path.display()
            ))
        })?
        .validate()
}

/// Refusal error for invalid policy files.
#[derive(Debug)]
pub struct PolicyRefusal {
    pub reason: String,
}

impl std::fmt::Display for PolicyRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "policy refusal: {}", self.reason)
    }
}

impl std::error::Error for PolicyRefusal {}

impl PolicyRefusal {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::NamedTempFile;

    use super::{PHASE_ONE_POLICY_ID, Policy, load_policy};
    use crate::contracts::vocabulary::{PropertyType, SourceKind};

    #[test]
    fn loads_phase_one_policy_with_typed_accessors() {
        let policy = load_inline_policy(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists", "schema", "constraint"],
                "min_corroboration": {
                    "reads": 2,
                    "valid_values": 3
                },
                "source_priority": {
                    "liveness": ["db_scan", "file_scan", "repo_scan"]
                }
            }"#,
        );

        assert_eq!(policy.policy_id, PHASE_ONE_POLICY_ID);
        assert!(policy.auto_resolves(PropertyType::Exists));
        assert!(!policy.auto_resolves(PropertyType::Liveness));
        assert_eq!(policy.corroboration_threshold(PropertyType::Reads), Some(2));
        assert_eq!(
            policy.source_priority_for(PropertyType::Liveness),
            Some(
                &[
                    SourceKind::DbScan,
                    SourceKind::FileScan,
                    SourceKind::RepoScan,
                ][..]
            )
        );
    }

    #[test]
    fn rejects_unknown_top_level_keys() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists"],
                "min_corroboration": {},
                "source_priority": {},
                "surprise": true
            }"#,
        );

        assert!(error.reason.contains("unknown field"));
    }

    #[test]
    fn rejects_wrong_policy_id() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v1",
                "auto_resolve": ["exists"],
                "min_corroboration": {},
                "source_priority": {}
            }"#,
        );

        assert!(error.reason.contains("expected policy_id"));
    }

    #[test]
    fn rejects_unsupported_auto_resolve_property() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["liveness"],
                "min_corroboration": {},
                "source_priority": {}
            }"#,
        );

        assert!(error.reason.contains("auto_resolve"));
    }

    #[test]
    fn rejects_unsupported_min_corroboration_property() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists"],
                "min_corroboration": {
                    "schema": 2
                },
                "source_priority": {}
            }"#,
        );

        assert!(error.reason.contains("min_corroboration"));
    }

    #[test]
    fn rejects_first_invalid_min_corroboration_property_in_file_order() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists"],
                "min_corroboration": {
                    "schema": 2,
                    "liveness": 3
                },
                "source_priority": {}
            }"#,
        );

        assert!(error.reason.contains("Schema"));
    }

    #[test]
    fn rejects_unsupported_source_priority_property() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists"],
                "min_corroboration": {},
                "source_priority": {
                    "depends_on": ["repo_scan"]
                }
            }"#,
        );

        assert!(error.reason.contains("source_priority"));
    }

    #[test]
    fn rejects_empty_source_priority_lists() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists"],
                "min_corroboration": {},
                "source_priority": {
                    "liveness": []
                }
            }"#,
        );

        assert!(error.reason.contains("at least one source kind"));
    }

    fn load_inline_policy(json: &str) -> Policy {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), json).unwrap();
        load_policy(file.path()).unwrap()
    }

    fn load_inline_policy_error(json: &str) -> super::PolicyRefusal {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), json).unwrap();
        load_policy(file.path()).unwrap_err()
    }
}
