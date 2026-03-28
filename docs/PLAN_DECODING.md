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
    "kind": "db_scan | repo_scan | file_scan",
    "scanner": "crucible.scan.repo@0.1.0",
    "evidence_ref": "content-addressed pointer or file/line locator"
  },
  "subject": {
    "kind": "table | column | view | procedure | job | report | feed | artifact | consumer | mapping",
    "id": "stable subject id"
  },
  "property_type": "stable archaeology vocabulary value",
  "value": {},
  "confidence": 0.0
}
```

Rules:

- `claim_id` is content-addressed from normalized payload
- decoding trusts provenance, not narrative
- identical claims must replay identically

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

### Buckets

Each unique `(subject.kind, subject.id, property_type)` is a bucket.

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
  "subject": {
    "kind": "report",
    "id": "close.pack.ebitda"
  },
  "property_type": "depends_on",
  "canonical_value": {
    "upstreams": ["job:fdmee.load.actuals", "artifact:calc/ebitda.csc"]
  },
  "convergence": {
    "state": "converged",
    "sources": 3,
    "claims": 4
  },
  "explain": {
    "contributing_claims": ["sha256:...", "sha256:..."],
    "resolution": "repo scan and log scan agree; metadata export adds dependency detail"
  }
}
```

### `escalation.v0`

Unresolved or conflicting buckets emit escalations.

Required shape:

```json
{
  "event": "escalation.v0",
  "subject": {
    "kind": "mapping",
    "id": "adj.ebitda.rule.family"
  },
  "property_type": "semantic_label",
  "reason": "conflicted",
  "claims": ["sha256:...", "sha256:..."],
  "summary": "two incompatible semantic interpretations remain"
}
```

### `convergence.v0`

Phase 1 also emits one report summarizing:

- bucket counts by state
- top conflicted subjects
- marginal value by source class
- unresolved areas by surface

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
