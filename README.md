# Flekks Medical Workspace

Flekks Medical Workspace is an open-source Rust terminal UI for patient-centered chart workflows and human-reviewed AI agent recommendations. It is derived from [OpenAI Codex](https://github.com/openai/codex) and explores an IDE-style model for clinical documentation: clinicians own the canonical chart, agents propose reviewable changes, and every request, source access, recommendation, decision, and accepted revision is auditable.

> [!WARNING]
> This project is alpha research software for synthetic data only. It is not an electronic health record, medical device, clinical decision-support system, or HIPAA-ready product. Do not enter protected health information or use it for patient care.

This is an independent community project. It is not an official OpenAI product and is not endorsed or supported by OpenAI.

## Product model

The medical workspace is organized like an IDE:

```text
Patient / Chart Explorer | Canonical Human Chart | Agent Work
```

- **Explorer:** active and archived synthetic patients, demographics, safety, contacts, coverage, encounters, notes, patient files, tasks, and audit history.
- **Canonical Chart:** human-controlled records, saved revisions, and clinician working drafts.
- **Agent Work:** immutable requests, run status, sources accessed, recommendations, provenance, and accept/reject history.

The core safety invariant is:

> An agent may inspect explicitly authorized synthetic context and propose changes, but only an explicit clinician review action may create a new canonical chart revision.

See [Agent proposal workflow](docs/agent-proposal-workflow.md) and [Architecture](docs/architecture.md).

## Current capabilities

- Local SQLite patient, note, encounter, task, file-metadata, safety, packet, result, proposal, and audit persistence.
- Local-only patient search across identifiers, contact, emergency-contact, and coverage fields.
- Note revision history, local note locking, addenda, and stale-proposal checks.
- Explicit packet selection for multimodal file metadata and human-reviewed excerpts.
- Review-pending agent results with patient, note, and packet-hash provenance.
- Responsive Ratatui layouts and deterministic snapshot/readability harnesses.

## Build and run

Requirements:

- Rust toolchain declared in `codex-rs/rust-toolchain.toml`
- Git
- `just` for the repository's canonical checks

```bash
git clone https://github.com/Dylan-Flekks/flekks-medical-workspace.git
cd flekks-medical-workspace/codex-rs
cargo build -p codex-cli
./target/debug/codex
```

Inside the TUI, run:

```text
/workspacemedical
```

Use synthetic fixtures only. See [Development](docs/development.md) for focused tests and snapshot review.

## Project status and limitations

The repository is being opened early because the system needs help from Rust/TUI developers, clinical-workflow designers, privacy and security engineers, accessibility reviewers, and agent-system builders.

Known blockers include:

- change-scoped and atomic chart saves;
- truthful `prepared -> submitted -> running -> reviewed` agent lifecycle;
- packet-scoped database access and durable source manifests;
- IDE-style proposal comparison and partial review;
- synthetic-workspace enforcement and secure storage design;
- authenticated clinician identity and production privacy controls.

See the [Roadmap](ROADMAP.md) and issues labeled `help wanted`.

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md), open an issue for design-sensitive changes, and never include real patient data, credentials, or personal database files.

## Upstream and license

This repository contains a modified source snapshot of OpenAI Codex. See [UPSTREAM.md](UPSTREAM.md) for provenance and the major modification areas.

Licensed under the [Apache License 2.0](LICENSE). The original [NOTICE](NOTICE) is retained.
