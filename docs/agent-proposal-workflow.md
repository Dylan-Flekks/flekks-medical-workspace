# Agent proposal workflow

The design follows an IDE agent-edit model rather than direct database mutation.

```text
Clinician draft
  -> Agent Request / context packet
  -> packet-scoped agent run
  -> recorded source reads
  -> recommendation
  -> note proposal
  -> clinician compare/edit/accept/reject
  -> canonical note revision
```

## Audit objects

1. **Packet:** who requested what, when, against which patient/note/revision, and which categories were authorized.
2. **Run:** provider/model, thread/turn, lifecycle, and errors.
3. **Sources:** the records and exact revisions/snapshots actually returned to the run.
4. **Result:** generated recommendation and concise review rationale.
5. **Proposal:** a change against the packet's base note revision.
6. **Decision:** clinician acceptance, edit, rejection, or selected-copy action.
7. **Revision:** canonical human-controlled chart state after an accepted decision.

## Concurrency rule

If the canonical note has changed since the packet's base revision, the proposal is stale. It may be compared or regenerated, but it must not be applied automatically.

## Non-goals

- Persisting hidden model chain-of-thought.
- Allowing an agent to sign notes, submit claims, contact payers, or write directly to canonical chart tables.
- Treating a BAA or model-provider setting as a complete privacy/compliance program.
