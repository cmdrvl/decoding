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
    pub numeric_tolerance: IndexMap<PropertyType, NumericTolerance>,
}

/// Absolute and relative compatibility thresholds for numeric scalar claims.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NumericTolerance {
    pub relative_percent: Option<f64>,
    pub absolute: Option<f64>,
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

    /// Numeric tolerance for a property type, when one is configured.
    pub fn numeric_tolerance_for(&self, property_type: PropertyType) -> Option<&NumericTolerance> {
        self.numeric_tolerance.get(&property_type)
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
                    | PropertyType::NumericScalar
                    | PropertyType::SemanticLabel
                    | PropertyType::AuthoritativeFor
            ) {
                return Err(PolicyRefusal::new(format!(
                    "property `{property_type:?}` is not allowed in min_corroboration"
                )));
            }
        }

        for (property_type, priorities) in &self.source_priority {
            if !matches!(
                property_type,
                PropertyType::Liveness | PropertyType::NumericScalar
            ) {
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

        for (property_type, tolerance) in &self.numeric_tolerance {
            if *property_type != PropertyType::NumericScalar {
                return Err(PolicyRefusal::new(format!(
                    "property `{property_type:?}` is not allowed in numeric_tolerance"
                )));
            }

            validate_numeric_tolerance(tolerance)?;
        }

        Ok(self)
    }
}

fn validate_numeric_tolerance(tolerance: &NumericTolerance) -> Result<(), PolicyRefusal> {
    if tolerance.relative_percent.is_none() && tolerance.absolute.is_none() {
        return Err(PolicyRefusal::new(
            "numeric_tolerance entries must provide relative_percent or absolute",
        ));
    }

    validate_optional_non_negative_finite("relative_percent", tolerance.relative_percent)?;
    validate_optional_non_negative_finite("absolute", tolerance.absolute)?;

    Ok(())
}

fn validate_optional_non_negative_finite(
    name: &str,
    value: Option<f64>,
) -> Result<(), PolicyRefusal> {
    let Some(value) = value else {
        return Ok(());
    };

    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(PolicyRefusal::new(format!(
            "numeric_tolerance {name} must be a non-negative finite number"
        )))
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

    use super::{NumericTolerance, PHASE_ONE_POLICY_ID, Policy, load_policy};
    use crate::contracts::vocabulary::{PropertyType, SourceKind};

    #[test]
    fn loads_phase_one_policy_with_typed_accessors() {
        let policy = load_inline_policy(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists", "schema", "constraint"],
                "min_corroboration": {
                    "reads": 2,
                    "valid_values": 3,
                    "numeric_scalar": 2
                },
                "source_priority": {
                    "liveness": ["db_scan", "file_scan", "repo_scan"],
                    "numeric_scalar": ["sec_xbrl", "dera", "balance_sheet", "parser_extraction"]
                },
                "numeric_tolerance": {
                    "numeric_scalar": {
                        "relative_percent": 0.01,
                        "absolute": 1000000
                    }
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
        assert_eq!(
            policy.source_priority_for(PropertyType::NumericScalar),
            Some(
                &[
                    SourceKind::SecXbrl,
                    SourceKind::Dera,
                    SourceKind::BalanceSheet,
                    SourceKind::ParserExtraction,
                ][..]
            )
        );
        assert_eq!(
            policy.numeric_tolerance_for(PropertyType::NumericScalar),
            Some(&NumericTolerance {
                relative_percent: Some(0.01),
                absolute: Some(1_000_000.0),
            })
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
                "numeric_tolerance": {},
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
                "source_priority": {},
                "numeric_tolerance": {}
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
                "source_priority": {},
                "numeric_tolerance": {}
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
                "source_priority": {},
                "numeric_tolerance": {}
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
                "source_priority": {},
                "numeric_tolerance": {}
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
                },
                "numeric_tolerance": {}
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
                },
                "numeric_tolerance": {}
            }"#,
        );

        assert!(error.reason.contains("at least one source kind"));
    }

    #[test]
    fn rejects_unsupported_numeric_tolerance_property() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists"],
                "min_corroboration": {},
                "source_priority": {},
                "numeric_tolerance": {
                    "valid_values": {
                        "absolute": 1
                    }
                }
            }"#,
        );

        assert!(error.reason.contains("numeric_tolerance"));
    }

    #[test]
    fn rejects_empty_numeric_tolerance() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists"],
                "min_corroboration": {},
                "source_priority": {},
                "numeric_tolerance": {
                    "numeric_scalar": {}
                }
            }"#,
        );

        assert!(error.reason.contains("relative_percent or absolute"));
    }

    #[test]
    fn rejects_negative_numeric_tolerance() {
        let error = load_inline_policy_error(
            r#"{
                "policy_id": "legacy.decode.v0",
                "auto_resolve": ["exists"],
                "min_corroboration": {},
                "source_priority": {},
                "numeric_tolerance": {
                    "numeric_scalar": {
                        "relative_percent": -0.1
                    }
                }
            }"#,
        );

        assert!(error.reason.contains("non-negative finite"));
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
