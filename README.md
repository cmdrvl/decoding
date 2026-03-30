# decoding

<div align="center">

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Multiple legacy systems claim conflicting things about the same entity. decoding settles it.**

```bash
brew install cmdrvl/tap/decoding
```

</div>

---

A legacy estate has dozens of overlapping systems that each "know" something about the same report, feed, or mapping — but they disagree. One scan says a feed is alive, another says it's stale, a third doesn't mention it. One source says a report depends on three tables, another says two. These aren't parsing errors — they're genuinely conflicting claims from different vantage points.

**decoding is a deterministic convergence engine that takes messy claims from multiple legacy surfaces and produces the first usable, auditable canonical understanding of a bounded slice.** Claims that agree get resolved into canonical entries. Claims that conflict get escalated into a bounded human review queue. Nothing gets silently dropped or guessed.

### What makes this different

- **Deterministic** — same input claims + same policy = byte-identical output, every time. No ML, no heuristics, no timestamp-dependent behavior.
- **Auditable** — every canonical entry carries an explanation payload: which claims won, which corroborated, what resolution strategy was used.
- **Conservative** — structural facts auto-resolve, behavioral claims need corroboration, and liveness never overclaims death from absence alone.
- **Bounded escalation** — conflicts produce a finite review queue with actionable reasons, not an unbounded error log.
- **Pipeline-composable** — consumes `claim.v0` JSONL from `crucible scan`, emits `canon_entry.v0` + `escalation.v0` + `convergence.v0` JSONL.

---

## Quick Example

```bash
$ decoding archaeology claims/*.jsonl \
    --policy legacy.decode.v0.json \
    --output canon-map.jsonl \
    --escalations escalations.jsonl \
    --convergence convergence.json
```

```
decoding: 147 claims → 42 buckets
  converged:     31  (74%)
  single_source: 6   (14%)
  escalated:     5   (12%)

Exit 1 (escalations emitted)
```

Resolved entries land in `canon-map.jsonl`. Conflicts land in `escalations.jsonl`. The convergence report shows what settled and where the next scan should focus.

---

## The Problem

When decommissioning legacy systems, the first step is understanding what exists and how it's wired. Scanners like `crucible` crawl repositories, databases, and file systems to discover evidence. Some of that evidence is unambiguous — a table exists, a column has a type, a file is present. That goes straight into the metadata catalog.

But some evidence is inferential. A code scan suggests a report *probably* depends on a feed. A file scan says a mapping *might* still be active. A database scan finds a table that *could* be dead. These are claims, not facts — and different scanners make conflicting claims about the same subject.

Without a convergence layer, the operator is left with a pile of scan output and no way to know what's settled, what's contradicted, and what needs more evidence.

## The Solution

decoding groups claims into buckets by subject and property type, tracks corroboration across sources, and applies conservative resolution rules from a declarative policy file. The output is three artifacts:

- **Canonical entries** — resolved propositions with full provenance
- **Escalations** — conflicts that need human review, with actionable reasons
- **Convergence report** — summary of what settled and what didn't

```
legacy estate
  -> crucible scan
  -> metadata catalog (direct observations — bypass decoding)
  -> derived claim.v0 (ambiguous — goes through decoding)
  -> decoding archaeology
  -> canon_entry.v0 + escalation.v0 + convergence.v0
```

---

## How It Works

### Input: Claims

decoding consumes `claim.v0` JSONL — derived propositions emitted by `crucible` when direct observation alone is not enough:

```json
{
  "event": "claim.v0",
  "claim_id": "sha256:...",
  "source": {
    "kind": "repo_scan",
    "scanner": "crucible.scan.repo@0.1.0",
    "artifact_id": "sha256:...",
    "locator": { "kind": "file_range", "value": "src/close_pack.py#L40-L65" }
  },
  "subject": { "kind": "report", "id": "hyperion.close_pack_ebitda" },
  "property_type": "depends_on",
  "value": { "kind": "feed", "id": "fdmee.actuals_load" },
  "confidence": 0.88
}
```

### Bucketing

Claims are grouped into buckets by a logical key:

| Property type | Bucket key |
|---------------|-----------|
| Singular (`schema`, `liveness`, `valid_values`, ...) | `(subject.kind, subject.id, property_type)` |
| Edge (`reads`, `writes`, `depends_on`, `used_by`, `authoritative_for`) | `(subject.kind, subject.id, property_type, value.kind, value.id)` |

Edge properties get their own bucket per target so independent relationships don't collapse.

### State Machine

Each bucket moves through a small state machine:

```
EMPTY -> SINGLE_SOURCE -> CONVERGING -> CONVERGED
                          |
                          v
                     CONFLICTED -> ESCALATED
```

| State | Meaning |
|-------|---------|
| `SINGLE_SOURCE` | One claim only |
| `CONVERGING` | Multiple compatible claims |
| `CONVERGED` | Enough evidence to publish canonical entry |
| `CONFLICTED` | Incompatible claims exist |
| `ESCALATED` | Conflict or ambiguity requires human review |

### Resolution Policy

A declarative policy file controls resolution behavior:

```json
{
  "policy_id": "legacy.decode.v0",
  "auto_resolve": ["exists", "schema", "constraint"],
  "min_corroboration": {
    "reads": 2, "writes": 2, "depends_on": 2, "used_by": 2,
    "schedule": 2, "valid_values": 2, "semantic_label": 2, "authoritative_for": 2
  },
  "source_priority": {
    "liveness": ["db_scan", "file_scan", "repo_scan"]
  }
}
```

- **Auto-resolve** — structural properties (`exists`, `schema`, `constraint`) resolve with a single high-confidence claim
- **Min corroboration** — behavioral and semantic properties need multiple compatible claims
- **Source priority** — liveness uses source-type ranking when claims are compatible but varied

### Comparators

Each property type has a frozen compatibility rule:

| Property type | Compatible when |
|---------------|-----------------|
| `exists` | Both claims are `true` |
| `schema` | Normalized JSON deep-equal |
| `reads`, `writes`, `depends_on`, `used_by`, `authoritative_for` | Same subject ref |
| `valid_values` | Same sorted set of strings |
| `semantic_label` | Same normalized string |
| `liveness` | Same state, or `alive` + `stale`, or `stale` + `unknown` |

`alive` and `dead` conflict. `dead` never auto-wins from absence alone.

---

## Output Contracts

### Canonical Entry (`canon_entry.v0`)

```json
{
  "event": "canon_entry.v0",
  "bucket_id": "sha256:...",
  "subject": { "kind": "report", "id": "hyperion.close_pack_ebitda" },
  "property_type": "depends_on",
  "canonical_value": { "kind": "feed", "id": "fdmee.actuals_load" },
  "policy_id": "legacy.decode.v0",
  "convergence": { "state": "converged", "source_count": 3, "claim_count": 4 },
  "explain": {
    "winner_claim_ids": ["sha256:...", "sha256:..."],
    "compatible_claim_ids": ["sha256:...", "sha256:..."],
    "resolution_kind": "corroborated"
  }
}
```

Resolution kinds: `single_source`, `corroborated`, `priority_break`, `liveness_fold`.

### Escalation (`escalation.v0`)

```json
{
  "event": "escalation.v0",
  "bucket_id": "sha256:...",
  "subject": { "kind": "mapping", "id": "adj.ebitda.rule.family" },
  "property_type": "semantic_label",
  "reason": "conflicted",
  "claim_ids": ["sha256:...", "sha256:..."],
  "candidate_values": [
    {"kind": "scalar", "value": "Adjusted EBITDA rule family"},
    {"kind": "scalar", "value": "EBITDA exception class"}
  ],
  "recommended_action": "review",
  "summary": "two incompatible semantic interpretations remain"
}
```

Escalation reasons: `conflicted`, `missing_corroboration`, `no_resolution_path`.
Recommended actions: `review`, `scan_more`, `fix_scanner`, `fix_policy`.

### Convergence Report (`convergence.v0`)

```json
{
  "event": "convergence.v0",
  "policy_id": "legacy.decode.v0",
  "totals": {
    "buckets": 42, "converged": 31, "converging": 0,
    "single_source": 6, "conflicted": 5, "escalated": 5
  },
  "by_property_type": {},
  "by_source_kind": {},
  "top_escalations": []
}
```

---

## Archaeology Vocabulary

Phase 1 freezes a small, stable property vocabulary:

| Property type | Typical subjects | Meaning |
|---------------|------------------|---------|
| `exists` | all | Subject exists |
| `schema` | table, column, view | Structural definition |
| `constraint` | column, table | Not null, FK, check, uniqueness |
| `reads` | job, procedure, report, consumer | Reads from another subject |
| `writes` | job, procedure, feed | Writes to another subject |
| `depends_on` | report, mapping, artifact | Dependency edge |
| `used_by` | table, column, view, report | Downstream usage |
| `schedule` | job, feed | Cadence or trigger info |
| `valid_values` | column, mapping | Allowed values |
| `semantic_label` | column, report line, mapping | Business meaning hint |
| `liveness` | all | Alive, dead, stale, unknown |
| `authoritative_for` | report, extract, consumer | Authoritative output hint |

---

## How decoding Compares

| Capability | decoding | Manual triage | Custom reconciliation script | MDM platform |
|------------|----------|---------------|------------------------------|-------------|
| Deterministic convergence | Same claims + policy = same output | Depends on the person | Depends on the code | Usually |
| Auditable resolution | Explanation payload per entry | Spreadsheet notes | You build it | Varies |
| Bounded escalation | Finite queue with reasons | Unbounded email threads | Error logs | Ticket system |
| Conservative liveness | Never overclaims death | Varies | Often overclaims | N/A |
| Policy-driven | Declarative JSON | Tribal knowledge | Hardcoded | Config-heavy |

**When to use decoding:**
- Converging conflicting legacy scan output into a canonical understanding
- Producing a bounded human review queue from ambiguous archaeology
- Building the first usable map of a legacy estate slice

**When decoding is not the right tool:**
- Direct observations (table existence, file inventory) — use the metadata catalog
- Entity resolution across naming variants — use `canon org`
- Financial claim resolution — deferred after Phase 1

---

## Installation

### Homebrew (Recommended)

```bash
brew install cmdrvl/tap/decoding
```

### Shell Script

```bash
curl -fsSL https://raw.githubusercontent.com/cmdrvl/decoding/main/scripts/install.sh | bash
```

### From Source

```bash
cargo build --release
./target/release/decoding --help
```

---

## CLI Reference

```
decoding archaeology <CLAIMS>... --policy <FILE> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `<CLAIMS>...` | One or more claim JSONL files |

### Flags

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--policy <FILE>` | string | *(required)* | Archaeology decode policy JSON |
| `--output <FILE>` | string | stdout | Canon entry JSONL output |
| `--escalations <FILE>` | string | *(none)* | Escalation JSONL output |
| `--convergence <FILE>` | string | *(none)* | Convergence report JSON output |
| `--json` | flag | `false` | JSON status messages on stderr |

### Exit Codes

| Code | Meaning |
|------|---------|
| `0` | No escalations — all claims converged or resolved |
| `1` | Escalations emitted — some claims could not be resolved |
| `2` | Refusal — invalid claim set, invalid policy, or contract violation |

---

## Refusal Boundary

Phase 1 keeps a hard split between invalid input and unresolved meaning.

**Refusal (exit 2):**

| Condition | Meaning |
|-----------|---------|
| Malformed JSONL | Can't parse the input |
| Missing required fields | Claim contract violated |
| Malformed `claim_id` | Content hash is invalid |
| Unknown `source.kind` | Unrecognized source type |
| Unknown `subject.kind` | Unrecognized subject type |
| Unknown `property_type` | Unrecognized property |
| Value shape mismatch | Value doesn't match the property contract |
| Unknown policy keys | Policy contains unrecognized configuration |

**Escalation (exit 1):**

| Condition | Meaning |
|-----------|---------|
| Conflicting propositions | Incompatible claims in the same bucket |
| Insufficient corroboration | Not enough sources to resolve |
| No resolution path | Policy has no declared path to resolution |

If the decoder accepts a claim into a bucket, it has already passed the validity gate.

---

## Scripting Examples

Basic archaeology run:

```bash
decoding archaeology claims/*.jsonl \
  --policy legacy.decode.v0.json \
  --output canon-map.jsonl \
  --escalations escalations.jsonl \
  --convergence convergence.json
```

Check exit code in CI:

```bash
decoding archaeology claims/*.jsonl --policy legacy.decode.v0.json > /dev/null 2>&1
echo $?  # 0 = clean, 1 = escalations, 2 = refused
```

Inspect escalations:

```bash
cat escalations.jsonl | jq 'select(.reason == "conflicted")'
```

Convergence summary:

```bash
cat convergence.json | jq '.totals'
```

Find all unresolved liveness claims:

```bash
cat escalations.jsonl | jq 'select(.property_type == "liveness")'
```

Full crucible-to-decoding pipeline:

```bash
crucible scan repo ./legacy-codebase --emit claims > claims/repo.jsonl
crucible scan db ./legacy-db --emit claims > claims/db.jsonl
decoding archaeology claims/*.jsonl \
  --policy legacy.decode.v0.json \
  --output canon-map.jsonl \
  --escalations escalations.jsonl \
  --convergence convergence.json
```

Handle refusals programmatically:

```bash
decoding archaeology claims/*.jsonl --policy legacy.decode.v0.json --json 2>status.json
if [ $? -eq 2 ]; then
  cat status.json  # refusal details
fi
```

---

## Troubleshooting

### Exit 2 on valid-looking claims

Claims are validated strictly against the frozen Phase 1 vocabulary. Check that `source.kind`, `subject.kind`, and `property_type` are all recognized values. The refusal message will name the first offending field.

### Everything escalates

If the first real slice produces mostly escalations, the vocabulary or policy surface may be too broad for the data. Check `convergence.json` — if `by_property_type` shows one property dominating escalations, tighten the policy or scan with more sources for that property.

### Bucket ID instability

If the same claims produce different `bucket_id` values across runs, canonical JSON normalization is broken. This is a stop-ship bug — fix `src/normalize.rs` before continuing.

### Edge properties collapsing

If two independent `depends_on` edges for the same subject land in the same bucket, the bucket key is not including `(value.kind, value.id)`. Check `src/bucket.rs` — edge properties must use the extended bucket key.

### Liveness overclaiming death

By design, `dead` should never auto-win from absence alone. If a subject is marked dead in the canonical map without strong executed evidence, the liveness fold logic in `src/compare.rs` needs review.

---

## Relationship to Other Tools

| Tool | Role | Relationship |
|------|------|-------------|
| **crucible** | Discovers evidence from legacy surfaces | Upstream — emits `claim.v0` that decoding consumes |
| **canon** | Resolves entity identifiers | Complementary — canon resolves *names*, decoding resolves *propositions* |
| **shape** / **rvl** | Structural comparison and change explanation | Different domain — CSV reconciliation vs legacy archaeology |
| **metadata catalog** | Stores directly observed facts | Parallel — direct observations bypass decoding entirely |

---

## Limitations

| Limitation | Detail |
|------------|--------|
| **Archaeology mode only** | Phase 1 supports legacy-system archaeology. Document extraction mode is deferred. |
| **No mutation emission** | Produces canonical entries and escalations only. Does not write to production databases. |
| **No entity resolution** | Does not resolve entity identity. Use `canon org` for that. |
| **Frozen vocabulary** | Phase 1 freezes the property and subject vocabularies. Unknown types are refusal conditions. |
| **No model-assisted reasoning** | Resolution is purely deterministic. No LLM or ML in the loop. |

---

## Repo Structure

| Path | Role |
|------|------|
| `src/main.rs` | Thin binary entrypoint |
| `src/lib.rs` | Module root and shared library surface |
| `src/cli.rs` | Clap argument parsing, exit-code mapping |
| `src/contracts/claim.rs` | `claim.v0` parsing and validation |
| `src/contracts/vocabulary.rs` | Frozen Phase 1 enums (`SourceKind`, `SubjectKind`, `PropertyType`) |
| `src/contracts/canon_entry.rs` | `canon_entry.v0` output schema |
| `src/contracts/escalation.rs` | `escalation.v0` output schema |
| `src/contracts/convergence.rs` | `convergence.v0` report schema |
| `src/contracts/policy.rs` | `legacy.decode.v0` policy loader |
| `src/normalize.rs` | Canonical JSON, hashing, string normalization |
| `src/bucket.rs` | Logical bucket keys, grouping, bucket store |
| `src/compare.rs` | Property-aware comparator registry, liveness fold |
| `src/resolve.rs` | State machine, resolution decisions |
| `src/render.rs` | JSONL output rendering for canonical entries and escalations |
| `src/report.rs` | Convergence summary generation |
| `tests/` | Contract, fixture, and snapshot test suites |
| `docs/PLAN_DECODING.md` | Full implementation spec |

---

## Contributing

If you are working in this repo:

1. Read [docs/PLAN_DECODING.md](./docs/PLAN_DECODING.md)
2. Read [AGENTS.md](./AGENTS.md)
3. Inspect ready work with `br ready`
4. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `ubs .`
5. Implement only behavior already specified in the plan

Current work should improve one of:

- contract fidelity
- bucket and comparator correctness
- test and fixture coverage
- documentation and release hygiene

---

## Roadmap

Near-term:

- Implement all Phase 1 modules (contracts, bucketing, comparators, resolver, outputs)
- Add CI quality-gate workflow
- Cut first tagged release
- Run against first real Hyperion archaeology slice

Deferred by design:

- Document extraction mode
- Mutation emission for production databases
- Entity resolution (`canon org` owns this)
- Neo4j / data-fabric graph integration
- Model-assisted reasoning

---

## Source of Truth

If the README and the plan ever disagree, follow:

1. [docs/PLAN_DECODING.md](./docs/PLAN_DECODING.md)
2. [AGENTS.md](./AGENTS.md)
3. this README

---

## FAQ

### Why "decoding"?

Legacy systems encode knowledge in scattered, overlapping, sometimes contradictory forms. decoding is the process of converging that mess into something canonical and usable.

### How does decoding relate to crucible?

`crucible` discovers evidence. `decoding` converges only the subset of that evidence that is a claim-resolution problem. Directly observed metadata goes to the catalog, not through decoding.

### Why not just use the strongest signal?

Because "strongest" is often wrong. A code scan might show a dependency that was removed last month. A database scan might show a table that's technically alive but functionally dead. Conservative convergence with corroboration requirements catches these.

### What if everything escalates?

If the first real slice produces an unbounded escalation queue, the vocabulary or policy surface is too broad. Phase 1 is intentionally narrow to avoid this.

### Can I use this in CI/CD?

Yes. Exit codes (0/1/2) and JSONL output are designed for automation.

---

## Agent Integration

For the full toolchain guide, see the [Agent Operator Guide](https://github.com/cmdrvl/.github/blob/main/profile/AGENT_PROMPT.md).

---

## Spec

The full specification is [`docs/PLAN_DECODING.md`](./docs/PLAN_DECODING.md). This README covers everything needed to use the tool; the spec adds implementation details, edge-case definitions, test coverage requirements, and go/no-go checkpoints.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## License

MIT
