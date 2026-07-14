# Upstream provenance

Flekks Medical Workspace is an independent derivative of [OpenAI Codex](https://github.com/openai/codex).

- Original private-fork main checkpoint: `f2b9ba9fcac097dcc6f7aaa658de2377e6ab40bc`
- OpenAI Codex ancestry base: `bf72be59278e23002a352a53207182985cabb9d0`
- Latest synchronized OpenAI Codex revision: `9e552e9d15ba52bed7077d5357f3e18e330f8f38` (2026-07-11)
- Flekks private checkpoint used for the public source snapshot: `98a831040`
- Public project: `Dylan-Flekks/flekks-medical-workspace`

OpenAI Codex remains copyright OpenAI and is licensed under Apache-2.0. This project preserves the upstream LICENSE and NOTICE. OpenAI does not maintain, endorse, or provide support for this derivative.

The public repository connects directly to OpenAI's public commit ancestry at
the recorded base and merges subsequent upstream revisions. Private fork commit
history is not imported into the public repository.

## Updating the harness

The local `upstream` remote should point to `https://github.com/openai/codex.git`.
Create a review branch, fetch `upstream main`, and merge `upstream/main`; do not
rewrite the public project history. Preserve this project's focused medical CI,
synthetic-data boundary, and human-review invariants while resolving conflicts,
then update the synchronized revision above.

## Major modifications

The derivative adds or substantially changes:

- regulated workflow and approval/audit primitives;
- a local SQLite workspace domain for patients, encounters, notes, revisions, files, tasks, safety records, context packets, agent results, proposals, and audits;
- the `/workspace-medical` full-screen TUI;
- local patient search and patient/chart navigation;
- explicit multimodal metadata selection and reviewed text/clip handling;
- packet-scoped agent handoff and returned-work review;
- macOS file-drop and local preview support;
- medical-workspace tests, snapshots, and deterministic render harnesses.

Upstream package names and SDK metadata may remain in the source tree for build compatibility. This repository does not publish packages under OpenAI names.
