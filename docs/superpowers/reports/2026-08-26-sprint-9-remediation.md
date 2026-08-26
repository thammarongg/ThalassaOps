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

## Remediation round 2: frontend gates

- Updated the two acceptance journeys to use `/Query type/`, preserving the pre-existing `Query type:` UI markup without changing production text.
- Widened `GrafanaPanel`'s `loadingKey` state to `number | undefined`, preserving the reset effect and ensuring `undefined` does not equal a numeric `resetKey`.
- Changed `npm run typecheck` to `tsc -b --force`, so CI typechecks the referenced app project instead of an empty root project.
- Ran the repository formatter over the seven files reported by `format:check`.

### Required F4-before-F2 proof

Applied F4 alone, before changing `GrafanaPanel.tsx`, then ran `npm run typecheck`:

```text
ui/src/observability/GrafanaPanel.tsx(59,19): error TS2345: Argument of type 'undefined' is not assignable to parameter of type 'SetStateAction<number>'.
```

After applying F2, `npm run typecheck` passed with the fixed `tsc -b --force` script.

### Verification

All commands ran from the repository root after `npm ci` installed the locked dependencies. The table records the actual command output summaries from this remediation round.

| Command | Result |
| --- | --- |
| `npm ci` | PASS — added 298 packages and audited 299; npm reported 5 vulnerabilities (3 moderate, 1 high, 1 critical) plus deprecation/install-script warnings |
| `npm run format:check` | PASS — `All matched files use Prettier code style!` |
| `npm run lint` | PASS — ESLint exited 0 with no diagnostics |
| `npm run typecheck` | PASS — `tsc -b --force` exited 0 |
| `npm test` | PASS — 4 test files, 19 tests passed, 0 failed, 0 skipped |
| `npm run build` | PASS — TypeScript and Vite build completed; 76 modules transformed |
| `cargo fmt --all -- --check` | PASS — exited 0 with no output |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS — finished dev profile with no warnings |
| `cargo test --workspace` | PASS — 79 package tests passed, 0 failed; all doctests passed |
| `git diff --check` | PASS — exited 0 with no output |

This round's frontend gates are executable and green; independent review is still required.
