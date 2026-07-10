# Regulated Workflow Harness

This fork explores an upstreamable Codex CLI primitive for supervised regulated workflows. Medical workflows are the motivating use case, but the implementation should stay generic enough for legal, finance, security, and other high-accountability terminal workflows.

The goal is not to turn Codex into a medical application. The goal is to make Codex better at running bounded, inspectable, auditable agent loops where policy gates and human approval matter.

## Design Goals

- Keep execution local-first unless a configured provider boundary explicitly allows outbound work.
- Represent workflow state with typed protocol data, not terminal-only strings.
- Surface policy gates, blocked reasons, approval checkpoints, and audit events in durable session history.
- Let extensions and plugins contribute workflow state without hardcoding domain logic into `codex-core`.
- Keep irreversible actions behind explicit human approval.
- Prefer structured APIs, local files/databases, and accessibility-tree integrations before OCR or coordinate-based automation.
- Avoid storing PHI, secrets, credentials, or raw chart content in workflow audit payloads.

## Non-Goals

- Autonomous diagnosis.
- Autonomous billing submission.
- Production EHR behavior.
- Claim finalization.
- Unsupervised PHI export.
- Payer-policy or coding advice presented as authoritative clinical or billing guidance.

## Upstreamable Primitive

The first reusable primitive should be a regulated workflow snapshot. A snapshot describes the current state of one supervised workflow:

- workflow identity and display title
- state, such as idle, planning, running, waiting for approval, blocked, or completed
- sensitive-data boundary status
- active policy gates
- pending approval checkpoint
- latest redacted audit event

This snapshot should be domain-neutral. Medical extensions can map chart review, note drafting, billing-readiness checks, or PHI boundaries onto the generic model without making the core runtime medical-specific.

## Integration Points

The implementation should use existing Codex boundaries:

- `codex-rs/protocol` owns serializable workflow types.
- `codex-rs/ext/extension-api` lets extensions contribute workflow snapshots.
- `codex-rs/core` should only orchestrate lifecycle calls and emit workflow updates.
- `codex-rs/tui` renders a compact workflow status surface from protocol data.
- Hooks and approval review remain the policy and approval interception points.
- App-server v2 should only gain dedicated workflow methods if external clients need direct workflow APIs beyond turn history.

## Implementation Sequence

1. Add protocol types and extension API contributor surfaces.
2. Wire core lifecycle checkpoints to collect workflow snapshots.
3. Emit workflow status into durable turn history.
4. Render a compact TUI workflow status surface with snapshot coverage.
5. Add a small medical example extension or profile that demonstrates supervised documentation and billing-readiness checks without real PHI or authoritative clinical logic.
6. Add app-server v2 workflow APIs only if a client cannot consume the turn-history representation.

## Safety Rules

- Never commit real PHI, patient examples, credentials, tokens, payer contracts, or screenshots from production clinical systems.
- Audit event metadata should be short, redacted, and string-valued.
- Raw tool arguments and raw document/chart text should not be copied into workflow audit events by default.
- Any action that signs, submits, exports, deletes, finalizes, or changes billing state must require explicit human approval.
- OCR and point-click automation are supervised fallback integrations, not primary integration paths.

## Public Readiness Checklist

Before making this fork public:

- Keep the Apache-2.0 license and upstream attribution intact.
- Confirm no secrets, PHI, or private clinical examples are present in git history.
- Keep README language clear that this is supervised workflow infrastructure, not medical advice or autonomous billing.
- Keep the first public milestone documentation-only or prototype-labeled.
- Run the relevant repo checks for every code-bearing PR.
