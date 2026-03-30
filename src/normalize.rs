//! Canonical JSON rendering, string normalization, sorted-set helpers, and hash helpers.

use sha2::{Digest, Sha256};

/// Compute a `sha256:<64 lowercase hex>` hash of the given bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let hash = Sha256::digest(data);
    format!("sha256:{}", hex::encode(hash))
}

/// Render a value as canonical JSON (sorted keys, no trailing whitespace).
pub fn canonical_json(_value: &serde_json::Value) -> String {
    todo!("canonical JSON rendering with sorted keys")
}

/// Normalize a string for comparison (trim, lowercase).
pub fn normalize_string(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Sort and deduplicate a set of strings for set comparison.
pub fn sorted_set(values: &[String]) -> Vec<String> {
    let mut sorted: Vec<String> = values.to_vec();
    sorted.sort();
    sorted.dedup();
    sorted
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
