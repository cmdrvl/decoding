//! Property-aware comparator registry and liveness fold logic.

use crate::contracts::vocabulary::PropertyType;

/// Result of comparing two claim values for compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Compatible,
    Incompatible,
}

/// Compare two claim values for compatibility under the given property type.
pub fn compare(
    _property_type: PropertyType,
    _a: &serde_json::Value,
    _b: &serde_json::Value,
) -> Compatibility {
    todo!("property-aware comparison dispatch")
}

/// Liveness fold: resolve compatible liveness states.
/// `alive` + `stale` -> compatible. `alive` + `dead` -> incompatible.
pub fn liveness_fold(_a: &serde_json::Value, _b: &serde_json::Value) -> Compatibility {
    todo!("liveness fold logic")
}
