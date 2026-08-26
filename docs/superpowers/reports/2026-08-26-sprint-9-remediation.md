# Terra Sprint 9 remediation report

Branch: `sprint-9-logs-traces`
Date: 2026-08-26

## Remediated findings

- Added Loki and Tempo connector options and tenant-ID metadata fields to the UI; tenant IDs stay in `config_metadata`, separate from credentials, with English and Thai coverage.
- Masked Loki stream-label values with the shared sensitive-key deny list before returning `LogStream`/`IpcResult`; added serialized-result regression coverage.
- Centralized observability authorization checks for command name, capability, descriptor permission, active membership, principal identity, workspace scope, and membership grants; added permission, identity, scope, and regression coverage.
- Correlated traces only from parsed structured fields; `traceparent` now yields a validated lowercase trace ID and malformed/unparsed values are rejected without text scanning.
- Made alert-derived LogQL require namespace plus exactly one pod/service/deployment label, with localized missing and ambiguous states.
- Removed independent metric/Grafana time fallbacks and verified the shared workspace window in the metric-to-log acceptance fixture.
- Added Tempo tenant-header present/absent coverage and removed credential-shaped literals from UI-only fixtures while retaining parsed/unparsed masking assertions.

## Verification

All commands ran from the repository root; no npm dependencies were installed.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo test -p thalassaops observability::loki::` | PASS — 4 tests |
| `cargo test -p thalassaops observability_authorization_` | PASS — 2 tests |
| `cargo test -p thalassaops observability::client::` | PASS — 8 tests |
| `cargo test --workspace` | PASS — 57 package tests; all doctests pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `git diff --check` | PASS |
| `npm test -- shell.test.tsx` | BLOCKED — exit 127, `vitest: command not found` |
| `npm run typecheck` | BLOCKED — exit 127, `tsc: command not found` |
| `npm run lint` | BLOCKED — exit 127, `eslint: command not found` |
| `npm run build` | BLOCKED — exit 127, `tsc: command not found` |
| `npm run format:check` | BLOCKED — exit 127, `prettier: command not found` |

The frontend gates remain blocked because the repository has no installed npm binaries; they were not installed per task constraints. This worker did not self-approve; independent review is still required.
