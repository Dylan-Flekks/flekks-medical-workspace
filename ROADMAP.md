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
- Bind completed agent turns automatically to their prepared runs. Implemented for matching packet id/hash turns; external output uses the explicit recovery import.
- Persist recommendations and append-only clinician decisions. Implemented for whole-proposal review.

## Three-zone workspace

- Patient/Chart Explorer on the left ([#7](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/7)).
- Canonical human chart in the center.
- Pending, History, and Audit Agent Work on the right ([#8](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/8)).
- IDE-style current-versus-proposed comparison with stale-revision handling ([#9](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/9)).

## Synthetic-data and security program

- Enforce synthetic workspace mode ([#10](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/10)).
- Remove absolute paths from agent-visible packets.
- Add purge, retention, permissions, and bounded file processing.
- Design authenticated identity, encryption, access control, and telemetry isolation before any real-data discussion ([#12](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/12)).
- Reconcile runs abandoned by abrupt process termination without mislabeling concurrent work.

## Help wanted

High-value contribution areas include Ratatui accessibility, state-machine testing, SQLite migrations, privacy threat modeling, human-factors review, diff UX, Windows/Linux validation, and [contributor onboarding](https://github.com/Dylan-Flekks/flekks-medical-workspace/issues/13).
