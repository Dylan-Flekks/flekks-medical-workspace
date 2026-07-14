# Architecture

Flekks Medical Workspace extends the Codex TUI and app-server with a persistent, auditable domain workspace above ordinary single-task Plan Mode. The living patient workspace changes as records are saved; each agent submission freezes one decision-complete context plan with its source revisions and authorization boundary.

```text
/workspace-medical
  -> WorkspaceDashboard state/reducer/rendering
  -> living context-plan draft and readiness
  -> immutable packet + source checkpoint
  -> App effect dispatcher
  -> AppServerSession v2 RPC
  -> bounded master Codex run
  -> review-pending recommendation
  -> WorkspaceStore transactions
  -> workspace SQLite database and local file references
```

## Intended UI boundaries

- **Explorer:** local patient search, active/archived patients, and selected-patient chart tree.
- **Canonical Chart:** human-owned records and working drafts.
- **Context Plan / Agent Review:** objectives, evidence selection, readiness, frozen requests, runs, accessed sources, recommendations, proposal review, and audit history.

The dashboard must not embed ChatWidget or render parent-harness usage, MCP, token, or notification details. Transitioning to the full agent harness is explicit, and returning work remains review-pending. Workspace chrome may expose only the bridge lifecycle: idle, working, returned, or attention.

The local Workspace Planner owns deterministic packet readiness, including hard gates, warnings,
and checkpoint-bound acknowledgements. It does not own deep patient inference. Once a clinician
reviews and freezes the plan, the full Codex harness performs the higher-capability run in the
background; opening that run is an explicit navigation action, while its result returns to Agent
Review without changing canonical chart data.

## Data boundaries

- Canonical notes are revisioned.
- Context plans are living drafts until review and submission.
- Submitted context packets are immutable clinician work orders bound to a plan checkpoint.
- Agent runs are attempts against one packet and base note revision.
- Source-access rows record the exact version/snapshot returned to an agent.
- Agent results are immutable recommendations.
- Note proposals are reviewable diffs.
- Human decisions are append-only audit records.
- Patient-rooted chart changesets commit all included human edits in one SQLite transaction with note-revision and entity-version guards.
- Every chart changeset has a durable idempotency receipt; uncertain delivery retries the exact request. A stale note-only edit requires an explicit visible reconciliation, while stale non-note or multi-entity drafts stay frozen until the clinician discards and reloads canonical data.

Only an explicit human decision may create a new canonical chart revision. If relevant patient, note, or source revisions change after submission, the original packet and result remain auditable but must not be treated as current without a refreshed plan.

## Current status

The first vertical slice now implements prepared/submitted packet lifecycle, idempotent runs, immutable envelope and hashed authorization-contract sources, a model-visible packet-authorized reader for bounded/path-redacted visit-history and progress-note snapshots, automatic matching-turn final-answer capture with thread/turn attribution, run-bound results, revision-bound proposals, append-only decisions, patient-rooted atomic multi-record saves, and the responsive three-zone UI. The explicit `:agent result save` path remains a clinician-attributed recovery import for output produced outside the bound turn. Atomic multi-document intake, authenticated identity, abrupt-restart reconciliation, and extending the authorized reader to additional clinical categories remain active roadmap work. See [ROADMAP.md](../ROADMAP.md).
