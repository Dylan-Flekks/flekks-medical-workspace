# Contributing

Thank you for helping build Flekks Medical Workspace. Contributions from clinicians, developers, designers, privacy/security reviewers, and accessibility testers are welcome.

## Before opening a pull request

1. Search existing issues and Discussions.
2. Open an issue first for schema, agent-authority, clinical-workflow, or major UI changes.
3. Keep one pull request focused on one reviewable concern.
4. Use synthetic data only. Never include PHI, credentials, local databases, session logs, or personal screenshots.
5. Preserve the invariant that agent output is review-pending until an explicit human decision.

## Development setup

```bash
cd codex-rs
cargo build -p codex-cli
```

Follow `AGENTS.md` and any nested instructions. Rust UI changes require snapshot coverage.

Focused checks:

```bash
just test -p codex-tui workspace_dashboard
just test -p codex-tui medical_workspace
just test -p codex-state workspace
```

Before finalizing Rust changes, run the scoped format/fix commands described in `AGENTS.md`.

## Pull requests

Include:

- the problem and intended user outcome;
- architecture or safety implications;
- schema migration and compatibility behavior;
- tests run and results;
- synthetic screenshots for visible TUI changes;
- remaining risks or follow-up issues.

By submitting a contribution, you agree that it may be distributed under this repository's Apache-2.0 license.

## Conduct and security

Follow [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md); never place sensitive details or patient information in a public issue.
