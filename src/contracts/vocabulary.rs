//! Frozen Phase 1 vocabulary enums: source kinds, subject kinds, and property types.

use serde::{Deserialize, Serialize};

/// Known source kinds for Phase 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    RepoScan,
    DbScan,
    FileScan,
}

/// Known subject kinds for Phase 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubjectKind {
    Table,
    Column,
    View,
    Job,
    Procedure,
    Report,
    Feed,
    Mapping,
    Consumer,
    Artifact,
    Extract,
    ReportLine,
}

/// Frozen Phase 1 property types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    Exists,
    Schema,
    Constraint,
    Reads,
    Writes,
    DependsOn,
    UsedBy,
    Schedule,
    ValidValues,
    SemanticLabel,
    Liveness,
    AuthoritativeFor,
}

impl PropertyType {
    /// Whether this property type uses edge bucket keys (includes value ref in key).
    pub fn is_edge(&self) -> bool {
        matches!(
            self,
            PropertyType::Reads
                | PropertyType::Writes
                | PropertyType::DependsOn
                | PropertyType::UsedBy
                | PropertyType::AuthoritativeFor
        )
    }
}

/// A reference to an archaeology subject.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SubjectRef {
    pub kind: SubjectKind,
    pub id: String,
}

/// A reference to an edge-property target value.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueRef {
    pub kind: SubjectKind,
    pub id: String,
}

impl From<SubjectRef> for ValueRef {
    fn from(subject: SubjectRef) -> Self {
        Self {
            kind: subject.kind,
            id: subject.id,
        }
    }
}

impl From<ValueRef> for SubjectRef {
    fn from(value: ValueRef) -> Self {
        Self {
            kind: value.kind,
            id: value.id,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{PropertyType, SourceKind, SubjectKind, SubjectRef, ValueRef};

    #[test]
    fn source_kind_accepts_frozen_values() {
        let parsed: SourceKind = serde_json::from_str("\"repo_scan\"").unwrap();
        assert_eq!(parsed, SourceKind::RepoScan);
    }

    #[test]
    fn source_kind_rejects_unknown_values() {
        let error = serde_json::from_str::<SourceKind>("\"api_scan\"").unwrap_err();
        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn subject_kind_accepts_frozen_values() {
        let parsed: SubjectKind = serde_json::from_str("\"report_line\"").unwrap();
        assert_eq!(parsed, SubjectKind::ReportLine);
    }

    #[test]
    fn subject_kind_rejects_unknown_values() {
        let error = serde_json::from_str::<SubjectKind>("\"dashboard\"").unwrap_err();
        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn property_type_accepts_frozen_values() {
        let parsed: PropertyType = serde_json::from_str("\"authoritative_for\"").unwrap();
        assert_eq!(parsed, PropertyType::AuthoritativeFor);
    }

    #[test]
    fn property_type_rejects_unknown_values() {
        let error = serde_json::from_str::<PropertyType>("\"refreshes\"").unwrap_err();
        assert!(error.to_string().contains("unknown variant"));
    }

    #[test]
    fn subject_ref_round_trips() {
        let parsed: SubjectRef =
            serde_json::from_str(r#"{"kind":"report","id":"hyperion.close_pack"}"#).unwrap();
        assert_eq!(parsed.kind, SubjectKind::Report);
        assert_eq!(parsed.id, "hyperion.close_pack");
    }

    #[test]
    fn value_ref_round_trips() {
        let parsed: ValueRef =
            serde_json::from_str(r#"{"kind":"feed","id":"fdmee.actuals_load"}"#).unwrap();
        assert_eq!(parsed.kind, SubjectKind::Feed);
        assert_eq!(parsed.id, "fdmee.actuals_load");
    }

    #[test]
    fn property_type_deserializes_as_policy_map_key() {
        let parsed: HashMap<PropertyType, usize> =
            serde_json::from_str(r#"{"reads":2,"schedule":3}"#).unwrap();

        assert_eq!(parsed.get(&PropertyType::Reads), Some(&2));
        assert_eq!(parsed.get(&PropertyType::Schedule), Some(&3));
    }

    #[test]
    fn source_kind_deserializes_in_priority_lists() {
        let parsed: HashMap<PropertyType, Vec<SourceKind>> =
            serde_json::from_str(r#"{"liveness":["db_scan","file_scan","repo_scan"]}"#).unwrap();

        assert_eq!(
            parsed.get(&PropertyType::Liveness),
            Some(&vec![
                SourceKind::DbScan,
                SourceKind::FileScan,
                SourceKind::RepoScan,
            ])
        );
    }

    #[test]
    fn edge_property_detection_matches_phase_one_plan() {
        assert!(PropertyType::DependsOn.is_edge());
        assert!(PropertyType::AuthoritativeFor.is_edge());
        assert!(!PropertyType::Schema.is_edge());
        assert!(!PropertyType::Liveness.is_edge());
    }
}
