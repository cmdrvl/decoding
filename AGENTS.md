# AGENTS.md — decoding

> Guidelines for AI coding agents working in this Rust codebase.

---

## RULE 0 — THE FUNDAMENTAL OVERRIDE PREROGATIVE

If the user tells you to do something, even if it goes against what follows below, YOU MUST LISTEN. THE USER IS IN CHARGE, NOT YOU.

---

## RULE 1 — NO FILE DELETION

**YOU ARE NEVER ALLOWED TO DELETE A FILE WITHOUT EXPRESS PERMISSION.** Even a new file that you yourself created, such as a test code file.

**YOU MUST ALWAYS ASK AND RECEIVE CLEAR, WRITTEN PERMISSION BEFORE EVER DELETING A FILE OR FOLDER OF ANY KIND.**

---

## Irreversible Git & Filesystem Actions — DO NOT EVER BREAK GLASS

1. **Absolutely forbidden commands:** `git reset --hard`, `git clean -fd`, `rm -rf`, or any command that can delete or overwrite code/data must never be run unless the user explicitly provides the exact command and states, in the same message, that they understand and want the irreversible consequences.
2. **No guessing:** If there is any uncertainty about what a command might delete or overwrite, stop immediately and ask the user for specific approval. "I think it's safe" is never acceptable.
3. **Safer alternatives first:** When cleanup or rollbacks are needed, request permission to use non-destructive options (`git status`, `git diff`, `git stash`, copying to backups) before ever considering a destructive command.
4. **Mandatory explicit plan:** Even after explicit user authorization, restate the command verbatim, list exactly what will be affected, and wait for a confirmation that your understanding is correct. Only then may you execute it — if anything remains ambiguous, refuse and escalate.
5. **Document the confirmation:** When running any approved destructive command, record (in the session notes / final response) the exact user text that authorized it, the command actually run, and the execution time. If that record is absent, the operation did not happen.

---

## Git Branch: ONLY Use `main`, NEVER `master`

**The default branch is `main`. The `master` branch exists only for legacy URL compatibility.**

- **All work happens on `main`** — commits, PRs, feature branches all merge to `main`
- **Never reference `master` in code or docs** — if you see `master` anywhere, it's a bug
- **The `master` branch must stay synchronized with `main`** — after pushing to `main`, also push to `master`:
  ```bash
  git push origin main:master
  ```

---

## Repository Role

**decoding** is a deterministic convergence engine for legacy-system archaeology. It consumes derived `claim.v0` events from `crucible scan` and produces canonical entries where claims converge, escalations where they conflict, and convergence reports summarizing the state of resolution.

### Position in Stack

decoding sits downstream of crucible and upstream of human review:

```
legacy estate
  -> crucible scan
  -> metadata catalog (direct observations — bypass decoding)
  -> derived claim.v0 (ambiguous/inferential — goes through decoding)
  -> decoding archaeology
  -> canon_entry.v0 + escalation.v0 + convergence.v0
```

decoding only owns derived claims — inferred values, liveness assessments, semantic labels, weak dependency edges. Directly observed metadata (table existence, file inventory, mechanically extractable lineage) lands in the catalog and bypasses decode entirely.

### Key Concept: Observation vs Decode

- **Observed metadata** — facts directly recoverable from scans, normalized into the metadata catalog
- **Derived claims** — propositions that are ambiguous, inferential, or contradicted across sources

If an implementation finds itself parsing catalog records directly, the boundary has drifted.

---

## Toolchain: Rust & Cargo

- **Package manager:** Cargo only, never anything else
- **Edition:** Rust 2024 (follow `rust-toolchain.toml`)
- **Unsafe code:** Forbidden (`#![forbid(unsafe_code)]`)
- **Dependencies:** Explicit versions, small and pinned

### Release Profile

```toml
[profile.release]
opt-level = "z"     # Optimize for size (lean binary for distribution)
lto = true          # Link-time optimization
codegen-units = 1   # Single codegen unit for better optimization
panic = "abort"     # Smaller binary, no unwinding overhead
strip = true        # Remove debug symbols
```

### Quality Gate (Rust)

Run after any substantive code changes:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

---

## Code Editing Discipline

### No Script-Based Changes

**NEVER** run a script that processes/changes code files. Make code changes manually.

### No File Proliferation

Revise existing code files in place. **NEVER** create variations like `main_v2.rs`.

### No Backwards-Compatibility Shims

We do not care about backwards compatibility — we're in early development. Do things the **RIGHT** way with **NO TECH DEBT**.

---

## Quick Reference

```bash
# Read the spec first
sed -n '1,100p' docs/PLAN_DECODING.md

# See the execution graph
br ready
br blocked

# AI-agent prioritization
bv --robot-next
bv --robot-triage --robot-max-results 5

# Quality gate
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
ubs .
```

---

## Source of Truth

- **Spec:** [`docs/PLAN_DECODING.md`](./docs/PLAN_DECODING.md) — all behavior must follow this document
- **Execution graph:** [.beads/issues.jsonl](./.beads/issues.jsonl)
- Do not invent behavior not present in the plan
- If code, README, and plan disagree, the plan wins

### Key Files

| Path | Responsibility |
|------|----------------|
| `src/main.rs` | Thin binary entrypoint only |
| `src/lib.rs` | Module root and shared library surface |
| `src/cli.rs` | Clap surface, exit-code mapping, file loading orchestration |
| `src/contracts/{mod,claim,canon_entry,escalation,convergence,policy,vocabulary}.rs` | Wire contracts, serde schemas, contract validation, frozen enums |
| `src/normalize.rs` | Canonical JSON, string normalization, sorted-set helpers, hash helpers |
| `src/bucket.rs` | Logical bucket keys, edge/base bucket construction, bucket grouping |
| `src/compare.rs` | Property-aware comparator registry |
| `src/resolve.rs` | State machine and resolution decisions |
| `src/render.rs` | canon_entry and escalation JSONL output rendering |
| `src/report.rs` | Convergence summary generation |
| `tests/contracts/*.rs` | Parse/refusal and schema tests |
| `tests/fixtures/*.rs` | Mixed-source archaeology fixtures |
| `tests/snapshots/*.rs` | Explanation and output snapshots |

Critical structural rule:

- `src/main.rs` stays thin
- module declarations and shared APIs belong in `src/lib.rs`

---

## Output Contract (Critical)

Target domain outcomes:

| Exit | Outcome | Meaning |
|------|---------|---------|
| `0` | Clean | All claims converged, no escalations |
| `1` | Escalations | One or more buckets escalated for human review |
| `2` | Refusal | Invalid claim set, invalid policy, or contract violation |

Target output routing:

- `--output <FILE>`: canon_entry.v0 JSONL (default: stdout)
- `--escalations <FILE>`: escalation.v0 JSONL
- `--convergence <FILE>`: convergence.v0 JSON summary
- `--json`: JSON status messages on stderr
- stderr without `--json`: human-readable status only

Refusal output goes to stderr. Refusals are contract violations, not domain outcomes.

---

## Core Invariants (Do Not Break)

### 1. Deterministic output

Same input claim set + same policy file = byte-for-byte identical `canon_entry.v0`, `escalation.v0`, and `convergence.v0` outputs. No randomness, no timestamp-dependent behavior.

### 2. Hard refusal boundary

Malformed or unknown claims must fail fast at the refusal boundary (exit 2) and never leak into escalation handling. Refusal conditions: malformed JSONL, missing required fields, malformed `claim_id`, unknown `source.kind`, unknown `subject.kind`, unknown `property_type`, value shape mismatches, unknown policy keys.

### 3. Edge bucket independence

Edge properties (`reads`, `writes`, `depends_on`, `used_by`, `authoritative_for`) use an extended bucket key that includes `(value.kind, value.id)`. One subject can have many independent targets without collapsing into a single bucket.

### 4. Duplicate claim collapse

Repeated identical `claim_id`s collapse to one logical claim before bucketing. Source-artifact distinct counting is computed from surviving distinct claims. Explanation payloads never repeat the same `claim_id`.

### 5. Conservative liveness

`liveness` uses special fold rules. Structural evidence alone is weak. Absence of evidence is not death. Prefer `alive`, `stale`, or `unknown` over overclaiming `dead`.

### 6. Observation vs decode boundary

decoding only owns derived claims. If the implementation starts parsing table/file/resource/link catalog records directly, the boundary has drifted and should be corrected. Direct observations belong in the metadata catalog.

### 7. Frozen vocabulary in Phase 1

Unknown `source.kind`, `subject.kind`, or `property_type` values are refusal conditions, not escalation conditions. Do not add vocabulary entries without freezing them in the plan first.

### 8. Explanation payloads are structured

Every canonical entry carries a structured `explain` block with `winner_claim_ids`, `compatible_claim_ids`, and `resolution_kind`. Free-text commentary is not part of Phase 1.

### 9. Stable bucket identity

`bucket_id` must be computed from canonical JSON of the logical bucket key with deterministic key ordering. If bucket identity is unstable across identical reruns, stop and fix normalization before widening the fixture corpus.

---

## Beads (`br`) — Issue Tracking

**Note:** `br` is non-invasive — it NEVER executes git commands. After `br sync --flush-only`, you must manually run `git add .beads/` and `git commit`.

Beads is the execution source of truth in this repo.

- Beads = task graph, state, priorities, dependencies
- Agent Mail = coordination, reservations, audit trail

```bash
br ready              # Show unblocked ready work
br list --status=open # All open issues
br show <id>          # Full issue details with dependencies
br create --title="..." --type=task --priority=2
br update <id> --status=in_progress
br close <id> --reason "Completed"
br sync --flush-only  # Export to JSONL (NO git operations)
```

### Conventions

- Include bead IDs in coordination subjects, e.g. `[dc-1cp] Start: bucket store`
- Use the bead ID in reservation reasons for traceability
- Prefer concrete ready beads over the epic tracker

### Workflow

1. Start with `br ready` and pick one unblocked bead.
2. Mark it `in_progress` before coding.
3. Reserve exact files and send start message.
4. Implement + validate.
5. Close bead, send completion summary, release reservations.

### Idle Rule

If you are blocked or idle:
1. Run `br ready`
2. Pick an unblocked bead and continue
3. If none are ready, report blockers and state the next fallback task

---

## bv — Graph-Aware Triage (Optional)

Use `bv` robot mode when dependency-aware prioritization is unclear:

```bash
bv --robot-triage  # Full triage view with recommendations
bv --robot-next    # Single top recommendation
```

**Important:** use only `--robot-*` commands in automation. Bare `bv` opens an interactive TUI.

---

## UBS — Pre-Commit Scanner

**Golden Rule:** `ubs <changed-files>` before every commit. Exit 0 = safe.

Useful patterns:

```bash
ubs $(git diff --name-only --cached)   # staged files
ubs --only=rust,toml src/              # language-filtered scan
ubs --ci --fail-on-warning .           # CI-style strict run
```

---

## ast-grep vs ripgrep

Use `ast-grep` when structure matters:
- codemods/refactors
- syntax-aware policy checks
- safe pattern rewrites

Use `rg` when text search is enough:
- finding literals/config keys/TODOs
- fast repository reconnaissance

Rule of thumb:
- structural match or rewrite -> `ast-grep`
- textual search -> `rg`

---

## Commit Cadence — One Bead, One Commit, One Push

**Commit and push after completing each bead.** Do not accumulate work across multiple beads before committing. The workflow for every bead is:

1. Claim the bead (`br show <id>`, add a comment that you're starting)
2. Implement the work
3. Run quality gates: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
4. Commit and push immediately:
   ```bash
   br sync --flush-only
   git add .beads/ <changed files>
   git commit -m "<bead-id>: <short description of what was done>"
   git push
   ```
5. Close the bead: `br close <id> --reason "Completed"`
6. Move to the next bead

**Do NOT:**
- Work on multiple beads before committing
- Accumulate a large uncommitted diff
- Wait until the end of the session to commit
- Skip the push step

**Do:**
- Keep commits small and focused (one bead = one commit)
- Push after every commit so other agents can pull your changes
- Run `git pull --rebase` before starting each new bead to pick up others' work

---

## MCP Agent Mail — Multi-Agent Coordination

Agent Mail is the coordination layer for multi-agent sessions in this repo: identities, inbox/outbox, thread history, and advisory file reservations.

### Session Baseline

1. If direct MCP Agent Mail tools are available, ensure project and reuse your identity:
   - `ensure_project(project_key="/Users/zac/Source/cmdrvl/decoding")`
   - `whois(project_key, agent_name)` or `register_agent(...)` only if identity does not exist
2. Reserve only exact files you will edit:
   - Allowed: `src/bucket.rs`, `src/compare.rs`
   - Not allowed: `src/**`, `src/contracts/**`, whole directories
3. Send a short start message and finish message for each bead, reusing the bead ID as thread.
4. Check inbox at moderate cadence (roughly every 2-5 minutes), not continuously.

### Important `ntm` Boundary

When this repo is worked via `ntm`, the session may be connected to Agent Mail even if the spawned harness does **not** expose direct `mcp__mcp-agent-mail__...` tools.

If direct MCP Agent Mail tools are unavailable:

- do **not** stop working just because mail tools are absent
- continue with `br`, exact file reservations via the available coordination surface, and overseer instructions
- treat Beads + narrow file ownership as the minimum coordination contract

### Stability Rules

- Do not run retry loops for `register_agent`, `create_agent_identity`, or `macro_start_session`.
- If a call fails with a transient DB/SQLite lock error, back off for **90 seconds** before retrying.
- Continue bead work while waiting for retry windows; do not block all progress on mail retries.

### Communication Rules

- If a message has `ack_required=true`, acknowledge it promptly.
- Keep bead updates short and explicit: start message, finish message, blocker message.
- Reuse a stable bead thread when possible for searchable history.

### Reservation Rules

- Reserve only specific files you are actively editing.
- Never reserve entire directories or broad patterns.
- If a reservation conflict appears, pick another unblocked bead or a non-overlapping file.

---

## File Reservation Guidance

This repo is designed for parallel agent work. Reserve exact files only.

Per-lane target surfaces:

| Lane | Expected files |
|------|----------------|
| bootstrap | `Cargo.toml`, `src/lib.rs`, `src/main.rs` |
| cli | `src/cli.rs` |
| contracts — claim | `src/contracts/claim.rs`, `src/contracts/vocabulary.rs` |
| contracts — canon_entry | `src/contracts/canon_entry.rs` |
| contracts — escalation | `src/contracts/escalation.rs` |
| contracts — convergence | `src/contracts/convergence.rs` |
| contracts — policy | `src/contracts/policy.rs` |
| normalize | `src/normalize.rs` |
| bucket | `src/bucket.rs` |
| compare | `src/compare.rs` |
| resolve | `src/resolve.rs` |
| render | `src/render.rs` |
| report | `src/report.rs` |
| fixtures | `tests/fixtures/**`, test harness files |
| snapshots | `tests/snapshots/**` |

Do not reserve broad globs like `src/**` or `src/contracts/**`.

---

## Project-Specific Guidance

### Keep contracts separate from logic

`src/contracts/` owns wire shapes, serde schemas, and validation. Do not put resolution logic or state-machine behavior in contract modules.

### Keep compare separate from resolve

`src/compare.rs` owns property-aware compatibility rules. `src/resolve.rs` owns state transitions and decision-making. Do not leak resolution policy into the comparator.

### Keep render separate from report

`src/report.rs` owns convergence summary generation and math. `src/render.rs` only formats existing data into JSONL lines. Do not re-derive counts in the renderer.

### Keep bucket identity isolated

`src/bucket.rs` owns logical bucket keys and `bucket_id` hashing. `src/normalize.rs` provides the canonical JSON and hashing primitives. Do not compute bucket IDs anywhere else.

### Prefer plan terms in code and tests

Use the plan vocabulary directly:

- `claim.v0`, `canon_entry.v0`, `escalation.v0`, `convergence.v0`
- `single_source`, `corroborated`, `priority_break`, `liveness_fold`
- `conflicted`, `missing_corroboration`, `no_resolution_path`
- `review`, `scan_more`, `fix_scanner`, `fix_policy`
- `bucket_id`, `claim_id`

Avoid renaming these into "friendlier" local synonyms.

---

## CI / Release Status

Current repo reality:

- Phase 1 fully implemented — 4,200+ lines of Rust, 75+ tests
- CI workflow at `.github/workflows/ci.yml` (fmt + clippy + test)
- Release workflow at `.github/workflows/release.yml` (cross-platform binaries on tag push)
- Smoke workflow at `.github/workflows/smoke.yml` (CLI execution tests)
- v0.1.0 is the first tagged release

CI/release discipline:

- `fmt` / `clippy` / `test` / `ubs` before publish
- deterministic artifacts
- `main` as primary branch
- sync `master` for legacy compatibility

---

## Multi-Agent Coordination

When working alongside other agents:

- **Never stash, revert, or overwrite other agents' work**
- Treat unexpected changes in the working tree as if you made them
- If you see changes you didn't make in `git status`, those are from other agents working concurrently — commit them together with your changes
- This is normal and happens frequently in multi-agent environments

**Do NOT:**
- Stop and ask "I see unexpected changes, what should I do?"
- Offer options like "triage these changes" or "run a full suite"
- Express concern about uncommitted work you don't recognize

**Do:**
- Continue working as normal
- Include those changes when you commit (they belong to the shared effort)
- Trust that other agents know what they're doing

---

## Landing the Plane (Session Completion)

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

1. **File issues for remaining work** — Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) — fmt, clippy, test
3. **Update issue status** — Close finished work, update in-progress items
4. **PUSH TO REMOTE** — This is MANDATORY:
   ```bash
   git pull --rebase
   br sync --flush-only
   git add .beads/ <other files>
   git commit -m "..."
   git push
   git push origin main:master
   git status  # MUST show "up to date with origin"
   ```
5. **Verify** — All changes committed AND pushed
6. **Summarize** — what changed, what was validated, remaining risks

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing — that leaves work stranded locally
- NEVER say "ready to push when you are" — YOU must push
- If push fails, resolve and retry until it succeeds
