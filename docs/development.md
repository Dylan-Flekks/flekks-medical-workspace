# Development

## Setup

Install Git, the Rust toolchain from `codex-rs/rust-toolchain.toml`, and `just`.

```bash
just medical-workspace
```

Run `/workspacemedical` when the TUI opens. Use only synthetic records and files.

The supported launcher sets both `CODEX_SQLITE_HOME` and the higher-precedence
`sqlite_home` CLI override, then lets the TUI provision an empty database
through the app-server policy API. It defaults to
`$HOME/.codex/flekks-medical-synthetic`. An alternate location must be an
absolute directory reserved only for synthetic medical workspace SQLite state;
do not use another Codex SQLite home:

```bash
FLEKKS_MEDICAL_WORKSPACE_SQLITE_HOME=/absolute/private/path just medical-workspace
```

For a manual debug build, run `cargo build -p codex-cli` from `codex-rs`, then
launch through `../scripts/run_medical_workspace.sh`. A plain `cargo run` may
inspect an already-classified synthetic store, but it cannot classify a new
store and the TUI will refuse its first workspace mutation.

## Focused tests

Follow `AGENTS.md`; the repository's canonical runner is `just test`.

```bash
just test -p codex-tui workspace_dashboard
just test -p codex-tui medical_workspace
just test -p codex-state workspace
```

For visible TUI changes, review pending Insta snapshots and validate 80×20, 100×28, 120×32, and 160×40 layouts. Keep screenshots synthetic and outside the repository unless they are intentionally curated documentation assets.

## Secret and artifact checks

```bash
gitleaks dir . --config .gitleaks.toml --redact
find . -type f \( -name '*.sqlite*' -o -name '*.db' -o -name '*-wal' -o -name '*-shm' \)
```

Never commit local workspace databases, vault files, thumbnails, Codex sessions, logs, credentials, or real patient data.

## Pull-request expectations

- Keep changes reviewable and scoped.
- Add state/API tests for migrations or authority changes.
- Add snapshot coverage for UI changes.
- Document compatibility and backfill behavior.
- Call out unresolved privacy or clinical-workflow assumptions.
