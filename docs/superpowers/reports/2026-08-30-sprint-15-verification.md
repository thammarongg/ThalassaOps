# Sprint 15 Incident Domain and Lifecycle Verification

Date: 2026-09-02

Branch: `thammarongg/sprint-15-incident-domain-lifecycle`

Comparison base: `b914607` (Sprint 15 plan commit; implementation reviewed commit by commit through `5dba8d7`)

Reviewer: implementation verification pass (Task 8)

## Executive result

Sprint 15 delivers the canonical, local-first Incident write model. A permitted
responder explicitly creates an Incident from six supported trigger kinds
(`alert`, `anomaly`, `user_report`, `scheduled_health_check`,
`vulnerability_finding`, `manual_report`), then advances it through the
validated status graph `detected → triage → investigating → mitigating →
monitoring → resolved → closed`, with `monitoring|resolved|closed → reopened →
investigating`. Every mutation is actor-attributed with server time, request id
and policy version, and current state plus its ordered timeline events are
committed in one SQLite transaction.

The exit criterion is met and asserted end to end from the committed replay
fixtures: all six source kinds create Incidents, a correlation candidate is
consumed by submitting its selected underlying Signals (never as a trigger
kind), the created Incidents reach `closed`, reopen to `investigating`, and the
timeline stays ordered with non-nil actor attribution throughout.

## Focused Sprint 15 test results (Step 3)

| Command | Result |
| --- | --- |
| `cargo test -p thalassa-domain --test incident_contracts --test incident_lifecycle` | exit=0, 13 + 22 = 35 passed |
| `cargo test -p thalassa-ipc --test contracts` | exit=0, 8 passed |
| `cargo test -p thalassaops --test incident_repository --test incident_creation --test incident_mutations --test incident_ipc --test incident_acceptance` | exit=0, 10 + 8 + 8 + 9 + 5 = 40 passed |
| `npm test -- ui/src/incident/incident-contracts.test.ts` | exit=0, 10 passed |

`node_modules` was present, so `npm ci` was not required.

## Acceptance evidence (`src-tauri/tests/incident_acceptance.rs`, 5 tests)

- `sprint_15_exit_criterion_is_reachable_from_committed_fixtures`: creates one
  Incident from each of the six source kinds (all `detected` at version 1),
  creates an Incident from two selected candidate Signals, walks the full
  lifecycle to `closed`, reopens to `investigating`, and asserts the timeline is
  strictly ordered by sequence with non-nil actor ids, request ids and the
  expected policy version. Before any assertion leans on the replay catalog,
  every source-backed kind is asserted non-empty (empty replay associations
  fail silently). A read-only SQLite query on `incident_trigger` asserts the
  persisted trigger rows for the multi-trigger Incident are exactly `alert` and
  `anomaly` — no `correlation_candidate` row can exist. Seven Incidents total,
  zero written by replay or projection.
- `every_disposition_is_recorded_without_transition_or_merge`: `duplicate`,
  `false_positive`, `suppressed`, `cancelled` and the retained `informational`
  are each recorded without changing `status`; `duplicate` points at another
  same-workspace Incident and the target is proven byte-identical afterwards.
- `every_responder_role_can_be_held_and_one_principal_may_hold_several`: on an
  S2 Incident (asserted `derived_severity`), Owner plus the five exclusive
  responder roles plus two Stakeholders all persist; one Principal holds five
  distinct roles; the stored aggregate round-trips.
- `a_stale_writer_changes_neither_state_nor_timeline`: a stale
  `expected_version` returns the typed `VersionConflict { expected: 1, actual: 2 }`
  and current state and timeline are proven unchanged.
- `a_policy_denied_write_changes_nothing`: with audit-log retention restricted
  to `Public`, `incident.create` through the secured IPC returns
  `PolicyDenied` / "incident audit retention policy denied"; the incident count
  stays at zero; restoring the baseline policy, the same command succeeds and
  the count becomes one.

## Release gate results (Step 4, re-run from clean after repairs)

Every gate was run unpiped with a real exit code.

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | exit=0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit=0 |
| `cargo test --workspace` | exit=0, 529 tests passed across 51 suites, 0 failed |
| `npm run format:check` | exit=0 |
| `npm run lint` | exit=0 |
| `npm run typecheck` | exit=0 |
| `npm test` | exit=0, 132 tests passed across 17 files, 0 failed |

Observed totals: 529 Rust tests, 132 frontend tests.

## Pre-existing gate failures repaired inside Task 8

The first full gate pass failed two frontend gates on committed code that
predates Task 8. The coordinator granted a scope exception for exactly these
two items; both were repaired in this task's commit.

1. `npm run format:check` failed on `ui/src/incident/incident-contracts.test.ts`
   (committed at `24d8393`): the lockfile-resolved Prettier 3.9.6 collapses
   short arrays and single-argument calls that the file had committed in
   expanded form. Repaired by running the repo Prettier in write mode on that
   file only — formatting only, no assertion or test-logic change.
2. `npm run lint` reported three unused symbols. Decided per symbol:
   - `ui/contracts/guards.ts` `incidentSourceKinds`: **wired**, not deleted. The
     six trigger wire values are a frozen contract, so
     `isIncidentTriggerInput` now checks membership in the list before its
     per-kind shape branches, making the list the enforced source of truth for
     the allowed `kind` values.
   - `ui/contracts/guards.ts` `IncidentRoleAssignmentInput` import: **deleted**.
     The only inbound role data this file guards (timeline `created` payloads
     and `Incident.roles`) is the full `IncidentRoleAssignment` shape with
     server-side `assigned_by`/`assigned_at`, which its existing guard already
     validates correctly; role-assignment *inputs* appear only in outgoing
     create requests, which these inbound guards never inspect.
   - `ui/src/operations/contractValidation.ts` `BusinessImpact` import:
     **deleted**. Impact validation flows through the `isIncidentBusinessImpact`
     guard alias (`isBusinessImpact`), so the type import had no annotation to
     attach to.

## Remaining exclusions

- Fixture replay stays local and deterministic: no provider network request,
  credential read, AI call, action execution, remediation, notification,
  external integration write or background scheduler participates in Incident
  creation or mutation.
- Source-backed triggers persist validated provenance digests and evidence ids,
  never full provider payloads; user and manual reports are screened bounded
  text, not masked into the timeline.
- The Sprint 16 Incident Workspace (UI consumption of these contracts) is not
  part of this sprint and is not claimed anywhere in the product status.
