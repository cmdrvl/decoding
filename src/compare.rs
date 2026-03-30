//! Property-aware comparator registry and liveness fold logic.

use crate::contracts::vocabulary::{PropertyType, ValueRef};
use crate::normalize::{canonical_json, normalize_string, sorted_set};
use serde::Deserialize;

/// Result of comparing two claim values for compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compatibility {
    Compatible,
    Incompatible,
}

/// Compare two claim values for compatibility under the given property type.
pub fn compare(
    property_type: PropertyType,
    a: &serde_json::Value,
    b: &serde_json::Value,
) -> Compatibility {
    match property_type {
        PropertyType::Exists => compare_exists(a, b),
        PropertyType::Schema | PropertyType::Constraint | PropertyType::Schedule => {
            compare_canonical_json(a, b)
        }
        PropertyType::Reads
        | PropertyType::Writes
        | PropertyType::DependsOn
        | PropertyType::UsedBy
        | PropertyType::AuthoritativeFor => compare_value_refs(a, b),
        PropertyType::ValidValues => compare_valid_values(a, b),
        PropertyType::SemanticLabel => compare_semantic_labels(a, b),
        PropertyType::Liveness => liveness_fold(a, b),
    }
}

/// Liveness fold: resolve compatible liveness states.
/// `alive` + `stale` -> compatible. `alive` + `dead` -> incompatible.
pub fn liveness_fold(a: &serde_json::Value, b: &serde_json::Value) -> Compatibility {
    let Some(left) = parse_liveness_state(a) else {
        return Compatibility::Incompatible;
    };
    let Some(right) = parse_liveness_state(b) else {
        return Compatibility::Incompatible;
    };

    if left == right {
        return Compatibility::Compatible;
    }

    match (left, right) {
        (LivenessState::Alive, LivenessState::Stale)
        | (LivenessState::Stale, LivenessState::Alive)
        | (LivenessState::Stale, LivenessState::Unknown)
        | (LivenessState::Unknown, LivenessState::Stale) => Compatibility::Compatible,
        _ => Compatibility::Incompatible,
    }
}

fn compare_exists(a: &serde_json::Value, b: &serde_json::Value) -> Compatibility {
    if a.as_bool() == Some(true) && b.as_bool() == Some(true) {
        Compatibility::Compatible
    } else {
        Compatibility::Incompatible
    }
}

fn compare_canonical_json(a: &serde_json::Value, b: &serde_json::Value) -> Compatibility {
    if canonical_json(a) == canonical_json(b) {
        Compatibility::Compatible
    } else {
        Compatibility::Incompatible
    }
}

fn compare_value_refs(a: &serde_json::Value, b: &serde_json::Value) -> Compatibility {
    match (parse_value_ref(a), parse_value_ref(b)) {
        (Some(left), Some(right)) if left == right => Compatibility::Compatible,
        _ => Compatibility::Incompatible,
    }
}

fn compare_valid_values(a: &serde_json::Value, b: &serde_json::Value) -> Compatibility {
    match (parse_string_set(a), parse_string_set(b)) {
        (Some(left), Some(right)) if left == right => Compatibility::Compatible,
        _ => Compatibility::Incompatible,
    }
}

fn compare_semantic_labels(a: &serde_json::Value, b: &serde_json::Value) -> Compatibility {
    match (parse_scalar_string(a), parse_scalar_string(b)) {
        (Some(left), Some(right)) if normalize_string(&left) == normalize_string(&right) => {
            Compatibility::Compatible
        }
        _ => Compatibility::Incompatible,
    }
}

fn parse_value_ref(value: &serde_json::Value) -> Option<ValueRef> {
    serde_json::from_value(value.clone()).ok()
}

fn parse_string_set(value: &serde_json::Value) -> Option<Vec<String>> {
    let string_set: StringSetValue = serde_json::from_value(value.clone()).ok()?;
    if string_set.kind != "string_set" {
        return None;
    }

    Some(sorted_set(&string_set.values))
}

fn parse_liveness_state(value: &serde_json::Value) -> Option<LivenessState> {
    match normalize_string(&parse_scalar_string(value)?).as_str() {
        "alive" => Some(LivenessState::Alive),
        "dead" => Some(LivenessState::Dead),
        "stale" => Some(LivenessState::Stale),
        "unknown" => Some(LivenessState::Unknown),
        _ => None,
    }
}

fn parse_scalar_string(value: &serde_json::Value) -> Option<String> {
    let scalar: ScalarStringValue = serde_json::from_value(value.clone()).ok()?;
    if scalar.kind != "scalar" {
        return None;
    }

    Some(scalar.value)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LivenessState {
    Alive,
    Dead,
    Stale,
    Unknown,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Compatibility, compare, liveness_fold};
    use crate::contracts::vocabulary::PropertyType;

    #[test]
    fn exists_only_matches_true_claims() {
        assert_eq!(
            compare(PropertyType::Exists, &json!(true), &json!(true)),
            Compatibility::Compatible
        );
        assert_eq!(
            compare(PropertyType::Exists, &json!(false), &json!(true)),
            Compatibility::Incompatible
        );
        assert_eq!(
            compare(PropertyType::Exists, &json!(false), &json!(false)),
            Compatibility::Incompatible
        );
    }

    #[test]
    fn canonical_json_properties_ignore_object_key_order() {
        for property_type in [
            PropertyType::Schema,
            PropertyType::Constraint,
            PropertyType::Schedule,
        ] {
            assert_eq!(
                compare(
                    property_type,
                    &json!({"kind":"cron","spec":{"minute":0,"hour":1}}),
                    &json!({"spec":{"hour":1,"minute":0},"kind":"cron"}),
                ),
                Compatibility::Compatible
            );

            assert_eq!(
                compare(
                    property_type,
                    &json!({"kind":"cron","spec":{"minute":0,"hour":1}}),
                    &json!({"kind":"cron","spec":{"minute":15,"hour":1}}),
                ),
                Compatibility::Incompatible
            );
        }
    }

    #[test]
    fn edge_properties_require_the_same_subject_ref() {
        for property_type in [
            PropertyType::Reads,
            PropertyType::Writes,
            PropertyType::DependsOn,
            PropertyType::UsedBy,
            PropertyType::AuthoritativeFor,
        ] {
            assert_eq!(
                compare(
                    property_type,
                    &json!({"kind":"feed","id":"fdmee.actuals_load"}),
                    &json!({"kind":"feed","id":"fdmee.actuals_load"}),
                ),
                Compatibility::Compatible
            );

            assert_eq!(
                compare(
                    property_type,
                    &json!({"kind":"feed","id":"fdmee.actuals_load"}),
                    &json!({"kind":"feed","id":"fdmee.budget_load"}),
                ),
                Compatibility::Incompatible
            );
        }
    }

    #[test]
    fn valid_values_compare_as_sorted_sets() {
        assert_eq!(
            compare(
                PropertyType::ValidValues,
                &json!({"kind":"string_set","values":["beta", "alpha", "beta"]}),
                &json!({"kind":"string_set","values":["alpha", "beta"]}),
            ),
            Compatibility::Compatible
        );

        assert_eq!(
            compare(
                PropertyType::ValidValues,
                &json!({"kind":"string_set","values":["alpha", "gamma"]}),
                &json!({"kind":"string_set","values":["alpha", "beta"]}),
            ),
            Compatibility::Incompatible
        );
    }

    #[test]
    fn semantic_labels_compare_after_normalization() {
        assert_eq!(
            compare(
                PropertyType::SemanticLabel,
                &json!({"kind":"scalar","value":" Adjusted EBITDA "}),
                &json!({"kind":"scalar","value":"adjusted ebitda"}),
            ),
            Compatibility::Compatible
        );

        assert_eq!(
            compare(
                PropertyType::SemanticLabel,
                &json!({"kind":"scalar","value":"Adjusted EBITDA"}),
                &json!({"kind":"scalar","value":"Revenue"}),
            ),
            Compatibility::Incompatible
        );
    }

    #[test]
    fn liveness_fold_matches_phase_one_rules() {
        assert_eq!(
            liveness_fold(
                &json!({"kind":"scalar","value":"alive"}),
                &json!({"kind":"scalar","value":"alive"}),
            ),
            Compatibility::Compatible
        );
        assert_eq!(
            liveness_fold(
                &json!({"kind":"scalar","value":"alive"}),
                &json!({"kind":"scalar","value":"stale"}),
            ),
            Compatibility::Compatible
        );
        assert_eq!(
            liveness_fold(
                &json!({"kind":"scalar","value":"stale"}),
                &json!({"kind":"scalar","value":"unknown"}),
            ),
            Compatibility::Compatible
        );
        assert_eq!(
            liveness_fold(
                &json!({"kind":"scalar","value":"alive"}),
                &json!({"kind":"scalar","value":"dead"}),
            ),
            Compatibility::Incompatible
        );
        assert_eq!(
            liveness_fold(
                &json!({"kind":"scalar","value":"alive"}),
                &json!({"kind":"scalar","value":"unknown"}),
            ),
            Compatibility::Incompatible
        );
    }
}
