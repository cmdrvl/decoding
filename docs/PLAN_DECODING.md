# decoding — Claims to Canonical Understanding

## One-line promise
**Turn messy, redundant claims from any source — document extractors, database scans, codebase analysis — into canonical truth using deterministic policy and convergence, not LLM inference.**

---

## Problem

Two versions of the same problem:

**Document extraction.** Multiple extractors run against messy document corpora — Excel files, PDFs, CSVs from trustee sites and data rooms. They produce overlapping, sometimes contradictory claims about the same data. "Loan LN-00421 has NOI of $450,000" from one file and "$452,000" from another. Which one wins?

**Legacy system archaeology.** You scan a 1000-table Oracle database — DDL, stored procs, views, jobs, audit logs. Then you scan every application codebase that touches it — Java, Python, COBOL, Crystal Reports. Each scan produces claims about what the database is, what it does, who uses it. Claims are messy, overlapping, and contradictory. The Java risk app says `loan_master.status` has 4 valid values. The stored proc says 6. The Crystal Report assumes 3. Which view of reality wins?

Both problems share the same structure:
1. Multiple independent sources emit claims about the same subjects
2. Claims overlap and contradict
3. Resolution must be deterministic, explainable, and auditable
4. You need to know when you have *enough* claims to be confident

`decoding` handles both. Same engine, different claim shapes, different bucket keys, same convergence model.

---

## Non-goals

`decoding` is NOT:
- An extractor or scanner (those emit claims; decoding consumes them)
- A database (that's `twinning` for dev, real Postgres for production)
- An identity compiler or registry builder (that belongs to `canon` / `canon org`)
- Probabilistic or ML-based (v0 is fully deterministic)
- A model-boundary attestation layer (that belongs to `airlock`)
- A general-purpose ETL tool
- A message bus consumer (claims are JSONL files, not events on a stream)

It does not extract data from documents or scan databases. It resolves competing claims into canonical truth using declared policy.

---

## Two modes

### Mode 1: Document extraction claims

Claims about data values extracted from documents. The bucket key is `(entity, period, field, def)`.

**Claim shape:**
```json
{
  "event": "claim.v0",
  "claim_id": "sha256:...",
  "mode": "extraction",
  "agent": "agent-7",
  "timestamp": "2026-02-10T14:30:00Z",
  "source": {
    "file": "sha256:abc...",
    "template": "wells-fargo-financial-detail.v1",
    "extractor": "wf-fin-detail@0.3.0",
    "locator": { "kind": "xlsx_cell", "sheet": "Financial Detail", "cell": "C37" },
    "raw": "450,000"
  },
  "proposed": {
    "entity_ref": {
      "kind": "property",
      "ids": { "property_id": null, "loan_id": "LN-00421", "cusip": null },
      "hints": { "name": "Sunset Plaza", "address": "123 MAIN ST, MIAMI, FL" }
    },
    "as_of": "2024-12-15",
    "period": "2024-12",
    "field": "financials.noi",
    "value": 450000,
    "unit": "USD",
    "def": "noi",
    "null": false
  },
  "confidence": 0.92
}
```

- `field` targets a canonical column name, but `entity_ref` may be ambiguous or incomplete. Ambiguity is normal.
- `claim_id` is content-addressed (hash of normalized payload) so claim replay is idempotent.
- `def` tags semantics when the same label can mean different things ("NOI before reserves" vs "NOI after reserves").

### Mode 2: Legacy system archaeology claims

Claims about database structure, usage, semantics, and liveness from database scans and codebase analysis. The bucket key is `(table, column, property_type)`.

**Claim shape:**
```json
{
  "event": "claim.v0",
  "claim_id": "sha256:...",
  "mode": "archaeology",
  "agent": "agent-3",
  "timestamp": "2026-03-01T10:15:00Z",
  "source": {
    "kind": "codebase_scan",
    "repo": "risk-reporting-app",
    "file": "src/main/java/com/bank/risk/LoanService.java:142",
    "evidence": "SELECT balance, status FROM loan_master WHERE deal_id = ?"
  },
  "proposed": {
    "subject": "table:loan_master",
    "column": "status",
    "property_type": "valid_values",
    "value": ["active", "closed", "default", "foreclosure"],
    "detail": {
      "application": "risk-reporting-app",
      "usage": "WHERE clause filter",
      "frequency": "daily"
    }
  },
  "confidence": 0.75
}
```

Claims can be about:
- **Structure** — "table X has FK to table Y" (from DDL, confidence 1.0)
- **Usage** — "the risk app queries columns A, B, C daily" (from codebase scan)
- **Rules** — "column balance is never negative" (from CHECK constraint or app validator)
- **Liveness** — "table old_backup hasn't been read since 2021" (from audit logs)
- **Semantics** — "column 'status' means loan status with values active/closed/default" (from Java enum)
- **Lineage** — "stored proc P reads from table A, writes to table B" (from proc analysis)
- **Valid values** — "column status accepts 4 values" (from Java enum) vs "6 values" (from stored proc)

---

## Core concepts

### Mutations (Mode 1 output)

In document extraction mode, decoding emits canonical upserts suitable for `twinning` and the real DB:

```json
{
  "event": "mutation.v0",
  "timestamp": "2026-02-10T14:30:01Z",
  "policy": "cmbs.decode.v1",
  "registries": {
    "canon.entity": "2.1.0",
    "canon.cusip-isin": "1.3.0"
  },
  "target": {
    "table": "financials",
    "op": "upsert",
    "key": { "property_id": "P-123", "period": "2024-12" }
  },
  "row": { "noi": 450000 },
  "explain": {
    "winner_claim_ids": ["sha256:..."],
    "rule_id": "source_hierarchy.v1",
    "confidence": "confirmed"
  }
}
```

### Canonical map (Mode 2 output)

In legacy system archaeology mode, decoding emits a canonical understanding of the database — every table classified, every column annotated, every relationship traced:

```json
{
  "event": "canon_entry.v0",
  "timestamp": "2026-03-01T10:16:00Z",
  "policy": "legacy.decode.v1",
  "subject": "table:loan_master.status",
  "property_type": "valid_values",
  "canonical_value": ["active", "closed", "default", "delinquent", "foreclosure", "reo"],
  "convergence": {
    "state": "converged",
    "sources": 3,
    "claims": 4,
    "agreement": "superset_union"
  },
  "explain": {
    "contributing_claims": ["sha256:aaa...", "sha256:bbb...", "sha256:ccc...", "sha256:ddd..."],
    "resolution": "DDL CHECK constraint is authoritative (confidence 1.0), supersedes application subsets",
    "conflicts_resolved": 1,
    "conflicts_detail": "Java risk app listed 4 of 6 values (subset, not contradiction)"
  }
}
```

### Three honest options

When claims don't provide enough information to resolve, the decoder has exactly three options:

1. **Resolve** — enough signal to pick a winner. Emit a canonical mutation (Mode 1) or canonical map entry (Mode 2). Most claims land here.
2. **Hold as hypothesis** — multiple plausible interpretations, insufficient signal to choose. Emit the best-guess AND preserve alternatives. The canonical projection picks one; alternatives are preserved.
3. **Escalate** — ambiguity is material. Route to human review with full context.

There is no fourth option. The decoder does not silently guess — every resolution carries an explanation, and every hypothesis is preserved.

---

## The convergence model

The core insight borrowed from fountain codes: **you don't need every source to be complete. You need enough independent sources agreeing to converge.**

### Buckets

Each property being decoded is a **bucket**. Claims pour into buckets from multiple independent sources.

**Mode 1 bucket key:** `(entity, period, field, def)` — one bucket per data value being resolved.

**Mode 2 bucket key:** `(table, column, property_type)` — one bucket per property of the database being understood.

### Bucket state machine

```
EMPTY → SINGLE_SOURCE → CONVERGING → CONVERGED → ESCALATED
                              ↓            ↑
                         CONFLICTED ───────┘
```

| State | Meaning | Action |
|-------|---------|--------|
| **Empty** | No source has said anything about this | Unknown — may be dead, may be undiscovered |
| **Single-source** | One source, one claim | Low confidence — need corroboration |
| **Converging** | Multiple sources, consistent so far | Confidence rises with each independent confirmation |
| **Converged** | Enough independent sources agree | High confidence — resolved |
| **Conflicted** | Sources actively disagree | Needs cascade policy or human resolution |
| **Escalated** | Conflicted and material | Routed to human with full context |

The state machine is the same for both modes. What differs is the bucket key, the claim shape, and the convergence thresholds.

### Convergence thresholds

**Mode 1 (extraction):**

| Property | Converges when | Rationale |
|----------|---------------|-----------|
| Data value | 2+ independent extractors agree within tolerance | Different templates, same answer |
| Entity identity | Promoted canon registry resolves to a canonical ID, OR 2+ sources converge on the same unresolved handle without minting canonical identity | Identity may converge operationally before canon promotion, but unresolved is still unresolved |

**Mode 2 (archaeology):**

| Property | Converges when | Rationale |
|----------|---------------|-----------|
| Column exists + type | DDL alone (confidence 1.0) | Schema is fact |
| NOT NULL / FK / CHECK | DDL alone (confidence 1.0) | Hard constraint |
| Valid value set | 2+ independent sources agree | App enums may be stale; DDL CHECK is definitive |
| Usage (which apps query it) | All scanned apps processed | Usage is additive, not convergent |
| Liveness (alive/dead) | Audit logs + 1+ app source agree | Both behavioral and structural evidence |
| Semantics (what it means) | 2+ sources with consistent interpretation | Column names are ambiguous; usage reveals intent |

### What convergence tells you

**When to stop scanning.** If you've scanned 3 of 5 apps and 90% of buckets have converged, the marginal value of reverse-engineering the COBOL is measurable: it would fill at most N remaining empty/single-source buckets. If N is small, skip it. If N is large, it's worth the effort. The decision is data-driven, not gut-driven.

**Where to focus humans.** Conflicted buckets are where expert attention has the highest leverage — two sources disagree, a human resolves it in 30 seconds, the bucket converges. The gap dashboard ranks conflicts by impact (how many downstream data products depend on this bucket?) so humans work the highest-leverage conflicts first.

### Convergence dashboard

```
Buckets: 14,200 total (1000 tables x ~14.2 properties avg)

  Converged:      11,340  (79.9%)  --------------------------------  done
  Converging:      1,420  (10.0%)  ----                              need 1 more source
  Single-source:     890  ( 6.3%)  ---                               low confidence
  Conflicted:        210  ( 1.5%)  -                                 needs human
  Empty:             340  ( 2.4%)  -                                 undiscovered

Sources scanned: 5 of 7 known applications
  Marginal value of next source (python-etl): ~180 buckets (fills 120 single-source, 60 empty)
  Marginal value of next source (cobol-batch): ~40 buckets (fills 30 single-source, 10 empty)
  -> Recommendation: scan python-etl next, defer cobol-batch
```

---

## The decode loop

### Mode 1: Extraction decode

1. Extractors emit `claim.v0` events as JSONL files.
2. Each claim is content-addressed (hash of normalized payload) and tagged with derivation metadata (independent vs derived source).
3. `decoding` canonicalizes entity references using promoted `canon` registries. `canon org` may build those registries offline, but it does not sit in the hot decode path.
4. If lookup succeeds, claims bucket under the resolved canonical ID. If lookup does not succeed, claims bucket under a decoder-local unresolved entity handle linked to the source hints and registry snapshot. The decoder does not mint provisional canonical IDs, and it does not treat `canon org` escrow IDs as canonical identity.
5. After each insertion, the decoder attempts to solve the bucket. Resolution follows the per-field cascade policy: temporal precedence -> source hierarchy -> anchor agreement -> extractor track record -> majority -> tolerance -> hold hypothesis -> escalate.
6. Resolved buckets emit canonical `mutation.v0` events (plus a decode explanation graph: winning claims, losing claims, policy rule fired, alternatives preserved).
7. Late claims for already-resolved buckets are checked for consistency. Contradictions from independent sources re-open the bucket and re-solve. Redundant confirmations increase confidence without changing state.
8. Gold set invariants are structural — gold rows are precode in every bucket's constraint matrix. A resolution that regresses gold is never emitted.

### Mode 2: Archaeology decode

1. Scanners (`crucible scan --db`, `crucible scan --repo`) emit `claim.v0`
   events as JSONL files — one file per source.
2. Each claim is content-addressed and carries source provenance (which scanner, which file, which line).
3. Claims are inserted into their `(table, column, property_type)` bucket.
4. After each insertion, the decoder updates convergence state. DDL claims auto-resolve structural buckets (confidence 1.0). Usage and semantics buckets require corroboration.
5. Converged buckets emit `canon_entry.v0` events — the canonical understanding of that property.
6. Conflicted buckets surface for human review with full context: all competing claims, their sources, and the specific disagreement.
7. The decoder handles legitimate multiplicity: a table used differently by three apps doesn't have one usage profile — it has three. All three are valid claims. The canonical entry captures the full picture, not false convergence.

### Constraint propagation (the real peeling)

In Mode 1, verify rules are constraints on the output space. When a bucket resolves, its value propagates as a constraint on related buckets (e.g., individual loan balances must sum to pool total). This IS peeling — over real-valued fields with tolerance windows.

In Mode 2, structural constraints propagate similarly. If table A has a FK to table B, and table B is classified as dead, that propagates a "likely dead" constraint on table A's liveness bucket.

---

## Entity resolution (Mode 1)

Entity resolution for CMBS is a hierarchy that resolves bottom-up, where each level uses a different mechanism — and for v0, every level is deterministic.

```
Properties     ->  geospatial anchor (customer's cleaned data -> canon registry)
    ^
Loans          ->  structural fingerprint (shape of collateral + counterparties)
    ^
Deals          ->  structural fingerprint (shape of loans in deal)
    ^
Counterparties ->  canon registries (known entities with known aliases)
```

| Level | Mechanism | Infrastructure | Confidence |
|-------|-----------|---------------|------------|
| Properties | Geospatial anchor | Customer data -> `canon` registry | 1.0 |
| Counterparties | Name registry | `canon` registries | 1.0 |
| Loans | Structural fingerprint | data-fabric Neo4j graph | 1.0 (given resolved properties + counterparties) |
| Deals | Structural fingerprint | data-fabric Neo4j graph | 1.0 (given resolved loans) |

Each level resolves from the level below it. Properties anchor the base. Everything cascades upward deterministically. No probabilistic matching infrastructure needed for v0.

Mode 2 doesn't need entity resolution — tables and columns are already identified by DDL. The challenge is semantic resolution (what does it *mean*), not identity resolution (what *is* it).

---

## Contract with `canon org`

`decoding` and `canon org` are adjacent but not interchangeable.

`canon org` is an **identity compiler** for organization-like entities. It takes
normalized observations, incumbent alias state, trusted anchors, and escrow
memory, then emits one of four outcomes for each solved component:

- inherit an existing canonical ID
- promote a new canonical ID
- abstain with pending escrow
- abstain with contradiction / cannot-link evidence

`decoding` is a **claim resolver**. It takes already-emitted claims and decides
what canonical value to publish for a bucket, whether to hold alternatives as
hypotheses, or whether to escalate.

That yields a strict contract:

1. **`canon org` owns org identity formation.** Alias expansion, anchor-based
   inheritance, escrow carry-forward, and safe promotion into flat registries
   belong to `canon org`, not `decoding`.
2. **`decoding` owns claim/value convergence.** Once an entity is resolved to a
   stable canonical ID, `decoding` decides which claims win for each
   `(entity, period, field, def)` bucket.
3. **Only promoted IDs are canonical.** `canon org` `escrow_id` values are
   provisional memory, not canonical identity. They must never appear as bucket
   keys, mutation keys, or published canonical foreign keys.
4. **Unresolved identity stays unresolved.** If hot-path lookup into promoted
   `canon` registries fails, `decoding` creates a decoder-local unresolved
   handle and preserves the evidence. It may rank candidate identities, but it
   must not mint a fake canonical ID just to keep the loop moving.
5. **Registry growth is asynchronous.** When `canon org` promotes new aliases or
   new org IDs, later decode runs may re-resolve prior unresolved handles
   against the new registry snapshot. That replay loop is valid; hidden hot-path
   identity mutation is not.

### Hot path vs cold path

In v0 the default split is:

```text
claims -> decoding -> mutations / canon_entry
                ^
                |
      promoted flat canon registries
                ^
                |
      canon org run/audit/promote (offline)
```

The hot path uses the flat lookup registry only. `canon org` remains a cold-path
compiler that improves the next registry snapshot.

### Snapshot contract

For the default v0 contract, `decoding` should record the exact promoted
registry snapshot it consumed for identity resolution. That means at minimum:

- registry id
- registry version
- lookup snapshot hash

If a future version of `decoding` ever consumes `canon org` sidecars directly
(for example trusted anchors or escrow memory in the hot path), then replay
determinism must also capture:

- escrow snapshot hash
- `canon org` strategy content hash
- the content hash of the audited `canon org` result or promote artifact that
  justified the sidecar state

Without that stronger snapshot contract, decode replay is underspecified.

### Boundary of responsibility

`canon org` does not make `decoding` obsolete because identity resolution is
only one upstream dependency of claim resolution.

- `canon org` answers: **who is this organization?**
- `decoding` answers: **given these competing claims, what is true?**

`decoding` still has to resolve field values, temporal precedence, source
hierarchy, tolerance windows, derivation weighting, structural constraints, and
escalation policy even after identity is known.

---

## Contract with `twinning` and real Postgres

`decoding` is upstream of both `twinning` and any real database materialization.

The implementation wedge matters:

- first prove the decode loop on deterministic artifacts plus real Postgres
- then add `twinning` when protocol fidelity and iteration speed become the
  bottleneck

That yields a strict split:

1. **`decoding` owns truth convergence.** It resolves claims into canonical
   mutations or canonical archaeology entries with explanation graphs.
2. **Real Postgres or `twinning` own materialized-state behavior.** They apply
   already-decided mutations, enforce runtime/storage constraints, and surface
   downstream contradictions.
3. **`twinning` is not a truth layer.** It does not canonicalize entities,
   resolve conflicting claims, or rank policy alternatives.
4. **Constraint failure is downstream evidence, not adjudication.** If a
   resolved mutation fails against Postgres or a twin, that failure feeds back
   into decode policy, upstream claims, or transform logic. The storage/runtime
   layer still does not decide what is true.

Short version:

- `decoding` decides what canonical state should be
- Postgres / `twinning` decide whether that state behaves and validates
- `twinning` is the later speed/protocol layer, not the first proof step

---

## Gold set

The gold set turns years of edge cases into executable acceptance tests.

```
gold/
+-- locks/                 # lockfiles for nasty, representative corpora slices
+-- expected/              # expected canonical outputs (CSV/JSON) + tolerances
+-- policies/              # decode + conflict policies pinned for the gold set
+-- notes/                 # why each case exists (human-readable)
```

Any change to fingerprints, extractors, registries, or decode policy must:
1. Improve anchored coverage / reduce unresolved claims, **and**
2. Not regress gold outputs (or explicitly version a breaking change with recorded diffs + rationale).

### Novelty scoring

Every claim and every decode decision carries a novelty score — how structurally similar is this input to patterns the gold set covers? High novelty = outside the gold set's tested distribution = lower confidence in decode correctness, even if all invariants pass. The gap dashboard flags novel patterns for human review.

The gold set grows. Every production run is an opportunity to discover new edge cases. Escalations become gold candidates. High-novelty decodes that survive human spot-checks become gold entries.

---

## Build order

| Order | Component | LOC | What it does | Test strategy |
|-------|-----------|-----|-------------|---------------|
| 1 | **Bucket state machine** | ~250 | Six states (empty/single-source/converging/converged/conflicted/escalated), transitions, content-addressed claim insertion. Works for both mode 1 and mode 2 bucket keys. | Synthetic claims, property-based testing |
| 2 | **Convergence tracker** | ~300 | Track independent source count per bucket, compute convergence state, report marginal value of next source | Synthetic multi-source scenarios |
| 3 | **Cascade policy engine** | ~500 | Priority-ordered decision tree driven by JSON policy file. Mode-specific cascade rules. | Hand-written gold cases, determinism tests |
| 4 | **Constraint propagation** | ~300 | Verify rule propagation (Mode 1), structural constraint propagation (Mode 2) | Known propagation scenarios |
| 5 | **Derivation graph + clone detection** | ~300 | Template derivation declarations, effective vote weight adjustment, code similarity clustering for COBOL copy-paste programs. Two programs with the same DATA DIVISION and 85% identical PROCEDURE DIVISION vote as one source, not two. Prevents inflated convergence from cloned logic. | Known derivation scenarios, "confident wrongness" prevention, synthetic clone clusters |
| 6 | **Gold set regression harness** | ~100 | Replay all gold cases, green/red gate on every decode change | Must pass before any decode or policy change ships |
| 7 | **Convergence dashboard** | ~300 | Report bucket states, marginal value estimates, human review queue | Integration tests with multi-source scenarios |

### The math that matters

Four mathematical properties make the decode loop work:

1. **Content-addressed determinism.** `sorted(claims, key=content_hash)` -> deterministic matrix -> deterministic solution. Replay is exact. Debugging is possible. Regressions are detectable.

2. **Convergence via independent corroboration.** Multiple shitty sources, none complete, none authoritative on their own. Enough independent sources agreeing = convergence. This is the fountain model applied to evidence, not packets.

3. **Submodular coverage.** Each new source increases the "converged" fraction of buckets — with diminishing returns. The marginal value of the next source is measurable before you scan it. Chao1 estimator and rarefaction curves give principled stopping criteria.

4. **Escalation rate as loss function.** Crucible minimizes escalation rate
   subject to gold-set correctness. Measurable, monotonic in system quality,
   bounded above by "escalate everything" (the safe default).

---

## CLI

```
decoding <CLAIMS>... --policy <FILE> [OPTIONS]

Arguments:
  <CLAIMS>...              Claim files (JSONL) to decode

Options:
  --policy <FILE>          Decode policy file (JSON)
  --mode <MODE>            Claim mode: extraction | archaeology (auto-detected from claims if omitted)
  --registries <DIR>       Canon registry directory (for entity resolution in extraction mode)
  --gold <DIR>             Gold set directory (for regression checking)
  --output <FILE>          Output file for mutations/canon entries (JSONL)
  --convergence <FILE>     Write convergence report (JSON)
  --json                   JSON output for status messages
```

### Exit codes

`0` all buckets resolved or converged | `1` escalations exist | `2` refusal (gold regression, bad claims, etc.)

---

## Relationship to other tools

| Tool | Relationship |
|------|-------------|
| **crucible** | Crucible scan produces archaeology claims and orchestrates the loop. The first proof path is deterministic artifacts plus real Postgres; `twinning` is added later as the speed/protocol layer. |
| **canon** | Provides versioned entity registries for identity resolution (Mode 1) |
| **twinning** | Optional downstream speed/protocol layer. Receives resolved mutations or assembled candidate state, enforces runtime constraints, and surfaces behavior; it does not resolve claims. |
| **verify** | Rules are precode constraints in bucket resolution (Mode 1) |
| **assess** | Conflict policies align with assess decision bands |
| **benchmark** | Gold set assertions validate decode correctness |
| **fingerprint** | Template matches determine which extractor runs (Mode 1) |
| **airlock** | If decode outputs or decode-derived telemetry are sent to a model, airlock proves exactly what crossed the model boundary. It does not score decode correctness or resolve claims. |
| **pack** | Decode explanations + mutations/canon entries sealed as evidence |

---

## Relationship to `airlock`

`airlock` sits **after deterministic decode artifacts and before model
execution**.

```text
claims
  -> decoding
  -> mutations / canon_entry / decode explanations
  -> airlock assemble + verify
  -> model request
  -> downstream evaluation / sealing
```

This matters because the two tools solve different problems:

- `decoding` proves how canonical values were chosen from competing claims
- `airlock` proves what bytes and derived fragments crossed into the model zone

Airlock is therefore not part of bucket resolution, cascade policy, or entity
resolution. It is the boundary-attestation primitive used when decoded outputs
become model inputs.

### Typical uses together

1. `decoding` resolves document claims into canonical mutations and explanation
   artifacts.
2. A downstream workflow wants a model to review escalations, summarize novel
   cases, or mutate strategy proposals from decode telemetry.
3. `airlock` assembles the model prompt from those deterministic artifacts,
   verifies the request against boundary policy, and emits a manifest proving
   what crossed.

That preserves the spine split:

- `decoding` owns truth convergence
- `airlock` owns boundary attestation
- downstream tools such as `benchmark`, `verify`, `assess`, and `pack` score,
  classify, and seal the resulting workflow artifacts

Airlock becomes especially important for decode-adjacent human-in-the-loop
workflows: escalation review, gold-set mutation proposals, or archaeology
summaries for experts. In those cases the question is no longer "did the decode
policy resolve the claim correctly?" but "what exactly did we expose to the
model while asking for help?"

---

## Implementation notes

### Candidate crates

| Need | Crate | Notes |
|------|-------|-------|
| JSON parsing | `serde_json` | Claims, mutations, policy files, canon entries |
| Content hashing | `sha2` | Claim content addressing |
| Policy engine | Custom | ~500 LOC cascade resolver |
| Graph queries | `neo4rs` | Entity resolution via data-fabric (Mode 1 only) |
| CLI | `clap` | derive-based |

### Implementation scope

~2000 LOC core decode engine (bucket state machine + convergence tracker + cascade + constraint propagation + derivation graph + gold harness + convergence dashboard). Additional infrastructure for entity resolution and reporting brings total to ~3-5K LOC Rust.

Follows the same implementation standards as protocol tools: `#![forbid(unsafe_code)]`, clap derive CLI, MIT license, CI (fmt -> clippy -> test).

---

## Determinism

Same claims + same policy + same registry snapshots = same output. Content-addressed claims ensure replay is exact. The decode explanation graph records every decision. Regressions are detectable by diffing outputs across decode policy versions.
