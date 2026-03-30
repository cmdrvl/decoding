//! `legacy.decode.v0` policy loader and validator.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::vocabulary::PropertyType;

/// The decoded archaeology policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub policy_id: String,
    pub auto_resolve: Vec<PropertyType>,
    pub min_corroboration: HashMap<String, usize>,
    pub source_priority: HashMap<String, Vec<String>>,
}

/// Load and validate a policy file. Refuses on unknown keys.
pub fn load_policy(_path: &Path) -> Result<Policy, PolicyRefusal> {
    todo!("policy loading and validation")
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
