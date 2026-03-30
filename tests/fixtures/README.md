# Archaeology Fixture Corpus

This directory holds shared Phase 1 archaeology fixture assets only.

Use these files to exercise the frozen contract surface without coupling tests to a specific loader implementation.

## Files

- `claims/mixed_source.jsonl`
  - Valid `claim.v0` corpus with 12 claims from `repo_scan`, `db_scan`, and `file_scan`.
  - Covers converging edge claims, converging schedule claims, compatible `liveness` evidence, conflicting `semantic_label` claims, and single-source `valid_values` / `authoritative_for` claims.
  - Intended for mixed-source fixture tests, replay determinism, bucketing, corroboration, and escalation coverage.

- `claims/refusal_invalid.jsonl`
  - Line-oriented refusal corpus with one invalid case per line.
  - Covers malformed JSON, missing required fields, unknown `source.kind`, unknown `subject.kind`, unknown `property_type`, and malformed `claim_id`.
  - Intended for contract refusal tests that verify Phase 1 rejects invalid inputs before bucket handling.

- `policies/legacy.decode.v0.json`
  - Minimal Phase 1 archaeology policy fixture copied from the plan surface.
  - Intended for policy parsing, refusal-boundary tests, and end-to-end archaeology fixture runs.

## Notes

- All valid claim IDs use the `sha256:<64 lowercase hex>` format.
- Edge-property values are encoded as subject refs with `kind` and `id`.
- Scalar-like values use a small tagged object so future tests can distinguish scalar and subject-ref payloads without guessing.
