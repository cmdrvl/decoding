//! `canon_entry.v0` output contract.

use serde::{Deserialize, Serialize};

use super::vocabulary::{PropertyType, SubjectRef};

/// A resolved canonical entry emitted for a converged bucket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonEntry {
    pub event: String,
    pub bucket_id: String,
    pub subject: SubjectRef,
    pub property_type: PropertyType,
    pub canonical_value: serde_json::Value,
    pub policy_id: String,
    pub convergence: ConvergenceState,
    pub explain: Explanation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConvergenceState {
    pub state: String,
    pub source_count: usize,
    pub claim_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Explanation {
    pub winner_claim_ids: Vec<String>,
    pub compatible_claim_ids: Vec<String>,
    pub resolution_kind: ResolutionKind,
}

/// How the canonical value was chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionKind {
    SingleSource,
    Corroborated,
    PriorityBreak,
    LivenessFold,
}
