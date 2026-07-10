# Roadmap

The project is intentionally public before it is finished so contributors can help shape the safety and architecture.

## P0: correctness

- Make saves change-scoped, prevalidated, and atomic.
- Add optimistic note revision checks and immutable patient ownership.
- Bind every request, run, result, and proposal to patient, note, and base revision.
- Fix responsive render/cursor/mouse geometry and unsafe implicit actions.

## Agent provenance

- Separate prepared packets from submitted runs.
- Persist agent runs and exact source-access manifests.
- Enforce packet-scoped database retrieval.
- Persist recommendations and append-only clinician decisions.

## Three-zone workspace

- Patient/Chart Explorer on the left.
- Canonical human chart in the center.
- Pending, History, and Audit Agent Work on the right.
- IDE-style current-versus-proposed comparison with stale-revision handling.

## Synthetic-data and security program

- Enforce synthetic workspace mode.
- Remove absolute paths from agent-visible packets.
- Add purge, retention, permissions, and bounded file processing.
- Design authenticated identity, encryption, access control, and telemetry isolation before any real-data discussion.

## Help wanted

High-value contribution areas include Ratatui accessibility, state-machine testing, SQLite migrations, privacy threat modeling, human-factors review, diff UX, Windows/Linux validation, and documentation.
