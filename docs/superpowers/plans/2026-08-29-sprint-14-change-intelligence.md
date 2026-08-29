# Sprint 14 Change Intelligence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize replayable GitHub, GitLab and Argo CD change records into the canonical source-preserving `ChangeEvent` contract, order them into a deterministic change timeline, and attach them to Sprint 13 correlation candidates as explainable structural context with native source links.

**Architecture:** A new `src-tauri/src/change` module owns fixture replay, post-policy retention in the existing append-only source-record ledger, normalization, timeline ordering and association. Association reuses the Sprint 13 `SignalTarget` type and delegates all graph work to the Sprint 12 topology engine, so a change and a signal that name the same deployment compare exactly. Two capability-scoped read commands expose a snapshot to a localized, read-only Operations Console view.

**Tech Stack:** Rust 2021, Tauri 2, Serde, Chrono, Uuid, SQLite through the existing local-first storage layer, React 18, TypeScript, Vite, Vitest, Testing Library, and the existing ThalassaOps design system.

**Spec:** docs/design/sprint-14-change-intelligence.md

## Global Constraints

- There is one type per concept. `thalassa_domain::ChangeEvent` is the canonical change record. Do not create `NormalizedChange`, `ChangeRecord`, a provider-specific change struct, a second timeline type or a UI-only change model. Reuse `ResourceScope`, `SignalTarget`, `EvidenceRef`, `ConsoleEvidenceId`, `SourceRecordRef`, `DrillDownTarget`, `DrillDownReference`, `TimeWindow`, `NumberUnit`, `SourceStatus` and the Sprint 12 topology types.
- Extend the existing `EvidenceSourceKind` with exactly the wire values `github`, `gitlab` and `argo_cd`. Extend the existing `ChangeKind` with exactly `code_commit`, `code_merge`, `sync` and `rollback`, leaving `deployment`, `configuration`, `maintenance` and `connector` unchanged. Do not create a private source or kind enum and do not use a free-form source string anywhere.
- A `ChangeEvent` is never a member of a `CorrelationCandidate`. Changes attach through `ChangeAssociation` only. `CorrelationReasonKind::PrecedingChange` always carries `CorrelationQualification::ProbableStructural`. Never add or render `root_cause`, `caused_by`, `triggered_by`, `blast_radius` or a probability score in any contract, wire value, locale key or string.
- Temporal precedence alone never creates an association. A change must also share an exact `SignalTarget` with the candidate or connect through at least one Sprint 12 topology path. Both conditions are required.
- Rust numeric fields are `f64` and TypeScript numeric fields are `number`. Reject NaN, positive infinity, negative infinity and negative counts with typed errors before IPC serialization. `lookback_seconds` is validated to `0.0..=86_400.0` and defaults to `3_600.0`; `lead_time_seconds` is finite and non-negative.
- Diff hunk content is never parsed into a contract, retained, serialized or rendered. Adapters drop diff-body fields before the source record is admitted to the ledger, not after. `changed_paths` holds repository-relative paths only.
- `occurred_at` is required on every `ChangeEvent`. A source payload with no usable timestamp is a typed adapter error, never a substitution of ingestion time.
- A native link is admitted only when it parses as absolute `https`, its host matches the per-source allowlist, and it carries no userinfo component, no query string and no fragment. A failing link is omitted and reported through `SourceStatus`; it is never emitted unvalidated.
- Absent source data is `Option`/`null`, an explicit unavailable `SourceStatus` or an omitted record. Empty strings are never placeholders. Fabricated timestamps, actors, revisions, targets, outcomes and links are forbidden.
- Unsafe identities are rejected, not blanked. An email-shaped or credential-shaped actor handle yields `ChangeActorKind::Unknown` with `handle: None` plus a typed source status.
- No credential, token, ARN, account ID, subscription ID, authorization header, cookie, pagination cursor, real or routable email address (the single reserved-TLD marker `sprint14-fixture-actor@example.invalid` is the one permitted exception, and only in `github/pull-request-merged.json`, to exercise actor rejection) or diff body may enter a normalized change, association, log line, committed fixture, retained record or serialized result.
- The complete post-policy source record, unknown fields included, is retained in the append-only ledger. Normalized fields are typed indexes over that record, never a lossy flattening or paraphrase.
- Every displayed value carries verified evidence IDs and a typed drill-down reference. Every evidence ID in a snapshot resolves inside that same snapshot. No returned ID may resolve outside the current workspace.
- New IPC commands are exactly `change.snapshot` (`WorkspaceRead`/`Read`) and `change.evidence` (`ResourceRead`/`Read`). There is no ingest, adapter-trigger, provider-query, sync, revert, change-write or candidate-mutation command.
- Every command follows the established authorization order: exact descriptor and capability, envelope scope, active membership/principal/workspace grant/role permission, request parsing and limits, source and local-storage policy, adapter and projection work, evidence-ID validation, then `EgressDestination::Ui` and `EgressDestination::AuditLog` policy with verified `Internal` data before serialization.
- Adapters consume committed synthetic replay fixtures only. Do not provision infrastructure, run Terraform/OpenTofu, capture live provider data, invoke a provider CLI, add an outbound network path or add credential storage.
- Do not make adapter output, timelines, associations or snapshots depend on wall-clock time, fixture file order, input order or background schedulers. An injected fixture clock, an explicit request evaluation time, sorted IDs and stable digests must produce byte-identical output for identical inputs and policy version.
- Rust never emits user-facing English sentences. React maps stable wire values to `en` and `th` locale keys. Localized copy uses precedence wording ("changed before", "preceded"), never causal wording.
- Run `npm ci` before any frontend gate. A gate that cannot run is blocked and must be reported; it is not a passing gate. Run gates unpiped with a plain `; echo exit=$?` — piping reports the pipe tail's status, and `${PIPESTATUS[0]}` is empty under this machine's zsh.
- The exact sprint exit criterion is: "A user can identify what changed before an incident and inspect the supporting source/diff."

## File map and parallel handoff

Task 1 is the synchronization point for domain contracts, IPC descriptors, the TypeScript mirror and the committed fixtures. After Task 1:

- the backend worker owns Tasks 2–6: `crates/thalassa-domain`, `crates/thalassa-ipc`, `src-tauri/src/change`, `src-tauri/src/operations`, `src-tauri/src/app/change.rs`, the migration and Rust tests;
- the React worker owns the UI portion of Task 7: `ui/contracts/ipc.ts`, `ui/src/change`, `ui/src/operations` change-stream wiring, locale files, styles and frontend tests, consuming the copied fixture without importing Rust code; and
- Task 8 starts only after the contract, retention, adapter, timeline, association, reconciliation, IPC and UI tests are green.

No worker changes a field name, enum wire value, nullability rule, identity rule or fixture ID without updating `docs/design/sprint-14-change-intelligence.md` and the copied fixture in the same change.

---

### Task 1: Define change contracts, IPC descriptors and the replay fixture catalog

**Files:**

- Modify: `crates/thalassa-domain/src/lib.rs` — add `ChangeEventId`, `ChangeEvent`, `ChangeOutcome`, `ChangeActorKind`, `ChangeActor`, `ChangeRevision`, `ChangeRepositoryRef`, `ChangeDiffStat`, `ChangeLinkKind`, `ChangeSourceLink`, `ChangeTimeline`, `ChangeAssociation`, `ChangeMetricKey`, `ChangeMetric`, `ChangeRequest`, `ChangeEvidenceRequest`, `ChangeSnapshot` and `ChangeError`; extend `EvidenceSourceKind` and `ChangeKind`; change `ChangeStreamItem.source` to `EvidenceSourceKind`; add `CorrelationReasonKind::PrecedingChange`.
- Create: `crates/thalassa-domain/tests/change_contracts.rs` — wire values, JSON shape, `Option`/null behaviour, finite-number validation, evidence and drill-down invariants.
- Modify: `crates/thalassa-ipc/src/lib.rs` — add `change_snapshot_descriptor()` and `change_evidence_descriptor()` as the only command metadata source.
- Modify: `crates/thalassa-ipc/tests/contracts.rs` — assert both command names, capabilities, permissions and descriptor scopes.
- Create: `src-tauri/src/change/mod.rs` — declare the module and re-export domain contracts without introducing a second model.
- Create: `src-tauri/src/change/fixtures.rs` — replay catalog, injected fixture clock and safe synthetic values.
- Modify: `src-tauri/src/lib.rs` — add `pub mod change;` for integration tests.
- Create: `docs/superpowers/fixtures/2026-08-29-change/github/push.json`
- Create: `docs/superpowers/fixtures/2026-08-29-change/github/pull-request-merged.json`
- Create: `docs/superpowers/fixtures/2026-08-29-change/github/deployment-status.json`
- Create: `docs/superpowers/fixtures/2026-08-29-change/gitlab/push.json`
- Create: `docs/superpowers/fixtures/2026-08-29-change/gitlab/merge-request-merged.json`
- Create: `docs/superpowers/fixtures/2026-08-29-change/gitlab/pipeline-deployment.json`
- Create: `docs/superpowers/fixtures/2026-08-29-change/argocd/sync-succeeded.json`
- Create: `docs/superpowers/fixtures/2026-08-29-change/argocd/sync-failed.json`
- Create: `docs/superpowers/fixtures/2026-08-29-change/argocd/rollback.json`
- Modify: `ui/contracts/ipc.ts` — mirror every new wire contract exactly, including the three source values, four kind values and the `preceding_change` reason value.
- Create: `ui/src/change/change-fixtures.ts` — copied, typed fixture snapshot for frontend work.
- Create: `ui/src/change/change-contracts.test.ts` — copied-fixture field, enum, nullability and finite-number assertions.

**Interfaces:**

- Consumes: existing `ResourceScope`, `SignalTarget`, `SourceRecordRef`, `ConsoleEvidenceId`, `DrillDownTarget`, `DrillDownReference`, `TimeWindow`, `NumberUnit`, `SourceStatus`, `CorrelationQualification`, `CorrelationReasonKind`, `CorrelationCandidate`.
- Produces: every contract above; `ChangeEvent::validate(&self) -> Result<(), ChangeError>`; `ChangeSnapshot::validate_evidence_closure(&self) -> Result<(), ChangeError>`; `change_snapshot_descriptor()` and `change_evidence_descriptor()`; `change::fixtures::catalog() -> Vec<ChangeFixture>` and `change::fixtures::fixture_clock() -> DateTime<Utc>`.

- [ ] **Step 1: Write the failing domain contract test**

```rust
// crates/thalassa-domain/tests/change_contracts.rs
use serde_json::json;
use thalassa_domain::{
    ChangeActor, ChangeActorKind, ChangeDiffStat, ChangeEvent, ChangeKind, ChangeLinkKind,
    ChangeOutcome, ChangeSourceLink, CorrelationQualification, CorrelationReasonKind,
    EvidenceSourceKind, NumberUnit,
};

#[test]
fn change_source_kinds_use_stable_wire_values() {
    assert_eq!(
        serde_json::to_value(EvidenceSourceKind::GitHub).unwrap(),
        json!("github")
    );
    assert_eq!(
        serde_json::to_value(EvidenceSourceKind::GitLab).unwrap(),
        json!("gitlab")
    );
    assert_eq!(
        serde_json::to_value(EvidenceSourceKind::ArgoCd).unwrap(),
        json!("argo_cd")
    );
}

#[test]
fn change_kind_keeps_sprint_11_values_and_adds_sprint_14_values() {
    for (kind, wire) in [
        (ChangeKind::Deployment, "deployment"),
        (ChangeKind::Configuration, "configuration"),
        (ChangeKind::Maintenance, "maintenance"),
        (ChangeKind::Connector, "connector"),
        (ChangeKind::CodeCommit, "code_commit"),
        (ChangeKind::CodeMerge, "code_merge"),
        (ChangeKind::Sync, "sync"),
        (ChangeKind::Rollback, "rollback"),
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(wire));
    }
}

#[test]
fn preceding_change_reason_is_always_probable_structural() {
    assert_eq!(
        serde_json::to_value(CorrelationReasonKind::PrecedingChange).unwrap(),
        json!("preceding_change")
    );
    assert_eq!(
        serde_json::to_value(CorrelationQualification::ProbableStructural).unwrap(),
        json!("probable_structural")
    );
}

#[test]
fn diff_stat_rejects_non_finite_and_negative_counts() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        let stat = ChangeDiffStat {
            files_changed: value,
            insertions: 0.0,
            deletions: 0.0,
            unit: NumberUnit::Count,
        };
        assert!(stat.validate().is_err(), "expected rejection for {value}");
    }
}

#[test]
fn source_link_rejects_query_strings_and_non_https() {
    for url in [
        "https://github.com/acme/api/commit/abc?token=secret",
        "http://github.com/acme/api/commit/abc",
        "https://user:pass@github.com/acme/api/commit/abc",
        "https://github.com/acme/api/commit/abc#frag",
    ] {
        let link = ChangeSourceLink {
            kind: ChangeLinkKind::Commit,
            url: url.to_string(),
        };
        assert!(
            link.validate(EvidenceSourceKind::GitHub).is_err(),
            "expected rejection for {url}"
        );
    }
}

#[test]
fn actor_handle_rejects_email_shaped_identity() {
    let actor = ChangeActor {
        kind: ChangeActorKind::Human,
        handle: Some("someone@example.com".to_string()),
    };
    assert!(actor.validate().is_err());
}

#[test]
fn change_event_requires_occurred_at_and_serializes_optional_fields_as_null() {
    let event: ChangeEvent = serde_json::from_str(include_str!("fixtures/change_event.json"))
        .expect("fixture parses");
    assert!(event.validate().is_ok());
    let value = serde_json::to_value(&event).unwrap();
    assert!(value.get("occurred_at").unwrap().is_string());
    assert!(value.get("environment").unwrap().is_null());
    assert_eq!(event.outcome, ChangeOutcome::Succeeded);
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p thalassa-domain --test change_contracts`
Expected: FAIL — the contracts do not exist yet (`unresolved import`).

- [ ] **Step 3: Add the contracts to the domain crate**

Add the structs and enums exactly as written in `docs/design/sprint-14-change-intelligence.md` under "Data model". Implement `validate` for `ChangeDiffStat`, `ChangeActor`, `ChangeSourceLink`, `ChangeEvent` and `ChangeSnapshot`, reusing the existing `validate_safe_identifier` helper for handles, revisions, repository parts and changed paths. Add the host allowlist as a private function keyed by `EvidenceSourceKind`: `github.com` for `GitHub`, `gitlab.com` for `GitLab`, and for `ArgoCd` accept any host, because an Argo CD install is self-hosted — but still require `https`, no userinfo, no query and no fragment.

Change `ChangeStreamItem.source` from `Option<String>` to `EvidenceSourceKind` and fix `crates/thalassa-domain/tests/operations_contracts.rs` to match.

Create `crates/thalassa-domain/tests/fixtures/change_event.json` holding one complete serialized `ChangeEvent` with `environment: null`.

- [ ] **Step 4: Run the domain tests and confirm they pass**

Run: `cargo test -p thalassa-domain`
Expected: PASS, including the updated `operations_contracts` test.

- [ ] **Step 5: Write the failing IPC descriptor test**

```rust
// crates/thalassa-ipc/tests/contracts.rs (append)
#[test]
fn change_commands_expose_read_only_descriptors() {
    let snapshot = change_snapshot_descriptor();
    assert_eq!(snapshot.command, "change.snapshot");
    assert_eq!(snapshot.capability, Capability::WorkspaceRead);
    assert_eq!(snapshot.permission, Permission::Read);

    let evidence = change_evidence_descriptor();
    assert_eq!(evidence.command, "change.evidence");
    assert_eq!(evidence.capability, Capability::ResourceRead);
    assert_eq!(evidence.permission, Permission::Read);
}
```

- [ ] **Step 6: Run it, confirm failure, then add both descriptors**

Run: `cargo test -p thalassa-ipc --test contracts`
Expected: FAIL, then PASS after adding `change_snapshot_descriptor()` and `change_evidence_descriptor()` beside the Sprint 13 correlation descriptors.

- [ ] **Step 7: Write the nine fixtures and the fixture catalog**

Each fixture is a synthetic payload shaped like the documented provider response. Build in the safety cases the design requires, spread across files: `github/pull-request-merged.json` carries an email-shaped author field; `gitlab/pipeline-deployment.json` carries a native URL with a `?private_token=` query string; `github/push.json` carries a `patch` diff body; `argocd/sync-failed.json` carries unknown top-level fields that must survive in the retained record. Every timestamp is fixed; no fixture contains a real credential.

`src-tauri/src/change/fixtures.rs` exposes:

```rust
pub struct ChangeFixture {
    pub source: EvidenceSourceKind,
    pub path: &'static str,
    pub payload: &'static str,
}

pub fn catalog() -> Vec<ChangeFixture> { /* include_str! each of the nine files, sorted by path */ }

pub fn fixture_clock() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-08-29T09:00:00Z").unwrap().with_timezone(&Utc)
}
```

- [ ] **Step 8: Mirror the contracts in TypeScript and assert them against a copied fixture**

Add every new type to `ui/contracts/ipc.ts` with identical field names and wire values. Create `ui/src/change/change-fixtures.ts` as a typed copy of one snapshot, and `ui/src/change/change-contracts.test.ts` asserting the three source values, the eight change kinds, `preceding_change`, null handling for `environment`, `repository`, `revision`, `diff_stat` and `source_link`, and that every numeric field is a finite `number`.

- [ ] **Step 9: Run the frontend gates**

Run: `npm ci` then `npm test -- change-contracts`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/thalassa-domain crates/thalassa-ipc src-tauri/src/change src-tauri/src/lib.rs docs/superpowers/fixtures/2026-08-29-change ui/contracts/ipc.ts ui/src/change
git commit -m "feat(domain): add change intelligence contracts and replay fixtures"
```

---

### Task 2: Retain post-policy change records and normalize them

**Files:**

- Create: `src-tauri/migrations/0005_change_records.sql` — append-only change source records keyed by content digest. Evidence rows are NOT duplicated here: change evidence reuses the existing `source_record_evidence` table from `0004_source_record_evidence.sql`.
- Create: `src-tauri/src/change/records.rs` — policy evaluation, record admission, digest computation, evidence minting and ledger writes.
- Create: `src-tauri/src/change/normalize.rs` — payload to `ChangeEvent` normalization with identity, link and timestamp safety.
- Create: `src-tauri/tests/change_records.rs` — retention, restart survival, unknown-field preservation, rejection paths.
- Modify: `src-tauri/src/change/mod.rs` — declare `records` and `normalize`.

**Interfaces:**

- Consumes: `change::fixtures::catalog()`, `ChangeEvent`, `SourceRecordRef`, `EvidenceRef`, the Sprint 13 ledger in `src-tauri/src/correlation/source_records.rs` (including its `source_record_evidence` storage and `evidence_for` lookup) and the existing policy classification helpers.
- Produces: `records::admit(payload: &str, source: EvidenceSourceKind, clock: DateTime<Utc>) -> Result<AdmittedRecord, ChangeError>` where `AdmittedRecord { record_ref: SourceRecordRef, body: serde_json::Value, evidence: Vec<EvidenceRef> }`; `normalize::to_change_event(record: &AdmittedRecord) -> Result<ChangeEvent, ChangeError>`.

**Evidence decision (binding for Tasks 3, 7 and 8):** change evidence is minted by `records::admit` and stored through the existing Sprint 13 evidence store, writing to the existing `source_record_evidence` table. Migration `0005` therefore adds no evidence table. `ChangeEvent.evidence_ids` holds the IDs minted here, and `change.evidence` resolves them through the same `evidence_for` path Sprint 13's `correlation.evidence` uses. Do not create a second evidence store, a change-specific evidence ID format or a parallel lookup.

- [ ] **Step 1: Write the failing retention test**

```rust
// src-tauri/tests/change_records.rs
use thalassaops::change::{fixtures, records};

#[test]
fn admitted_record_preserves_unknown_fields_and_drops_diff_bodies() {
    let fixture = fixtures::catalog()
        .into_iter()
        .find(|f| f.path.ends_with("argocd/sync-failed.json"))
        .expect("fixture present");
    let admitted = records::admit(fixture.payload, fixture.source, fixtures::fixture_clock())
        .expect("record admitted");

    assert!(admitted.body.get("unknownOperatorField").is_some());
    assert!(admitted.record_ref.content_digest.len() >= 32);
}

#[test]
fn diff_bodies_never_enter_the_retained_record() {
    let fixture = fixtures::catalog()
        .into_iter()
        .find(|f| f.path.ends_with("github/push.json"))
        .expect("fixture present");
    let admitted = records::admit(fixture.payload, fixture.source, fixtures::fixture_clock())
        .expect("record admitted");

    let serialized = serde_json::to_string(&admitted.body).unwrap();
    assert!(!serialized.contains("\"patch\""));
    assert!(!serialized.contains("@@ -"));
}

#[test]
fn retained_records_survive_a_reopen_of_the_database() {
    // open the app database, admit every fixture, drop the handle,
    // reopen and assert every content digest is still readable
}

#[test]
fn admitted_evidence_resolves_through_the_sprint_13_evidence_store() {
    let fixture = fixtures::catalog().into_iter().next().expect("fixture present");
    let admitted = records::admit(fixture.payload, fixture.source, fixtures::fixture_clock())
        .expect("record admitted");

    assert!(!admitted.evidence.is_empty());
    for evidence in &admitted.evidence {
        assert!(
            records::resolve_evidence(&evidence.id).is_some(),
            "evidence must resolve through the existing source_record_evidence store"
        );
    }
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p thalassaops --test change_records`
Expected: FAIL — `records` does not exist.

- [ ] **Step 3: Write the migration**

```sql
-- src-tauri/migrations/0005_change_records.sql
CREATE TABLE IF NOT EXISTS change_source_record (
    content_digest TEXT PRIMARY KEY,
    source_kind    TEXT NOT NULL,
    native_id      TEXT,
    revision       TEXT,
    occurred_at    TEXT NOT NULL,
    admitted_at    TEXT NOT NULL,
    body           TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS change_source_record_occurred_at
    ON change_source_record (occurred_at);
```

The table is append-only: the implementation issues `INSERT OR IGNORE` and never `UPDATE` or `DELETE`.

- [ ] **Step 4: Implement admission and normalization**

`records::admit` parses the payload to `serde_json::Value`, strips diff-body fields (`patch`, `diff`, `content`) before anything else, evaluates source and local-storage policy, computes the content digest over the post-policy body, writes the `change_source_record` row, mints one `EvidenceRef` per admitted record through the Sprint 13 evidence store, and returns the `SourceRecordRef` together with the minted evidence.

`normalize::to_change_event` maps the retained body to a `ChangeEvent`, returning `ChangeError::MissingTimestamp` when no usable `occurred_at` exists, downgrading an unsafe actor to `ChangeActorKind::Unknown` with `handle: None`, dropping a link that fails `ChangeSourceLink::validate`, and dropping any changed path that fails safe-path validation. Each downgrade records a typed `SourceStatus` on the returned result rather than silently succeeding.

- [ ] **Step 5: Run the tests and confirm they pass**

Run: `cargo test -p thalassaops --test change_records`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/migrations/0005_change_records.sql src-tauri/src/change src-tauri/tests/change_records.rs
git commit -m "feat(change): retain post-policy change records and normalize them"
```

---

### Task 3: Implement the GitHub, GitLab and Argo CD replay adapters

**Files:**

- Create: `src-tauri/src/change/adapters/mod.rs` — adapter trait surface and dispatch by `EvidenceSourceKind`.
- Create: `src-tauri/src/change/adapters/github.rs`
- Create: `src-tauri/src/change/adapters/gitlab.rs`
- Create: `src-tauri/src/change/adapters/argocd.rs`
- Create: `src-tauri/tests/change_adapters.rs` — one assertion group per fixture plus the safety cases.

**Interfaces:**

- Consumes: `records::admit`, `normalize::to_change_event`, `fixtures::catalog()`.
- Produces: `adapters::replay_all(clock: DateTime<Utc>) -> Result<AdapterOutput, ChangeError>` and `adapters::replay_from(fixtures: Vec<ChangeFixture>, clock: DateTime<Utc>) -> Result<AdapterOutput, ChangeError>`, where `AdapterOutput { events: Vec<ChangeEvent>, statuses: Vec<SourceStatus> }` and `events` is sorted by `(occurred_at, id)`.

- [ ] **Step 1: Write the failing adapter test**

```rust
// src-tauri/tests/change_adapters.rs
use thalassaops::change::{adapters, fixtures};
use thalassa_domain::{ChangeActorKind, ChangeKind, ChangeOutcome, EvidenceSourceKind, SourceState};

#[test]
fn every_fixture_normalizes_to_exactly_one_event() {
    let output = adapters::replay_all(fixtures::fixture_clock()).expect("replay succeeds");
    assert_eq!(output.events.len(), 9);
}

#[test]
fn merged_pull_request_maps_to_code_merge_with_rejected_email_actor() {
    let output = adapters::replay_all(fixtures::fixture_clock()).unwrap();
    let event = output
        .events
        .iter()
        .find(|e| e.source == EvidenceSourceKind::GitHub && e.kind == ChangeKind::CodeMerge)
        .expect("merged pull request present");

    assert_eq!(event.actor.kind, ChangeActorKind::Unknown);
    assert!(event.actor.handle.is_none());
    assert!(output
        .statuses
        .iter()
        .any(|s| s.state != SourceState::Fresh));
}

#[test]
fn credentialed_link_is_dropped_not_emitted() {
    let output = adapters::replay_all(fixtures::fixture_clock()).unwrap();
    let event = output
        .events
        .iter()
        .find(|e| e.source == EvidenceSourceKind::GitLab && e.kind == ChangeKind::Deployment)
        .expect("gitlab deployment present");

    assert!(event.source_link.is_none());
}

#[test]
fn failed_argo_sync_maps_to_failed_outcome_and_rollback_to_rollback_kind() {
    let output = adapters::replay_all(fixtures::fixture_clock()).unwrap();
    assert!(output.events.iter().any(|e| e.source == EvidenceSourceKind::ArgoCd
        && e.kind == ChangeKind::Sync
        && e.outcome == ChangeOutcome::Failed));
    assert!(output
        .events
        .iter()
        .any(|e| e.kind == ChangeKind::Rollback));
}

#[test]
fn replay_is_order_independent() {
    let first = adapters::replay_all(fixtures::fixture_clock()).unwrap();
    let mut shuffled = fixtures::catalog();
    shuffled.reverse();
    let second = adapters::replay_from(shuffled, fixtures::fixture_clock()).unwrap();
    assert_eq!(
        serde_json::to_string(&first.events).unwrap(),
        serde_json::to_string(&second.events).unwrap()
    );
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p thalassaops --test change_adapters`
Expected: FAIL — `adapters` does not exist.

- [ ] **Step 3: Implement the three adapters**

Each adapter maps its own payload shape onto the shared normalization path: GitHub push to `CodeCommit`, merged pull request to `CodeMerge`, deployment status to `Deployment`; GitLab push to `CodeCommit`, merged merge request to `CodeMerge`, pipeline deployment to `Deployment`; Argo CD sync to `Sync` and rollback to `Rollback`. Targets are built as `SignalTarget` values using the same identifier convention the Sprint 13 adapters use for deployment and service targets, so exact comparison holds. `replay_all` calls `replay_from(fixtures::catalog(), clock)` and sorts by `(occurred_at, id)`.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p thalassaops --test change_adapters`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/change/adapters src-tauri/tests/change_adapters.rs
git commit -m "feat(change): add replayable GitHub, GitLab and Argo CD adapters"
```

---

### Task 4: Build the bounded, deterministic change timeline

**Files:**

- Create: `src-tauri/src/change/timeline.rs`
- Create: `src-tauri/tests/change_timeline.rs`

**Interfaces:**

- Consumes: `adapters::replay_all`, `ChangeEvent`, `TimeWindow`.
- Produces: `timeline::build(events: &[ChangeEvent], window: &TimeWindow, limit: usize) -> Result<ChangeTimeline, ChangeError>`.

- [ ] **Step 1: Write the failing timeline test**

```rust
// src-tauri/tests/change_timeline.rs
#[test]
fn window_is_half_open_on_the_end_boundary() {
    // an event at exactly window.start is included;
    // an event at exactly window.end is excluded
}

#[test]
fn entries_are_ordered_by_occurred_at_then_id() {
    // two events sharing occurred_at order by ascending id, stably across runs
}

#[test]
fn exceeding_the_limit_drops_oldest_entries_and_sets_truncated() {
    // build with limit = 3 over 9 events;
    // assert entry_ids.len() == 3, truncated == true,
    // and that the three newest events survive
}

#[test]
fn an_invalid_window_is_a_typed_error() {
    // window.end <= window.start yields ChangeError::InvalidWindow
}
```

Write each test body out fully against `timeline::build` before implementing; the comments above name the assertions, not placeholders to leave in the file.

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p thalassaops --test change_timeline`
Expected: FAIL.

- [ ] **Step 3: Implement `timeline::build`**

Validate the window, filter to `[start, end)`, sort by `(occurred_at, id)`, truncate from the oldest end when over the limit, and set `truncated`.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p thalassaops --test change_timeline`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/change/timeline.rs src-tauri/tests/change_timeline.rs
git commit -m "feat(change): add bounded deterministic change timeline"
```

---

### Task 5: Associate changes with correlation candidates

**Files:**

- Create: `src-tauri/src/change/association.rs`
- Create: `src-tauri/tests/change_association.rs`

**Interfaces:**

- Consumes: `ChangeEvent`, `CorrelationCandidate` and `Signal` from the Sprint 13 module, plus the Sprint 12 engine method `TopologyBuilder::correlation_relation(&self, left: &SignalTarget, right: &SignalTarget, window: &CorrelationWindow) -> Result<Option<TopologyPath>, TopologyError>`. Sprint 13 already routes this through the `TopologyCorrelationResolver` trait; reuse that trait rather than taking a concrete builder.
- Produces: `association::associate(events: &[ChangeEvent], candidates: &[CorrelationCandidate], signals: &[Signal], lookback_seconds: f64, topology: &dyn TopologyCorrelationResolver) -> Result<Vec<ChangeAssociation>, ChangeError>`, sorted by `(candidate_id, change_id)`.

- [ ] **Step 1: Write the failing association test**

```rust
// src-tauri/tests/change_association.rs
//
// Test helpers to write once at the top of this file:
//   fn change_at(occurred_at: &str, target: SignalTarget) -> ChangeEvent
//   fn signal_observed_at(observed_at: &str, target: SignalTarget) -> Signal
//   fn candidate_of(signals: &[Signal]) -> CorrelationCandidate
//   fn target(kind: SignalTargetKind, id: &str) -> SignalTarget
//   struct NoTopology;  // impl TopologyCorrelationResolver, always returns Ok(None)

#[test]
fn temporal_proximity_without_structure_yields_no_association() {
    let deploy = target(SignalTargetKind::Deployment, "checkout-api");
    let unrelated = target(SignalTargetKind::Deployment, "billing-worker");

    let change = change_at("2026-08-29T08:59:00Z", unrelated);
    let signal = signal_observed_at("2026-08-29T09:00:00Z", deploy);
    let candidate = candidate_of(&[signal.clone()]);

    let associations = association::associate(
        &[change],
        &[candidate],
        &[signal],
        3_600.0,
        &NoTopology,
    )
    .expect("association succeeds");

    assert!(
        associations.is_empty(),
        "60 seconds of precedence with no shared target and no topology path must not associate"
    );
}

#[test]
fn exact_shared_target_within_lookback_associates_as_probable_structural() {
    // qualification is ProbableStructural even though the target match is exact
}

#[test]
fn topology_path_qualifies_and_records_path_ids() {
    // topology_path_ids is non-empty and every id resolves in the topology snapshot
}

#[test]
fn a_change_after_the_earliest_signal_never_associates() {
    // occurred_at == earliest observed_at is excluded (half-open on the later edge)
}

#[test]
fn a_change_exactly_at_the_lookback_horizon_associates() {
    // lead_time_seconds == lookback_seconds is included
}

#[test]
fn lookback_above_the_cap_is_a_typed_error() {
    // 86_401.0 yields ChangeError::InvalidLookback
}

#[test]
fn a_candidate_whose_signals_lack_observed_at_produces_no_associations() {
    // no fallback to ingested_at
}

#[test]
fn lead_time_is_finite_non_negative_and_measured_from_the_earliest_signal() {
    // three signals in one candidate; lead time uses the earliest observed_at
}
```

Write each body out fully. Every one of these eight cases is a distinct regression risk.

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p thalassaops --test change_association`
Expected: FAIL.

- [ ] **Step 3: Implement `association::associate`**

For each candidate, find the earliest `observed_at` among its signals; skip the candidate when none exists. For each event, require `occurred_at < earliest` and `lead_time <= lookback_seconds`, then require an exact `SignalTarget` match against `candidate.grouping_targets` or at least one topology path returned by `correlation_relation` for the candidate's own `CorrelationWindow`. Emit `ChangeAssociation` with `CorrelationQualification::ProbableStructural`, the measured lead time, the matched target when exact, the path IDs when structural, and the union of change and path evidence IDs. Sort the result.

- [ ] **Step 4: Run the tests and confirm they pass**

Run: `cargo test -p thalassaops --test change_association`
Expected: PASS, 8 tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/change/association.rs src-tauri/tests/change_association.rs
git commit -m "feat(change): associate changes with candidates as structural context"
```

---

### Task 6: Derive the Sprint 11 change stream from canonical change events

**Files:**

- Modify: `src-tauri/src/operations/fixtures.rs` — stop inventing `ChangeStreamItem` values.
- Modify: `src-tauri/src/operations/aggregate.rs` — build the change stream by projecting `ChangeEvent` values.
- Modify: `src-tauri/src/operations/model.rs` — typed source field.
- Create: `src-tauri/src/change/projection.rs` — `to_stream_item(event: &ChangeEvent) -> ChangeStreamItem`.
- Modify: `src-tauri/tests/operations_aggregation.rs` — assert the derived stream.

The matching frontend change to `ui/src/operations/contractValidation.ts` belongs to the React worker in Task 7, not here, so that no two workers edit `ui/src/operations` at once. Until Task 7 lands, the frontend validates the typed `source` field against the copied fixture from Task 1.

**Interfaces:**

- Consumes: `ChangeEvent`, existing `ChangeStreamItem`, `ChangeStreamStatus`.
- Produces: `change::projection::to_stream_item`.

- [ ] **Step 1: Write the failing projection test**

```rust
// src-tauri/tests/operations_aggregation.rs (append)
#[test]
fn change_stream_items_are_projected_from_canonical_change_events() {
    let output = adapters::replay_all(fixtures::fixture_clock()).unwrap();
    let item = change::projection::to_stream_item(&output.events[0]);

    assert_eq!(item.source, output.events[0].source);
    assert_eq!(item.kind, output.events[0].kind);
    assert_eq!(item.occurred_at, output.events[0].occurred_at);
    assert_eq!(item.evidence_ids, output.events[0].evidence_ids);
}

#[test]
fn the_console_change_stream_no_longer_invents_items() {
    // assert operations::fixtures exposes no ChangeStreamItem constructor
}
```

- [ ] **Step 2: Run it, confirm failure, implement the projection, run again**

Run: `cargo test -p thalassaops --test operations_aggregation`
Expected: FAIL, then PASS. `ChangeStreamItem.summary` stays `String`, so the projection never invents English: it uses the source-supplied title when the record has one, otherwise the revision `short_id`, otherwise the `native_id`. All three are source-supplied identifiers, not generated prose. React renders the surrounding sentence from typed fields and locale keys as it does today.

- [ ] **Step 3: Run the Rust gates**

Run: `cargo test -p thalassaops; echo exit=$?`
Expected: exit=0, with the Sprint 11 operations suite still green.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/operations src-tauri/src/change/projection.rs src-tauri/tests/operations_aggregation.rs
git commit -m "refactor(operations): derive the change stream from canonical change events"
```

---

### Task 7: Expose the change IPC commands and build the localized read-only UI

**Files:**

- Create: `src-tauri/src/app/change.rs` — `change_snapshot` and `change_evidence` handlers.
- Create: `src-tauri/src/change/metrics.rs` — snapshot metrics.
- Modify: `src-tauri/src/app/mod.rs` — register both commands.
- Create: `src-tauri/tests/change_ipc.rs` — authorization order, egress, evidence closure, error mapping.
- Create: `ui/src/change/ChangeTimeline.tsx`
- Create: `ui/src/change/ChangeDetail.tsx`
- Create: `ui/src/change/CandidateChangeSection.tsx`
- Create: `ui/src/change/change.css`
- Create: `ui/src/change/en.ts`
- Create: `ui/src/change/th.ts`
- Create: `ui/src/change/ChangeTimeline.test.tsx`
- Create: `ui/src/change/change.acceptance.test.tsx`
- Modify: `ui/src/correlation/CandidateDetails.tsx` — render the change section.
- Modify: `ui/src/operations/contractValidation.ts` — typed `source` validation for the derived change stream (handed over from Task 6; the React worker owns every file under `ui/`).

**Interfaces:**

- Consumes: `adapters::replay_all`, `timeline::build`, `association::associate`, `change_snapshot_descriptor()`, `change_evidence_descriptor()`.
- Also produces `metrics::build(events: &[ChangeEvent], associations: &[ChangeAssociation]) -> Vec<ChangeMetric>` in `src-tauri/src/change/metrics.rs`, emitting `ChangesInWindow`, `AssociatedChanges` and one `ChangesBySource` per source present, each with finite values, `NumberUnit::Count` and evidence IDs drawn from the contributing events.
- Produces: the `change.snapshot` and `change.evidence` command surface and the `ChangeSnapshot` wire payload the UI consumes.

- [ ] **Step 1: Write the failing IPC test**

```rust
// src-tauri/tests/change_ipc.rs
#[test]
fn change_snapshot_requires_workspace_read_capability() {
    // a principal without WorkspaceRead yields IpcErrorCode::PermissionDenied
}

#[test]
fn a_bounded_envelope_scope_is_rejected_before_any_adapter_runs() {
    // assert no record is admitted when the envelope scope is wrong
}

#[test]
fn lookback_above_the_cap_maps_to_invalid_request() {
    // 86_401.0 yields IpcErrorCode::InvalidRequest
}

#[test]
fn every_evidence_id_in_the_snapshot_resolves_inside_it() {
    // walk events, associations and metrics; each id resolves via change.evidence
}

#[test]
fn change_evidence_rejects_an_id_absent_from_the_current_snapshot() {
    // yields IpcErrorCode::NotFound
}

#[test]
fn a_policy_denial_omits_the_record_and_reports_a_typed_status() {
    // never a healthy record with missing fields
}

#[test]
fn repeated_snapshots_are_byte_identical() {
    // same request, same evaluation time, identical serialized output
}
```

- [ ] **Step 2: Run it, confirm failure, implement both handlers**

Run: `cargo test -p thalassaops --test change_ipc`
Expected: FAIL, then PASS. Follow the seven-step authorization order from the design exactly; `change_evidence` resolves only IDs present in the snapshot computed for the same evaluation time.

- [ ] **Step 3: Build the UI against the copied fixture**

`ChangeTimeline` renders ordered entries with source, kind, outcome, actor, target and time, plus an explicit truncation notice when `truncated` is true. `ChangeDetail` renders revision, repository, environment, diff statistics, changed paths and the native link, and states that diff content is read at the source — it never renders an empty diff viewer. `CandidateChangeSection` renders associations with the localized `preceding_change` label, the qualification label, the lead time and the matched target or topology path.

- [ ] **Step 4: Write the failing UI tests, then make them pass**

`ChangeTimeline.test.tsx` covers ordering, truncation, empty state and that every rendered value carries evidence IDs. `change.acceptance.test.tsx` covers the exit criterion: from a candidate, the user sees what changed before it and reaches the native source link. Add a test asserting no locale value in `en.ts` or `th.ts` matches `/caus|root cause|trigger/i`.

Run: `npm test -- change`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/app src-tauri/src/change/metrics.rs src-tauri/tests/change_ipc.rs ui/src/change ui/src/correlation/CandidateDetails.tsx ui/src/operations/contractValidation.ts
git commit -m "feat(change): expose read-only change intelligence IPC and console view"
```

---

### Task 8: Run regression, determinism, secret-leak and acceptance verification

**Files:**

- Create: `src-tauri/tests/change_acceptance.rs`
- Create: `docs/superpowers/reports/2026-08-29-sprint-14-verification.md`

**Interfaces:**

- Consumes: everything from Tasks 1–7.
- Produces: the verification report and a green gate set.

- [ ] **Step 1: Write the acceptance and secret-leak tests**

```rust
// src-tauri/tests/change_acceptance.rs
#[test]
fn the_exit_criterion_holds_end_to_end() {
    // from a correlation candidate, resolve its associations,
    // assert each names a change that precedes it and shares a target or path,
    // and assert each change exposes a validated https native link
}

#[test]
fn no_snapshot_field_contains_a_credential_email_or_diff_body() {
    // serialize the full snapshot and assert it matches none of:
    // an email pattern, "Bearer ", "private_token", "@@ -", "\"patch\""
}

#[test]
fn shuffled_fixture_order_produces_an_identical_snapshot() {
    // byte-for-byte equality
}
```

- [ ] **Step 2: Run the full Rust suite**

Run: `cargo test --workspace; echo exit=$?`
Expected: exit=0, with the Sprint 13 correlation suite still green.

- [ ] **Step 3: Run every release gate unpiped**

```bash
npm ci
npm run format:check; echo exit=$?
npm run lint; echo exit=$?
npm run typecheck; echo exit=$?
npm test; echo exit=$?
cargo fmt --all -- --check; echo exit=$?
cargo clippy --workspace --all-targets -- -D warnings; echo exit=$?
cargo test --workspace; echo exit=$?
```

Every gate must print `exit=0`. A gate that cannot run is blocked, not passed.

- [ ] **Step 4: Write the verification report**

Follow the shape of `docs/superpowers/reports/2026-08-28-sprint-13-verification.md`: deliverable-to-evidence table, defects found and fixed, gate results with real exit codes, and a "deliberately left open" section naming the fixture-backed boundary, the no-diff-body decision and the absence of a live provider path.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/tests/change_acceptance.rs docs/superpowers/reports/2026-08-29-sprint-14-verification.md
git commit -m "test(change): verify sprint 14 change intelligence end to end"
```

---

## Exit criterion

The sprint is complete only when the validated fixture snapshot and its UI evidence controls demonstrate:

> "A user can identify what changed before an incident and inspect the supporting source/diff."

with every release gate green on the final tree, and with no causal language in any contract, wire value, locale key or rendered string.
