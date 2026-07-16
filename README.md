# Flekks Medical Workspace

Flekks Medical Workspace is an open-source Rust terminal UI for building auditable clinical context plans and reviewing AI agent recommendations. It is derived from [OpenAI Codex](https://github.com/openai/codex) and explores a domain-workspace model for clinical documentation: clinicians maintain the living patient record, freeze an explicitly scoped context plan for a master-agent run, and retain the final decision over every proposed chart change.

> [!WARNING]
> This project is alpha research software for synthetic data only. It is not an electronic health record, medical device, clinical decision-support system, or HIPAA-ready product. Do not enter protected health information or use it for patient care.

This is an independent community project. It is not an official OpenAI product and is not endorsed or supported by OpenAI.

## Workspace Mode

`/workspace-medical` is a persistent domain workspace built one level above ordinary Codex Plan
Mode. Plan Mode makes one task decision-complete. Workspace Mode maintains changing patient context
over time, gives each synthetic patient a dedicated database-backed Plan conversation, and produces
immutable, decision-complete context plans for individual audited runs.

```text
Living patient workspace -> checkpointed patient Plan -> reviewed plan revision
          ^                       |                              |
          |                 guided questions                    v
          +---- human-reviewed recommendations <- optional master Codex handoff
```

The medical workspace is organized into three zones:

```text
Evidence / Chart Explorer | Canonical Human Chart | Plan with Codex
```

- **Evidence / Explorer:** active and archived synthetic patients, demographics, safety, contacts, coverage, encounters, notes, patient files, tasks, and audit history.
- **Canonical Chart:** human-controlled records, saved revisions, and clinician working drafts.
- **Plan with Codex:** a focused `Chat / Context / Audit` rail containing the patient-scoped Plan
  conversation, selected evidence, current/outdated plan revision, accessed-source provenance, and
  human review history. It deliberately does not render parent-harness MCP, usage, or token chrome.

The right rail runs a small, persistent version of the harness's real Plan workflow. It uses the
same resolved Plan model and reasoning-effort settings as the parent harness, but a stricter tool
surface: it can ask a focused question and read only patient/checkpoint-bound context through the
audited `workspace_context_read` tool. It cannot execute commands, mutate or sign chart data,
submit claims, or silently accept its own recommendations. Human and assistant messages, exact
context reads, and published plan revisions are stored in local SQLite. If Codex asks a question,
it appears as a normal saved response; answer in the composer to start a fresh persisted turn.

When the living workspace advances to a different checkpoint, the previous current plan is marked
outdated instead of being silently regenerated. The clinician decides when to ask Codex to refresh
and when a reviewed plan is ready for an explicit, separate parent-harness run using the model
currently selected there. Prior conversations, revisions, submissions, and results remain
immutable for audit.

### Plan-to-handoff workflow

1. Save the synthetic patient and note so the workspace can create an exact local checkpoint.
2. In `Plan with Codex`, send conversational questions or requests with `Enter`.
3. If Codex asks a focused question, answer it with another composer send.
4. When the context is complete, explicitly ask Codex to publish or update the reviewed Plan.
5. Inspect the Plan, selected evidence, and provenance in `Chat`, `Context`, and `Audit`.
6. Press `Ctrl-G` to submit that current reviewed Plan revision to the separate parent-harness run;
   returned work remains review-only until a clinician accepts a proposal.

The core safety invariant is:

> An agent may inspect explicitly authorized synthetic context and propose changes, but only an explicit clinician review action may create a new canonical chart revision.

See [Agent proposal workflow](docs/agent-proposal-workflow.md),
[Patient identity, contact, and coverage](PATIENT_IDENTITY_AND_COVERAGE.md), and
[Architecture](docs/architecture.md).

## Current capabilities

- Local SQLite patient, note, encounter, task, file-metadata, safety, packet, result, proposal, and audit persistence.
- `/workspace-medical` refuses to open against a remote app-server store; the current medical-data
  path is intentionally local-only.
- The supported launcher marks and exclusively locks its dedicated synthetic SQLite home. A
  read-only store check verifies the synthetic policy, expected workspace table-name inventory,
  `workspace_1.sqlite` quick-check and foreign keys, and filesystem permissions. It also reports
  free space and active/stale lock state.
- Structured patient display and legal identity, contact methods, mailing address, emergency contact,
  preferred language, interpreter need, and local contact notes.
- Ordered primary, secondary, and tertiary coverage records with optimistic version guards and a
  compatibility projection for older primary-coverage clients.
- Local-only patient search across identifiers, contact, emergency-contact, and coverage fields.
- Note revision history, local note locking, addenda, and stale-proposal checks.
- Local working-draft checkpoints for note title and body, context-plan instructions, and selected
  context, with explicit restore or discard after an interrupted session. Canonical chart changes
  still require an explicit human save.
- Explicit packet selection for multimodal file metadata and human-reviewed excerpts.
- Persistent patient Plan sessions, ordered human and assistant messages (including focused
  questions and clinician answers), immutable
  checkpoint-bound context reads, evidence-linked plan revisions, locked proposal records, and
  append-only review decisions.
- Dedicated medical Plan threads with memory generation disabled and a fail-closed
  `workspacePlanningOnly` tool mode. The allowed planning reads are bounded `patient_chart`,
  `visit_history`, `progress_notes`, and `selected_context`; selected context includes the exact
  checkpoint draft plus selected file metadata/reviewed text/clip transcripts, never raw bytes.
- Durable prepared packets, idempotent master-agent runs, immutable packet-source snapshots,
  review-pending results, revision-bound proposals, and append-only clinician decisions.
- First-class packet and master-run bindings to the exact reviewed Plan revision, Plan content
  hash, and evidence-manifest hash; revision submission is accepted only for that bound run.
- Patient-rooted atomic chart changesets with optimistic note revisions, opaque entity-version guards, durable idempotency receipts, exact-request retry, explicit note-only reconciliation, and fail-closed discard/reload for broader stale drafts.
- A one-turn medical handoff boundary that exposes only `workspace_context_read`, binds that reader
  to the submitted run and its explicitly authorized visit-history or progress-note categories,
  excludes prior-turn transcript data, skills, plugins, extension context, and hooks, then restores
  the live harness's previous tool mode. The boundary requires a persisted root thread with an
  exact packet-id, envelope-hash, run-id, thread, and model binding; it rejects inline attachments,
  out-of-band additional context, steering, compaction, review, realtime startup, and shell-command
  escape paths. The thread is permanently excluded from Codex memory generation, and a later
  resume or fork fails closed to no-tool mode. Selected files therefore stay inside the audited
  packet path. Returned note bodies are byte-bounded and local-path tokens are redacted before
  immutable snapshot hashing; generic logs and telemetry receive redacted tool arguments/results.
- Automatic packet-id/hash turn binding and review-pending capture of the final agent answer with thread/turn provenance.
- A responsive three-zone Evidence / Patient Chart / Plan with Codex layout with dedicated
  `Chat`, `Context`, and `Audit` views.
- Conventional multiline note editing and a bottom-pinned Plan composer. Note-body `Enter` inserts
  a newline; in the Plan composer `Enter` sends and `Shift-Enter` inserts a newline. Failed sends
  retain the exact draft for retry.
- Human-entered coverage-card comparison tied to a selected local source document, with append-only
  provenance and advisory `match`, `mismatch`, `unverified`, `stale`, or `incomplete` readiness.
- Read-only current-versus-proposed comparison with stale and signed-note guards.
- Deterministic Ratatui snapshot and readability harnesses.

## Build and run

Requirements:

- Rust toolchain declared in `codex-rs/rust-toolchain.toml`
- Git
- Python 3 for repository launch and check helpers
- `just` for the repository's canonical checks
- At least 10 GiB of free disk space; 30 GiB or more is recommended for the first build

```bash
git clone https://github.com/Dylan-Flekks/flekks-medical-workspace.git
cd flekks-medical-workspace
just medical-workspace
```

The first launch compiles the Codex Rust workspace and may take several minutes. The launcher uses
the smaller `dev-small` build profile with incremental compilation disabled, checks available disk
space before Cargo starts, and never deletes files automatically.

By default, medical SQLite data is isolated at
`$HOME/.codex/flekks-medical-synthetic`. The launcher creates that directory with private
permissions, sets the explicit synthetic-data classification, and refuses a relative path or a
path that resolves to the normal `CODEX_HOME`. To choose another location, reserve an absolute
directory for synthetic medical-workspace SQLite data only:

```bash
FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME=/absolute/private/synthetic/path just medical-workspace
```

The launcher does not copy records from another Codex or medical-workspace database and cannot
classify a nonempty, unclassified workspace database as synthetic. Do not point the override at
another Codex SQLite home. Codex configuration and authentication remain under `CODEX_HOME`; the
medical workspace's SQLite state remains separate.

To update an existing clone to the latest public version, first make sure `git status --short` is
empty. Commit or stash any local changes before switching branches or pulling, then run:

```bash
cd flekks-medical-workspace
git status --short
git switch main
git pull --ff-only origin main
just medical-workspace
```

When the TUI opens, run:

```text
/workspace-medical
```

The previous `/workspacemedical` spelling remains a deprecated compatibility alias but is hidden
from the command palette. On an empty isolated store, `/workspace-medical` performs the local
synthetic-policy preflight before opening the patient workspace. A plain `cargo run` intentionally
lacks that launch authority. Use `just medical-workspace` (or
`scripts/run_medical_workspace.sh`) for the supported synthetic demo flow.

Use synthetic fixtures only. Never enter PHI, real patient details, or production credentials. See
[Development](docs/development.md) for focused tests and snapshot review.

### Recover from `No space left on device`

If Cargo reports `No space left on device`, the later Rust and LLVM errors are usually a cascade
from the full disk rather than separate source-code failures. Stop the build, inspect the current
checkout and Cargo caches, and identify stale checkout/worktree targets before removing anything:

```bash
cd flekks-medical-workspace
df -h . "$HOME/.codex/flekks-medical-synthetic"
du -sh codex-rs/target "$HOME/.cargo/registry" "$HOME/.cargo/git" 2>/dev/null
find "$HOME" -type d -path '*/codex-rs/target' -prune -exec du -sh {} \; 2>/dev/null | sort -h
```

After verifying that the current checkout's build artifacts are safe to rebuild, this command
cleans only its Cargo target directory; it does not delete the synthetic medical SQLite directory:

```bash
cargo clean --manifest-path codex-rs/Cargo.toml
just medical-workspace
```

If another checkout or worktree owns the large stale target, run `cargo clean` against that
checkout's `codex-rs/Cargo.toml` instead. Do not delete an unfamiliar database or target directory.

### Inspect or reset the synthetic SQLite store

Run the read-only health report while the workspace is closed:

```bash
just medical-workspace-store-status
```

The checker refuses ordinary `CODEX_HOME`, missing or altered store markers, unclassified stores,
workspace table-name inventory drift, unsafe symlinks, `workspace_1.sqlite` quick-check failures,
and workspace foreign-key failures. It inventories every file in the dedicated store, but its
database health claims are intentionally limited to `workspace_1.sqlite`; other allowed Codex
SQLite files are included in the purge inventory, not reported as integrity-checked. It reports
free space, overly broad permissions, unexpected files, and stale launcher locks instead of
repairing or deleting them.

The purge command is a dry run unless both an exact canonical path and an absolute start-receipt
path outside the store are supplied:

```bash
just medical-workspace-store-purge

# Only after reviewing the printed path and inventory:
just medical-workspace-store-purge \
  --confirm "$HOME/.codex/flekks-medical-synthetic" \
  --receipt "$HOME/flekks-medical-purge-receipt.json"
```

Actual purge is refused while any launcher lock exists, when unexpected top-level content or a
cross-device descendant is present, or when either external receipt target already exists. The
receipt parent must already be a non-symlink directory. Purge creates the requested durable
start receipt without overwriting anything, removes the store, then creates a separate
`<receipt>.complete` file with the completion record. It never rewrites either receipt. Purge
acquires the same exclusive store lock as the launcher, revalidates the store while holding it,
removes the entire dedicated SQLite home, and verifies that it is gone; the next supported launch
creates a new empty store. It never deletes ordinary `CODEX_HOME`. Codex rollouts or logs retained
there are outside this reset boundary and must be reviewed separately—do not assume this command
is a complete regulated-data erasure workflow.

## Keyboard quick start

The medical workspace keeps pane navigation separate from the action performed inside a pane:

| Key | Medical workspace action |
| --- | --- |
| `Tab` / `Shift-Tab` | Move to the next or previous pane. |
| Arrow keys | Move, scroll, or edit only inside the focused pane. |
| `Ctrl-P` | Open Commands from any pane, including an active text editor. |
| `:` | Open Commands from navigation and read-only panes; remains typable in medical text fields. |
| `Ctrl-S` | Explicitly save the current human chart draft. |
| `Ctrl-G` | Submit the current reviewed Plan revision to the parent Codex harness. |
| `?` | Show the workspace action reference when the focused pane is not consuming text. |
| `Enter` | In the Plan composer, send the exact draft; in the note body, insert a newline. |
| `Shift-Enter` | Insert a newline in the Plan composer without sending. |

The command palette leads with actions relevant to the focused pane, followed by common chart actions and the Agent bridge. While a clinician edits an unsigned note, its live title appears in Patient Notes with an `[unsaved]` marker; that marker is working-state feedback and does not imply a canonical chart revision.

## Project status and limitations

The repository is being opened early because the system needs help from Rust/TUI developers, clinical-workflow designers, privacy and security engineers, accessibility reviewers, and agent-system builders.

Known blockers include:

- production safeguards for PHI, secure storage, authenticated clinician identity, role-based
  access, and privacy controls; the current local database is for synthetic/test data only;
- coverage-card verification is a human transcription and deterministic comparison workflow only:
  there is no remote card upload, OCR, model extraction, payer query, eligibility lookup, claim
  creation, EDI submission, or automatic chart mutation;
- working-draft recovery currently covers note text, context-plan instructions, and context
  selections tied to the exact patient, note, and base revision; demographic and coverage edits
  remain explicit-save canonical drafts rather than recoverable background checkpoints;
- working-draft checkpoints are local full snapshots; production-grade compression, retention,
  storage-health reporting, and authenticated ownership are not implemented yet;
- patient Plan conversations are model-generated recommendations for synthetic research data only;
  they do not replace clinician judgment, diagnosis, or review, and the separate parent-harness
  handoff remains explicit;
- atomic multi-document batch intake and durable restart recovery for an unresolved local changeset;
- partial per-change proposal review in both the app-server API and TUI; edited whole-proposal acceptance is state/API-ready;
- broader lifecycle recovery beyond the implemented unclaimed planning-turn and orphaned claimed
  master-turn reconciliation;
- technical enforcement that prevents accidental entry of real patient data.

The final Assistant message from a matching model turn is withheld from the UI and rollout history
until the exact response, body hash, thread/turn/model receipt, packet, submitted Plan revision, and
completed run commit together as review-pending Agent Review. Restricted prompts, reasoning, and
tool lifecycle events may still exist in the dedicated private rollout, but they are redacted from
generic diagnostics and excluded from later model context unless an exact durable completion
authorizes the turn. The explicit `:agent result save` path remains available only as a
clinician-attributed import for output produced outside the bound harness turn; it cannot claim that
the bound agent produced caller-supplied text.

See the [Patient identity, contact, and coverage design](PATIENT_IDENTITY_AND_COVERAGE.md), the
[Roadmap](ROADMAP.md), and issues labeled `help wanted`.

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md), open an issue for design-sensitive changes, and never include real patient data, credentials, or personal database files.

## Upstream and license

This repository contains a modified source snapshot of OpenAI Codex. See [UPSTREAM.md](UPSTREAM.md) for provenance and the major modification areas.

Licensed under the [Apache License 2.0](LICENSE). The original [NOTICE](NOTICE) is retained.
