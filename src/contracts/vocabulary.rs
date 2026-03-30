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
