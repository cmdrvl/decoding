//! Canonical JSON rendering, string normalization, sorted-set helpers, and hash helpers.

use sha2::{Digest, Sha256};

/// Compute a `sha256:<64 lowercase hex>` hash of the given bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    format!("sha256:{}", hex::encode(hash))
}

/// Render a value as canonical JSON (sorted keys, no trailing whitespace).
pub fn canonical_json(value: &serde_json::Value) -> String {
    let mut output = String::new();
    write_canonical_json(&mut output, value);
    output
}

/// Normalize a string for comparison (trim, lowercase).
pub fn normalize_string(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Sort and deduplicate a set of strings for set comparison.
pub fn sorted_set(values: &[String]) -> Vec<String> {
    let mut sorted: Vec<String> = values.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted
}

fn write_canonical_json(output: &mut String, value: &serde_json::Value) {
    match value {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => output.push_str(&value.to_string()),
        serde_json::Value::Array(values) => {
            output.push('[');
            for (index, item) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical_json(output, item);
            }
            output.push(']');
        }
        serde_json::Value::Object(map) => {
            output.push('{');

            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));

            for (index, (key, item)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }

                output.push_str(&serde_json::Value::String(key.clone()).to_string());
                output.push(':');
                write_canonical_json(output, item);
            }

            output.push('}');
        }
    }
}

// hex encoding helper — small inline implementation to avoid an extra crate.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().fold(String::new(), |mut s, b| {
            use std::fmt::Write;
            let _ = write!(s, "{b:02x}");
            s
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{canonical_json, normalize_string, sha256_hex, sorted_set};
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_keys_recursively() {
        let value = json!({
            "z": {
                "b": 2,
                "a": 1
            },
            "a": [
                {
                    "d": 4,
                    "c": 3
                },
                "keep-order"
            ]
        });

        assert_eq!(
            canonical_json(&value),
            r#"{"a":[{"c":3,"d":4},"keep-order"],"z":{"a":1,"b":2}}"#
        );
    }

    #[test]
    fn sha256_hex_uses_prefixed_lowercase_format() {
        assert_eq!(
            sha256_hex(b"decoding"),
            "sha256:b73f72d02dd75838f237b1a214eecf3394f8d02999fa3d6b85071627e6dd44fb"
        );
    }

    #[test]
    fn normalize_string_trims_and_lowercases() {
        assert_eq!(normalize_string("  HeLLo World\t"), "hello world");
    }

    #[test]
    fn sorted_set_orders_and_deduplicates_values() {
        let values = vec![
            "beta".to_string(),
            "alpha".to_string(),
            "beta".to_string(),
            "alpha".to_string(),
            "gamma".to_string(),
        ];

        assert_eq!(
            sorted_set(&values),
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]
        );
    }
}
