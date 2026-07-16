# Architecture

Flekks Medical Workspace extends the Codex TUI and app-server with a persistent, auditable domain
workspace above ordinary single-task Plan Mode. The living patient workspace changes as records
are saved. Every planning turn persists ordinary guidance; only an explicit valid publish/update
response creates an immutable Plan revision with its source revisions and authorization boundary.

```text
/workspace-medical
  -> WorkspaceDashboard state/reducer/rendering
  -> living chart + immutable local checkpoint
  -> persistent patient Plan session and dedicated Codex thread
  -> workspacePlanningOnly turn (audited read + ordinary saved response)
  -> ordered messages, source reads, and evidence-linked plan revision
  -> explicit reviewed packet + bounded master Codex run
  -> review-pending recommendation/proposal
  -> WorkspaceStore transactions and local synthetic SQLite
```

## Intended UI boundaries

- **Explorer:** local patient search, active/archived patients, and selected-patient chart tree.
- **Canonical Chart:** human-owned records and working drafts.
- **Plan with Codex:** patient-scoped Chat, Context, and Audit views for guided questions,
  checkpointed evidence, plan revisions, master-handoff readiness, and returned recommendations.

The dashboard must not embed ChatWidget or render parent-harness usage, MCP, token, or notification details. Transitioning to the full agent harness is explicit, and returning work remains review-pending. Workspace chrome may expose only the bridge lifecycle: idle, working, returned, or attention.

The right rail is a persistent, restricted Plan harness rather than a deterministic hint engine. It
uses the parent harness's resolved Plan model/effort but exposes only an audited workspace reader.
If Codex asks a focused question, it is stored as an ordinary Assistant message and the clinician
answers in a fresh Human turn. The dedicated thread is patient-bound, feature-tagged, generically
named, omitted from the normal resume picker, excluded from memory generation, and must not accept
ordinary parent-harness turns or generic tools. The full parent
harness remains an explicit second stage for a reviewed, frozen handoff; returned work cannot
change canonical chart data without a human review action.

## Data boundaries

- Canonical notes are revisioned.
- Human chart/context instructions remain mutable in checkpointed working drafts.
- Patient Plan conversations are ordered, durable messages linked to one patient session.
- Published Plan revisions are immutable, checkpoint-, thread-, turn-, and evidence-manifest-bound.
- Submitted context packets are immutable clinician work orders bound to a Plan revision ID,
  content hash, evidence-manifest hash, and source checkpoint.
- Agent runs inherit that exact Plan binding and are attempts against one packet and base note
  revision; a revision cannot become submitted without its matching packet/run pair.
- Source-access rows record the exact version/snapshot returned to an agent.
- Agent results are immutable recommendations.
- Note proposals are reviewable diffs.
- Human decisions are append-only audit records.
- Patient-rooted chart changesets commit all included human edits in one SQLite transaction with note-revision and entity-version guards.
- Every chart changeset has a durable idempotency receipt; uncertain delivery retries the exact request. A stale note-only edit requires an explicit visible reconciliation, while stale non-note or multi-entity drafts stay frozen until the clinician discards and reloads canonical data.

Only an explicit human decision may create a new canonical chart revision. If relevant patient, note, or source revisions change after submission, the original packet and result remain auditable but must not be treated as current without a refreshed plan.

## Completion boundaries

The core buffers the final planning Assistant message until the state transaction succeeds. That
single transaction terminalizes the guide run, appends the Assistant message, records its ordered
evidence manifest, and, when explicitly published, creates the immutable Plan revision. Restricted
prompts, reasoning, and tool lifecycle events may exist in the dedicated private rollout, but failed
turns are excluded from later model context and neither uncommitted final output nor hidden artifact
markers reach the workspace UI as authoritative guidance. A commit failure fails the turn without
presenting a partially persisted completion as authoritative.

The master-agent boundary follows the same rule. Its final Assistant message is withheld from
streaming, rollout history, and Agent Review until one transaction verifies the exact submitted
Plan receipt and packet/run binding, writes the immutable result and body hash, records the model
message/thread/turn/provider receipt, and completes the run. Public result creation is manual-import
only and cannot attach caller-supplied text to a bound model run.

## Current status

The current vertical slice implements persistent patient Plan sessions and messages, dedicated
planning-only threads, immutable checkpoint/token-bound reads for patient chart, visit history,
progress notes, and selected context, evidence-linked Plan revisions, prepared/submitted master
packets, atomic matching-turn result persistence, locked proposals, append-only decisions, patient-rooted
atomic chart saves, and the responsive three-zone UI. The explicit `:agent result save` path
remains a clinician-attributed import for externally produced work and never impersonates a bound
agent turn. Atomic multi-document intake, authenticated identity, production data controls, and
broader lifecycle recovery remain roadmap work.
See [ROADMAP.md](../ROADMAP.md).
