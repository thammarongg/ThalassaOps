# Sprint 11 operations console verification

Date: 2026-08-28
Branch: `thammarongg/sprint-11-operations-console`
Comparison base: `main` (`e0e785750fe8f0c988f0b24b8c52000d537c1524`)

## Executive result

The complete Sprint 11 diff was reviewed as an independent defect pass. The branch now has a fixture-backed acceptance journey for the operations command center, and the final verification rerun covers formatting, linting, type checking, build output, frontend tests, Rust formatting, Clippy, and the complete Rust workspace test suite.

The exit criterion is credibly met for the shipped read model:

> A user can open the application and understand what needs attention within 30 seconds.

The first viewport places the business-impact health summary and active incident queue before secondary widgets. The acceptance fixture makes an S1 alert, S2 anomaly, AWS critical environment, GCP healthy environment, recent change, and alert/anomaly/health-check counts visible in one journey. Every critical number is an actionable control that requests its corresponding evidence, and the evidence panel displays source, connector, query/endpoint, observed time, masking, and unparsed-data state.

## Deliverable traceability

| Sprint 11 deliverable | Implementation | Verification |
| --- | --- | --- |
| Business-impact-first health summary | `src-tauri/src/operations/aggregate.rs`, `ui/src/OperationsConsole.tsx` | `src-tauri/tests/operations_aggregation.rs`, `ui/src/operations/OperationsConsole.test.tsx`, `ui/src/operations/operations-console.acceptance.test.tsx` |
| Active incident queue | `src-tauri/src/operations/aggregate.rs`, `ui/src/OperationsConsole.tsx` | aggregation tests, focused console tests, acceptance journey |
| Alert and anomaly summary | `src-tauri/src/operations/aggregate.rs`, `src-tauri/src/operations/anomaly.rs`, `ui/src/OperationsConsole.tsx` | `operations_aggregation.rs`, `operations_anomaly.rs`, focused console tests, acceptance journey |
| Rule-based anomaly producer (threshold and rate-of-change) | `src-tauri/src/operations/anomaly.rs` | `src-tauri/tests/operations_anomaly.rs` |
| Scheduled health-check producer (interval, scope, timeout, cooldown, audit metadata) | `src-tauri/src/operations/health_check.rs` | `src-tauri/tests/operations_health_check.rs` |
| Recent change stream | `src-tauri/src/operations/aggregate.rs`, `ui/src/OperationsConsole.tsx` | aggregation tests, focused console tests, acceptance journey |
| Environment status overview | `src-tauri/src/operations/aggregate.rs`, `ui/src/OperationsConsole.tsx` | aggregation tests, malformed/duplicate projection tests, acceptance journey |
| Curated configurable widgets | `src-tauri/src/operations/aggregate.rs`, `ui/src/operations/widgetConfig.ts`, `ui/src/OperationsConsole.tsx` | console tests, widget persistence/required-widget tests, acceptance journey |
| Drill-down from every critical number | `src-tauri/src/operations/aggregate.rs`, `src-tauri/src/app/operations.rs`, `ui/src/OperationsConsole.tsx`, `ui/src/operations/contractValidation.ts` | IPC tests, contract fixtures, focused console tests, acceptance journey |

The route is wired through `ui/src/shell.tsx`. Rust domain contracts and TypeScript runtime validation cover the same operation snapshot, evidence, widget, status, identity, numeric, and drill-down invariants.

## Defects found and fixed

Each item below was fixed in a focused conventional commit on this branch.

1. Eligible anomaly samples with duplicate timestamps were ordered by input, making the latest value and rate-of-change result nondeterministic. Duplicate timestamps are now rejected and covered by an aggregation test.

2. Out-of-scope changes were admitting evidence before the scope filter ran. Scope admission now precedes evidence lookup, with a regression test proving foreign changes cannot add evidence.

3. Cloud identifiers, subscription/account markers, ARNs, pagination cursors, and similar provider values could survive into evidence-derived text. Redaction covers these marker families, native links require a trusted HTTPS URL without credentials, and a synthetic marker scan protects serialized evidence and logs.

4. Snapshot validation did not fully enforce evidence-reference integrity. Duplicate evidence content, missing or mismatched drill-down references, and unparsed evidence marked as masked are now rejected in the Rust store/domain validator and TypeScript runtime validator.

5. A fresh source with no usable evidence could be presented as healthy, while an unavailable source without evidence could be presented as a verified outage. Source posture now distinguishes verified unavailable from unverified/no-data, with tests for both cases.

6. Malformed environment resource counts or timestamps were converted to fabricated `0`/`unknown` values and could enter generated evidence. Invalid projections are now rejected and validation occurs before fallback evidence admission.

7. Duplicate source records were effectively first-wins, allowing conflicting alert, anomaly, metric, health-check, change, environment, or widget records to depend on input order. Ambiguous duplicates are now skipped and the source marked unverified; evidence IDs are also unique. Source-status ties use a canonical secondary sort key.

8. Unknown business impact could lose to a no-impact record during headline selection. An explicit impact rank now orders `Critical`, `High`, `Medium`, `Low`, `Unknown`, and `None` deterministically, with scope as a tie-breaker.

9. Operations authorization failures reused an error containing caller scope details, leaking workspace/account identifiers. The operations IPC path now emits only the command required for authorization failure, and the IPC test asserts caller scope identifiers are absent.

10. The UI accepted shallow or malformed backend snapshots, including duplicate identities, invalid numeric forms, inconsistent source-status/reason pairs, unsafe widget defaults, and incomplete evidence responses. Runtime validation now mirrors Rust parsing and invariants, including exact evidence-ID set validation.

11. A slow drill-down response could overwrite a newer selection. Request identity guards now discard stale success and error responses; the regression test uses `waitFor` rather than a timing-dependent zero-delay timer.

12. Backend severity keys such as `active_by_severity.S1` did not map to the UI's localized severity labels. The label resolver now handles the actual key shape, and the acceptance fixture verifies the visible S1 queue item.

13. Evidence responses could be treated as trusted when returned as an unvalidated raw array, and native source opening could be invoked with an unsafe URL. Responses are accepted only as a validated exact-ID set; the UI revalidates trusted HTTPS links before invoking the shell.

14. Required health-summary and incident-queue widgets could be collapsed, defeating the business-impact-first attention path. Required widgets remain expanded and their settings controls are disabled; persistence tests cover saved collapsed preferences.

15. The persisted widget preference key differed from the documented contract. Storage now uses `thalassaops.operations.widgets.v1`, with tests exercising persistence under that key.

16. The frontend numeric parser accepted JavaScript-only hexadecimal, binary, and octal forms that Rust rejected. TypeScript validation now uses the shared decimal grammar and rejects those forms, with contract fixture coverage.

## Security, privacy, and capability review

The new operations surface is read-only. The only new IPC commands are `operations.snapshot` (`WorkspaceRead`) and `operations.evidence` (`ResourceRead`); they require the exact command/capability pair and validate principal, active membership, workspace grant, role, UI policy, and audit context. No `IncidentWrite` or other mutation capability is reachable through the operations path, and the operations producers do not make live network calls.

Evidence references are server-issued and bounded to the snapshot. Provider identifiers, credentials, tokens, account/subscription identifiers, pagination cursors, raw request bodies, and authorization scope details are excluded from serialized evidence and error details. The redaction tests use synthetic markers only and assert that they cannot appear in the serialized output. The evidence panel exposes masking and unparsed status explicitly so a user cannot mistake incomplete evidence for a complete read.

## Accessibility and localization review

All new operations labels and status text have English and Thai entries. The UI uses labelled sections, semantic status roles, keyboard-reachable buttons for critical numbers, and the existing drawer interaction for drill-down. Focused UI tests cover localized labels, keyboard/action affordances, evidence states, widget settings, and drill-down behavior; the acceptance journey covers the visible attention path in the English locale and runs against the same runtime contract used by both locales.

## Acceptance evidence

The acceptance fixture uses a fixed timestamp (`2026-08-28T09:00:00Z`), a bounded workspace scope, deterministic evidence IDs, and only the two operations IPC commands. It verifies:

- the health summary and first active incident are visible at the top of the screen;
- S1 alert, S2 anomaly, alert/anomaly/health-check counts, AWS critical status, GCP healthy status, and recent change data are visible;
- the first two widgets are `health_summary` and `incident_queue`, while all configured widgets are present;
- every critical number issues a `ResourceRead` evidence request with the exact expected IDs;
- no provider, write, or unrelated IPC command is invoked;
- masked and unparsed evidence states, connector identity, endpoint, and query are visible.

Test file: `ui/src/operations/operations-console.acceptance.test.tsx`.

## Final gates

The following commands were run on the final committed tree; each exited 0:

| Gate | Result |
| --- | --- |
| `npm ci` | passed; npm reported the existing audit inventory below |
| `npm run format:check` | passed |
| `npm run lint` | passed |
| `npm run typecheck` | passed |
| `npm run build` | passed |
| `npm test` | passed, 7 files / 40 tests |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | passed |
| `cargo test --workspace` | passed, including the operations aggregation, anomaly, health-check, IPC, redaction, and domain contract suites |
| `npm test -- --run ui/src/operations/operations-console.acceptance.test.tsx` | passed, 1 test |

`git diff --check main...HEAD` also passed, and the final worktree is clean. No push or merge was performed.

## Deliberately open

- `npm ci` reports five dependency audit advisories (three moderate, one high, one critical). They predate this verification pass, no dependency was introduced or upgraded by Sprint 11, and remediation requires a separate dependency-risk decision; this is not silently treated as resolved.
- Live provider integrations and a manual health-check trigger are intentionally outside the Sprint 11 fixture-backed read-model scope. The shipped scheduler/producers are deterministic and testable without network access; live integration acceptance remains a follow-up for the owning integration work.

No other Sprint 11 defect found during this pass was deliberately left open.
