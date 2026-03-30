//! `claim.v0` input contract: parsing, validation, and refusal boundary.

use serde::{Deserialize, Serialize};

use super::vocabulary::{PropertyType, SourceKind, SubjectKind};

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

/// A reference to a subject or value entity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SubjectRef {
    pub kind: SubjectKind,
    pub id: String,
}

/// Parse a single claim from a JSON line. Returns the claim or a refusal error.
pub fn parse_claim(_line: &str) -> Result<Claim, ClaimRefusal> {
    todo!("claim parsing and validation")
}

/// Refusal error for claims that violate the input contract.
#[derive(Debug)]
pub struct ClaimRefusal {
    pub reason: String,
}

impl std::fmt::Display for ClaimRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "claim refusal: {}", self.reason)
    }
}

impl std::error::Error for ClaimRefusal {}
