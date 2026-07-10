# Architecture

Flekks Medical Workspace extends the Codex TUI and app-server with a local patient-workspace domain.

```text
/workspacemedical
  -> WorkspaceDashboard state/reducer/rendering
  -> WorkspaceDashboardAction
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

Only an explicit human decision may create a new canonical chart revision.

## Current status

Several of these boundaries exist today, but packet submission lifecycle, run/source persistence, atomic saves, and the three-zone UI remain active roadmap work. See [ROADMAP.md](../ROADMAP.md).
