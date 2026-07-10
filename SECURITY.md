# Security policy

## Project maturity

Flekks Medical Workspace is alpha research software for synthetic data only. It does not currently provide the encryption, authenticated clinical identity, access control, retention, telemetry isolation, or compliance program required for real patient data.

Do not enter PHI, credentials, production payer data, or other regulated information.

## Reporting a vulnerability

Use GitHub's private security-advisory flow for this repository. Do not open a public issue containing exploit details, secrets, patient information, local database contents, or screenshots with sensitive data.

Include:

- affected revision;
- reproduction using synthetic fixtures;
- expected and observed behavior;
- likely confidentiality, integrity, or availability impact;
- a suggested mitigation when available.

This project is independent from OpenAI. Do not send Flekks-specific reports to OpenAI's Bugcrowd program unless the issue is independently reproducible in unmodified upstream Codex.

## Supported versions

Only the latest `main` revision is actively reviewed. No production or long-term-support release exists yet.
