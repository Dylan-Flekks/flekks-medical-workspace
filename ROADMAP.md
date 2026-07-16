# Roadmap

The project is intentionally public before it is finished so contributors can help shape the safety and architecture.

## P0: correctness

- [Make saves change-scoped, prevalidated, and atomic](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/4). Implemented for one patient-rooted chart changeset; atomic multi-document batch intake remains.
- Add optimistic note revision checks and immutable patient ownership ([#5](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/5)). Implemented for atomic chart changesets and proposal acceptance.
- Bind every request, run, result, and proposal to patient, note, and base revision ([#5](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/5)).
- Fix responsive render/cursor/mouse geometry and unsafe implicit actions ([#11](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/11)).

## Agent provenance

- Separate prepared packets from submitted runs. Implemented in the first vertical slice.
- Persist agent runs, the immutable packet envelope, and a separately hashed authorization/output contract; extend the manifest to every authorized database read ([#6](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/6)).
- Enforce packet-scoped database retrieval and record each bounded, path-redacted visit/note snapshot. Implemented through the model-visible `workspace_context_read` tool for visit history and progress notes; expand categories through explicit clinician scope.
- Bind completed agent turns automatically to their submitted Plan revision and prepared run.
  Implemented with an atomic result/run/completion receipt covering the exact assistant message,
  body hash, thread, turn, provider, model, packet, and Plan-submission binding; external output is
  always a clinician-attributed manual import.
- Persist recommendations and append-only clinician decisions. Implemented for whole-proposal review.

## Living context workspace

- Patient/Chart Explorer on the left ([#7](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/7)): implemented for the synthetic vertical slice; accessibility and large-chart navigation remain.
- Canonical human chart in the center: implemented with explicit-save revisions and recoverable
  note/context working checkpoints; extend recovery to demographic and coverage drafts.
- Persistent `Plan with Codex` Chat, Context, and Audit rail on the right
  ([#8](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/8)): implemented with a
  restricted patient-scoped thread; add full-history pagination and richer evidence inspection.
- Immutable checkpoint- and evidence-bound Plan revisions plus first-class packet/master-run
  bindings are implemented. Unclaimed planning runs and orphaned claimed master turns now recover
  deterministically; extend recovery across the remaining delivery and accepted-turn boundaries.
- IDE-style current-versus-proposed comparison with stale-revision handling ([#9](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/9)).

## Synthetic-data and security program

- Enforce synthetic workspace mode ([#10](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/10)).
- Remove absolute paths from agent-visible packets.
- Dedicated synthetic workspace-database health reporting and dry-run-first whole-store purge are
  implemented; extend health checks to every allowed Codex SQLite database, then extend lifecycle
  coverage to Codex rollouts/logs, managed files, derivatives, backups, retention schedules, and a
  complete erasure receipt.
- Design authenticated identity, encryption, access control, and telemetry isolation before any real-data discussion ([#12](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/12)).
- Extend the implemented run reconciliation to every remaining chart, proposal, and response
  delivery boundary without mislabeling concurrent work.

## Help wanted

High-value contribution areas include Ratatui accessibility, state-machine testing, SQLite migrations, privacy threat modeling, human-factors review, diff UX, Windows/Linux validation, and [contributor onboarding](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/13).
