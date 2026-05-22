use serde_json::{Value, json};

pub const CANONICAL_ROOT: &str = "~/.cmdrvl";
pub const MIGRATION_LOG: &str = "~/.cmdrvl/migrations/applied.jsonl";
pub const DEPRECATION_NOTICES: &str = "~/.cmdrvl/notices/deprecated-paths.jsonl";

pub fn config_footprint() -> Value {
    json!({
        "canonical_root": CANONICAL_ROOT,
        "managed_config_paths": [],
        "managed_state_paths": [],
        "managed_cache_paths": [],
        "migration_log": MIGRATION_LOG,
        "deprecation_notices": DEPRECATION_NOTICES,
        "legacy_paths": [],
        "legacy_migration_required": false,
        "explicit_output_policy": "canon_entry.v0 defaults to stdout; escalation, convergence, and alternate canon outputs are written only to explicit operator-supplied paths."
    })
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::config_footprint;

    #[test]
    fn footprint_declares_no_legacy_or_implicit_managed_paths() {
        let footprint = config_footprint();

        assert_eq!(footprint["canonical_root"], "~/.cmdrvl");
        assert_eq!(
            footprint
                .pointer("/managed_config_paths")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(
            footprint
                .pointer("/legacy_paths")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert_eq!(footprint["legacy_migration_required"], false);
    }
}
