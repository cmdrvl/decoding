//! Family contract enforcement through spine-rules `golden_rules_suite!`.

use spine_rules::golden_rules_suite;

golden_rules_suite! {
    binary: env!("CARGO_BIN_EXE_decoding"),
    operator_json: include_str!("../operator.json"),
    source_files: &["src/cli.rs"],
}
