//! Render canon_entry.v0 and escalation.v0 records to JSONL output.

use std::collections::BTreeMap;
use std::io::Write;

use crate::contracts::canon_entry::CanonEntry;
use crate::contracts::escalation::{CandidateValue, Escalation, ScalarCandidateKind};
use crate::contracts::vocabulary::SubjectKind;
use crate::normalize::canonical_json;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

#[derive(serde::Serialize)]
#[serde(untagged)]
enum RedactedCandidate {
    Scalar {
        kind: ScalarCandidateKind,
        value: serde_json::Value,
    },
    Ref {
        kind: SubjectKind,
        id: serde_json::Value,
    },
}

#[derive(serde::Serialize)]
struct RedactedEscalation {
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<crate::contracts::claim::ClaimContext>,
    event: String,
    bucket_id: String,
    subject: crate::contracts::vocabulary::SubjectRef,
    property_type: crate::contracts::vocabulary::PropertyType,
    reason: crate::contracts::escalation::EscalationReason,
    claim_ids: Vec<String>,
    candidate_values: Vec<RedactedCandidate>,
    recommended_action: crate::contracts::escalation::RecommendedAction,
    summary: String,
}

/// The redaction mode for one emitted escalation output set.
pub enum RedactionMode {
    Explicit,
    Ordinal,
    Keyed(Vec<u8>),
}

impl RedactionMode {
    pub fn explicit() -> Self {
        Self::Explicit
    }

    pub fn ordinal() -> Self {
        Self::Ordinal
    }

    pub fn keyed(key: Vec<u8>) -> Self {
        Self::Keyed(key)
    }
}

/// Tracks redaction equality and ordinal state across one output set.
pub struct Redactor {
    mode: RedactionMode,
    ordinals: BTreeMap<String, String>,
    next_ordinal: usize,
}

impl Redactor {
    pub fn new(mode: RedactionMode) -> Self {
        Self {
            mode,
            ordinals: BTreeMap::new(),
            next_ordinal: 1,
        }
    }

    fn is_explicit(&self) -> bool {
        matches!(self.mode, RedactionMode::Explicit)
    }

    fn redact_candidate(&mut self, candidate: &CandidateValue) -> serde_json::Value {
        let value = serde_json::to_value(candidate).expect("candidate values must be serializable");
        let canonical = canonical_json(&value);
        match &self.mode {
            RedactionMode::Explicit => unreachable!("explicit output bypasses redaction"),
            RedactionMode::Ordinal => {
                let reference = match self.ordinals.get(&canonical) {
                    Some(reference) => reference.clone(),
                    None => {
                        let reference = format!("v{}", self.next_ordinal);
                        self.next_ordinal += 1;
                        self.ordinals.insert(canonical, reference.clone());
                        reference
                    }
                };
                serde_json::json!({
                    "redacted": true,
                    "ref": reference,
                    "scope": "ordinal",
                })
            }
            RedactionMode::Keyed(key) => {
                let mut mac = HmacSha256::new_from_slice(key)
                    .expect("HMAC-SHA256 accepts keys of any non-empty length");
                mac.update(canonical.as_bytes());
                let digest = mac.finalize().into_bytes();
                serde_json::json!({
                    "redacted": true,
                    "marker": format!("hmac-sha256:{}", hex_lower(&digest)),
                    "scope": "key",
                })
            }
        }
    }
}

/// Write a canon entry as a single JSONL line.
pub fn write_canon_entry(
    writer: &mut dyn Write,
    entry: &CanonEntry,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entry = entry.clone();
    entry.explain.winner_claim_ids.sort_unstable();
    entry.explain.compatible_claim_ids.sort_unstable();

    serde_json::to_writer(&mut *writer, &entry)?;
    writer.write_all(b"\n")?;
    Ok(())
}

/// Write an escalation as a single JSONL line.
pub fn write_escalation(
    writer: &mut dyn Write,
    escalation: &Escalation,
    explicit: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mode = if explicit {
        RedactionMode::explicit()
    } else {
        RedactionMode::ordinal()
    };
    let mut redactor = Redactor::new(mode);
    write_escalation_with_redactor(writer, escalation, &mut redactor)
}

/// Write an escalation using redaction state shared by one output set.
pub fn write_escalation_with_redactor(
    writer: &mut dyn Write,
    escalation: &Escalation,
    redactor: &mut Redactor,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut escalation = escalation.clone();
    escalation.claim_ids.sort_unstable();
    escalation.candidate_values.sort_by_key(candidate_sort_key);
    if !redactor.is_explicit() {
        let redacted_candidates = escalation
            .candidate_values
            .iter()
            .map(|candidate| {
                let marker = redactor.redact_candidate(candidate);
                match candidate {
                    CandidateValue::Scalar(scalar) => RedactedCandidate::Scalar {
                        kind: scalar.kind,
                        value: marker,
                    },
                    CandidateValue::Ref(reference) => RedactedCandidate::Ref {
                        kind: reference.kind,
                        id: marker,
                    },
                }
            })
            .collect::<Vec<_>>();
        let rendered = RedactedEscalation {
            context: escalation.context,
            event: escalation.event,
            bucket_id: escalation.bucket_id,
            subject: escalation.subject,
            property_type: escalation.property_type,
            reason: escalation.reason,
            claim_ids: escalation.claim_ids,
            candidate_values: redacted_candidates,
            recommended_action: escalation.recommended_action,
            summary: escalation.summary,
        };
        serde_json::to_writer(&mut *writer, &rendered)?;
    } else {
        serde_json::to_writer(&mut *writer, &escalation)?;
    }
    writer.write_all(b"\n")?;
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn candidate_sort_key(candidate: &CandidateValue) -> String {
    match serde_json::to_value(candidate) {
        Ok(value) => canonical_json(&value),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        RedactionMode, Redactor, write_canon_entry, write_escalation,
        write_escalation_with_redactor,
    };
    use crate::contracts::canon_entry::{
        CanonEntry, ConvergenceState, ConvergenceStateKind, Explanation, ResolutionKind,
    };
    use crate::contracts::escalation::{
        CandidateValue, Escalation, EscalationReason, RecommendedAction, ScalarCandidateKind,
        ScalarCandidateValue,
    };
    use crate::contracts::vocabulary::{PropertyType, SubjectKind, SubjectRef, ValueRef};

    #[test]
    fn writes_canon_entry_jsonl_with_sorted_explain_ids() {
        let entry = CanonEntry {
            context: None,
            event: "canon_entry.v0".to_string(),
            bucket_id: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Report,
                id: "hyperion.close_pack_ebitda".to_string(),
            },
            property_type: PropertyType::DependsOn,
            canonical_value: json!({"kind":"feed","id":"fdmee.actuals_load"}),
            policy_id: "legacy.decode.v0".to_string(),
            convergence: ConvergenceState {
                state: ConvergenceStateKind::Converged,
                source_count: 3,
                claim_count: 3,
            },
            explain: Explanation {
                winner_claim_ids: vec![
                    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                        .to_string(),
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                ],
                compatible_claim_ids: vec![
                    "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                        .to_string(),
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                ],
                resolution_kind: ResolutionKind::Corroborated,
                scale_inference: None,
            },
        };

        let mut output = Vec::new();
        write_canon_entry(&mut output, &entry).unwrap();

        let rendered = String::from_utf8(output).unwrap();
        let expected = serde_json::to_string(&CanonEntry {
            explain: Explanation {
                winner_claim_ids: vec![
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                    "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                        .to_string(),
                ],
                compatible_claim_ids: vec![
                    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                        .to_string(),
                    "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
                        .to_string(),
                ],
                scale_inference: None,
                ..entry.explain.clone()
            },
            ..entry.clone()
        })
        .unwrap()
            + "\n";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn writes_escalation_jsonl_with_sorted_claim_ids_and_candidate_values() {
        let escalation = Escalation {
            context: None,
            event: "escalation.v0".to_string(),
            bucket_id: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Mapping,
                id: "adj.ebitda.rule.family".to_string(),
            },
            property_type: PropertyType::SemanticLabel,
            reason: EscalationReason::Conflicted,
            claim_ids: vec![
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
            ],
            candidate_values: vec![
                CandidateValue::Scalar(ScalarCandidateValue {
                    kind: ScalarCandidateKind::Scalar,
                    value: json!("Adjusted EBITDA rule family"),
                }),
                CandidateValue::Ref(ValueRef {
                    kind: SubjectKind::Feed,
                    id: "fdmee.actuals_load".to_string(),
                }),
            ],
            recommended_action: RecommendedAction::Review,
            summary: "2 incompatible candidate values remain".to_string(),
        };

        let mut output = Vec::new();
        write_escalation(&mut output, &escalation, true).unwrap();

        let rendered = String::from_utf8(output).unwrap();
        let expected = serde_json::to_string(&Escalation {
            claim_ids: vec![
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .to_string(),
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                    .to_string(),
            ],
            candidate_values: vec![
                CandidateValue::Ref(ValueRef {
                    kind: SubjectKind::Feed,
                    id: "fdmee.actuals_load".to_string(),
                }),
                CandidateValue::Scalar(ScalarCandidateValue {
                    kind: ScalarCandidateKind::Scalar,
                    value: json!("Adjusted EBITDA rule family"),
                }),
            ],
            ..escalation.clone()
        })
        .unwrap()
            + "\n";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn redacts_candidate_values_without_explicit_mode() {
        let escalation = Escalation {
            context: None,
            event: "escalation.v0".to_string(),
            bucket_id: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Mapping,
                id: "adj.ebitda.rule.family".to_string(),
            },
            property_type: PropertyType::SemanticLabel,
            reason: EscalationReason::Conflicted,
            claim_ids: Vec::new(),
            candidate_values: vec![CandidateValue::Scalar(ScalarCandidateValue {
                kind: ScalarCandidateKind::Scalar,
                value: json!("CANARY-SENSITIVE-VALUE"),
            })],
            recommended_action: RecommendedAction::Review,
            summary: "one incompatible candidate value remains".to_string(),
        };

        let mut output = Vec::new();
        write_escalation(&mut output, &escalation, false).unwrap();
        let rendered = String::from_utf8(output).unwrap();

        assert!(!rendered.contains("CANARY-SENSITIVE-VALUE"));
        assert!(rendered.contains("\"redacted\":true"));
        assert!(rendered.contains("\"ref\":\"v1\""));
        assert!(rendered.contains("\"scope\":\"ordinal\""));
        assert!(!rendered.contains("\"sha256\""));
    }

    #[test]
    fn redaction_preserves_scalar_and_reference_discriminators() {
        let escalation = Escalation {
            context: None,
            event: "escalation.v0".to_string(),
            bucket_id: "sha256:4444444444444444444444444444444444444444444444444444444444444444"
                .to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Mapping,
                id: "redaction.shape.test".to_string(),
            },
            property_type: PropertyType::DependsOn,
            reason: EscalationReason::Conflicted,
            claim_ids: Vec::new(),
            candidate_values: vec![
                CandidateValue::Scalar(ScalarCandidateValue {
                    kind: ScalarCandidateKind::Scalar,
                    value: json!("SCALAR-CANARY-VALUE"),
                }),
                CandidateValue::Ref(ValueRef {
                    kind: SubjectKind::Consumer,
                    id: "REFERENCE-CANARY-ID".to_string(),
                }),
            ],
            recommended_action: RecommendedAction::Review,
            summary: "two incompatible candidate values remain".to_string(),
        };

        for mode in [
            RedactionMode::ordinal(),
            RedactionMode::keyed(b"0123456789abcdef".to_vec()),
        ] {
            let mut output = Vec::new();
            let mut redactor = Redactor::new(mode);
            write_escalation_with_redactor(&mut output, &escalation, &mut redactor).unwrap();
            let rendered: serde_json::Value = serde_json::from_slice(&output).unwrap();
            let candidates = rendered["candidate_values"].as_array().unwrap();

            let scalar = candidates
                .iter()
                .find(|candidate| candidate["kind"] == "scalar")
                .unwrap();
            assert_eq!(scalar["value"]["redacted"], true);
            assert!(!output.starts_with(b"SCALAR-CANARY-VALUE"));
            assert!(!String::from_utf8_lossy(&output).contains("SCALAR-CANARY-VALUE"));

            let reference = candidates
                .iter()
                .find(|candidate| candidate["kind"] == "consumer")
                .unwrap();
            assert_eq!(reference["id"]["redacted"], true);
            assert!(!String::from_utf8_lossy(&output).contains("REFERENCE-CANARY-ID"));
        }

        let mut explicit_output = Vec::new();
        write_escalation(&mut explicit_output, &escalation, true).unwrap();
        let explicit = String::from_utf8(explicit_output).unwrap();
        assert!(explicit.contains("SCALAR-CANARY-VALUE"));
        assert!(explicit.contains("\"kind\":\"consumer\",\"id\":\"REFERENCE-CANARY-ID\""));
    }

    #[test]
    fn ordinal_redaction_is_first_appearance_and_shared_across_output_set() {
        let first = escalation_with_values(&["beta", "alpha", "beta"], "first");
        let second = escalation_with_values(&["gamma", "alpha"], "unrelated summary");
        let mut output = Vec::new();
        let mut redactor = Redactor::new(RedactionMode::ordinal());

        write_escalation_with_redactor(&mut output, &first, &mut redactor).unwrap();
        write_escalation_with_redactor(&mut output, &second, &mut redactor).unwrap();

        let rows = output
            .split(|byte| *byte == b'\n')
            .filter(|row| !row.is_empty())
            .map(|row| serde_json::from_slice::<serde_json::Value>(row).unwrap())
            .collect::<Vec<_>>();
        let first_refs = rows[0]["candidate_values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|candidate| candidate["value"]["ref"].as_str().unwrap())
            .collect::<Vec<_>>();
        let second_refs = rows[1]["candidate_values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|candidate| candidate["value"]["ref"].as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(first_refs, ["v1", "v2", "v2"]);
        // Candidate values are emitted in canonical order, so alpha's v1 is
        // encountered before gamma's v3 in the second escalation.
        assert_eq!(second_refs, ["v1", "v3"]);
        assert_eq!(rows[0]["candidate_values"][0]["value"]["scope"], "ordinal");
    }

    #[test]
    fn keyed_redaction_matches_hmac_sha256_known_answer() {
        let escalation = escalation_with_values(&["CANARY-SENSITIVE-VALUE"], "known answer");
        let mut output = Vec::new();
        let mut redactor = Redactor::new(RedactionMode::keyed(b"0123456789abcdef".to_vec()));

        write_escalation_with_redactor(&mut output, &escalation, &mut redactor).unwrap();

        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains(
            "hmac-sha256:4b46ceaec08be6577e32390c43e3b66e2d92a88d0ea5e4f4e542bc3b154e9672"
        ));
        assert!(rendered.contains("\"scope\":\"key\""));
        assert!(!rendered.contains("0123456789abcdef"));
        assert!(!rendered.contains("CANARY-SENSITIVE-VALUE"));
    }

    fn escalation_with_values(values: &[&str], summary: &str) -> Escalation {
        Escalation {
            context: None,
            event: "escalation.v0".to_string(),
            bucket_id: "sha256:3333333333333333333333333333333333333333333333333333333333333333"
                .to_string(),
            subject: SubjectRef {
                kind: SubjectKind::Mapping,
                id: "redaction.test".to_string(),
            },
            property_type: PropertyType::SemanticLabel,
            reason: EscalationReason::Conflicted,
            claim_ids: Vec::new(),
            candidate_values: values
                .iter()
                .map(|value| {
                    CandidateValue::Scalar(ScalarCandidateValue {
                        kind: ScalarCandidateKind::Scalar,
                        value: json!(value),
                    })
                })
                .collect(),
            recommended_action: RecommendedAction::Review,
            summary: summary.to_string(),
        }
    }
}
