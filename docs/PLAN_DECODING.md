# decoding — Phase 1

## One-line promise
**Turn archaeology claims into a deterministic canonical map for one bounded
legacy outcome slice.**

This repo is no longer planning the whole end-state decode universe as the
current implementation wedge. Phase 1 is intentionally narrow:

- archaeology mode only
- consume `claim.v0` from `crucible scan`
- converge claims into a canonical map
- emit escalations where ambiguity remains

Document extraction mode, entity resolution, and broader claim-resolution
surfaces are deferred.

---

## What Phase 1 is

Phase 1 is a deterministic convergence engine for legacy-system archaeology.

For the first Hyperion-style slices, `decoding` does not need to:

- canonicalize extracted financial rows
- resolve entity identity graphs
- talk to Neo4j
- drive twins
- mutate production databases

It needs to do one thing well:

**take messy claims from multiple legacy surfaces and produce the first usable,
auditable canonical understanding of the slice.**

---

## What Phase 1 is not

Phase 1 is NOT:

- document extraction decode
- mutation emission for production targets
- hot-path entity resolution
- a database
- a workflow engine
- a model-assisted reasoner

Those may return later. They are not part of the current build wedge.

---

## Target user story

An operator should be able to run:

```bash
decoding archaeology claims/*.jsonl \
  --policy legacy.decode.v0.json \
  --output canon-map.jsonl \
  --escalations escalations.jsonl \
  --convergence convergence.json
```

and receive:

- canonical entries for resolved archaeology subjects
- escalations for unresolved or conflicting subjects
- a convergence report showing what is settled and what still needs work

That is the Phase 1 bar.

---

## Phase 1 input contract

Phase 1 consumes the `claim.v0` contract emitted by `crucible`.

Required shape:

```json
{
  "event": "claim.v0",
  "claim_id": "sha256:...",
  "source": {
    "kind": "repo_scan",
    "scanner": "crucible.scan.repo@0.1.0",
    "artifact_id": "sha256:...",
    "locator": {
      "kind": "file_range",
      "value": "src/close_pack.py#L40-L65"
    }
  },
  "subject": {
    "kind": "report",
    "id": "hyperion.close_pack_ebitda"
  },
  "property_type": "depends_on",
  "value": {
    "kind": "feed",
    "id": "fdmee.actuals_load"
  },
  "confidence": 0.88
}
```

Rules:

- `claim_id` is content-addressed from normalized payload
- decoding trusts provenance, not narrative
- identical claims must replay identically
- unknown `subject.kind` or `property_type` values are refusal conditions in
  Phase 1

---

## Core model

### Why the old archaeology model was too narrow

The earlier archaeology plan bucketed claims around
`(table, column, property_type)`. That is too SQL-centric for the first
Hyperion slices.

We need to reason about more than tables and columns:

- jobs
- reports
- feeds
- mappings
- downstream consumers
- artifacts on disk

So Phase 1 uses a more general bucket key:

```text
(subject.kind, subject.id, property_type)
```

This is still simple and deterministic, but it fits the real legacy surfaces
better.

### Edge-aware buckets

The simple bucket key works for singular or set-valued properties such as
`schema`, `valid_values`, or `liveness`. It is too coarse for edge properties
like `reads` or `depends_on`, where one subject can have many independent
targets.

Phase 1 therefore uses a logical bucket key:

```text
base bucket: (subject.kind, subject.id, property_type)
edge bucket: (subject.kind, subject.id, property_type, value.kind, value.id)
```

Edge bucket rules apply to:

- `reads`
- `writes`
- `depends_on`
- `used_by`
- `authoritative_for`

The rest stay on the base bucket key.

### Buckets

Each unique logical bucket key is a bucket.

Claims pour into buckets from multiple sources. The bucket moves through a
small state machine:

```text
EMPTY -> SINGLE_SOURCE -> CONVERGING -> CONVERGED
                          |
                          v
                     CONFLICTED -> ESCALATED
```

### State meanings

| State | Meaning |
|-------|---------|
| `EMPTY` | no claim yet |
| `SINGLE_SOURCE` | one claim only |
| `CONVERGING` | multiple compatible claims |
| `CONVERGED` | enough evidence to publish canonical entry |
| `CONFLICTED` | incompatible claims exist |
| `ESCALATED` | conflict or ambiguity requires human review |

---

## Phase 1 archaeology vocabulary

Phase 1 needs a small stable property vocabulary. Do not over-design it.

Initial property types:

| Property type | Typical subjects | Meaning |
|---------------|------------------|---------|
| `exists` | all | subject exists |
| `schema` | table, column, view | structural definition |
| `constraint` | column, table | not null, FK, check, uniqueness |
| `reads` | job, procedure, report, consumer | reads from another subject |
| `writes` | job, procedure, feed | writes to another subject |
| `depends_on` | report, mapping, artifact | dependency edge |
| `used_by` | table, column, view, report | downstream usage |
| `schedule` | job, feed | cadence or trigger info |
| `valid_values` | column, mapping | allowed values |
| `semantic_label` | column, report line, mapping | business meaning hint |
| `liveness` | all | alive, dead, stale, unknown |
| `authoritative_for` | report, extract, consumer | authoritative output hint |

This list may grow, but Phase 1 should freeze a versioned vocabulary before
code starts.

### Value compatibility rules

Phase 1 needs a small property-aware comparator registry. Freeze the
compatibility rules before code starts:

| Property type | Compatible when |
|---------------|-----------------|
| `exists` | both claims are `true` |
| `schema` | normalized JSON deep-equal |
| `constraint` | normalized JSON deep-equal |
| `reads` | same subject ref |
| `writes` | same subject ref |
| `depends_on` | same subject ref |
| `used_by` | same subject ref |
| `schedule` | normalized JSON deep-equal |
| `valid_values` | same sorted set of strings |
| `semantic_label` | same normalized string |
| `liveness` | same state, or `alive` + `stale`, or `stale` + `unknown` |
| `authoritative_for` | same subject ref |

`alive` and `dead` conflict. `dead` should never auto-win from absence alone.

---

## Resolution rules

Phase 1 should stay conservative.

### Auto-resolve

These can resolve with little or no corroboration:

- `exists` from high-confidence structural scans
- `schema` from database metadata
- `constraint` from database metadata

### Need corroboration

These should normally require multiple compatible claims:

- `reads`
- `writes`
- `depends_on`
- `used_by`
- `schedule`
- `valid_values`
- `semantic_label`
- `authoritative_for`

### Liveness

`liveness` is special:

- structural evidence alone is weak
- executed evidence is stronger
- absence of evidence is not death

Phase 1 should prefer `alive`, `stale`, or `unknown` and avoid overclaiming
that something is dead.

---

## Phase 1 output contracts

### `canon_entry.v0`

Resolved buckets emit canonical entries.

Required shape:

```json
{
  "event": "canon_entry.v0",
  "bucket_id": "sha256:...",
  "subject": {
    "kind": "report",
    "id": "hyperion.close_pack_ebitda"
  },
  "property_type": "depends_on",
  "canonical_value": {
    "kind": "feed",
    "id": "fdmee.actuals_load"
  },
  "policy_id": "legacy.decode.v0",
  "convergence": {
    "state": "converged",
    "source_count": 3,
    "claim_count": 4
  },
  "explain": {
    "winner_claim_ids": ["sha256:...", "sha256:..."],
    "compatible_claim_ids": ["sha256:...", "sha256:..."],
    "resolution_kind": "corroborated"
  }
}
```

Frozen field contract:

| Field | Type | Rules |
|-------|------|-------|
| `event` | string | exactly `canon_entry.v0` |
| `bucket_id` | string | `sha256:<64 lowercase hex>` of the logical bucket key |
| `subject.kind` | enum | same frozen vocabulary as input |
| `subject.id` | string | same normalized ID as input |
| `property_type` | enum | same frozen vocabulary as input |
| `canonical_value` | JSON | normalized value chosen by policy |
| `policy_id` | string | `legacy.decode.v0` for Phase 1 |
| `convergence.state` | enum | `single_source`, `converging`, `converged` |
| `convergence.source_count` | integer | number of distinct source artifacts contributing |
| `convergence.claim_count` | integer | total contributing claims |
| `explain.winner_claim_ids` | array | sorted winning claim IDs |
| `explain.compatible_claim_ids` | array | sorted compatible claim IDs included in support |
| `explain.resolution_kind` | enum | `single_source`, `corroborated`, `priority_break`, `liveness_fold` |

The explanation payload should stay structured. Free-text commentary can wait.

### `escalation.v0`

Unresolved or conflicting buckets emit escalations.

Required shape:

```json
{
  "event": "escalation.v0",
  "bucket_id": "sha256:...",
  "subject": {
    "kind": "mapping",
    "id": "adj.ebitda.rule.family"
  },
  "property_type": "semantic_label",
  "reason": "conflicted",
  "claim_ids": ["sha256:...", "sha256:..."],
  "recommended_action": "review",
  "summary": "two incompatible semantic interpretations remain"
}
```

Frozen field contract:

| Field | Type | Rules |
|-------|------|-------|
| `event` | string | exactly `escalation.v0` |
| `bucket_id` | string | same bucket hash used for canonical entries |
| `subject.kind` | enum | same frozen vocabulary as input |
| `subject.id` | string | same normalized ID as input |
| `property_type` | enum | same frozen vocabulary as input |
| `reason` | enum | `conflicted`, `missing_corroboration`, `invalid_value`, `unknown_property_type`, `unknown_subject_kind` |
| `claim_ids` | array | sorted claim IDs in the bucket |
| `recommended_action` | enum | `review`, `scan_more`, `fix_scanner`, `fix_policy` |
| `summary` | string | short human-readable one-line explanation |

Escalations are the bounded review queue. If a bucket cannot produce one of
the above reasons, the reason model is still underspecified.

### `convergence.v0`

Phase 1 also emits one report summarizing:

- bucket counts by state
- top conflicted subjects
- marginal value by source class
- unresolved areas by surface

At minimum the convergence report must contain:

```json
{
  "event": "convergence.v0",
  "policy_id": "legacy.decode.v0",
  "totals": {
    "buckets": 0,
    "converged": 0,
    "converging": 0,
    "single_source": 0,
    "conflicted": 0,
    "escalated": 0
  },
  "by_property_type": {},
  "by_source_kind": {},
  "top_escalations": []
}
```

### `legacy.decode.v0.json`

Phase 1 also needs a frozen minimal policy contract:

```json
{
  "policy_id": "legacy.decode.v0",
  "auto_resolve": ["exists", "schema", "constraint"],
  "min_corroboration": {
    "reads": 2,
    "writes": 2,
    "depends_on": 2,
    "used_by": 2,
    "schedule": 2,
    "valid_values": 2,
    "semantic_label": 2,
    "authoritative_for": 2
  },
  "source_priority": {
    "liveness": ["db_scan", "file_scan", "repo_scan"]
  }
}
```

Phase 1 policy should remain declarative and small. If the engine needs
property-specific code beyond the comparator registry and liveness fold, the
policy surface is too ambitious.

---

## CLI

Phase 1 should ship a narrow CLI:

```text
decoding archaeology <CLAIMS>... --policy <FILE> [OPTIONS]

Arguments:
  <CLAIMS>...              Claim JSONL files

Options:
  --policy <FILE>          Archaeology decode policy
  --output <FILE>          Canon entry JSONL output
  --escalations <FILE>     Escalation JSONL output
  --convergence <FILE>     Convergence report JSON output
  --json                   JSON status messages
```

Exit codes:

- `0` no escalations
- `1` escalations emitted
- `2` refusal / invalid claim set / invalid policy

Do not ship a broader CLI in Phase 1.

---

## Build order

1. **Freeze archaeology vocabulary**
   Finalize `subject.kind`, `property_type`, and the normalized claim schema.

2. **Bucket state machine**
   Insert claims deterministically and drive state transitions.

3. **Convergence tracker**
   Count sources, detect compatible vs conflicting claims, and generate
   convergence summaries.

4. **Archaeology policy engine**
   Encode the conservative resolution rules for structural, behavioral, and
   semantic properties.

5. **`canon_entry.v0` / `escalation.v0` outputs**
   Emit stable JSONL outputs with explanation payloads.

6. **Convergence report**
   Show what settled, what conflicted, and where the next scan should focus.

That is enough for Phase 1.

---

## Test strategy

Phase 1 does not need a massive gold-set system to start. It needs strong
determinism and fixture coverage.

Required test layers:

1. synthetic bucket transition tests
2. conflicting claim fixtures
3. replay determinism tests
4. mixed-source archaeology fixtures
5. explanation payload snapshots

If archaeology decode proves valuable on the first real slices, we can promote
repeating fixtures into a larger regression harness later.

---

## Phase 1 implementation checklist

Decoding is Phase 1 implementation-ready when this checklist is concrete enough
to code without reopening the model:

1. **Contract module**
   - input claim parser
   - frozen enums for `subject.kind` and `property_type`
   - bucket-id builder
   - normalized value helpers

2. **Bucket store**
   - deterministic grouping by `(subject.kind, subject.id, property_type)`
   - claim ordering by canonical claim ID
   - source-artifact distinct counting

3. **Comparator registry**
   - one comparator per frozen property type
   - compatibility tests for every comparator
   - liveness fold logic isolated and explicit

4. **Policy loader**
   - parse `legacy.decode.v0.json`
   - refuse unknown policy keys in Phase 1
   - wire `auto_resolve`, `min_corroboration`, and `source_priority`

5. **State machine + resolver**
   - drive bucket states
   - choose canonical value or escalation
   - emit structured explanation payloads

6. **Output writers**
   - `canon_entry.v0` JSONL
   - `escalation.v0` JSONL
   - `convergence.v0` JSON

7. **Determinism and fixture tests**
   - replay-identical input test
   - mixed-source archaeology fixture
   - conflicted bucket fixture
   - invalid-contract refusal fixture

Phase 1 coding should start only after the comparator registry and minimal
policy contract are frozen.

---

## Deferred after Phase 1

These are explicitly parked:

- document extraction mode
- mutation emission for target databases
- entity resolution
- `canon org` hot-path integration
- Neo4j / data-fabric graph queries
- extraction-mode gold set infrastructure
- broader cascade machinery for financial claim resolution

Phase 1 should not carry these abstractions.

---

## Relationship to `crucible`

`crucible` discovers evidence. `decoding` converges it.

```text
legacy estate
  -> crucible scan
  -> claim.v0
  -> decoding archaeology
  -> canon_entry.v0 + escalation.v0 + convergence.v0
```

For the first Hyperion slices, that is the entire product surface of
`decoding`.

---

## Hyperion slice #1 readiness

Decoding Phase 1 is implementation-ready when all of the following are true:

- `claim.v0` is frozen with stable subject and property vocabularies
- archaeology CLI scope is fixed
- canonical entry and escalation outputs are fixed
- a conservative default policy exists
- determinism and fixture tests can be written without open design questions

Decoding Phase 1 is functionally successful when one real legacy slice can be
fed from `crucible scan` into `decoding` and produce:

- a useful first canonical map
- a bounded human review queue
- clear next-scan guidance

That is enough to start the manual replacement loop.
