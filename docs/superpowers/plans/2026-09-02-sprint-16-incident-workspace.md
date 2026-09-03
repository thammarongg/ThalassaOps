# Sprint 16 Incident Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Incident Workspace — a split list/detail surface where a responder manages one incident from any supported source through resolution, including comments, assignment, status changes, evidence and frozen association tabs.

**Architecture:** A shell component owns every IPC call and all selection state; panels are pure props-in/callbacks-out components that can be tested alone. Comments become a new immutable timeline event kind rather than a separate entity, which forces one prerequisite change in the Rust write path: timeline sequence allocation moves inside the write transaction for every mutation.

**Tech Stack:** Rust (thalassa-domain, thalassa-ipc, src-tauri, rusqlite), TypeScript, React, Vitest, @testing-library/react.

**Spec:** `docs/design/sprint-16-incident-workspace.md`

## Global Constraints

- Sprint 16 touches Rust only for Tasks 1-3. Every other task is confined to `ui/`.
- Comment body validation uses `validate_incident_text(body, INCIDENT_NOTE_MAXIMUM)`. `INCIDENT_NOTE_MAXIMUM` is 4000.
- `incident.add_comment` uses `Capability::IncidentWrite` and `Permission::ManageIncident`. Do not add a new capability or permission.
- No migration. `incident_timeline_event.event_kind` is plain `TEXT` with no `CHECK` constraint.
- Comment writes must not predicate on or mutate the `version` column. Loading the aggregate necessarily reads it, and the result deliberately carries the stored version so a comment never hands back a stale one; what is forbidden is a version predicate and a version write.
- The sequence-contention error must be a distinct variant. Never reuse `VersionConflict` for it.
- Panels never call IPC. Only the shell and the two hooks in Task 6 do.
- Evidence commands must never be called with an empty or duplicated identifier list.
- Every user-visible string exists in both `ui/src/locales/en.ts` and `ui/src/locales/th.ts` in the same commit that introduces it.
- Fixtures use the shared fixture day `2026-08-28` and must assert a non-empty result before anything is built on top of them.
- The card is named **Incident Summary Card** everywhere, never "Incident Card".
- Gates before any task is considered done: `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test` for Rust tasks; `npm run format:check`, `npm run lint`, `npm run typecheck`, `npm test` for UI tasks.

## Task DAG

```
Task 1  sequence allocation in transaction        (Rust, no deps)
   |
Task 2  Commented event kind + add_comment        (Rust)
   |
Task 3  incident.add_comment service/IPC          (Rust)
   |
Task 4  TypeScript contracts and guards           (ui)
   |
   +-- Task 5  locale parity test + scaffolding   (ui)
   |
   +-- Task 6  useIncidentList / useIncidentTimeline
              |
        Task 7  shell + IncidentList
              |
              +-- Task 8  IncidentNarrative
              +-- Task 9  evidence resolution + IncidentEvidencePanel
              |        |
              |     Task 10  IncidentTabs + incidentTabConfig
              +-- Task 11 IncidentCommentThread
              +-- Task 12 IncidentActions + version-conflict reload
              +-- Task 13 IncidentSummaryCard
                       |
                 Task 14 acceptance test
```

Tasks 8, 9, 11, 12 and 13 are independent of one another and may run in parallel once Task 7 lands. Task 10 depends on Task 9 because it reuses the evidence resolution helper.

## File Map

| File | Responsibility | Task |
| --- | --- | --- |
| `src-tauri/src/incident/repository.rs` | in-transaction sequence allocation, retry, comment append | 1, 3 |
| `src-tauri/src/incident/service.rs` | drop external allocation, comment command | 1, 3 |
| `crates/thalassa-domain/src/lib.rs` | `Commented` kind, `CommentedPayload`, `add_comment` | 2 |
| `crates/thalassa-ipc/src/lib.rs` | `incident_add_comment_descriptor` | 3 |
| `ui/contracts/guards.ts` | comment payload guard | 4 |
| `ui/src/incident/contractValidation.ts` | incident payload guards | 4 |
| `ui/src/locales/en.ts`, `th.ts` | strings | 5 and every UI task |
| `ui/src/incident/useIncidentList.ts` | incident page fetch and cursor paging | 6 |
| `ui/src/incident/useIncidentTimeline.ts` | timeline page fetch and sequence paging | 6 |
| `ui/src/incident/IncidentWorkspace.tsx` | layout, selection, wiring | 7 |
| `ui/src/incident/IncidentList.tsx` | queue, badges, filters | 7 |
| `ui/src/incident/IncidentNarrative.tsx` | lifecycle event rendering | 8 |
| `ui/src/incident/incidentEvidence.ts` | dedupe, empty guard, four states | 9 |
| `ui/src/incident/IncidentEvidencePanel.tsx` | evidence rendering | 9 |
| `ui/src/incident/incidentTabConfig.ts`, `IncidentTabs.tsx` | tab registry and chrome | 10 |
| `ui/src/incident/IncidentCommentThread.tsx` | comment list and composer | 11 |
| `ui/src/incident/IncidentActions.tsx` | transition, severity, role controls | 12 |
| `ui/src/incident/IncidentSummaryCard.tsx` | bounded summary and clipboard copy | 13 |
| `ui/src/incident/incident-fixtures.ts` | deterministic fixtures | 6 onward |
| `ui/src/incident/incident.css` | module styles | 7 onward |

---

### Task 1: Allocate Timeline Sequences Inside the Write Transaction

Today `IncidentService::load_for_write` reads `highest_event_sequence` and adds one *before* the transaction opens. That is safe only because the update statement carries `AND version = ?`, which rejects the losing writer. Task 3 adds a write with no version predicate, so this guard has to move.

**Files:**
- Modify: `src-tauri/src/incident/repository.rs:209` (`apply_mutation`)
- Modify: `src-tauri/src/incident/service.rs:424-448` (`load_for_write`)
- Test: `src-tauri/tests/incident_repository.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `IncidentStoreError::WriteContention` — new variant, returned after the retry budget is exhausted.
  - `IncidentServiceError::WriteContention { }` — new variant mapped from the store error.
  - `SqliteIncidentRepository::apply_mutation` keeps its signature; the `first_event_sequence` carried on the mutation is now treated as advisory and recomputed inside the transaction.

- [x] **Step 1: Write the failing test**

Add to `src-tauri/tests/incident_repository.rs`, using the fixture helpers that
already exist in that file — `fixture()` and
`triage_mutation(incident, request_id, sequence)`. Do not invent new fixture
types.

A true two-writer race cannot be staged before Task 3, because every write today
carries a version predicate that rejects the losing writer first. What *can* be
staged now is the underlying defect: the caller's sequence is trusted. Build a
valid mutation and overwrite its event sequence with one that is already taken,
which is exactly what a stale external allocation produces.

```rust
#[test]
fn a_stale_event_sequence_is_reallocated_rather_than_rejected() {
    let fixture = fixture();
    let incident = fixture.create_and_store_incident();
    let highest_before = fixture.highest_event_sequence(incident.id);

    let mut mutation = triage_mutation(&incident, REQUEST_B, highest_before + 1);
    // Simulate a stale read: the caller believes sequence 1 is still free.
    mutation.events[0].sequence = 1;

    fixture
        .repository
        .apply_mutation(WORKSPACE, mutation)
        .expect("a stale sequence is reallocated, not rejected");

    let sequences: Vec<u64> = fixture
        .timeline(incident.id)
        .iter()
        .map(|event| event.sequence)
        .collect();
    let mut unique = sequences.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(sequences.len(), unique.len(), "sequences must stay unique");
    assert_eq!(
        *sequences.iter().max().expect("at least one event"),
        highest_before + 1,
        "the appended event takes the next free sequence"
    );
}
```

If `create_and_store_incident`, `highest_event_sequence` or `timeline` do not
exist under those names in this file, use whatever the file already provides and
say so in your `worker_done` body. Do not add a new fixture struct.

**The existing `event_sequences_must_continue_the_stored_timeline` test will
fail after this change, and that is expected.** It asserts that a gapped caller
sequence is rejected. Once the repository assigns positions itself, a gapped
stored timeline is impossible by construction, so that assertion is obsolete
rather than wrong. Do not invert it into "gaps are accepted". Rewrite it to the
stronger property and rename it, for example
`stored_event_sequences_are_contiguous_regardless_of_the_supplied_base`,
asserting that a gapped or stale base is accepted, that stored sequences start
at `highest_before + 1`, that they are contiguous, and — the case the old test
never covered — that a multi-event mutation keeps its **relative event order**
after rebasing. A transition can emit status, severity and role events together;
if rebasing reorders them the audit timeline is silently wrong and no other test
would catch it.

`first_event_sequence` stays in the domain signatures. The pure aggregate has no
database access and still needs a base to number a multi-event mutation
consistently; the repository rebases that block onto the real tail. Add a
comment at the aggregate saying so so the argument does not read as dead.
Removing the parameter would change every mutation signature and every caller,
which is a separate task and is not in this sprint.

- [x] **Step 2: Run test to verify it fails**

Run: `cargo test -p thalassaops --test incident_repository concurrent_appends -- --nocapture`
Expected: FAIL with a `UNIQUE constraint failed: incident_timeline_event.incident_id, incident_timeline_event.sequence` error surfaced as a database error.

- [x] **Step 3: Move allocation inside the transaction**

In `repository.rs`, inside `apply_mutation` after the transaction is opened with `TransactionBehavior::Immediate` and after the existing version recheck, recompute the base sequence and renumber the mutation's events:

```rust
let allocated_base = highest_event_sequence_in(&transaction, incident_id)?
    .checked_add(1)
    .ok_or_else(|| invalid("incident timeline sequence exceeds the stored integer range"))?;

for (offset, event) in mutation.events.iter_mut().enumerate() {
    let offset = u64::try_from(offset)
        .map_err(|_| invalid("mutation event count exceeds the stored integer range"))?;
    event.sequence = allocated_base
        .checked_add(offset)
        .ok_or_else(|| invalid("incident timeline sequence exceeds the stored integer range"))?;
}
```

- [x] **Step 4: Add the bounded retry and the new error**

```rust
#[derive(Debug, thiserror::Error)]
pub enum IncidentStoreError {
    // ... existing variants ...
    #[error("the incident timeline is under write contention")]
    WriteContention,
}

const SEQUENCE_RETRY_BUDGET: usize = 3;
```

Wrap the transaction body so a unique-constraint violation on
`incident_timeline_event.sequence` retries the whole transaction, at most
`SEQUENCE_RETRY_BUDGET` times, then returns `IncidentStoreError::WriteContention`.
Any other error propagates unchanged on the first occurrence.

- [x] **Step 5: Stop allocating outside the transaction**

In `service.rs`, `load_for_write` keeps the version check and stops calling
`highest_event_sequence`. It returns only the incident:

```rust
fn load_for_write(
    &self,
    context: &IncidentCommandContext,
    incident_id: IncidentId,
    expected_version: u64,
) -> Result<Incident, IncidentServiceError> {
    if context.request_id.is_nil() || context.actor_id.is_nil() {
        return Err(IncidentServiceError::InvalidRequest);
    }
    let workspace_id = self.workspace(context)?;
    let incident = self.repository.get(workspace_id, incident_id)?;
    if incident.version != expected_version {
        return Err(IncidentServiceError::VersionConflict {
            expected: expected_version,
            actual: incident.version,
        });
    }
    Ok(incident)
}
```

Every caller passes `1` as `first_event_sequence` to the aggregate; the
repository assigns the real value. Update the four mutation methods
(`transition`, `set_severity`, `set_disposition`, `assign_role`) accordingly.

Map the store error in `IncidentServiceError`:

```rust
IncidentStoreError::WriteContention => Self::WriteContention,
```

- [x] **Step 6: Run tests to verify they pass**

Run: `cargo test -p thalassaops --test incident_repository --test incident_mutations --test incident_acceptance 2>&1 | tail -30`
Expected: PASS, including every test that existed before this task.

- [x] **Step 7: Add the contention-exhausted test**

Forcing SQLite to collide three times in a row is not worth a fault-injection
seam in production code. Extract the retry as a pure helper in
`repository.rs` and unit-test it directly:

```rust
pub(crate) fn with_sequence_retry<T>(
    budget: usize,
    mut attempt: impl FnMut(usize) -> Result<T, IncidentStoreError>,
) -> Result<T, IncidentStoreError> {
    for tries in 0..budget {
        match attempt(tries) {
            Err(IncidentStoreError::SequenceCollision) => continue,
            other => return other,
        }
    }
    Err(IncidentStoreError::WriteContention)
}
```

`SequenceCollision` is an internal variant that never escapes the repository;
`with_sequence_retry` is the only place that converts it to `WriteContention`.

```rust
#[test]
fn exhausted_sequence_retries_report_write_contention_not_version_conflict() {
    let mut attempts = 0;
    let outcome: Result<(), IncidentStoreError> =
        with_sequence_retry(SEQUENCE_RETRY_BUDGET, |_| {
            attempts += 1;
            Err(IncidentStoreError::SequenceCollision)
        });

    assert_eq!(attempts, SEQUENCE_RETRY_BUDGET, "the budget is respected exactly");
    assert!(
        matches!(outcome, Err(IncidentStoreError::WriteContention)),
        "contention must not be reported as a version conflict: {outcome:?}"
    );
}

#[test]
fn a_succeeding_attempt_stops_retrying() {
    let mut attempts = 0;
    let outcome = with_sequence_retry(SEQUENCE_RETRY_BUDGET, |tries| {
        attempts += 1;
        if tries == 0 { Err(IncidentStoreError::SequenceCollision) } else { Ok(7) }
    });
    assert_eq!(attempts, 2);
    assert!(matches!(outcome, Ok(7)));
}
```

The genuine two-writer concurrency test — a comment racing a status transition —
is deferred to Task 3, where a version-free write finally exists to race with.

- [x] **Step 8: Run the full Rust gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test 2>&1 | tail -20`
Expected: all green, test count at or above the pre-task count.

- [x] **Step 9: Commit**

```bash
git add src-tauri/src/incident/repository.rs src-tauri/src/incident/service.rs src-tauri/tests/incident_repository.rs
git commit -m "fix(incident): allocate timeline sequences inside the write transaction"
```

---

### Task 2: Add the Commented Event Kind and add_comment

**Files:**
- Modify: `crates/thalassa-domain/src/lib.rs` (`IncidentEventKind` at line 875, `IncidentTimelinePayload` at line 908, `IncidentError` at line 590, and the aggregate methods before `ensure_version`)
- Modify: `src-tauri/src/incident/repository.rs` (`event_kind_wire`, `parse_event_kind`) — both are exhaustive matches, so the new variant breaks the build without them
- Modify: `src-tauri/src/app/incident.rs` (`incident_domain_reason`) — likewise exhaustive over `IncidentError`
- Test: `crates/thalassa-domain/tests/incident_lifecycle.rs`, `crates/thalassa-domain/tests/incident_contracts.rs`

**Interfaces:**
- Consumes: `IncidentStoreError::WriteContention` from Task 1 (indirectly; the domain crate does not reference it).
- Produces:
  - `IncidentEventKind::Commented` with serde rename `"commented"`, wire string `"commented"` in both repository mappings.
  - `CommentedPayload { pub body: String }` behind `IncidentTimelinePayload::Commented`, renamed `"commented"`.
  - `IncidentError::InvalidComment`, surfaced by IPC as the reason `"incident_invalid_comment"`.
  - `Incident::add_comment(&self, first_event_sequence: u64, body: &str, actor_id: PrincipalId, request_id: Uuid, policy_version: u64, now: DateTime<Utc>) -> Result<IncidentMutation, IncidentError>` — note there is **no** `expected_version` parameter.

- [x] **Step 1: Write the failing tests**

`crates/thalassa-domain/tests/incident_lifecycle.rs` has no `investigating_incident()`
fixture — `investigating()` there returns an `IncidentTransition`, not an
aggregate. Build the aggregate from the helpers that do exist, `created()` and
`transition(&incident, first_event_sequence, step)`; creation consumes sequences
1 and 2, so triage starts at 3 and the comment under test at 5.

```rust
fn investigating_incident() -> thalassa_domain::Incident {
    let triaged = transition(&created(), 3, triage()).unwrap().incident;
    transition(&triaged, 4, investigating()).unwrap().incident
}
```

Three tests follow: one asserting a single attributed `Commented` event with the
body in its payload and an unchanged `version`, `status`, `derived_severity` and
`roles` (`Incident` has no `severity` field — the derived value is
`derived_severity`); one walking empty, blank, oversized, control-bearing and
sensitive-marker bodies to `IncidentError::InvalidComment` while a body of
exactly `INCIDENT_NOTE_MAXIMUM` characters is accepted; and one asserting
`IncidentError::InvalidId` for a nil actor or request and
`IncidentError::InvalidEventSequence` for a zero sequence.

Add `IncidentTimelinePayload` and `INCIDENT_NOTE_MAXIMUM` to the file's `use`
list.

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p thalassa-domain --test incident_lifecycle --test incident_contracts 2>&1 | tail -20`
Expected: FAIL with "no method named `add_comment`" and no `Commented` variant.

- [x] **Step 3: Add the enum variants**

In `crates/thalassa-domain/src/lib.rs`, extend both enums and add the payload:

```rust
pub enum IncidentEventKind {
    // ... existing variants ...
    #[serde(rename = "commented")]
    Commented,
}

pub enum IncidentTimelinePayload {
    // ... existing variants ...
    #[serde(rename = "commented")]
    Commented(CommentedPayload),
}

/// Free text a responder attached to the incident timeline.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommentedPayload {
    pub body: String,
}
```

- [x] **Step 4: Implement add_comment**

```rust
/// Appends one immutable responder comment.  A comment changes no incident
/// state, so it deliberately takes no `expected_version` and does not
/// advance the version; see the Sprint 16 design, section 7.5.
pub fn add_comment(
    &self,
    first_event_sequence: u64,
    body: &str,
    actor_id: PrincipalId,
    request_id: Uuid,
    policy_version: u64,
    now: DateTime<Utc>,
) -> Result<IncidentMutation, IncidentError> {
    if first_event_sequence == 0 {
        return Err(IncidentError::InvalidEventSequence);
    }
    ensure_id(actor_id)?;
    ensure_id(request_id)?;
    validate_incident_text(body, INCIDENT_NOTE_MAXIMUM)
        .map_err(|_| IncidentError::InvalidComment)?;

    let mut next = self.clone();
    next.updated_at = now;

    let pending = PendingEvent {
        kind: IncidentEventKind::Commented,
        reason: None,
        payload: IncidentTimelinePayload::Commented(CommentedPayload {
            body: body.to_owned(),
        }),
    };
    let events = materialize_events(
        self.id,
        first_event_sequence,
        vec![pending],
        actor_id,
        request_id,
        policy_version,
        now,
    )?;
    Ok(IncidentMutation {
        incident: next,
        events,
    })
}
```

The event builder is `materialize_events`, a free function taking the incident
id first — there is no `build_events` method.

Add `IncidentError::InvalidComment` to the error enum with the message
`"the comment body is empty, too long or unsafe"`.

- [x] **Step 5: Close the exhaustive matches outside the domain crate**

The new variants break three `match` arms that the rest of the workspace relies
on. Without these the Task 2 gate cannot compile, so they belong here and not in
Task 3:

- `src-tauri/src/incident/repository.rs`, `event_kind_wire`: `IncidentEventKind::Commented => "commented"`.
- `src-tauri/src/incident/repository.rs`, `parse_event_kind`: `"commented" => Ok(IncidentEventKind::Commented)`.
- `src-tauri/src/app/incident.rs`, `incident_domain_reason`: `IncidentError::InvalidComment => "incident_invalid_comment"`.

`IncidentTimelinePayload` is only ever serialized as JSON into the payload
column, so it needs no mapping arm.

- [x] **Step 6: Add the wire-stability test**

`IncidentTimelinePayload` is adjacently tagged — `#[serde(tag = "kind", content = "data")]`
— so the body lives under `data`, not at the top level. Add to
`crates/thalassa-domain/tests/incident_contracts.rs`:

```rust
#[test]
fn commented_event_wire_names_are_stable() {
    let payload = thalassa_domain::IncidentTimelinePayload::Commented(
        thalassa_domain::CommentedPayload { body: "note".into() },
    );
    let encoded = serde_json::to_value(&payload).expect("payload encodes");
    assert_eq!(encoded["kind"], json!("commented"));
    assert_eq!(encoded["data"]["body"], json!("note"));
    assert_eq!(
        serde_json::from_value::<thalassa_domain::IncidentTimelinePayload>(encoded).unwrap(),
        payload
    );

    assert_eq!(
        serde_json::to_value(thalassa_domain::IncidentEventKind::Commented).expect("kind encodes"),
        json!("commented")
    );
    assert_eq!(
        serde_json::from_value::<thalassa_domain::IncidentEventKind>(json!("commented")).unwrap(),
        thalassa_domain::IncidentEventKind::Commented
    );
}
```

- [x] **Step 7: Run tests to verify they pass**

Run: `cargo test -p thalassa-domain --test incident_lifecycle --test incident_contracts 2>&1 | tail -20`
Expected: PASS.

- [x] **Step 8: Run the full Rust gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test 2>&1 | tail -20`
Expected: all green.

- [x] **Step 9: Commit**

```bash
git add crates/thalassa-domain/src/lib.rs crates/thalassa-domain/tests/incident_lifecycle.rs \
        crates/thalassa-domain/tests/incident_contracts.rs \
        src-tauri/src/incident/repository.rs src-tauri/src/app/incident.rs
git commit -m "feat(incident): add the commented timeline event kind"
```

---

### Task 3: Expose incident.add_comment Through the Service and IPC

**Files:**
- Modify: `crates/thalassa-domain/src/lib.rs` (`IncidentCommentRequest`, beside `IncidentRoleRequest`)
- Modify: `crates/thalassa-ipc/src/lib.rs` (beside `incident_assign_role_descriptor`)
- Modify: `src-tauri/src/incident/repository.rs` (`append_comment`)
- Modify: `src-tauri/src/incident/service.rs`
- Modify: `src-tauri/src/app/incident.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `crates/thalassa-ipc/tests/contracts.rs`, `crates/thalassa-domain/tests/incident_contracts.rs`, `src-tauri/tests/incident_repository.rs`, `src-tauri/tests/incident_mutations.rs`, `src-tauri/tests/incident_ipc.rs`

**Interfaces:**
- Consumes: `Incident::add_comment` and `CommentedPayload` from Task 2; `IncidentStoreError::WriteContention` and `with_sequence_retry` from Task 1.
- Produces:
  - `IncidentCommentRequest { pub incident_id: IncidentId, pub body: String }` in `thalassa_domain` — no `expected_version` field.
  - `SqliteIncidentRepository::append_comment(&mut self, mutation) -> Result<IncidentMutation, IncidentStoreError>` — the version-free write path.
  - `incident_add_comment_descriptor() -> CommandDescriptor` with name `incident.add_comment`, `Capability::IncidentWrite`, `Permission::ManageIncident`.
  - `IncidentService::add_comment(&mut self, context, request) -> Result<IncidentMutation, IncidentServiceError>`.
  - IPC reason `"incident_write_contention"` for the contention case.

**The step the first draft of this plan missed.** Its service code called
`repository.apply_mutation`, which cannot carry a comment. `apply_mutation`
derives `expected_version` as `incident.version - 1` and its `UPDATE` carries
`WHERE ... AND version = ?`. A comment leaves the version alone, so that
subtraction underflows on a fresh incident and the predicate rejects the comment
outright the moment any other write lands in between — the exact failure the
"comment writes must not read or write the `version` column" constraint exists
to prevent. Task 3 therefore starts at the repository, not the service. (The
plan's File Map always assigned "comment append" to `repository.rs`; only the
steps forgot.)

- [x] **Step 1: Write the failing repository tests**

In `src-tauri/tests/incident_repository.rs`, using the helpers already there —
`fixture()`, `creation_record()`, `triage_mutation()`, `scope_for()` and the
`CREATE_REQUEST`/`SECOND_REQUEST`/`THIRD_REQUEST` constants:

1. `a_comment_appends_after_a_racing_transition_without_writing_the_version` —
   this is the two-writer race deferred from Task 1 in c1aa5a3. Build the
   comment mutation from the aggregate as first observed, apply a triage
   mutation so the stored row is a version ahead and the timeline two events
   taller, then append the stale comment. It must succeed, its sequence must
   land after the triage events, the stored `version` and `status` must be the
   triage writer's, `updated_at` must be the comment's, and every timeline
   sequence must stay unique.
2. `replaying_a_comment_request_id_returns_the_stored_comment` — a second
   `append_comment` with the same request id returns the stored event and
   appends nothing.
3. `a_comment_on_an_incident_from_another_workspace_is_not_found` — build the
   incident in `OTHER_WORKSPACE`, point its scope at `WORKSPACE`, and assert
   `IncidentStoreError::NotFound` with the foreign timeline untouched.
4. `two_comments_allocated_from_the_same_observation_do_not_collide` — the
   second case the design's repository row requires, and the distinct one:
   build two comments from the same aggregate, both asking for the sequence
   after the same observed height. Neither carries a version predicate, so
   neither can be rejected; only the in-transaction reallocation keeps them off
   the same sequence. Both must land, at consecutive sequences.

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p thalassaops --test incident_repository 2>&1 | tail -20`
Expected: FAIL with "no method named `append_comment`".

- [x] **Step 3: Implement the version-free repository path**

`append_comment` mirrors `apply_mutation` inside `with_sequence_retry` and
`BEGIN IMMEDIATE`, minus everything a comment cannot touch:

- no `expected_version` derivation and no version predicate; the `UPDATE` is
  `SET updated_at = ?1 WHERE id = ?2 AND workspace_id = ?3`;
- no `reconcile_roles`, no duplicate-reference or role-principal validation —
  a comment changes none of them;
- the replay check, the in-transaction sequence allocation and `validate_events`
  are kept unchanged;
- the returned mutation carries the **stored** incident with `updated_at`
  moved, not the caller's copy, so a comment never hands back a stale version.

It rejects a mutation that is not exactly one event.

- [x] **Step 4: Add the request type and the service path**

`IncidentCommentRequest` goes in `crates/thalassa-domain/src/lib.rs` beside
`IncidentRoleRequest`, without `expected_version`.

```rust
pub fn add_comment(
    &mut self,
    context: &IncidentCommandContext,
    request: IncidentCommentRequest,
) -> Result<IncidentMutation, IncidentServiceError> {
    if let Some(replayed) = self.replay_if_matching(
        context,
        request.incident_id,
        SINGLE_EVENT_REPLAY_MAX_EVENTS,
        |events| comment_replay_matches(events, &request.body),
    )? {
        return Ok(replayed);
    }
    let workspace_id = self.workspace(context)?;
    let incident = self.repository.get(workspace_id, request.incident_id)?;
    let mutation = incident.add_comment(
        1,
        &request.body,
        context.actor_id,
        context.request_id,
        context.policy_version,
        context.now,
    )?;
    Ok(self.repository.append_comment(mutation)?)
}
```

`comment_replay_matches` sits beside `role_replay_matches` and follows its
shape: one event, kind `Commented`, no reason, matching body. Note the service
takes `repository.get`, not `load_for_write` — there is no version to check —
and that the sequence `1` is advisory, since Task 1 made the repository
reallocate it inside the transaction. `replay_if_matching` already rejects a nil
request or actor id, so the service needs no separate guard.

- [x] **Step 5: Write the failing service tests**

The fixture in `src-tauri/tests/incident_mutations.rs` is `Fixture`, not
`ServiceFixture`; the service is the field `fixture.service`, contexts come from
`fixture.context()` (which increments the request id on each call) and
`fixture.read_context()`, and there is no `timeline` helper — read through
`fixture.service.timeline(...)` or count rows with `fixture.persisted_counts()`.
A foreign incident is staged the way
`duplicate_disposition_rejects_an_incident_from_another_workspace` stages one:
override `context.workspace_scope` and create through the service.

Seven tests: the happy path with an unchanged version; a replayed request id;
a reused request id with different text rejected as `IdempotencyConflict`;
empty and sensitive bodies rejected with nothing written; an unknown incident
as `NotFound`; a cross-workspace incident as `NotFound` with its rows untouched;
and a transition carrying the pre-comment `expected_version` still landing after
a comment, which is what proves the comment moved the timeline but not the
version.

- [x] **Step 6: Add the descriptor**

```rust
/// Stable command descriptor for appending one responder comment.
pub fn incident_add_comment_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "add_comment",
        Capability::IncidentWrite,
        Permission::ManageIncident,
    )
}
```

Add `(incident_add_comment_descriptor(), "incident.add_comment")` to the
existing write-descriptor table in `crates/thalassa-ipc/tests/contracts.rs`,
which already asserts the capability, the permission and an unbounded scope for
every row — no separate test is needed.

- [x] **Step 7: Wire the IPC command**

In `src-tauri/src/app/incident.rs`, add `CommentPayload { incident_id, body }`
with `#[serde(deny_unknown_fields)]` beside `RolePayload`, and
`incident_add_comment` following the shape of `incident_assign_role`. Register
the command in `src-tauri/src/main.rs`.

Change the `WriteContention` mapping. Task 1 pointed it at
`incident_unavailable()`, which is `INTERNAL_ERROR` with no reason, because no
caller could reach it yet. This task makes it reachable, and design section 7.3
requires the caller to tell contention from a version conflict — they instruct
opposite recoveries.

`INVALID_REQUEST` with a distinct reason was the first attempt and is wrong:
the request is by definition still valid and may be sent again unchanged.
Contention gets its own code, added to `IpcErrorCode` and to
`ui/contracts/ipc.ts` here rather than in Task 4, because Task 4 freezes the
wire shapes:

```rust
#[serde(rename = "WRITE_CONTENTION")]
WriteContention,
```

```rust
IncidentServiceError::WriteContention {} => IpcError::new(
    IpcErrorCode::WriteContention,
    "the incident timeline is under write contention",
    serde_json::json!({ "reason": "incident_write_contention" }),
),
```

Task 12 retries on this code without reloading; `incident_version_conflict`
still forces a reload first.

The typed path also has to be reachable in practice. `with_sequence_retry`
retried only `SequenceCollision`, but SQLite reports a lost race for the write
lock as `SQLITE_BUSY`/`SQLITE_LOCKED` at `BEGIN IMMEDIATE`, long before any
sequence is allocated, so real contention escaped as a storage error. Classify
those two codes at every transaction boundary as a retryable `LockContention`,
give the connection a short `busy_timeout` so one attempt waits rather than
failing instantly, and map any that still escapes to `WriteContention`.

- [x] **Step 8: Write the failing IPC tests**

`src-tauri/tests/incident_ipc.rs` builds state with `test_state()`, creates with
`created(&state)` and wraps payloads with
`envelope("add_comment", Capability::IncidentWrite, json!({ ... }))`. Four
tests: the comment appears on the timeline with the right actor and body while
the version holds; the wrong capability, an unknown key, a missing `body` and an
empty `body` are all rejected; a viewer's comment is denied without echoing the
incident id, for both a missing and an existing incident; and a comment issued
while a separate connection holds the write lock comes back as
`WRITE_CONTENTION`, not an internal error.

The repository proofs gain the same shape: two connections over one file
rebasing onto each other, and a held write lock producing `WriteContention` for
a comment and for a transition alike. The single-connection versions stage
stale caller state, which is a different claim.

Also extend the snake_case field loop at the tail of
`crates/thalassa-domain/tests/incident_contracts.rs` to cover
`IncidentCommentRequest` and to assert it carries no `expected_version`.

- [x] **Step 9: Run tests to verify they pass**

Run: `cargo test -p thalassaops --test incident_repository --test incident_mutations --test incident_ipc && cargo test -p thalassa-ipc --test contracts 2>&1 | tail -20`
Expected: PASS.

- [x] **Step 10: Run the full Rust gate**

Run: `cargo fmt --all -- --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test 2>&1 | tail -20`
Expected: all green.

- [x] **Step 11: Commit**

```bash
git add crates/thalassa-domain crates/thalassa-ipc src-tauri/src src-tauri/tests
git commit -m "feat(incident): expose incident.add_comment over IPC"
```

---

### Task 4: TypeScript Contracts and Runtime Guards

**Files:**
- Modify: `ui/contracts/guards.ts`
- Create: `ui/src/incident/contractValidation.ts`
- Test: `ui/src/incident/incident-contracts.test.ts` (exists; extend it)

**Interfaces:**
- Consumes: the `commented` wire names from Task 2 and the `incident.add_comment` command from Task 3.
- Produces:
  - `type IncidentTimelineEvent` with a discriminated `payload` union including `{ kind: "commented"; body: string }`.
  - `isIncidentTimelineEvent(value: unknown): value is IncidentTimelineEvent`
  - `isIncidentTimelinePage(value: unknown): value is IncidentTimelinePage`
  - `isIncidentPage(value: unknown): value is IncidentPage`
  - `INCIDENT_NOTE_MAXIMUM = 4000` exported for the composer in Task 11.

- [ ] **Step 1: Write the failing test**

Add to `ui/src/incident/incident-contracts.test.ts`:

```ts
it("accepts a commented timeline event and rejects a malformed one", () => {
  const event = {
    id: "6f1c1b0e-0000-4000-8000-000000000001",
    incident_id: "6f1c1b0e-0000-4000-8000-000000000002",
    sequence: 4,
    kind: "commented",
    actor_id: "6f1c1b0e-0000-4000-8000-000000000003",
    reason: null,
    occurred_at: "2026-08-28T09:00:00Z",
    request_id: "6f1c1b0e-0000-4000-8000-000000000004",
    policy_version: 7,
    payload: { kind: "commented", body: "checked the dashboards" }
  };

  expect(isIncidentTimelineEvent(event)).toBe(true);
  expect(isIncidentTimelineEvent({ ...event, payload: { kind: "commented" } })).toBe(false);
  expect(isIncidentTimelineEvent({ ...event, payload: { kind: "commented", body: 4 } })).toBe(false);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/incident-contracts.test.ts`
Expected: FAIL with "isIncidentTimelineEvent is not a function".

- [ ] **Step 3: Implement the guards**

In `ui/src/incident/contractValidation.ts`, follow the shape of
`ui/src/topology/contractValidation.ts`:

```ts
export const INCIDENT_NOTE_MAXIMUM = 4000;

export type IncidentTimelinePayload =
  | { kind: "created"; summary: string }
  | { kind: "triggers_attached" }
  | { kind: "status_transitioned" }
  | { kind: "severity_changed" }
  | { kind: "disposition_changed" }
  | { kind: "role_changed" }
  | { kind: "commented"; body: string };

export function isIncidentTimelineEvent(value: unknown): value is IncidentTimelineEvent {
  if (!isRecord(value)) return false;
  if (!isUuid(value.id) || !isUuid(value.incident_id) || !isUuid(value.actor_id)) return false;
  if (!isPositiveInteger(value.sequence)) return false;
  if (!isTimestamp(value.occurred_at)) return false;
  if (value.reason !== null && !isBoundedText(value.reason, INCIDENT_NOTE_MAXIMUM)) return false;
  return isIncidentTimelinePayload(value.payload);
}

function isIncidentTimelinePayload(value: unknown): value is IncidentTimelinePayload {
  if (!isRecord(value) || typeof value.kind !== "string") return false;
  if (value.kind === "commented") {
    return isBoundedText(value.body, INCIDENT_NOTE_MAXIMUM);
  }
  return [
    "created",
    "triggers_attached",
    "status_transitioned",
    "severity_changed",
    "disposition_changed",
    "role_changed"
  ].includes(value.kind);
}
```

Add `isIncidentTimelinePage` and `isIncidentPage` in the same file, each
validating the item array with the element guard and the cursor field
(`next_cursor: string | null`, `next_sequence: number | null`).

- [ ] **Step 4: Run test to verify it passes**

Run: `npm test -- ui/src/incident/incident-contracts.test.ts`
Expected: PASS.

- [ ] **Step 5: Run the UI gate and commit**

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/contracts/guards.ts ui/src/incident/contractValidation.ts ui/src/incident/incident-contracts.test.ts
git commit -m "feat(incident): add TypeScript guards for incident timeline payloads"
```

---

### Task 5: Locale Key Parity Test

This lands before the components so every later task is forced to add both
languages. `en.ts` and `th.ts` currently hold 801 keys each with no drift, so
this test passes on existing content the moment it is written.

**Files:**
- Create: `ui/src/locales/locales.test.ts`
- Modify: `ui/src/locales/en.ts`, `ui/src/locales/th.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: an `incident` namespace in both locale files that later tasks extend.

- [ ] **Step 1: Write the test**

```ts
import { describe, expect, it } from "vitest";
import en from "./en";
import th from "./th";

function keyPaths(value: unknown, prefix = ""): string[] {
  if (typeof value !== "object" || value === null) return [prefix];
  return Object.entries(value).flatMap(([key, child]) =>
    keyPaths(child, prefix ? `${prefix}.${key}` : key)
  );
}

describe("locale parity", () => {
  it("defines exactly the same key paths in en and th", () => {
    const enKeys = keyPaths(en).sort();
    const thKeys = keyPaths(th).sort();
    expect(thKeys.filter((key) => !enKeys.includes(key))).toEqual([]);
    expect(enKeys.filter((key) => !thKeys.includes(key))).toEqual([]);
  });
});
```

- [ ] **Step 2: Run it to confirm the existing files already pass**

Run: `npm test -- ui/src/locales/locales.test.ts`
Expected: PASS. If it fails, the drift is pre-existing — fix the missing keys in
this task before continuing, and say so in the commit body.

- [ ] **Step 3: Add the incident namespace to both files**

In `en.ts`:

```ts
  incident: {
    queueTitle: "Incidents",
    detailTitle: "Incident",
    emptyQueue: "No incidents match this filter",
    loading: "Loading…"
  },
```

In `th.ts`, the same key paths with Thai values:

```ts
  incident: {
    queueTitle: "เหตุการณ์",
    detailTitle: "รายละเอียดเหตุการณ์",
    emptyQueue: "ไม่มีเหตุการณ์ที่ตรงกับตัวกรองนี้",
    loading: "กำลังโหลด…"
  },
```

- [ ] **Step 4: Run the UI gate and commit**

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/locales
git commit -m "test(ui): assert en and th locale key parity"
```

---

### Task 6: Incident Data Hooks

**Files:**
- Create: `ui/src/incident/useIncidentList.ts`, `ui/src/incident/useIncidentTimeline.ts`
- Create: `ui/src/incident/incident-fixtures.ts`
- Test: `ui/src/incident/useIncidentList.test.ts`, `ui/src/incident/useIncidentTimeline.test.ts`

**Interfaces:**
- Consumes: `isIncidentPage`, `isIncidentTimelinePage` from Task 4.
- Produces:
  - `useIncidentList(invoke: Invoke): { incidents: IncidentSummary[]; loading: boolean; error: string | null; loadMore: () => void; hasMore: boolean; reload: () => void }`
  - `useIncidentTimeline(invoke: Invoke, incidentId: string | null): { events: IncidentTimelineEvent[]; loading: boolean; error: string | null; loadMore: () => void; hasMore: boolean; reload: () => void }`
  - `incidentFixturePage`, `incidentFixtureTimeline` — fixtures dated `2026-08-28`.

- [ ] **Step 1: Write the fixtures and assert they are non-empty**

In `incident-fixtures.ts`, build one page of three incidents and one timeline of
six events, all timestamped on `2026-08-28`. Add this test first, in
`useIncidentList.test.ts`:

```ts
it("ships a non-empty fixture page", () => {
  expect(incidentFixturePage.items.length).toBeGreaterThan(0);
  expect(incidentFixtureTimeline.events.length).toBeGreaterThan(0);
});
```

- [ ] **Step 2: Write the failing hook test**

```ts
it("pages the incident list with the returned cursor", async () => {
  const invoke = vi.fn<Invoke>()
    .mockResolvedValueOnce({ ok: true, value: { items: incidentFixturePage.items, next_cursor: "c2" } })
    .mockResolvedValueOnce({ ok: true, value: { items: [], next_cursor: null } });

  const { result } = renderHook(() => useIncidentList(invoke));
  await waitFor(() => expect(result.current.loading).toBe(false));
  expect(result.current.hasMore).toBe(true);

  act(() => result.current.loadMore());
  await waitFor(() => expect(result.current.hasMore).toBe(false));

  expect(invoke).toHaveBeenNthCalledWith(2, expect.objectContaining({
    name: "incident.list",
    payload: expect.objectContaining({ cursor: "c2" })
  }));
});

it("reports a guard failure as an error rather than rendering unvalidated data", async () => {
  const invoke = vi.fn<Invoke>().mockResolvedValue({ ok: true, value: { items: [{ bogus: true }], next_cursor: null } });
  const { result } = renderHook(() => useIncidentList(invoke));
  await waitFor(() => expect(result.current.error).not.toBeNull());
  expect(result.current.incidents).toEqual([]);
});
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `npm test -- ui/src/incident/useIncidentList.test.ts`
Expected: FAIL with "useIncidentList is not a function".

- [ ] **Step 4: Implement both hooks**

Each hook keeps `items`, `cursor`, `loading` and `error` in state, calls the
command through `invoke`, runs the Task 4 guard on the response, and appends on
`loadMore`. A guard failure sets `error` and leaves `items` untouched. Neither
hook renders anything; neither is used outside the shell.

`useIncidentTimeline` returns immediately with empty state when `incidentId` is
`null`, and refetches from scratch when it changes.

- [ ] **Step 5: Run tests to verify they pass**

Run: `npm test -- ui/src/incident/useIncidentList.test.ts ui/src/incident/useIncidentTimeline.test.ts`
Expected: PASS.

- [ ] **Step 6: Run the UI gate and commit**

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident
git commit -m "feat(incident): add incident list and timeline data hooks"
```

---

### Task 7: Workspace Shell and Incident List

**Files:**
- Create: `ui/src/incident/IncidentWorkspace.tsx`, `ui/src/incident/IncidentList.tsx`, `ui/src/incident/incident.css`
- Test: `ui/src/incident/IncidentWorkspace.test.tsx`, `ui/src/incident/IncidentList.test.tsx`

**Interfaces:**
- Consumes: `useIncidentList`, `useIncidentTimeline` from Task 6.
- Produces:
  - `IncidentWorkspace({ invoke }: { invoke: Invoke })`
  - `IncidentList({ incidents, selectedId, onSelect, filter, onFilterChange })` — pure, no IPC.
  - The shell passes `incident`, `events`, and callbacks down to the panels added by Tasks 8-13.

- [ ] **Step 1: Write the failing list test**

```tsx
it("renders severity and priority as separate fields", () => {
  render(
    <I18nProvider i18n={i18n}>
      <IncidentList
        incidents={incidentFixturePage.items}
        selectedId={null}
        onSelect={() => {}}
        filter={{ status: "all" }}
        onFilterChange={() => {}}
      />
    </I18nProvider>
  );
  const row = screen.getByRole("option", { name: /checkout/i });
  expect(within(row).getByTestId("incident-severity")).toHaveTextContent("S1");
  expect(within(row).getByTestId("incident-priority")).toHaveTextContent("P1");
});

it("calls onSelect with the incident id when a row is chosen", async () => {
  const onSelect = vi.fn();
  render(/* same tree with onSelect */);
  await userEvent.click(screen.getByRole("option", { name: /checkout/i }));
  expect(onSelect).toHaveBeenCalledWith(incidentFixturePage.items[0].id);
});
```

`incident-severity` and `incident-priority` must be distinct elements. The spec
forbids using one as a label for the other.

- [ ] **Step 2: Run tests to verify they fail**

Run: `npm test -- ui/src/incident/IncidentList.test.tsx`
Expected: FAIL with "IncidentList is not defined".

- [ ] **Step 3: Implement IncidentList as a pure component**

It receives arrays and callbacks only. It imports no hook from Task 6 and calls
no `invoke`.

- [ ] **Step 4: Write the failing shell test**

```tsx
it("selects the first incident and loads its timeline", async () => {
  const invoke = incidentInvokeMock();
  render(<I18nProvider i18n={i18n}><IncidentWorkspace invoke={invoke} /></I18nProvider>);
  await waitFor(() => expect(screen.getByRole("option", { selected: true })).toBeInTheDocument());
  expect(invoke).toHaveBeenCalledWith(expect.objectContaining({ name: "incident.timeline" }));
});
```

- [ ] **Step 5: Implement the shell**

The shell wires the two hooks, holds `selectedId` and the queue filter, and
renders `IncidentList` plus a detail region that Tasks 8-13 fill. It is the only
component in the module that receives `invoke`.

- [ ] **Step 6: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): add the incident workspace shell and queue"
```

---

### Task 8: Incident Narrative

**Files:**
- Create: `ui/src/incident/IncidentNarrative.tsx`, `ui/src/incident/IncidentNarrative.test.tsx`

**Interfaces:**
- Consumes: `IncidentTimelineEvent` from Task 4, rendered by the shell from Task 7.
- Produces: `IncidentNarrative({ events }: { events: IncidentTimelineEvent[] })` — pure.

- [ ] **Step 1: Write the failing test**

```tsx
it("renders lifecycle events as a record and excludes comments", () => {
  render(
    <I18nProvider i18n={i18n}>
      <IncidentNarrative events={incidentFixtureTimeline.events} />
    </I18nProvider>
  );
  expect(screen.getByText(/investigating/i)).toBeInTheDocument();
  expect(screen.queryByText(/checked the dashboards/i)).not.toBeInTheDocument();
});

it("renders each row with a timestamp, actor, change and reason column", () => {
  render(/* same tree */);
  const row = screen.getAllByRole("row")[1];
  expect(within(row).getAllByRole("cell")).toHaveLength(4);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/IncidentNarrative.test.tsx`
Expected: FAIL with "IncidentNarrative is not defined".

- [ ] **Step 3: Implement**

Filter `events` to the six lifecycle kinds, drop `commented`, and render a table
with timestamp, actor, what changed and reason. Do not compose sentences; the
spec fixes this as a formatted record for translation and Sprint 19 reasons.

- [ ] **Step 4: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident/IncidentNarrative.test.tsx
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): render the deterministic incident narrative"
```

---

### Task 9: Evidence Resolution and Panel

**Files:**
- Create: `ui/src/incident/incidentEvidence.ts`, `ui/src/incident/IncidentEvidencePanel.tsx`
- Test: `ui/src/incident/incidentEvidence.test.ts`, `ui/src/incident/IncidentEvidencePanel.test.tsx`

**Interfaces:**
- Consumes: `Invoke` from the shell.
- Produces:
  - `type EvidenceState = { status: "loading" } | { status: "empty" } | { status: "unavailable"; cause: "missing" | "scope" | "unverified" | "unknown" } | { status: "ready"; evidence: EvidenceRef[] }`
  - `resolveEvidence(invoke: Invoke, command: string, ids: string[]): Promise<EvidenceState>`

- [ ] **Step 1: Write the failing test**

```ts
it("returns empty without issuing a command when there are no ids", async () => {
  const invoke = vi.fn<Invoke>();
  await expect(resolveEvidence(invoke, "operations.evidence", [])).resolves.toEqual({ status: "empty" });
  expect(invoke).not.toHaveBeenCalled();
});

it("de-duplicates ids before requesting them", async () => {
  const invoke = vi.fn<Invoke>().mockResolvedValue({ ok: true, value: [] });
  await resolveEvidence(invoke, "operations.evidence", ["a", "a", "b"]);
  expect(invoke).toHaveBeenCalledWith(
    expect.objectContaining({ payload: { evidence_ids: ["a", "b"] } })
  );
});

it("maps each failure code to a distinct cause", async () => {
  for (const [code, cause] of [
    ["evidence_unknown_id", "missing"],
    ["evidence_cross_scope", "scope"],
    ["evidence_unverified", "unverified"]
  ] as const) {
    const invoke = vi.fn<Invoke>().mockResolvedValue({ ok: false, error: { code } });
    await expect(resolveEvidence(invoke, "operations.evidence", ["a"])).resolves.toEqual({
      status: "unavailable",
      cause
    });
  }
});
```

The empty-list and duplicate rules are not stylistic: `EmptyRequest` and
`DuplicateId` are hard errors in the Rust evidence store and would make the tab
permanently unavailable.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/incidentEvidence.test.ts`
Expected: FAIL with "resolveEvidence is not a function".

- [ ] **Step 3: Implement resolveEvidence and the panel**

`resolveEvidence` short-circuits on an empty list, de-duplicates while preserving
order, calls the command, and maps error codes to causes. `IncidentEvidencePanel`
is pure: it takes an `EvidenceState` and renders one of the four states with a
distinct message per cause.

- [ ] **Step 4: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): resolve incident evidence with explicit failure states"
```

---

### Task 10: Association Tabs

**Files:**
- Create: `ui/src/incident/incidentTabConfig.ts`, `ui/src/incident/IncidentTabs.tsx`
- Test: `ui/src/incident/IncidentTabs.test.tsx`

**Interfaces:**
- Consumes: `resolveEvidence` and `EvidenceState` from Task 9.
- Produces: `INCIDENT_TABS: IncidentTab[]` and `IncidentTabs({ incident, states, activeId, onSelect })`.

- [ ] **Step 1: Write the failing test**

```tsx
it("reads the association set on every render rather than memoising it", () => {
  const { rerender } = render(<IncidentTabs incident={incidentWithNoActions} {...rest} />);
  expect(screen.getByRole("tab", { name: /vulnerabilit/i })).toHaveAttribute("aria-disabled", "true");

  rerender(<IncidentTabs incident={incidentWithVulnerabilityEvidence} {...rest} />);
  expect(screen.getByRole("tab", { name: /vulnerabilit/i })).toHaveAttribute("aria-disabled", "false");
});

it("distinguishes an empty tab from an unavailable one", () => {
  render(<IncidentTabs {...rest} states={{ alerts: { status: "empty" }, topology: { status: "unavailable", cause: "missing" } }} />);
  expect(screen.getByTestId("tab-alerts-empty")).toBeInTheDocument();
  expect(screen.getByTestId("tab-topology-unavailable")).toBeInTheDocument();
});
```

The first test is the guard required by the spec: Sprints 19 and 21 add
identifiers to open incidents, so a registry that captured the set at mount would
silently stop updating.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/IncidentTabs.test.tsx`
Expected: FAIL with "IncidentTabs is not defined".

- [ ] **Step 3: Implement the registry**

```ts
export type IncidentTab = {
  id: "alerts" | "topology" | "changes" | "vulnerabilities";
  labelKey: string;
  select: (incident: IncidentDetail) => string[];
  isEmpty: (ids: string[]) => boolean;
};

export const INCIDENT_TABS: IncidentTab[] = [
  { id: "alerts", labelKey: "incident.tabs.alerts", select: (i) => i.signal_ids, isEmpty: (ids) => ids.length === 0 },
  { id: "topology", labelKey: "incident.tabs.topology", select: (i) => i.evidence_ids, isEmpty: (ids) => ids.length === 0 },
  { id: "changes", labelKey: "incident.tabs.changes", select: (i) => i.evidence_ids, isEmpty: (ids) => ids.length === 0 },
  {
    id: "vulnerabilities",
    labelKey: "incident.tabs.vulnerabilities",
    select: (i) => i.triggers.filter((t) => t.source_kind === "vulnerability_finding").flatMap((t) => t.evidence_ids),
    isEmpty: (ids) => ids.length === 0
  }
];
```

`select` is called during render. Do not wrap it in `useMemo` keyed on anything
but the incident itself. Adding a fifth tab must require a new array entry and
nothing else.

- [ ] **Step 4: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): add the association tab registry"
```

---

### Task 11: Comment Thread

**Files:**
- Create: `ui/src/incident/IncidentCommentThread.tsx`, `ui/src/incident/IncidentCommentThread.test.tsx`

**Interfaces:**
- Consumes: `INCIDENT_NOTE_MAXIMUM` from Task 4; the shell supplies `events` and an `onSubmit` that calls `incident.add_comment`.
- Produces: `IncidentCommentThread({ events, onSubmit, submitting })` — pure.

- [ ] **Step 1: Write the failing test**

```tsx
it("shows only commented events, oldest first", () => {
  render(<I18nProvider i18n={i18n}><IncidentCommentThread events={incidentFixtureTimeline.events} onSubmit={() => {}} submitting={false} /></I18nProvider>);
  const items = screen.getAllByRole("listitem");
  expect(items).toHaveLength(2);
  expect(items[0]).toHaveTextContent("checked the dashboards");
  expect(screen.queryByText(/investigating/i)).not.toBeInTheDocument();
});

it("blocks an empty or oversized body before calling onSubmit", async () => {
  const onSubmit = vi.fn();
  render(/* same tree with onSubmit */);
  const send = screen.getByRole("button", { name: /comment/i });

  await userEvent.click(send);
  expect(onSubmit).not.toHaveBeenCalled();

  await userEvent.type(screen.getByRole("textbox"), "x".repeat(INCIDENT_NOTE_MAXIMUM + 1));
  await userEvent.click(send);
  expect(onSubmit).not.toHaveBeenCalled();
});

it("renders a submitted comment optimistically", async () => {
  const onSubmit = vi.fn().mockResolvedValue(undefined);
  render(/* same tree */);
  await userEvent.type(screen.getByRole("textbox"), "paged the on-call");
  await userEvent.click(screen.getByRole("button", { name: /comment/i }));
  expect(screen.getByText("paged the on-call")).toBeInTheDocument();
});
```

Comments are optimistic because they carry no version and only append. The
version-carrying mutations in Task 12 are not.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/IncidentCommentThread.test.tsx`
Expected: FAIL with "IncidentCommentThread is not defined".

- [ ] **Step 3: Implement**

Filter to `payload.kind === "commented"`, sort by `sequence`, render the
composer with the length bound enforced before `onSubmit` fires.

- [ ] **Step 4: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): add the incident comment thread"
```

---

### Task 12: Actions and Version-Conflict Recovery

**Files:**
- Create: `ui/src/incident/IncidentActions.tsx`, `ui/src/incident/IncidentActions.test.tsx`
- Modify: `ui/src/incident/IncidentWorkspace.tsx`

**Interfaces:**
- Consumes: the shell's `invoke`, and the incident's `version`.
- Produces: `IncidentActions({ incident, onTransition, onSeverity, onAssign, pending, conflict })` where `conflict` is `{ actor: string; at: string } | null`.

- [ ] **Step 1: Write the failing test**

```tsx
it("does not render a status change until the command resolves", async () => {
  let resolve: (value: unknown) => void = () => {};
  const onTransition = vi.fn(() => new Promise((r) => { resolve = r; }));
  render(/* actions with status "triage" */);

  await userEvent.click(screen.getByRole("button", { name: /investigating/i }));
  expect(screen.getByTestId("incident-status")).toHaveTextContent("triage");

  await act(async () => { resolve({ ok: true }); });
  await waitFor(() => expect(screen.getByTestId("incident-status")).toHaveTextContent("investigating"));
});

it("reports a version conflict, names the actor, and does not resubmit", async () => {
  const onTransition = vi.fn().mockResolvedValue({ ok: false, error: { code: "incident_version_conflict" } });
  render(/* actions with conflict wiring */);
  await userEvent.click(screen.getByRole("button", { name: /investigating/i }));

  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent(/changed by/i));
  expect(screen.getByRole("alert")).toHaveTextContent(/not applied/i);
  expect(onTransition).toHaveBeenCalledTimes(1);
});
```

The last assertion is load-bearing: the spec forbids automatic resubmission
because the responder's intent may no longer hold once the status has moved.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/IncidentActions.test.tsx`
Expected: FAIL with "IncidentActions is not defined".

- [ ] **Step 3: Implement**

Render controls disabled while `pending`. On `incident_version_conflict`, the
shell reloads the incident and passes `conflict`; the component renders an alert
naming the actor and time and states the command was not applied. It offers a
retry button that the responder must press.

- [ ] **Step 4: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): add incident actions with explicit conflict recovery"
```

---

### Task 13: Incident Summary Card

**Files:**
- Create: `ui/src/incident/IncidentSummaryCard.tsx`, `ui/src/incident/IncidentSummaryCard.test.tsx`

**Interfaces:**
- Consumes: the selected incident from the shell.
- Produces: `IncidentSummaryCard({ incident, onCopy })` and `buildSummaryMarkdown(incident): string`.

- [ ] **Step 1: Write the failing test**

```ts
const incident = incidentWithEvidenceAndComments;

it("copies only the allowlisted fields", () => {
  const markdown = buildSummaryMarkdown(incident);

  for (const allowed of [incident.id, incident.summary, "S1", "investigating"]) {
    expect(markdown).toContain(allowed);
  }
  for (const forbidden of ["checked the dashboards", "AKIA", "log excerpt", "Incident Commander"]) {
    expect(markdown).not.toContain(forbidden);
  }
});

it("is named the Incident Summary Card, not the Incident Card", () => {
  render(<I18nProvider i18n={i18n}><IncidentSummaryCard incident={incident} onCopy={() => {}} /></I18nProvider>);
  expect(screen.getByRole("heading")).toHaveTextContent(/summary card/i);
});
```

`incidentWithEvidenceAndComments` is a fixture added in Task 6 and extended
here: it carries at least one evidence reference whose excerpt contains the
literal `AKIA`, one comment body `checked the dashboards`, and an
`Incident Commander` role assignment, so the forbidden list actually has
something to catch.

```ts
```

The forbidden list is the spec's allowlist inverted: evidence excerpts, comment
bodies, trigger payloads, role assignments and timeline reasons never leave.

- [ ] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/IncidentSummaryCard.test.tsx`
Expected: FAIL with "buildSummaryMarkdown is not a function".

- [ ] **Step 3: Implement**

`buildSummaryMarkdown` reads exactly: id, summary, severity, derived severity,
status, disposition, created and updated timestamps. It must be written as an
explicit field list, never by serialising the incident object, so a new field
added later is excluded by default rather than leaked by default.

- [ ] **Step 4: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): add the incident summary card with a copy allowlist"
```

---

### Task 14: End-to-End Acceptance

**Files:**
- Create: `ui/src/incident/incident.acceptance.test.tsx`

**Interfaces:**
- Consumes: every component from Tasks 7-13.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the acceptance test**

```tsx
it("lets a responder work one incident from triage to resolved without leaving the workspace", async () => {
  const invoke = incidentInvokeMock();
  render(<I18nProvider i18n={i18n}><IncidentWorkspace invoke={invoke} /></I18nProvider>);

  await userEvent.click(await screen.findByRole("option", { name: /vulnerability/i }));
  expect(await screen.findByTestId("tab-vulnerabilities")).toBeInTheDocument();
  await userEvent.click(screen.getByRole("tab", { name: /vulnerabilit/i }));
  expect(await screen.findByTestId("evidence-item")).toBeInTheDocument();

  await userEvent.type(screen.getByRole("textbox", { name: /comment/i }), "confirmed the finding");
  await userEvent.click(screen.getByRole("button", { name: /comment/i }));
  expect(await screen.findByText("confirmed the finding")).toBeInTheDocument();

  await userEvent.click(screen.getByRole("button", { name: /assign/i }));
  await userEvent.click(screen.getByRole("button", { name: /investigating/i }));
  await userEvent.click(screen.getByRole("button", { name: /resolved/i }));

  await waitFor(() => expect(screen.getByTestId("incident-status")).toHaveTextContent("resolved"));

  const names = invoke.mock.calls.map(([envelope]) => envelope.name);
  expect(names).toContain("incident.add_comment");
  expect(names).toContain("incident.assign_role");
  expect(names).toContain("incident.transition");
});
```

This covers the sprint exit criterion, including the vulnerability-finding case
named in it.

- [ ] **Step 2: Run it and fix what it finds**

Run: `npm test -- ui/src/incident/incident.acceptance.test.tsx`
Expected: PASS. Failures here are integration gaps between tasks, not new
features — fix them in place.

- [ ] **Step 3: Run every gate**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test 2>&1 | tail -5
npm run format:check && npm run lint && npm run typecheck && npm test
```

- [ ] **Step 4: Commit**

```bash
git add ui/src/incident
git commit -m "test(incident): verify the sprint 16 workspace acceptance criterion"
```
