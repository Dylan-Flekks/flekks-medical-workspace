# Upstream provenance

Flekks Medical Workspace is an independent derivative of [OpenAI Codex](https://github.com/openai/codex).

- Upstream source revision: `f2b9ba9fcac097dcc6f7aaa658de2377e6ab40bc`
- Flekks private checkpoint used for the public source snapshot: `98a831040`
- Public project: `Dylan-Flekks/flekks-medical-workspace`

OpenAI Codex remains copyright OpenAI and is licensed under Apache-2.0. This project preserves the upstream LICENSE and NOTICE. OpenAI does not maintain, endorse, or provide support for this derivative.

## Major modifications

The derivative adds or substantially changes:

- regulated workflow and approval/audit primitives;
- a local SQLite workspace domain for patients, encounters, notes, revisions, files, tasks, safety records, context packets, agent results, proposals, and audits;
- the `/workspacemedical` full-screen TUI;
- local patient search and patient/chart navigation;
- explicit multimodal metadata selection and reviewed text/clip handling;
- packet-scoped agent handoff and returned-work review;
- macOS file-drop and local preview support;
- medical-workspace tests, snapshots, and deterministic render harnesses.

Upstream package names and SDK metadata may remain in the source tree for build compatibility. This repository does not publish packages under OpenAI names.
