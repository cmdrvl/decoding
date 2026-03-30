//! `claim.v0` input contract: parsing, validation, and refusal boundary.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::vocabulary::{PropertyType, SourceKind, SubjectRef, ValueRef};

/// A derived claim from a crucible scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    pub event: String,
    pub claim_id: String,
    pub source: ClaimSource,
    pub subject: SubjectRef,
    pub property_type: PropertyType,
    pub value: serde_json::Value,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimSource {
    pub kind: SourceKind,
    pub scanner: String,
    pub artifact_id: String,
    pub locator: SourceLocator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceLocator {
    pub kind: String,
    pub value: String,
}

impl Claim {
    /// Return the edge target as a typed value reference when this property uses edge buckets.
    pub fn value_ref(&self) -> Result<Option<ValueRef>, serde_json::Error> {
        if !self.property_type.is_edge() {
            return Ok(None);
        }

        serde_json::from_value(self.value.clone()).map(Some)
    }
}

/// Parse a single claim from a JSON line. Returns the claim or a refusal error.
pub fn parse_claim(line: &str) -> Result<Claim, ClaimRefusal> {
    let claim: Claim = serde_json::from_str(line)
        .map_err(|error| ClaimRefusal::new(format!("failed to parse claim JSON: {error}")))?;

    claim.validate()?;

    Ok(claim)
}

/// Refusal error for claims that violate the input contract.
#[derive(Debug)]
pub struct ClaimRefusal {
    pub reason: String,
}

impl ClaimRefusal {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for ClaimRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "claim refusal: {}", self.reason)
    }
}

impl std::error::Error for ClaimRefusal {}

impl Claim {
    fn validate(&self) -> Result<(), ClaimRefusal> {
        if self.event != "claim.v0" {
            return Err(ClaimRefusal::new(format!(
                "event must be claim.v0, got {}",
                self.event
            )));
        }

        validate_claim_id(&self.claim_id)?;
        validate_value_shape(self.property_type, &self.value)
    }
}

fn validate_claim_id(claim_id: &str) -> Result<(), ClaimRefusal> {
    let digest = claim_id
        .strip_prefix("sha256:")
        .ok_or_else(|| ClaimRefusal::new("claim_id must match sha256:<64 lowercase hex>"))?;

    let is_lowercase_hex = digest.len() == 64
        && digest
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));

    if is_lowercase_hex {
        Ok(())
    } else {
        Err(ClaimRefusal::new(
            "claim_id must match sha256:<64 lowercase hex>",
        ))
    }
}

fn validate_value_shape(property_type: PropertyType, value: &Value) -> Result<(), ClaimRefusal> {
    match property_type {
        PropertyType::Exists => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err(ClaimRefusal::new("exists value must be a boolean"))
            }
        }
        PropertyType::Schema | PropertyType::Constraint | PropertyType::Schedule => {
            if value.is_object() {
                Ok(())
            } else {
                Err(ClaimRefusal::new(format!(
                    "{property_type:?} value must be a JSON object"
                )))
            }
        }
        PropertyType::Reads
        | PropertyType::Writes
        | PropertyType::DependsOn
        | PropertyType::UsedBy
        | PropertyType::AuthoritativeFor => {
            serde_json::from_value::<ValueRef>(value.clone()).map_err(|error| {
                ClaimRefusal::new(format!(
                    "{} value must be a subject ref: {error}",
                    edge_property_name(property_type)
                ))
            })?;
            Ok(())
        }
        PropertyType::ValidValues => {
            let string_set: StringSetValue =
                serde_json::from_value(value.clone()).map_err(|error| {
                    ClaimRefusal::new(format!(
                        "valid_values value must be a string_set object: {error}"
                    ))
                })?;

            if string_set.kind != "string_set" {
                return Err(ClaimRefusal::new(
                    "valid_values value must use kind=string_set",
                ));
            }

            let _value_count = string_set.values.len();

            Ok(())
        }
        PropertyType::SemanticLabel => {
            let scalar: ScalarStringValue =
                serde_json::from_value(value.clone()).map_err(|error| {
                    ClaimRefusal::new(format!(
                        "semantic_label value must be a scalar string: {error}"
                    ))
                })?;

            if scalar.kind != "scalar" {
                return Err(ClaimRefusal::new(
                    "semantic_label value must use kind=scalar",
                ));
            }

            Ok(())
        }
        PropertyType::Liveness => {
            let scalar: ScalarStringValue =
                serde_json::from_value(value.clone()).map_err(|error| {
                    ClaimRefusal::new(format!("liveness value must be a scalar string: {error}"))
                })?;

            if scalar.kind != "scalar" {
                return Err(ClaimRefusal::new("liveness value must use kind=scalar"));
            }

            if matches!(
                scalar.value.as_str(),
                "alive" | "dead" | "stale" | "unknown"
            ) {
                Ok(())
            } else {
                Err(ClaimRefusal::new(
                    "liveness value must be one of alive, dead, stale, or unknown",
                ))
            }
        }
    }
}

fn edge_property_name(property_type: PropertyType) -> &'static str {
    match property_type {
        PropertyType::Reads => "reads",
        PropertyType::Writes => "writes",
        PropertyType::DependsOn => "depends_on",
        PropertyType::UsedBy => "used_by",
        PropertyType::AuthoritativeFor => "authoritative_for",
        _ => "value",
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScalarStringValue {
    kind: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StringSetValue {
    kind: String,
    values: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{ClaimRefusal, parse_claim};
    use crate::contracts::vocabulary::{PropertyType, SubjectKind};

    fn mixed_source_fixture() -> &'static str {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/claims/mixed_source.jsonl"
        ))
    }

    fn refusal_fixture() -> &'static str {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/claims/refusal_invalid.jsonl"
        ))
    }

    fn refusal_reason(line: &str) -> String {
        parse_claim(line).unwrap_err().reason
    }

    #[test]
    fn parse_claim_accepts_every_valid_fixture_line() {
        for line in mixed_source_fixture().lines() {
            let claim = parse_claim(line).unwrap();
            assert_eq!(claim.event, "claim.v0");
        }
    }

    #[test]
    fn parse_claim_extracts_typed_edge_value_ref() {
        let claim = parse_claim(mixed_source_fixture().lines().next().unwrap()).unwrap();
        let value_ref = claim.value_ref().unwrap().unwrap();

        assert_eq!(claim.property_type, PropertyType::DependsOn);
        assert_eq!(value_ref.kind, SubjectKind::Feed);
        assert_eq!(value_ref.id, "fdmee.actuals_load");
    }

    #[test]
    fn parse_claim_returns_no_value_ref_for_non_edge_property() {
        let claim = parse_claim(mixed_source_fixture().lines().nth(3).unwrap()).unwrap();

        assert_eq!(claim.property_type, PropertyType::Liveness);
        assert_eq!(claim.value_ref().unwrap(), None);
    }

    #[test]
    fn parse_claim_rejects_malformed_json_fixture_line() {
        let reason = refusal_reason(refusal_fixture().lines().next().unwrap());
        assert!(reason.contains("failed to parse claim JSON"));
    }

    #[test]
    fn parse_claim_rejects_missing_required_fields() {
        let reason = refusal_reason(refusal_fixture().lines().nth(1).unwrap());
        assert!(reason.contains("missing field `property_type`"));
    }

    #[test]
    fn parse_claim_rejects_unknown_source_kind() {
        let reason = refusal_reason(refusal_fixture().lines().nth(2).unwrap());
        assert!(reason.contains("unknown variant `api_scan`"));
    }

    #[test]
    fn parse_claim_rejects_unknown_subject_kind() {
        let reason = refusal_reason(refusal_fixture().lines().nth(3).unwrap());
        assert!(reason.contains("unknown variant `dashboard`"));
    }

    #[test]
    fn parse_claim_rejects_unknown_property_type() {
        let reason = refusal_reason(refusal_fixture().lines().nth(4).unwrap());
        assert!(reason.contains("unknown variant `owner`"));
    }

    #[test]
    fn parse_claim_rejects_malformed_claim_id() {
        let reason = refusal_reason(refusal_fixture().lines().nth(5).unwrap());
        assert_eq!(reason, "claim_id must match sha256:<64 lowercase hex>");
    }

    #[test]
    fn parse_claim_rejects_invalid_edge_value_shapes() {
        let line = r#"{"event":"claim.v0","claim_id":"sha256:abababababababababababababababababababababababababababababababab","source":{"kind":"repo_scan","scanner":"crucible.scan.repo@0.1.0","artifact_id":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","locator":{"kind":"file_range","value":"deps.sql#L1-L4"}},"subject":{"kind":"report","id":"hyperion.close_pack_ebitda"},"property_type":"depends_on","value":{"kind":"scalar","value":"fdmee.actuals_load"},"confidence":0.7}"#;

        let reason = refusal_reason(line);
        assert!(reason.contains("depends_on value must be a subject ref"));
    }

    #[test]
    fn parse_claim_rejects_invalid_liveness_states() {
        let line = r#"{"event":"claim.v0","claim_id":"sha256:bcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbcbc","source":{"kind":"db_scan","scanner":"crucible.scan.db@0.1.0","artifact_id":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","locator":{"kind":"table_row","value":"ops.feed_registry#fdmee.actuals_load"}},"subject":{"kind":"feed","id":"fdmee.actuals_load"},"property_type":"liveness","value":{"kind":"scalar","value":"zombie"},"confidence":0.8}"#;

        let reason = refusal_reason(line);
        assert_eq!(
            reason,
            "liveness value must be one of alive, dead, stale, or unknown"
        );
    }

    #[test]
    fn parse_claim_rejects_unexpected_event_names() {
        let line = r#"{"event":"canon_entry.v0","claim_id":"sha256:cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd","source":{"kind":"repo_scan","scanner":"crucible.scan.repo@0.1.0","artifact_id":"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","locator":{"kind":"file_range","value":"deps.sql#L1-L4"}},"subject":{"kind":"report","id":"hyperion.close_pack_ebitda"},"property_type":"depends_on","value":{"kind":"feed","id":"fdmee.actuals_load"},"confidence":0.7}"#;

        let reason = refusal_reason(line);
        assert_eq!(reason, "event must be claim.v0, got canon_entry.v0");
    }

    #[test]
    fn claim_refusal_display_includes_reason() {
        let refusal = ClaimRefusal {
            reason: "bad claim".to_string(),
        };

        assert_eq!(refusal.to_string(), "claim refusal: bad claim");
    }
}
