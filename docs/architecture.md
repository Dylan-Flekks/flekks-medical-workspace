# Architecture

Flekks Medical Workspace extends the Codex TUI and app-server with a local patient-workspace domain.

```text
/workspacemedical
  -> WorkspaceDashboard state/reducer/rendering
  -> WorkspaceDashboardAction
  -> synthetic data-policy preflight
  -> App effect dispatcher
  -> AppServerSession v2 RPC
  -> workspace request processor
  -> WorkspaceStore transactions
  -> workspace SQLite database and local file references
```

## Intended UI boundaries

- **Explorer:** local patient search, active/archived patients, and selected-patient chart tree.
- **Canonical Chart:** human-owned records and working drafts.
- **Agent Work:** requests, runs, accessed sources, recommendations, proposal review, and audit history.

The dashboard must not embed ChatWidget. Transitioning to the agent harness is explicit, and returning work remains review-pending.

## Data boundaries

- Canonical notes are revisioned.
- Context packets are immutable clinician work orders.
- Agent runs are attempts against one packet and base note revision.
- Source-access rows record the exact version/snapshot returned to an agent.
- Agent results are immutable recommendations.
- Note proposals are reviewable diffs.
- Human decisions are append-only audit records.
- Patient-rooted chart changesets commit all included human edits in one SQLite transaction with note-revision and entity-version guards.
- Every chart changeset has a durable idempotency receipt; uncertain delivery retries the exact request. A stale note-only edit requires an explicit visible reconciliation, while stale non-note or multi-entity drafts stay frozen until the clinician discards and reloads canonical data.

Only an explicit human decision may create a new canonical chart revision.

## Synthetic workspace boundary

The supported launcher isolates workspace SQLite databases from ordinary Codex
SQLite state by default, sets an exact synthetic-only environment assertion,
and repeats the selected SQLite home as the highest-precedence CLI override.
Normal Codex configuration, authentication, logs, and rollouts continue to use
`CODEX_HOME`. A custom SQLite path must be reserved by the user for synthetic
medical workspace state. The app-server snapshots transition authority at
startup. It can transition the durable singleton policy from `unclassified` to
`synthetic` only while every known workspace table is empty. The transition is
one-way and protected by both runtime validation and SQLite triggers; content
heuristics never classify a store.

Before the dashboard opens, `AppServerSession` reads this policy. A local empty
store may be provisioned only when the server reports valid launch authority.
An unclassified remote store is always rejected because the TUI never
auto-provisions remote state. The session caches a confirmed synthetic policy
and checks it again at its mutation facade for chart commits, signatures,
addenda, proposal decisions, document/task changes, packet creation, new agent
runs, and new agent results. Audit/history reads and terminal reconciliation
remain available. Third-party app-server clients can still write ordinary rows
before provisioning; doing so permanently prevents classification and keeps
model-visible workspace gates closed.

## Current status

The first vertical slice now implements prepared/submitted packet lifecycle, idempotent runs, immutable envelope and hashed authorization-contract sources, a model-visible packet-authorized reader for bounded/path-redacted visit-history and progress-note snapshots, automatic matching-turn final-answer capture with thread/turn attribution, run-bound results, revision-bound proposals, append-only decisions, patient-rooted atomic multi-record saves, supported-path synthetic enforcement, and the responsive three-zone UI. The explicit `:agent result save` path remains a clinician-attributed recovery import for output produced outside the bound turn. Atomic multi-document intake, authenticated identity, abrupt-restart reconciliation, broader app-server mutation gating, and extending the authorized reader to additional clinical categories remain active roadmap work. See [ROADMAP.md](../ROADMAP.md).
