# Sprint 14 Change Intelligence Verification

Date: 2026-08-30

Branch: `thammarongg/sprint-14-change-intelligence`

Comparison base: `main` (`f285f4c1c4b528c866636f5cd28d4c88375d13fc`)

Reviewer: implementation verification pass

## Executive result

Sprint 14 connects operational problems to recent changes. Committed GitHub,
GitLab and Argo CD payloads are replayed through the existing append-only
source-record ledger, normalized into the canonical `ChangeEvent`, ordered into
a bounded deterministic timeline, and attached to Sprint 13 correlation
candidates as explainable structural context with native source links. Two
capability-scoped read commands expose the projection to a localized read-only
console view, and the Sprint 11 change stream is now derived from the same
canonical events instead of being invented in the fixture module.

The exit criterion is met and asserted end to end: from a correlation
candidate, a responder sees the changes recorded before its first signal and
reaches the supporting source through a validated `https` link, with no diff
body, credential or email address anywhere in the snapshot.

Two defects were found and fixed during this pass, both of which silently
disabled the sprint's central mechanism:

1. **Change targets did not match the Sprint 13 identifier form.** The replay
   adapters emitted a bare source name (`checkout`) while signals name a
   deployment `deployment/checkout`, so no committed change could ever
   associate with a committed candidate. Fixed in `change/normalize.rs`
   (`6a772dc`) with a regression test.
2. **The committed change payloads were dated one day after the correlated
   signals.** Precedence could therefore never hold over committed data.
   Every fixture timestamp was shifted back one day, preserving the intra-set
   ordering (`b9ed11c`).

## Deliverable traceability

| Deliverable | Implementation | Verification |
| --- | --- | --- |
| GitHub/GitLab integration | `src-tauri/src/change/adapters/{github,gitlab}.rs` | `change_adapters.rs` (6 tests) |
| Argo CD integration | `src-tauri/src/change/adapters/argocd.rs` | `change_adapters.rs` |
| Deployment and configuration change events | `crates/thalassa-domain` `ChangeEvent`, `change/normalize.rs` | `change_records.rs` (7 tests) |
| Change timeline | `src-tauri/src/change/timeline.rs` | `change_timeline.rs` (4 tests) |
| Change-to-signal correlation | `src-tauri/src/change/association.rs` | `change_association.rs` (9 tests), `change_acceptance.rs` |
| Diff and native source links | `change/normalize.rs` link policy, `ui/src/change/ChangeDetail.tsx` | `change_ipc.rs`, `change.acceptance.test.tsx` |
| Sprint 11 reconciliation | `src-tauri/src/change/projection.rs`, `operations/{fixtures,aggregate}.rs` | `operations_aggregation.rs` (20 tests) |
| Read-only IPC surface | `src-tauri/src/app/change.rs`, `src-tauri/src/main.rs` | `change_ipc.rs` (12 tests) |
| Localized console view | `ui/src/change/*`, `ui/src/locales/{en,th}.ts` | `ChangeTimeline.test.tsx`, `change.acceptance.test.tsx` |

## Gate results

Every gate was run unpiped with a real exit code after `npm ci`.

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | exit=0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit=0 |
| `cargo test --workspace` | exit=0, 450 tests passed |
| `npm run format:check` | exit=0 |
| `npm run lint` | exit=0 |
| `npm run typecheck` | exit=0 |
| `npm test` | exit=0, 122 tests passed |

## Design-gate coverage

| Gate | Evidence |
| --- | --- |
| Domain contracts | `change_records.rs`, `correlation_contracts.rs`, `change-contracts.test.ts` |
| Sprint 11 reconciliation | `operations_aggregation.rs::change_stream_items_are_projected_from_canonical_change_events`, `::the_console_change_stream_no_longer_invents_items`, `::a_projected_summary_only_carries_source_supplied_identifiers` |
| Adapter replay | `change_adapters.rs::every_fixture_normalizes_to_exactly_one_event`, `::replay_is_order_independent` |
| Identity safety | `change_adapters.rs::merged_pull_request_maps_to_code_merge_with_rejected_email_actor`, `::credentialed_link_is_dropped_not_emitted` |
| Timeline | `change_timeline.rs` (half-open boundaries, `(occurred_at, id)` tie-break, truncation flag) |
| Association | `change_association.rs` (both precedence edges, lookback cap, exact target, topology path, negative case) and `change_ipc.rs::a_change_that_only_precedes_a_signal_is_listed_but_not_associated` |
| Determinism | `change_ipc.rs::repeated_snapshots_are_byte_identical`, `change_acceptance.rs::shuffled_fixture_order_produces_an_identical_snapshot` |
| Policy and egress | `change_ipc.rs` (capability, bounded envelope scope, audit retention, evidence closure, `NOT_FOUND` on an unknown ID) |
| Secret leak | `change_acceptance.rs::no_snapshot_field_contains_a_credential_email_or_diff_body`, `change_ipc.rs::the_snapshot_carries_no_credential_email_or_diff_body` |
| UI | `ChangeTimeline.test.tsx` (ordering, truncation, empty state, evidence), `change.acceptance.test.tsx` (en/th coverage, no causal copy, no diff viewer) |
| Acceptance | `change_acceptance.rs::the_exit_criterion_holds_end_to_end` |

## Behavior confirmed from the committed data

Replaying the nine committed fixtures against the Sprint 13 correlation
snapshot produces nine timeline entries and five associations on the
`deployment/checkout` candidate, with measured lead times of 140, 560, 620, 800
and 860 seconds. The four changes recorded at or after the candidate's first
signal appear in the timeline and in no association list, which is the design's
required negative case: temporal proximity alone never creates an association.
Eight of the nine events carry a validated `https` source link; the ninth is
the fixture whose credentialed URL is deliberately dropped by link policy and
reported through a typed source status.

## Deviations from the plan

- `change::metrics::build` takes the request scope as an explicit third
  argument. `ChangeMetric` carries a `DrillDownReference`, which requires a
  scope; deriving it from the first event would make the metric depend on event
  order.
- Change locale keys live in the existing central catalogs
  (`ui/src/locales/{en,th}.ts`) rather than new per-module locale files, which
  is the pattern every other workspace in this repository follows and needs no
  i18n plumbing change.
- `change_acceptance.rs` asserts that at least one associated change exposes a
  validated `https` link, not that every one does. One committed fixture
  carries a credentialed URL that link policy must drop, so requiring a link on
  every change would assert against the sprint's own safety requirement.
- The two Tauri commands are registered in `src-tauri/main.rs` beside the
  existing command wrappers, matching the layout every prior sprint uses;
  `src-tauri/src/app/mod.rs` declares the handler module.
- The change timeline, detail panel and per-candidate section are mounted in
  the correlation workspace, where the candidate and its preceding changes are
  read together. The Operations Console keeps its Sprint 11 change-stream
  widget, now derived from the same canonical events.

## Residual notes

- Adapters remain replay-only over committed fixtures. There is no ingest,
  provider query, credential store or outbound network path in this sprint.
- `change.evidence` resolves only IDs retained by the records behind the
  current snapshot; it is not a native record retrieval path.
