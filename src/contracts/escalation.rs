//! `escalation.v0` output contract.

use serde::{Deserialize, Serialize};

use super::vocabulary::{PropertyType, SubjectRef};

/// An escalation emitted for an unresolved or conflicting bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Escalation {
    pub event: String,
    pub bucket_id: String,
    pub subject: SubjectRef,
    pub property_type: PropertyType,
    pub reason: EscalationReason,
    pub claim_ids: Vec<String>,
    pub candidate_values: Vec<serde_json::Value>,
    pub recommended_action: RecommendedAction,
    pub summary: String,
}

/// Why the bucket was escalated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationReason {
    Conflicted,
    MissingCorroboration,
    NoResolutionPath,
}

/// What the operator should do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedAction {
    Review,
    ScanMore,
    FixScanner,
    FixPolicy,
}
