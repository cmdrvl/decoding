# decoding — Claims to Canonical Rows

## One-line promise
**Turn messy, redundant claim events into canonical database mutations — using deterministic policy, not LLM inference.**

---

## Problem

Extractors run against messy document corpora — Excel files, PDFs, CSVs from trustee sites and data rooms. Multiple extractors produce overlapping, sometimes contradictory claims about the same data. "Loan LN-00421 has NOI of $450,000" from one file and "$452,000" from another. Which one wins?

Today, extraction pipelines make ad-hoc choices — last writer wins, highest confidence wins, or worse, undefined behavior. `decoding` replaces this with declared, versioned, deterministic policy. The decoder doesn't guess. It resolves identity, disambiguates semantics, resolves conflicts, and emits canonical mutations with a full explanation chain.

The twin (`twinning`) is a constraint executor and scorekeeper. `decoding` is the layer that decides what a claim *means* and which competing claims win.

---

## Non-goals

`decoding` is NOT:
- An extractor (extractors emit claims; decoding consumes them)
- A database (that's `twinning` for dev, real Postgres for production)
- Probabilistic or ML-based (v0 is fully deterministic)
- A general-purpose ETL tool

It does not extract data from documents. It resolves competing claims into canonical truth using declared policy.

---

## Core concepts

### Claims

Extractors don't write rows. They emit **claims**: value + evidence + hints.

```json
{
  "event": "claim.v0",
  "claim_id": "sha256:...",
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

### Mutations

`decoding` consumes claims, resolves identity/semantics/conflicts, and emits canonical upserts suitable for `twinning` and the real DB:

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

### Three honest options

When the corpus doesn't contain enough information to resolve an identity, period, or semantic definition, the decoder has exactly three options:

1. **Resolve** — enough signal to pick a winner. Emit a canonical mutation. Most claims land here.
2. **Hold as hypothesis** — multiple plausible interpretations, insufficient signal to choose. Emit the best-guess canonical mutation AND preserve alternatives in a hypothesis graph. The canonical projection picks one; the hypothesis graph preserves the others.
3. **Escalate** — ambiguity is material (high-value field, large disagreement, no policy rule applies). Route to human review with full context.

There is no fourth option. The decoder does not silently guess — every resolution carries an explanation, and every hypothesis is preserved.

---

## The decode loop

### v0 implementation sequence

1. Extractors emit `claim.v0` events for matched files (hot path = message bus; cold path = event store).
2. Each claim is content-addressed (hash of normalized payload) and tagged with derivation metadata (independent vs derived source).
3. `decoding` canonicalizes entity references using `canon` registries; unresolved IDs become provisional hypotheses with alternatives tracked.
4. Claims are inserted as rows into their `(entity, period, field, def)` bucket's constraint matrix. Provisional entities bucket by best-guess canonical ID with alternatives linked.
5. After each insertion, the decoder attempts to solve the bucket. Resolution follows the per-field cascade policy: temporal precedence -> source hierarchy -> anchor agreement -> extractor track record -> majority -> tolerance -> hold hypothesis -> escalate. A bucket resolves when the cascade produces a winner that satisfies all precode constraints (schema + verify rules + gold).
6. Resolved buckets emit canonical `mutation.v0` events (plus a decode explanation graph: winning claims, losing claims, policy rule fired, alternatives preserved). Mutations apply to `twinning` (or real Postgres in Wave 2) for constraint enforcement + verify scoring + anchor coverage reporting.
7. Late claims for already-resolved buckets are added to the matrix and checked for consistency. Contradictions from independent sources re-open the bucket and re-solve. Redundant confirmations increase confidence without changing state.
8. Gold set invariants are structural — gold rows are precode in every bucket's constraint matrix. A resolution that regresses gold is never emitted; it triggers a contradiction. Publishing gates on: all gold buckets resolved correctly, anchored coverage >= threshold, escalation count <= threshold, verify pass rate >= threshold.

### Bucket state machine

Each `(entity, period, field, def)` tuple is a bucket with five states:

```
EMPTY → SINGLE_CLAIM → RESOLVED → CONFIRMED → ESCALATED
                ↓           ↓          ↑
            CONFLICTED ─────┘──────────┘
```

- **EMPTY**: no claims yet
- **SINGLE_CLAIM**: one claim, auto-resolves if constraints satisfied
- **RESOLVED**: cascade policy picked a winner
- **CONFIRMED**: multiple independent sources agree
- **CONFLICTED**: claims disagree, cascade couldn't resolve
- **ESCALATED**: conflicted and material, routed to human

---

## Build order

The core decode loop is ~1300 LOC of Rust, buildable in sequence:

| Order | Component | LOC | What it does | Test strategy |
|-------|-----------|-----|-------------|---------------|
| 1 | **Bucket state machine** | ~200 | Five states, transitions, content-addressed claim insertion | Synthetic claims, property-based testing |
| 2 | **Peeling phase** | ~300 | Single-claim resolution + constraint propagation via verify rules | Synthetic + gold cases, 80%+ resolution rate expected |
| 3 | **Cascade policy engine** | ~500 | 7-priority decision tree driven by JSON policy file | Hand-written gold cases, determinism tests |
| 4 | **Derivation graph** | ~200 | Template derivation declarations, effective vote weight adjustment | Known derivation scenarios, "confident wrongness" prevention |
| 5 | **Gold set regression harness** | ~100 | Replay all gold cases, green/red gate on every decode change | Must pass before any decode or policy change ships |

### The math that matters

Four mathematical properties make the decode loop work:

1. **Content-addressed determinism.** `sorted(claims, key=content_hash)` -> deterministic matrix -> deterministic solution. The spine gives this for free via vacuum + hash. Replay is exact. Debugging is possible. Regressions are detectable.

2. **Constraint propagation (the real peeling).** Verify rules are constraints on the output space. When a bucket resolves, its value propagates as a constraint on related buckets (e.g., individual loan balances must sum to pool total). This IS peeling — over real-valued fields with tolerance windows.

3. **Submodular coverage.** Each new gold case, template fingerprint, or verify rule increases the "resolved" fraction of buckets — with diminishing returns. Chao1 estimator and rarefaction curves give principled measures of marginal improvement.

4. **Escalation rate as loss function.** The factory minimizes escalation rate subject to gold-set correctness. Measurable, monotonic in system quality, bounded above by "escalate everything" (the safe default).

---

## Entity resolution

Entity resolution for CMBS is a hierarchy that resolves bottom-up, where each level uses a different mechanism — and for v0, every level is deterministic.

```
Properties     →  geospatial anchor (customer's cleaned data → canon registry)
    ↑
Loans          →  structural fingerprint (shape of collateral + counterparties)
    ↑
Deals          →  structural fingerprint (shape of loans in deal)
    ↑
Counterparties →  canon registries (known entities with known aliases)
```

| Level | Mechanism | Infrastructure | Confidence |
|-------|-----------|---------------|------------|
| Properties | Geospatial anchor | Customer data -> `canon` registry | 1.0 |
| Counterparties | Name registry | `canon` registries | 1.0 |
| Loans | Structural fingerprint | data-fabric Neo4j graph | 1.0 (given resolved properties + counterparties) |
| Deals | Structural fingerprint | data-fabric Neo4j graph | 1.0 (given resolved loans) |

Each level resolves from the level below it. Properties anchor the base. Everything cascades upward deterministically. No probabilistic matching infrastructure needed for v0.

---

## Gold set

The gold set turns years of edge cases into executable acceptance tests.

```
gold/
├── locks/                 # lockfiles for nasty, representative corpora slices
├── expected/              # expected canonical outputs (CSV/JSON) + tolerances
├── policies/              # decode + conflict policies pinned for the gold set
└── notes/                 # why each case exists (human-readable)
```

Any change to fingerprints, extractors, registries, or decode policy must:
1. Improve anchored coverage / reduce unresolved claims, **and**
2. Not regress gold outputs (or explicitly version a breaking change with recorded diffs + rationale).

### Novelty scoring

Every claim and every decode decision carries a novelty score — how structurally similar is this input to patterns the gold set covers? High novelty = outside the gold set's tested distribution = lower confidence in decode correctness, even if all invariants pass. The gap dashboard flags novel patterns for human review.

The gold set grows. Every production run is an opportunity to discover new edge cases. Escalations become gold candidates. High-novelty decodes that survive human spot-checks become gold entries. The system gets more correct over time because the gold set absorbs more of the long tail through expert-curated expansion.

---

## Relationship to other tools

| Tool | Relationship |
|------|-------------|
| **canon** | Provides versioned entity registries for identity resolution |
| **twinning** | Receives mutations, enforces constraints, scores coverage |
| **verify** | Rules are precode constraints in bucket resolution |
| **assess** | Conflict policies align with assess decision bands |
| **benchmark** | Gold set assertions validate decode correctness |
| **fingerprint** | Template matches determine which extractor runs |
| **pack** | Decode explanations + mutations sealed as evidence |
| **data-fabric** | Event store for claims/mutations; Neo4j for entity resolution |

---

## Implementation notes

### Candidate crates

| Need | Crate | Notes |
|------|-------|-------|
| JSON parsing | `serde_json` | Claims, mutations, policy files |
| Content hashing | `sha2` | Claim content addressing |
| Message bus (hot path) | `async-nats` or `redis` | Claim ingestion |
| Policy engine | Custom | ~500 LOC cascade resolver |
| Graph queries | `neo4rs` | Entity resolution via data-fabric |

### Implementation scope

~1300 LOC core decode loop (bucket state machine + peeling + cascade + derivation graph + gold harness). Additional infrastructure for message bus integration, entity resolution, and reporting brings total to ~3-5K LOC Rust.

Follows the same implementation standards as protocol tools: `#![forbid(unsafe_code)]`, clap derive CLI, MIT license, CI (fmt -> clippy -> test).

---

## Determinism

Same claims + same policy + same registries = same mutations. Content-addressed claims ensure replay is exact. The decode explanation graph records every decision. Regressions are detectable by diffing mutation outputs across decode policy versions.
