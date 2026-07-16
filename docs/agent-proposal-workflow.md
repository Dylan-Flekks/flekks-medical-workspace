# Agent proposal workflow

The design follows an IDE agent-edit model rather than direct database mutation.

```text
Clinician draft
  -> checkpointed patient Plan conversation
  -> evidence-linked Plan revision
  -> reviewed, frozen master context packet
  -> packet-scoped agent run
  -> recorded source reads
  -> recommendation
  -> note proposal
  -> clinician compare/edit/accept/reject
  -> canonical note revision
```

## Audit objects

1. **Workspace checkpoint:** the exact patient/note working state used by a Plan turn.
2. **Plan session/messages:** ordered Human and Assistant messages in the patient-bound
   conversation. A focused question is an ordinary saved Assistant response; its answer starts a
   fresh persisted Human turn.
3. **Plan run and source reads:** provider/model, thread/turn, checkpoint capability, and exact
   hashed context returned to the model.
4. **Plan revision:** a published, evidence-linked decision-complete plan; ordinary help replies do
   not replace it.
5. **Packet:** who submitted which reviewed plan, against which patient/note/revision, and which
   categories were authorized for the master run. Its first-class binding stores the Plan revision
   ID, Plan-content hash, and evidence-manifest hash.
6. **Master run/result:** a separate full parent-harness run using its currently selected model and
   its immutable recommendation. A run inherits the packet's exact Plan binding, and the Plan can
   be marked submitted only through that bound packet/run pair.
7. **Proposal:** a locked change against the packet's base note revision.
8. **Decision/revision:** append-only clinician review and resulting canonical chart state.

Each submitted packet also records the workspace profile, plan-schema version, source-checkpoint
identifier and hash, and structured readiness acknowledgements. That metadata binds the reviewed
plan to the exact living-workspace checkpoint used for the run.

## Current vertical-slice boundary

The TUI first maintains a patient-scoped Plan conversation. Every Plan turn is bound to a local
checkpoint and can read only `patient_chart`, `visit_history`, `progress_notes`, or
`selected_context`; every returned snapshot is immutable and hashed. A published Plan revision
records its evidence manifest and never changes canonical chart data. The clinician then explicitly
freezes a master packet and starts the durable master-agent run. That separate one-turn boundary
uses only its packet-authorized reader and automatically saves the matching final answer as
review-pending Agent Review with thread/turn attribution. `:agent result save` remains a
clinician-attributed recovery import for externally produced output.

## Concurrency rule

If the canonical note has changed since the packet's base revision, the proposal is stale. It may be compared or regenerated, but it must not be applied automatically.

Proposal decisions update the note, proposal status, append-only decision, and audit event in one serialized transaction. Exact same-decision retries converge on the stored outcome; opposite or altered retries fail closed. General human chart edits use a separate patient-rooted atomic changeset with durable idempotency receipts and optimistic version guards. After a stale note-only conflict, refreshed canonical data and the preserved human draft remain separate until the clinician makes an explicit merge edit. Stale non-note or multi-entity drafts cannot be rebased blindly; they remain frozen until the clinician explicitly discards them and reloads canonical data.

## Non-goals

- Persisting hidden model chain-of-thought.
- Allowing an agent to sign notes, submit claims, contact payers, or write directly to canonical chart tables.
- Treating a BAA or model-provider setting as a complete privacy/compliance program.
