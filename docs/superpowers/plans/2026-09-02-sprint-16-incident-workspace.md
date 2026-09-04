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
| `ui/contracts/ipc.ts` | `commented` wire shapes, `IncidentCommentRequest`, `INCIDENT_NOTE_MAXIMUM` | 4 |
| `ui/contracts/guards.ts` | commented payload guard, `isIncidentPage` | 4 |
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
- Modify: `ui/contracts/ipc.ts`
- Modify: `ui/contracts/guards.ts`
- Test: `ui/src/incident/incident-contracts.test.ts` (exists; extend it)

There is no `ui/src/incident/contractValidation.ts` and this task does not
create one. Sprint 15 froze the incident wire guards into
`ui/contracts/guards.ts` beside the types they check — `isIncident`,
`isIncidentTimelinePage`, `isIncidentTriggerInput` — so a second module under
`ui/src/incident/` would be a second source of truth for the same shapes.
`ui/src/topology/contractValidation.ts` is the precedent for a snapshot whose
guard was never frozen into the contract, not for these.

The payload shape assumed here was also wrong. Serde tags the payload
`#[serde(tag = "kind", content = "data")]`, so a comment arrives as
`{ kind: "commented", data: { body } }`, not flat, and the event `kind` is a
separate field whose only asymmetric pair is `incident_created`/`created`.

**Interfaces:**
- Consumes: the `commented` wire names from Task 2 and the
  `incident.add_comment` request from Task 3.
- Produces in `ui/contracts/ipc.ts`:
  - `"commented"` in `IncidentEventKind`, `CommentedPayload = { body: string }`,
    and the `{ kind: "commented"; data: CommentedPayload }` arm of
    `IncidentTimelinePayload`.
  - `IncidentCommentRequest = { incident_id: UUID; body: string }`, which the
    plan omitted and Task 11 sends. It carries no `expected_version`.
  - `INCIDENT_NOTE_MAXIMUM = 4000`, mirroring the domain constant, for the
    Task 11 composer. The existing literal `4000`s in `guards.ts` stay as they
    are; rewriting them is not this task.
- Produces in `ui/contracts/guards.ts`:
  - the `commented` case of `isIncidentTimelinePayload`.
  - `isIncidentPage(value: unknown): value is IncidentPage`, which did not
    exist and which Task 6's list hook needs.

`isIncidentTimelineEvent` and `isIncidentTimelinePage` already exist, so this
task does not produce them. The event guard is module-private, and the tests
reach it through `isIncidentTimelinePage`.

- [x] **Step 1: Write the failing tests**

Extend the existing `describe("incident wire guards")` in
`ui/src/incident/incident-contracts.test.ts`, which already has a
`timelineEvent(id, sequence, kind, payload)` helper and an `incidentFixture`.
Three tests:

```ts
test("commented events carry a bounded body under the commented tag", () => {
  const comment = timelineEvent(EVENT_ONE, 1, "commented", {
    kind: "commented",
    data: { body: "checked the checkout dashboards" }
  });
  const withPayload = (payload: unknown) => ({
    incident_id: INCIDENT,
    events: [{ ...comment, payload }],
    next_sequence: null
  });

  expect(isIncidentTimelinePage(withPayload(comment.payload))).toBe(true);
  // rejected: no body, a numeric body, "", "   ", a control character,
  // 4001 scalars, and an extra key beside `body`; 4000 scalars accepted.
});
```

a second asserting the kind and the payload tag must agree in both directions,
and a third for `isIncidentPage`: the canonical page, a page carrying the
repository's `<rfc3339>|<uuid>` cursor, and an empty page all accepted; an
empty, truncated, non-timestamp or non-UUID cursor rejected, a cursor on an
empty page rejected, a malformed item rejected, and an unknown key rejected.

- [x] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run ui/src/incident/incident-contracts.test.ts`
Expected: FAIL. Not with "isIncidentTimelineEvent is not a function" — that
guard exists and is private. The comment test fails on the accepted-comment
assertion returning `false`, because `commented` is not yet in
`incidentEventKinds`; `isIncidentPage` is genuinely not a function.

- [x] **Step 3: Implement the contract and the guards**

`ui/contracts/ipc.ts` takes the four additions above. `ui/contracts/guards.ts`
adds `"commented"` to `incidentEventKinds`, value-imports
`INCIDENT_NOTE_MAXIMUM` from `./ipc` beside the existing type-only import, and
adds one `case` to `isIncidentTimelinePayload`:

```ts
case "commented":
  return (
    hasExactKeys(data, ["body"]) &&
    isSafeBoundedText(data.body, INCIDENT_NOTE_MAXIMUM)
  );
```

`isSafeBoundedText` already matches `validate_incident_text`: it rejects
whitespace-only text through `isNonEmptyString`'s `trim`, rejects control
characters, and counts Unicode scalar values.

`isIncidentPage` validates the cursor as the repository writes it —
`format_cursor` emits `"<rfc3339>|<uuid>"` and the service passes it through
untouched — bounded by `INCIDENT_CURSOR_MAXIMUM`, and rejects a cursor on an
empty page, which `list` never emits because the cursor is taken from the last
item.

Nothing else in `ui/` needed a change: there is no TypeScript command-name
registry and no enumeration of IPC error reasons, so `incident.add_comment`
and `InvalidComment` have no second home to update.

- [x] **Step 4: Run the tests to verify they pass**

- [x] **Step 5: Run the UI gate and commit**

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/contracts/ipc.ts ui/contracts/guards.ts ui/src/incident/incident-contracts.test.ts
git commit -m "feat(incident): carry the commented event across the TypeScript contract"
```

`format:check` does not cover `ui/contracts`; `eslint ui` and `tsc -b` do.
Implemented on the branch as eb4ca86 and 1e4cbcb with the full gate green:
136 UI tests, plus the two Rust tests that read `ui/contracts/ipc.ts`.

One parity guard was deliberately not added. `crates/thalassa-ipc/tests/contracts.rs`
asserts the TypeScript union carries `"WRITE_CONTENTION"`, and the same
assertion for `"commented"` would catch this contract drifting from the domain
— but it is a Rust change, and this task is confined to `ui/`. Worth doing when
Rust is next open.

---

### Task 5: Locale Key Parity Test

This lands before the components so every later task is forced to add both
languages. `en.ts` and `th.ts` hold 729 key paths each with no drift — not the
801 stated here, which was wrong by 72 — so the test passes on existing content
the moment it is written.

The check is not new in kind. Two namespace-scoped copies already exist, inline
and each with its own `keyPaths`: `ui/src/topology/TopologyWorkspace.test.tsx`
and `ui/src/correlation/correlation-contracts.test.ts`. Neither covers a
namespace nobody thought to write one for, which is exactly what the Sprint 16
rule needs. Both are subsumed by the file below and can go when those two files
are next open; this task does not touch them.

**Files:**
- Create: `ui/src/locales/locales.test.ts`
- Modify: `ui/src/locales/en.ts`, `ui/src/locales/th.ts`

**Interfaces:**
- Consumes: nothing.
- Produces: an `incident` namespace in both locale files that later tasks extend.

- [x] **Step 1: Write the test**

One recursive walker over the catalog, since the key list is the leaf list with
the values dropped:

```ts
const leaves = (value: unknown, prefix = ""): [string, unknown][] =>
  Object.entries(value as Record<string, unknown>).flatMap(([key, inner]) =>
    typeof inner === "object" && inner !== null
      ? leaves(inner, `${prefix}${key}.`)
      : [[`${prefix}${key}`, inner] as [string, unknown]]
  );

const keyPaths = (value: unknown): string[] => leaves(value).map(([key]) => key);
```

The first assertion is set difference in both directions, which names the drift
rather than diffing 729 keys.

A second assertion covers what key parity alone misses: a key stubbed on one
side with an empty value. It cannot simply require non-empty text — `units.count`
is deliberately `""` in the `operations`, `correlation` and `topology`
namespaces, where a count renders bare and the other units append a suffix. So
the rule is that a key blank in one catalog must be blank in the other, and
every leaf must be a string.

- [x] **Step 2: Run it to confirm the existing files already pass**

Run: `npx vitest run ui/src/locales/locales.test.ts`
Result: PASS. There is no pre-existing key drift.

- [x] **Step 3: Add the incident namespace to both files**

Appended after `topology`, the last namespace, following the file's ordering by
feature rather than alphabetically. Values follow the house convention
`Loading <thing>…` / `กำลังโหลด<thing>…` rather than a bare `Loading…`:

```ts
  incident: {
    queueTitle: "Incidents",
    detailTitle: "Incident",
    emptyQueue: "No incidents match this filter",
    loading: "Loading incidents…"
  }
```

```ts
  incident: {
    queueTitle: "เหตุการณ์",
    detailTitle: "รายละเอียดเหตุการณ์",
    emptyQueue: "ไม่มีเหตุการณ์ที่ตรงกับตัวกรองนี้",
    loading: "กำลังโหลดเหตุการณ์…"
  }
```

- [x] **Step 4: Run the UI gate and commit**

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/locales
git commit -m "test(ui): assert en and th locale key parity across every namespace"
```

Implemented on the branch as 87b628c with the full gate green: 138 UI tests.

One finding this task deliberately did not fix. In `th.ts` the `topology`
namespace still carries the English unit suffixes `" ms"` and `" s"`, where
`operations` and `correlation` carry `" มิลลิวินาที"` and `" วินาที"`. The keys
exist in both catalogs, so neither the constraint nor this test is violated —
the strings are simply untranslated, and they belong to Sprint 13.

---

### Task 6: Incident Data Hooks

The code in this task was written against an `invoke` that does not exist. The
real contract is `ui/contracts/ipc.ts:1198`:

```ts
export type Invoke = <T, U>(command: string, args: { envelope: CommandEnvelope<T> }) => Promise<IpcResult<U>>;
```

Two positional arguments, not one object: the Tauri command name
(`incident_list`, `incident_timeline` — registered in `src-tauri/src/main.rs`)
and an envelope whose `command` field carries the dotted IPC name
(`incident.list`, `incident.timeline`) built with the `command()` helper. Every
assertion in the original steps — `expect.objectContaining({ name: "incident.list" })`
— would have passed against a hook that never called the real IPC at all.

Three further corrections, each of which would have failed silently:

- **The timeline does not page by cursor.** `IncidentTimelinePage` is
  `{ incident_id, events, next_sequence }`, and the request field is
  `after_sequence`. `IncidentPage` is `{ items, next_cursor }` as stated.
- **Both resume tokens are the last returned item, and the server filters
  strictly greater.** `repository.rs` sets `next_sequence` to
  `events.last().sequence` and loads `WHERE sequence > ?2`; `list` sets
  `next_cursor` to `format_cursor` of the *last item* and loads
  `updated_at < ?2 OR (updated_at = ?2 AND id > ?3)`. So each hook sends the
  token back verbatim. Adding one would skip an event; subtracting one would
  replay it, and neither shows up as a failure — just a wrong page.
- **`IncidentSummary` does not exist.** The list carries full `Incident`
  values.

The payloads are `#[serde(deny_unknown_fields)]` structs with a required
`limit` (`src-tauri/src/app/incident.rs:50-65`), validated in `1..=100`, so the
tests assert the whole payload with `toEqual` rather than `objectContaining`:
an extra or missing key is a real IPC rejection, and `objectContaining` cannot
see it.

**Files:**
- Create: `ui/src/incident/incident-envelope.ts`, `ui/src/incident/useIncidentList.ts`, `ui/src/incident/useIncidentTimeline.ts`, `ui/src/incident/incident-fixtures.ts`
- Test: `ui/src/incident/useIncidentList.test.ts`, `ui/src/incident/useIncidentTimeline.test.ts`

`incident-envelope.ts` is not in the original file list. Correlation and
topology each inline their own envelope helper in the one component that uses
it; the incident module cannot, because Tasks 11-12 need `IncidentWrite`
envelopes from components that are not these hooks. One helper, one file.

**Interfaces:**
- Consumes: `isIncidentPage`, `isIncidentTimelinePage` from Task 4 — already in
  `ui/contracts/guards.ts`, not in a module-local contracts file.
- Produces:
  - `incidentEnvelope(verb, capability, payload)` — `request_id: crypto.randomUUID()`, `command: command("incident", verb)`, `scope: { resource_ids: [] }`, following `CorrelationWorkspace`.
  - `INCIDENT_PAGE_LIMIT = 25`, `INCIDENT_TIMELINE_LIMIT = 50` — both inside the validated `1..=100`, exported so the tests assert the payload rather than restating a literal.
  - `useIncidentList(invoke: Invoke): { incidents: Incident[]; loading: boolean; error: IpcErrorCode | null; loadMore: () => void; hasMore: boolean; reload: () => void }`
  - `useIncidentTimeline(invoke: Invoke, incidentId: string | null): { events: IncidentTimelineEvent[]; loading: boolean; error: IpcErrorCode | null; loadMore: () => void; hasMore: boolean; reload: () => void }`
  - `incidentFixturePage`, `incidentFixtureTimeline` — dated `2026-08-28`.

`error` is an `IpcErrorCode`, not the `string` the original said. The hooks take
no `t`, and every workspace in this repo translates a code through its own
`localizedErrorKey` switch at the component. A guard failure reports
`MALFORMED_RESPONSE`, a rejected promise `INTERNAL_ERROR`, and an IPC error its
own code; Task 7 does the translating.

- [x] **Step 1: Write the fixtures and assert the guards accept them**

In `incident-fixtures.ts`, one page of three incidents and one timeline of six
events, all timestamped on `2026-08-28`, modelled on the literals in
`incident-contracts.test.ts` — which are the only shapes known to satisfy
`isIncident`'s cross-field invariants (`derived_severity` recomputed from the
impact dimensions, evidence closure over impact and override, sorted unique id
arrays, `owning_team_id === scope.team_id`). Item 0's summary mentions checkout,
because Task 7's list test matches `/checkout/i`. The six events cover
`incident_created`, `triggers_attached`, two `status_transitioned`,
`severity_changed` and the `commented` kind Task 2 added, with strictly
ascending sequences and one shared `incident_id`.

The fixture's `next_cursor` uses the `+00:00` offset form `format_cursor`
actually emits (pinned in `incident-contracts.test.ts` by 1e4cbcb), not `Z`.

Assert the guards, not just the lengths. A fixture that fails `isIncident`
produces exactly the symptom the Step 2 guard-failure test asserts — `error`
set, `incidents` empty — so without this the next two hours go into the hook:

```ts
it("ships fixtures the Task 4 guards accept", () => {
  expect(incidentFixturePage.items.length).toBeGreaterThan(0);
  expect(isIncidentPage(incidentFixturePage)).toBe(true);
  expect(incidentFixtureTimeline.events.length).toBeGreaterThan(0);
  expect(isIncidentTimelinePage(incidentFixtureTimeline)).toBe(true);
});
```

- [x] **Step 2: Write the failing hook tests**

```ts
it("pages the incident list with the cursor the page returned", async () => {
  const invoke = vi.fn().mockResolvedValueOnce(ok(incidentFixturePage))
    .mockResolvedValueOnce(ok({ items: [], next_cursor: null }));

  const { result } = renderHook(() => useIncidentList(invoke as unknown as Invoke));
  await waitFor(() => expect(result.current.loading).toBe(false));
  expect(result.current.hasMore).toBe(true);

  act(() => result.current.loadMore());
  await waitFor(() => expect(result.current.hasMore).toBe(false));

  expect(invoke.mock.calls[0][0]).toBe("incident_list");
  expect(invoke.mock.calls[0][1].envelope.command).toBe("incident.list");
  expect(invoke.mock.calls[0][1].envelope.capability).toBe("IncidentRead");
  expect(invoke.mock.calls[0][1].envelope.payload).toEqual({
    cursor: null,
    limit: INCIDENT_PAGE_LIMIT
  });
  expect(invoke.mock.calls[1][1].envelope.payload).toEqual({
    cursor: incidentFixturePage.next_cursor,
    limit: INCIDENT_PAGE_LIMIT
  });
});

it("reports a guard failure as MALFORMED_RESPONSE rather than rendering unvalidated data", async () => {
  const invoke = vi.fn().mockResolvedValue(ok({ items: [{ bogus: true }], next_cursor: null }));
  const { result } = renderHook(() => useIncidentList(invoke as unknown as Invoke));
  await waitFor(() => expect(result.current.error).toBe("MALFORMED_RESPONSE"));
  expect(result.current.incidents).toEqual([]);
});
```

and for the timeline, the three the original left unwritten:

```ts
it("resumes from next_sequence verbatim", /* payload toEqual { incident_id, after_sequence: page.next_sequence, limit: INCIDENT_TIMELINE_LIMIT } */);
it("does not call invoke when no incident is selected", /* expect(invoke).not.toHaveBeenCalled() */);
it("drops a page that arrives for a since-deselected incident", /* stale response, events stay empty */);
```

- [x] **Step 3: Run tests to verify they fail**

Run: `npx vitest run ui/src/incident/useIncidentList.test.ts`
Expected: FAIL — the module does not exist.

- [x] **Step 4: Implement both hooks**

Each hook keeps items, the resume token, `loading` and `error` in state, calls
`invoke` with an `IncidentRead` envelope, runs the Task 4 guard on the response,
and appends on `loadMore`. A guard failure sets `error` and leaves the items
untouched. Neither hook renders anything.

Three behaviours the original did not state:

- A `useRef` request counter discards stale responses, as `CorrelationWorkspace`
  does. `useIncidentTimeline` also checks `value.incident_id === incidentId`
  before accepting a page: the guard only proves a page is internally
  consistent, so a *valid* page for the previous incident can still land after
  the selection changed.
- `loadMore()` is a no-op while `loading` or `!hasMore`, or a double click
  appends the same page twice.
- `incidentId === null` returns empty state with `loading: false` and calls no
  command.

- [x] **Step 5: Run tests to verify they pass**

Run: `npx vitest run ui/src/incident/useIncidentList.test.ts ui/src/incident/useIncidentTimeline.test.ts`
Expected: PASS.

- [x] **Step 6: Run the UI gate and commit**

```bash
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident
git commit -m "feat(incident): add incident list and timeline data hooks"
```

---

### Task 7: Workspace Shell and Incident List

The original steps do not survive contact with the frozen contract. Four
corrections, three of which are silent failures rather than red tests.

- **`Incident` carries no priority.** `ui/contracts/ipc.ts:985-1004` has
  `derived_severity` and `severity_override` and nothing else severity-shaped;
  `ConsolePriority` exists only as a nullable, fixture-set field on
  `IncidentQueueItem` (`ipc.ts:441-447`), and no code in `crates/` or
  `src-tauri/` derives one from an incident. The design's file table (section
  5.2, "severity and priority badges") predates that check. The UX rule is
  "show severity separately from derived priority *wherever both are
  available*" (`docs/design/ux-ui-concept.md:175`); for an incident only one
  is. So the queue renders severity alone. A UI-side derivation would be a
  domain rule the Rust side does not make, and a placeholder element would
  still fail the original `toHaveTextContent("P1")`. Section 13's summary-card
  field list does not name priority either, so this holds through Task 13.
- **The badge must show the *effective* severity.** `severity_override.selected`
  when an override is present, `derived_severity` otherwise. The search fixture
  is exactly this case — derived `S2`, selected `S1` — so a component that
  renders `derived_severity` alone passes a checkout-only test and hides every
  override in the queue.
- **`I18nProvider` takes only `children`** (`ui/src/i18n.tsx`). The original
  `<I18nProvider i18n={i18n}>` is a typecheck failure.
- **The shell test asserted the `invoke` that does not exist**, the same defect
  Task 6 corrects: `expect(invoke).toHaveBeenCalledWith(expect.objectContaining({ name: "incident.timeline" }))`
  would pass against a shell that never called IPC. The real signature is two
  positional arguments, and `incidentInvokeMock()` has to be written — nothing
  in the repo provides it.

Two further things the original left undefined:

- **The filter is client-side.** `IncidentListRequest` is `{ cursor, limit }`
  (`ipc.ts:1134`) — there is no status parameter. `IncidentList` filters the
  incidents already loaded. A filter matching nothing on the loaded pages shows
  the empty state rather than fetching further pages; that is a limitation to
  state in the component's doc comment, not a defect to fix here.
- **Selection is a listbox, not a pressed button.** `correlation` uses
  `<button aria-pressed>`, but `getByRole("option", { selected: true })` reads
  `aria-selected`, and the design fixes "one incident is selected at a time"
  (section 4.1). So: `role="listbox"` on the container, `role="option"` with
  `aria-selected` on each row, and the roving `tabIndex` plus
  ArrowUp/ArrowDown/Home/End the pattern requires — a listbox whose options are
  not `<button>`s gets no keyboard handling for free.

**Files:**
- Create: `ui/src/incident/IncidentWorkspace.tsx`, `ui/src/incident/IncidentList.tsx`, `ui/src/incident/incident.css`
- Test: `ui/src/incident/IncidentWorkspace.test.tsx`, `ui/src/incident/IncidentList.test.tsx`
- Modify: `ui/src/locales/en.ts`, `ui/src/locales/th.ts`

**Interfaces:**
- Consumes: `useIncidentList`, `useIncidentTimeline`, `incidentFixturePage`,
  `INCIDENT_TIMELINE_LIMIT` from Task 6.
- Produces:
  - `IncidentWorkspace({ invoke }: { invoke: Invoke })`
  - `IncidentQueueFilter = { status: "all" | IncidentStatus }`
  - `effectiveSeverity(incident: Incident): IncidentSeverity` — exported, so
    Tasks 8-13 do not each re-derive it.
  - `IncidentList({ incidents, selectedId, onSelect, filter, onFilterChange })`
    — pure, no IPC, no hook from Task 6.
  - The shell passes `incident`, `events`, and callbacks down to the panels
    added by Tasks 8-13.

**Locale keys.** Task 5's parity test makes a missing `th` key a red test, so
both catalogs gain, under `incident`: `status.*` for all eight
`IncidentStatus` values, `filter.label` and `filter.all`, `loadMore`,
`detailEmpty`, `severityLabel`, and `errors.*` covering all ten
`IpcErrorCode` variants (`ipc.ts:46-56`) — `WRITE_CONTENTION` included, since
Task 3 added it and the shell's `localizedErrorKey` switch must be total.

- [x] **Step 1: Write the failing list test**

```tsx
it("renders the effective severity, not the derived one, when an override is present", () => {
  render(
    <I18nProvider>
      <IncidentList
        incidents={incidentFixturePage.items}
        selectedId={null}
        onSelect={() => {}}
        filter={{ status: "all" }}
        onFilterChange={() => {}}
      />
    </I18nProvider>
  );
  const checkout = screen.getByRole("option", { name: /checkout/i });
  expect(within(checkout).getByTestId("incident-severity")).toHaveTextContent("S1");
  // derived S2, override selects S1
  const search = screen.getByRole("option", { name: /search/i });
  expect(within(search).getByTestId("incident-severity")).toHaveTextContent("S1");
});

it("calls onSelect with the incident id when a row is chosen", async () => {
  const onSelect = vi.fn();
  render(/* same tree with onSelect */);
  await userEvent.click(screen.getByRole("option", { name: /checkout/i }));
  expect(onSelect).toHaveBeenCalledWith(incidentFixturePage.items[0].id);
});

it("shows only the incidents the status filter admits", () => {
  render(/* same tree, filter={{ status: "triage" }} */);
  expect(screen.getAllByRole("option")).toHaveLength(1);
  expect(screen.getByRole("option", { name: /search/i })).toBeInTheDocument();
});

it("moves the selection with the arrow keys", async () => {
  const onSelect = vi.fn();
  render(/* same tree, selectedId={items[0].id}, onSelect */);
  screen.getByRole("option", { selected: true }).focus();
  await userEvent.keyboard("{ArrowDown}");
  expect(onSelect).toHaveBeenCalledWith(incidentFixturePage.items[1].id);
});
```

No `incident-priority` element exists, so no test asserts one.

- [x] **Step 2: Run tests to verify they fail**

Run: `npx vitest run ui/src/incident/IncidentList.test.tsx`
Expected: FAIL — the module does not exist.

- [x] **Step 3: Implement IncidentList as a pure component**

It receives arrays and callbacks only. It imports no hook from Task 6 and calls
no `invoke`. `effectiveSeverity` lives here and is exported.

- [x] **Step 4: Write the failing shell test**

`incidentInvokeMock()` routes on the Tauri command name — the first positional
argument — and returns the Task 6 fixtures:

```tsx
const incidentInvokeMock = () =>
  vi.fn((name: string) =>
    Promise.resolve(
      name === "incident_list"
        ? { ok: true, value: incidentFixturePage }
        : { ok: true, value: incidentFixtureTimeline }
    )
  );

it("selects the first incident and loads its timeline", async () => {
  const invoke = incidentInvokeMock();
  render(<I18nProvider><IncidentWorkspace invoke={invoke as unknown as Invoke} /></I18nProvider>);
  await waitFor(() => expect(screen.getByRole("option", { selected: true })).toBeInTheDocument());

  const timeline = invoke.mock.calls.find((call) => call[0] === "incident_timeline");
  expect(timeline).toBeDefined();
  expect(timeline[1].envelope.command).toBe("incident.timeline");
  expect(timeline[1].envelope.capability).toBe("IncidentRead");
  expect(timeline[1].envelope.payload).toEqual({
    incident_id: incidentFixturePage.items[0].id,
    after_sequence: null,
    limit: INCIDENT_TIMELINE_LIMIT
  });
});

it("translates a list error code rather than printing it", async () => {
  const invoke = vi.fn().mockResolvedValue({
    ok: false,
    error: { code: "PERMISSION_DENIED", message: "", details: {} }
  });
  render(/* the shell */);
  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent(en.incident.errors.permissionDenied)
  );
});
```

`after_sequence` is `null` on the first page: `useIncidentTimeline` sends
`sequenceRef.current`, which the selection effect resets before fetching. The
fixture timeline's `incident_id` is the checkout incident — `items[0].id` —
so auto-selecting the first row also satisfies the hook's stale-page check.

- [x] **Step 5: Implement the shell**

The shell wires the two hooks, holds `selectedId` and the queue filter, and
renders `IncidentList` plus a detail region that Tasks 8-13 fill. It is the only
component in the module that receives `invoke`. It selects the first incident
once the first page arrives, and only while nothing is selected — re-selecting
on every page would fight the user during `loadMore`. It owns the
`localizedErrorKey` switch over all ten `IpcErrorCode` variants.

- [x] **Step 6: Run tests, gate, and commit**

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
- Modify: `ui/src/incident/IncidentWorkspace.tsx` and its test, `ui/src/incident/incident.css`,
  `ui/src/locales/en.ts`, `ui/src/locales/th.ts`

**Interfaces:**
- Consumes: `IncidentTimelineEvent` from Task 4, rendered by the shell from Task 7.
- Produces: `IncidentNarrative({ events }: { events: IncidentTimelineEvent[] })` — pure.

- [x] **Step 1: Write the failing test**

`I18nProvider` takes no props: it wraps the module-level `i18n` singleton
itself (`ui/src/i18n.tsx`), so `<I18nProvider i18n={i18n}>` does not typecheck.
The fixture's only comment body is "Payment provider confirms a regional
outage on their side", so that is what the exclusion assertion must look for.

```tsx
const events = incidentFixtureTimeline.events;
const lifecycle = events.filter((event) => event.payload.kind !== "commented");
const bodyRows = () => screen.getAllByRole("row").slice(1);

it("renders lifecycle events as a record and excludes comments", () => {
  render(
    <I18nProvider>
      <IncidentNarrative events={events} />
    </I18nProvider>
  );
  expect(bodyRows()).toHaveLength(lifecycle.length);
  expect(screen.getByText(/investigating/i)).toBeInTheDocument();
  expect(screen.queryByText(/regional outage/i)).not.toBeInTheDocument();
});

it("renders each row with a timestamp, actor, change and reason column", () => {
  render(/* same tree */);
  expect(within(bodyRows()[0]).getAllByRole("cell")).toHaveLength(4);
});
```

Four more tests earn their place. Sequence ordering is asserted on the
machine-readable `datetime` attribute rather than the rendered text, which
`toLocaleString` formats differently on every host. `disposition_changed` and
`role_changed` never occur on the fixture incident, so they are constructed in
the test — without them half the description switch is unexercised and could
render a blank cell in production. The remaining two cover a reason and its
absence, and the empty state.

- [x] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/IncidentNarrative.test.tsx`
Expected: FAIL — vite cannot resolve `./IncidentNarrative`, so no test runs.

- [x] **Step 3: Implement**

Render through the design-system `Table` (`captionKey`, `columns`, `rows`),
which every other tabular surface in the UI already uses and which supplies the
header row and four `<td>` cells the tests assert. Filter on `payload.kind`,
not `event.kind`: the payload discriminant is `"created"` where the event kind
is `"incident_created"`. Type the description function over
`Exclude<IncidentTimelinePayload, { kind: "commented" }>` with a `never`
default, so a kind added to the contract fails the typecheck rather than
rendering a blank cell.

Each change is a label and a value — never a composed sentence — per design 15.
`triggers_attached` shows a bare count; the association tabs resolve the ids.
Severity codes are identical in both catalogs and so are not translation keys,
but statuses, dispositions and roles are: `incident.disposition.*` and
`incident.role.*` are new in this task and go into `en.ts` and `th.ts` in this
commit, or Task 5's parity test goes red.

`actor_id` renders raw. No principal directory reaches the UI in this sprint,
and an invented display name would misattribute a change on an audit surface.
Sort by `sequence` in the component rather than trusting page order, because a
resumed read appends.

Wire it into the shell's detail region in this task: Task 7 left that region a
placeholder, and Task 14's acceptance test composes the whole surface.

- [x] **Step 4: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident/IncidentNarrative.test.tsx
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): render the deterministic incident narrative"
```

Done: `67ba921`. 168 frontend tests green; no Rust surface changed.

---

### Task 9: Evidence Resolution and Panel

**Files:**
- Create: `ui/src/incident/incidentEvidence.ts`, `ui/src/incident/IncidentEvidencePanel.tsx`
- Test: `ui/src/incident/incidentEvidence.test.ts`, `ui/src/incident/IncidentEvidencePanel.test.tsx`
- Modify: `ui/src/incident/incident-fixtures.ts`, `ui/src/incident/incident.css`,
  `ui/src/locales/en.ts`, `ui/src/locales/th.ts`

**Interfaces:**
- Consumes: `Invoke` from the shell.
- Produces:
  - `type EvidenceState = { status: "loading" } | { status: "empty" } | { status: "unavailable"; cause: "missing" | "scope" | "unverified" | "unknown" } | { status: "ready"; evidence: EvidenceRef[] }`
  - `resolveEvidence(invoke: Invoke, ids: ConsoleEvidenceId[]): Promise<EvidenceState>`

**The command is `correlation_evidence`, not `operations.evidence`.** An incident's
`evidence_ids` are the ids `normalize_operational` and `normalize_security`
admitted into the `SourceRecordStore`
(`src-tauri/src/incident/source.rs`, lines 182-186, over
`crate::correlation::{correlation_fixture_catalog, SourceRecordStore}`), and the
correlation snapshot's evidence set is exactly `records.evidence_refs()`
(`src-tauri/src/app/correlation.rs`, line 285). The operations snapshot carries
no security evidence at all — `evidence-security-trivy` exists only in
`src-tauri/src/correlation/fixtures.rs` — so `operations_evidence` returns
`NOT_FOUND` for any incident raised from a `vulnerability_finding` trigger and
leaves that tab permanently unavailable. Mocked `invoke` tests cannot see this,
which is the Sprint 14 failure shape: assert the command name in the test.

**Three further facts the first draft of this task had wrong:**

1. `Invoke` is `(command: string, args: { envelope: CommandEnvelope<T> })`
   (`ui/contracts/ipc.ts`, line 1198) — two arguments. The tauri command name is
   snake_case (`correlation_evidence`); the envelope's `command` field is
   dotted (`correlation.evidence`), built with capability `ResourceRead` to
   match `correlation_evidence_descriptor()`.
2. Ids must be **sorted ascending as well as de-duplicated**.
   `validate_correlation_evidence_ids` (`crates/thalassa-domain/src/lib.rs`,
   lines 5097-5108) rejects an unsorted list with `DuplicateId`, which surfaces
   as `INVALID_REQUEST`. Preserving arrival order is not enough: the
   vulnerability tab flattens ids across triggers and has no sorted source.
3. There are no `evidence_*` error codes. The wire codes are `IpcErrorCode`
   (`src-tauri/src/app/correlation.rs`, lines 392-414): `NOT_FOUND` → `missing`,
   `PERMISSION_DENIED` → `scope`, `POLICY_DENIED` → `unverified`, anything else
   → `unknown`. `INVALID_REQUEST` means the helper sent an empty, duplicated or
   unsorted list, so reaching it is a helper bug and `unknown` is the honest
   cause. A rejected promise is also `unknown`.

**Notes for Task 10, not fixed here:** the aggregate type is `Incident`, not
`IncidentDetail`, and it carries `trigger_ids`, not `triggers`
(`ui/contracts/ipc.ts`, lines 985-1004), so the vulnerability tab's `select`
cannot be written as that task's draft has it. With one evidence store behind
every tab, the draft's `topology` and `changes` tabs both select
`incident.evidence_ids` and would resolve to the same evidence; design 5.3
never says what separates them, and Task 10 has to settle that. Task 10 also owns the shell
wiring — one `EvidenceState` per tab and a request-id ref that discards stale
results, as all three existing workspaces do. Task 9 ships the helper and a
pure panel; neither is reachable from the shell until Task 10 lands.

- [x] **Step 1: Write the failing test**

```ts
it("returns empty without issuing a command when there are no ids", async () => {
  const invoke = vi.fn();
  await expect(resolveEvidence(invoke as unknown as Invoke, [])).resolves.toEqual({
    status: "empty"
  });
  expect(invoke).not.toHaveBeenCalled();
});

it("sorts and de-duplicates ids before requesting them", async () => {
  const invoke = vi.fn().mockResolvedValue({ ok: true, value: [] });
  await resolveEvidence(invoke as unknown as Invoke, ["b", "a", "a"]);
  expect(invoke).toHaveBeenCalledWith("correlation_evidence", {
    envelope: expect.objectContaining({
      command: "correlation.evidence",
      capability: "ResourceRead",
      payload: { evidence_ids: ["a", "b"] }
    })
  });
});

it.each([
  ["NOT_FOUND", "missing"],
  ["PERMISSION_DENIED", "scope"],
  ["POLICY_DENIED", "unverified"],
  ["INVALID_REQUEST", "unknown"]
])("maps %s to the %s cause", async (code, cause) => { /* ... */ });
```

The empty-list and duplicate rules are not stylistic: an empty or repeated id
list is a hard error in the domain validator and would make the tab permanently
unavailable rather than empty.

Two more tests earn their place. A `ready` result is gated on
`isEvidenceResponse(value, ids)` — the guard every other workspace already
applies — and a response that does not match the request maps to
`unavailable` / `unknown` rather than rendering unvalidated wire data. A
rejected promise maps to `unavailable` / `unknown` rather than escaping.

`incident-fixtures.ts` gains `incidentFixtureEvidence: EvidenceRef[]` covering
exactly the ids the checkout fixture incident carries, and the test asserts
`isEvidenceResponse(incidentFixtureEvidence, incidentFixtureIncident.evidence_ids)`
before the panel test builds on it. Task 14 needs the same fixture.

- [x] **Step 2: Run test to verify it fails**

Run: `npm test -- ui/src/incident/incidentEvidence.test.ts`
Expected: FAIL — vite cannot resolve `./incidentEvidence`, so no test runs.

- [x] **Step 3: Implement resolveEvidence and the panel**

`resolveEvidence` short-circuits on an empty list, sorts and de-duplicates,
calls `correlation_evidence`, validates the response with `isEvidenceResponse`
and maps error codes to causes. `IncidentEvidencePanel` is pure: it takes an
`EvidenceState` and renders one of the four states, with a distinct message per
cause under `incident.evidence.unavailable.*`. It follows
`CorrelationEvidencePanel` — source heading, id, an `isTrustedNativeUrl` gate on
the native link, a `<dl>` of endpoint, query, observation time and excerpt, and
the redaction line — with its own `incident.evidence.sources.*` keys rather than
borrowing another module's namespace.

- [x] **Step 4: Run tests, gate, and commit**

```bash
npm test -- ui/src/incident
npm run format:check && npm run lint && npm run typecheck && npm test
git add ui/src/incident ui/src/locales
git commit -m "feat(incident): resolve incident evidence with explicit failure states"
```

Done: `92fe911`, with `7671fd1` correcting the sort order to code points and the
`POLICY_DENIED` copy. 193 frontend tests green; no Rust surface changed.

**Review, spec axis (completed 2026-09-04).** No correctness defect. Verified
against the backend rather than the mocks: the tauri command name and the dotted
`correlation.evidence` envelope match `correlation_evidence_descriptor()`; the
payload is exactly `{ evidence_ids }`, which is what `has_exact_keys` demands;
`scope: { resource_ids: [] }` is right, because the handler authorizes against
`correlation_workspace_scope().contains(&reference.scope)` and never reads the
envelope's resource ids — the same envelope `CorrelationWorkspace` sends;
`byCodePoint` agrees with Rust `String` ordering, since UTF-8 byte order is code
point order; all three mapped error codes are really constructed
(`correlation_evidence_not_found` / `_scope_denied` / `_policy_denied`); and the
snapshot's evidence set is `records.evidence_refs()` passed through
`aggregate_snapshot` unfiltered, so an incident's ids — admitted by the same
`correlation_fixture_catalog` replay — do resolve. `incident.evidence.sources.*`
covers all thirteen `EvidenceSourceKind` members in both catalogs.

**Review, standards axis — open, not yet fixed.**

1. `IncidentEvidencePanel` is the third near-identical copy of the same panel
   (`CorrelationEvidencePanel` 104 lines, `TopologyEvidencePanel` 105,
   `IncidentEvidencePanel` 112), differing only in CSS prefix and locale
   namespace, with the CSS duplicated alongside it. This task chose to copy
   deliberately; extracting a shared entry component that takes a class prefix
   and a namespace is a separate decision that also touches two shipped
   workspaces.
2. `incidentEvidence.ts` is the only file under `ui/` that cites Rust source
   line numbers in a comment (`src-tauri/src/app/correlation.rs`, lines
   165-199). The citation is accurate today and will rot on the next edit
   there; name the symbols instead.
3. `ui/src/incident/incident-envelope.ts` is a logic module in kebab-case, while
   the repository names logic modules in camelCase (`timeContext.ts`,
   `contractValidation.ts`, `widgetConfig.ts`) and reserves kebab-case for
   `*-fixtures.ts`. `incidentEvidence.ts` follows the convention; its neighbour
   does not.
4. The locale parity test compares `en` against `th` only. Nothing asserts that
   `incident.evidence.sources.*` covers the `EvidenceSourceKind` union, so a
   member added to the contract would render as its raw key in both catalogs
   with every test green.

---

### Task 10: Association Tabs

**Files:**
- Create: `ui/src/incident/incidentTabConfig.ts`, `ui/src/incident/IncidentTabs.tsx`
- Test: `ui/src/incident/IncidentTabs.test.tsx`

**Interfaces:**
- Consumes: `resolveEvidence` and `EvidenceState` from Task 9.
- Produces: `INCIDENT_TABS: IncidentTab[]` and `IncidentTabs({ incident, states, activeId, onSelect })`.

**The registry in Step 3 cannot be written against the real contract.** Three
separate reasons, all verified against `ui/contracts/ipc.ts` and the incident IPC
surface:

1. `i.triggers` does not exist. `incident_get` returns `Incident`
   (`src-tauri/src/app/incident.rs`, `pub fn incident_get`), and that aggregate
   carries `trigger_ids: UUID[]` — identifiers only. **No command returns
   `IncidentTrigger` records**, and `TriggersAttachedPayload` on the timeline is
   `{ trigger_ids: UUID[] }`, so the trigger's `source_kind` and its
   `evidence_ids` are unreachable from the UI. The vulnerability tab as drafted
   cannot be selected at all.
2. `alerts.select = (i) => i.signal_ids` feeds signal UUIDs to `resolveEvidence`,
   which resolves `ConsoleEvidenceId`s through `correlation_evidence`. Every id
   would miss the snapshot's evidence set, so the tab would be permanently
   `unavailable` / `missing` — green under a mocked `invoke`, dead at runtime.
   This is the Sprint 14 failure shape again.
3. `topology` and `changes` both select `incident.evidence_ids`, so both tabs
   resolve to the same evidence. Design 5.3 never says what separates them.

**Settle it by partitioning the resolved evidence, not the identifiers.**
Resolve `incident.evidence_ids` once and group the returned `EvidenceRef[]` by
`source_kind`, which is on every reference (`ui/contracts/ipc.ts`,
`EvidenceSourceKind`): alerts ← `alertmanager` / `prometheus` / `health_check`;
topology ← `kubernetes` / `cloud`; changes ← `github` / `gitlab` / `argo_cd`;
vulnerabilities ← `trivy` / `falco` / `kyverno` / `opa_gatekeeper`. This needs no
backend change, keeps one resolve call instead of four, gives each tab a
distinct set, and honours 5.3's "read the association set on every render" rule
because the grouping is derived during render from the incident's current ids.
`fixture` evidence belongs to no tab and must be assigned explicitly rather than
dropped silently. The tab registry then keys off the grouped map, and
`IncidentTab.select` takes the grouped evidence rather than the incident.

If the four tabs must instead be driven by trigger provenance, that is a
backend change — embedding triggers in `Incident` or adding an
`incident.triggers` command — and it is not in this sprint's plan.

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
