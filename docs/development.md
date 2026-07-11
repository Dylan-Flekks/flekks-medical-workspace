# Development

## Setup

Install Git, the Rust toolchain from `codex-rs/rust-toolchain.toml`, and `just`.

```bash
just medical-workspace
```

Run `/workspacemedical` when the TUI opens. Use only synthetic records and files.

For a manual debug build, run `cargo build -p codex-cli` from `codex-rs`, set a
readable terminal size with `stty cols 160 rows 45`, then launch
`./target/debug/codex`.

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
