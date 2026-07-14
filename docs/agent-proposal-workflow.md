# Agent proposal workflow

The design follows an IDE agent-edit model rather than direct database mutation.

```text
Clinician draft
  -> living Context Plan
  -> reviewed, frozen context packet
  -> packet-scoped agent run
  -> recorded source reads
  -> recommendation
  -> note proposal
  -> clinician compare/edit/accept/reject
  -> canonical note revision
```

## Audit objects

1. **Plan checkpoint:** the living objective, expected output, selected evidence, readiness, and
   acknowledged context gaps at review time.
2. **Packet:** who requested what, when, against which patient/note/revision, and which categories were authorized.
3. **Run:** provider/model, thread/turn, lifecycle, and errors.
4. **Sources:** the records and exact revisions/snapshots actually returned to the run.
5. **Result:** generated recommendation and concise review rationale.
6. **Proposal:** a change against the packet's base note revision.
7. **Decision:** clinician acceptance, edit, rejection, or selected-copy action.
8. **Revision:** canonical human-controlled chart state after an accepted decision.

Each submitted packet also records the workspace profile, plan-schema version, source-checkpoint
identifier and hash, and structured readiness acknowledgements. That metadata binds the reviewed
plan to the exact living-workspace checkpoint used for the run.

## Current vertical-slice boundary

The TUI maintains a living context plan, freezes it for review, submits the immutable packet, and
starts the durable master-agent run. The model-visible `workspace_context_read` tool accepts only
that running ID plus an authorized visit-history or progress-note category; patient ownership comes
from the run, not model input. Each bounded, path-redacted returned row is frozen and hashed in the
source manifest, alongside a separately hashed packet authorization/output contract. A matching
user turn is bound by packet id/hash, and its final agent answer is saved automatically as
review-pending Agent Review with thread/turn attribution. `:agent result save` remains a
clinician-attributed recovery import for externally produced output. Additional clinical source
categories and abrupt-restart reconciliation remain roadmap work.

## Concurrency rule

If the canonical note has changed since the packet's base revision, the proposal is stale. It may be compared or regenerated, but it must not be applied automatically.

Proposal decisions update the note, proposal status, append-only decision, and audit event in one serialized transaction. Exact same-decision retries converge on the stored outcome; opposite or altered retries fail closed. General human chart edits use a separate patient-rooted atomic changeset with durable idempotency receipts and optimistic version guards. After a stale note-only conflict, refreshed canonical data and the preserved human draft remain separate until the clinician makes an explicit merge edit. Stale non-note or multi-entity drafts cannot be rebased blindly; they remain frozen until the clinician explicitly discards them and reloads canonical data.

## Non-goals

- Persisting hidden model chain-of-thought.
- Allowing an agent to sign notes, submit claims, contact payers, or write directly to canonical chart tables.
- Treating a BAA or model-provider setting as a complete privacy/compliance program.
