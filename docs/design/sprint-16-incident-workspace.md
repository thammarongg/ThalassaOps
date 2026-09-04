# Sprint 16 — Incident Workspace

**Status:** Approved

**Date:** 2026-09-02

**Roadmap:** `docs/planning/sprint-plan.md`, Sprint 16

**Jira:** SCRUM-47 under SCRUM-5

## 1. Outcome

Sprint 16 turns the Sprint 15 incident write model into the product's primary
deep-work surface. A responder opens one workspace and manages an incident from
any supported source through resolution: reading its narrative, inspecting the
evidence and associations captured at creation, commenting, assigning roles and
moving the lifecycle forward.

The exact exit criterion is:

> A responder can manage an incident from any supported source through
> resolution without leaving the workspace for basic coordination, including a
> vulnerability finding with evidence in the vulnerability tab.

"Basic coordination" is bounded deliberately: comment, assign, change status and
severity, and copy a bounded summary. It does not include notifying anyone,
exporting a file, running a remediation or asking an assistant.

## 2. Binding decisions

1. The workspace is a read/write surface over the Sprint 15 aggregate. Sprint 16
   introduces no second incident model and no projection table.
2. The incident narrative is composed deterministically from the existing
   timeline. It is not AI-generated. Sprint 17 delivers the provider gateway and
   Sprint 19 delivers AI investigation; the narrative reserves a slot for those
   findings and renders nothing there until they exist.
3. Comments are a new immutable timeline event kind, not a separate entity. They
   inherit actor attribution, ordering and append-only storage from Sprint 15.
4. Comments cannot be edited or deleted. This is structural, not policy: the
   `incident_timeline_no_update` and `incident_timeline_no_delete` triggers in
   `0006_incidents.sql` abort every `UPDATE` and `DELETE` on the timeline table.
   Making comments mutable would require dismantling the append-only contract of
   the whole timeline.
5. Adding a comment does not carry `expected_version` and does not advance the
   incident version. A comment changes no incident state, and versioning it would
   make every comment invalidate every other responder's read of the incident.
   This is preparation for Sprint 24, not a fix for a defect that exists today;
   see section 7.5.
6. Because decision 5 removes the version guard that currently serialises
   writers, timeline sequence allocation moves inside the write transaction for
   **every** mutation path, not only for comments. This is a prerequisite task,
   not a side effect of the UI work.
7. The four association tabs render only what is already associated with the
   incident. They do not re-query live snapshots, and Sprint 16 adds no way to
   associate more signals after creation. The tab registry must nevertheless
   treat an association set as something that can grow, because Sprints 19 and
   21 populate `hypothesis_ids` and `action_ids` on incidents that are still
   open. Frozen is this sprint's behaviour, never a structural assumption.
8. The card delivered by this sprint is the Incident Summary Card: an
   in-application view plus a bounded clipboard copy. No file export, no link,
   no image. It is one bounded view of the Incident Card described in
   `docs/requirements/system-requirements.md`, deliberately renamed here to stay
   distinct from the management-readable escalation artifact that Sprints 19 and
   23 complete.
9. Comment authorisation reuses `Capability::IncidentWrite` with
   `Permission::ManageIncident`, unchanged from every other incident write. The
   consequence is recorded as a debt in section 14.
10. Sprint 16 touches the Rust workspace only for the comment event kind, the
    sequence-allocation change and the `incident.add_comment` command. All other
    work is in `ui/`.

## 3. Scope

### 3.1 Included

- a split incident list/detail workspace with a shell that owns all IPC;
- a deterministic incident narrative built from lifecycle timeline events;
- an evidence panel resolving the incident's `evidence_ids`;
- alerts, topology, changes and vulnerability tabs over frozen associations;
- an incident comment thread backed by a new timeline event kind;
- interactive assignment, status transition and severity controls;
- an in-application Incident Summary Card with a bounded clipboard copy;
- transaction-scoped timeline sequence allocation with bounded retry;
- English and Thai strings for every new surface, with a key-parity test.

### 3.2 Excluded

- the Evidence Tide Line. It is the product's signature visual
  (`docs/design/ux-ui-concept.md`, line 170) but is not a Sprint 16 deliverable,
  and adding it here would consume unbudgeted time.
- attaching further signals or evidence to an existing incident. No public
  attach operation exists in the aggregate today, and adding one is a new domain
  mutation outside this sprint.
- AI narrative, hypotheses, findings or assistant log. Sprint 19.
- classification, redaction and any export path. Sprint 18.
- notifications, external integration writes and remediation actions.
- a separate comment permission. See section 14, debt 1.

## 4. Canonical language

### 4.1 Incident Workspace

The split surface: a filtered incident queue on the left, the selected
incident's detail on the right. One incident is selected at a time.

### 4.2 Narrative

The ordered, human-readable rendering of the lifecycle events of one incident:
`incident_created`, `triggers_attached`, `status_transitioned`,
`severity_changed`, `disposition_changed`, `role_changed`. The narrative
excludes comments.

### 4.3 Comment

An immutable, attributed, free-text timeline event of kind `commented`. A
comment records what a responder said, never what the system did.

### 4.4 Association

An identifier captured on the incident at creation: a `SignalId` in
`signal_ids` or a `ConsoleEvidenceId` in `evidence_ids`. Associations are frozen
for the life of the incident in this sprint.

The aggregate already carries two further association lists,
`hypothesis_ids` and `action_ids` (`crates/thalassa-domain/src/lib.rs`, lines
455-456). Both are always empty today. Sprint 19 populates hypotheses and Sprint
21 populates actions, in both cases onto incidents that are still open, so the
association set is only frozen for as long as nothing is producing those
identifiers.

### 4.5 Incident Summary Card

A bounded read-only summary of one incident, renderable on screen and copyable
to the clipboard as Markdown. Its field list is fixed by section 13.

The product requirements use "Incident Card" for a management-readable
escalation artifact that Sprint 19 generates from evidence and Sprint 23 pushes
into Jira, Slack and PagerDuty. This sprint delivers a much smaller thing, so it
carries a distinct name. The two must not be conflated in code or in copy.

## 5. Architecture

The workspace follows the module shape already used by `topology`,
`correlation` and `change`, with one deliberate change: the container does not
grow into a monolith. `TopologyWorkspace.tsx` is 19.2 KB and
`CorrelationWorkspace.tsx` is 17.5 KB today; the Incident Workspace carries more
surface than either, so a single container would become the largest file in the
repository and could not be tested in parts.

### 5.1 Module boundary rule

The shell is the only component that calls IPC and the only component that owns
selection state. Every panel receives props and emits callbacks. No panel
performs a command, and no panel reads global state. This is what makes each
panel testable in isolation; it is a rule, not a preference.

### 5.2 Files

| File | Responsibility |
| --- | --- |
| `IncidentWorkspace.tsx` | Layout, selection, wiring. Composition only. |
| `useIncidentList.ts` | Incident page fetch, cursor paging, list errors. |
| `useIncidentTimeline.ts` | Timeline page fetch, sequence paging, timeline errors. |
| `IncidentList.tsx` | Queue rendering, severity and priority badges, filters. |
| `IncidentNarrative.tsx` | Lifecycle event rendering. |
| `IncidentEvidencePanel.tsx` | Evidence resolution results and failure states. |
| `IncidentTabs.tsx` | Tab chrome and selection. |
| `incidentTabConfig.ts` | The four tab definitions. |
| `IncidentCommentThread.tsx` | Comment rendering and composer. |
| `IncidentActions.tsx` | Transition, severity and role controls. |
| `IncidentSummaryCard.tsx` | Bounded summary and clipboard copy. |
| `contractValidation.ts` | Runtime guards for incident IPC payloads. |
| `incident-fixtures.ts` | Deterministic fixtures for tests. |
| `incident.css` | Module styles. |

`useIncidentList` and `useIncidentTimeline` are custom hooks, a pattern that does
not yet exist anywhere in `ui/`. This is a deliberate, narrow exception: the
hooks relocate IPC calls and pagination that would otherwise sit inline in the
shell, and they introduce no state-management layer. They remain part of the
shell for the purposes of section 5.1 — panels still never call IPC.

### 5.3 Tab registry

```ts
type IncidentTab = {
  id: 'alerts' | 'topology' | 'changes' | 'vulnerabilities';
  labelKey: string;
  select: (incident: IncidentDetail) => AssociationIds;
  isEmpty: (ids: AssociationIds) => boolean;
};
```

The vulnerability tab has no dedicated snapshot command and Sprint 16 does not
add one. Its content is the subset of the incident's associations that arrived
through a `vulnerability_finding` trigger, resolved from the same evidence store
as the other tabs.

`select` reads the association set from the incident on every render rather than
capturing it once. Sprint 16 never changes that set, so the two are
indistinguishable today, but Sprints 19 and 21 add identifiers to open incidents
and a registry that memoised the set at mount would silently stop updating.
Adding a fifth tab must require a new entry in this array and nothing else.

## 6. Domain model changes

Two additions to `crates/thalassa-domain/src/lib.rs`:

```rust
IncidentEventKind::Commented                          // serde: "commented"
IncidentTimelinePayload::Commented(CommentedPayload)  // serde: "commented"

pub struct CommentedPayload {
    pub body: String,
}
```

and one aggregate operation:

```rust
pub fn add_comment(
    &self,
    first_event_sequence: u64,
    body: &str,
    actor_id: PrincipalId,
    request_id: Uuid,
    policy_version: u64,
    now: DateTime<Utc>,
) -> Result<IncidentMutation, IncidentError>
```

`add_comment` deliberately has no `expected_version` parameter. It validates
`body` with `validate_incident_text(body, INCIDENT_NOTE_MAXIMUM)`, the guard
already used for notes, reasons and transition context, which supplies both the
4000-character bound and the existing unsafe-content rejection. It returns a
mutation whose `incident` is unchanged apart from `updated_at`.

## 7. Concurrency and sequence allocation

### 7.1 The problem

`IncidentService::load_for_write` reads `highest_event_sequence` and adds one
**outside** the write transaction. That is safe today only because the
repository's update statement carries `WHERE id = ?1 AND workspace_id = ?2 AND
version = ?3`: the version predicate rejects the losing writer before its
sequence can collide.

A comment has no version predicate. Two concurrent comments, or a comment racing
a status transition, would both read the same highest sequence, both attempt
sequence *n+1*, and the loser would surface a raw
`UNIQUE (incident_id, sequence)` violation rather than a typed error.

### 7.2 The change

Sequence allocation moves inside the `BEGIN IMMEDIATE` transaction for every
mutation path. The repository allocates the first event sequence after acquiring
the write lock and before appending events.

On a collision the repository retries the transaction, at most three times, then
returns a new typed error meaning "write contention, the request is still valid".

### 7.3 This error is not `VersionConflict`

The two must stay distinct because they instruct the caller to do opposite
things. `VersionConflict` means the caller's copy of the incident is stale and
must be reloaded before retrying. Write contention means the caller's copy is
correct and the same request may simply be sent again. Collapsing them would
force an unnecessary reload on every contended write.

### 7.4 Ordering

This change lands before any workspace UI work and is a dependency of the
comment task. It carries its own concurrency tests: two simultaneous comments,
and a comment racing a status transition.

### 7.5 Why this is done now, honestly

The race described in section 7.1 cannot occur in the product as it ships today.
ThalassaOps is a single-user local workspace (`docs/planning/sprint-plan.md`,
line 67), and multi-user membership, shared incidents and shared comments are
Sprint 24 deliverables. One desktop user cannot issue two simultaneous incident
writes, and the retry-after-lost-response case that SCRUM-44 addressed is
sequential, so it never contends for a sequence.

Sections 7.1 to 7.4 are therefore preparation, not remediation. They are done in
this sprint for two reasons. First, comments are the first incident write whose
natural contract has no version, so the version-free path has to exist regardless
and it is cheaper to make it correct once than to add it unguarded and repair it
under Sprint 24 pressure. Second, an unguarded sequence allocation is the kind of
defect that stays silent until concurrency arrives and then corrupts an audit
timeline, which is the one structure in this product that cannot be repaired
after the fact.

The cost is real and should be stated plainly: this modifies every incident write
path, including code merged the same day this design was written. That is why it
is a separate, first task with its own tests rather than a change folded into the
comment work.

## 8. Persistence

No migration. `incident_timeline_event.event_kind` is `TEXT NOT NULL` with no
`CHECK` constraint (`src-tauri/migrations/0006_incidents.sql`, line 75), so a new
event kind is a domain-crate change only.

Comment idempotency reuses the existing
`UNIQUE (incident_id, request_id, event_kind)` constraint and the request-id
replay path added for SCRUM-44. A replayed comment returns the stored event and
appends nothing.

## 9. Application flow

### 9.1 Add comment

1. Reject a nil request id or actor id.
2. Resolve the incident in the caller's workspace; an unknown or
   cross-workspace incident maps to `NotFound` and writes nothing.
3. Check the request-id replay path. A matching stored comment is returned
   unchanged.
4. Open the write transaction, allocate the sequence, append one `commented`
   event, touch `updated_at`, commit. The version column is not written.

### 9.2 Lifecycle mutations

Unchanged from Sprint 15 apart from where the sequence is allocated.

### 9.3 Read

The shell issues `incident.list` for the queue and `incident.get` plus
`incident.timeline` for the selection. Evidence resolution is described in
section 11.

## 10. IPC contract

One new command:

| Command | Capability | Permission |
| --- | --- | --- |
| `incident.add_comment` | `IncidentWrite` | `ManageIncident` |

No other descriptor changes. Reading comments uses the existing
`incident.timeline` command, since comments are timeline events.

## 11. Read model and evidence resolution

Evidence resolution is all-or-nothing. `EvidenceStore::get_for_scope` iterates
the requested identifiers and returns `Err` on the first one it cannot satisfy
(`src-tauri/src/topology/evidence.rs`, lines 37-48, and the operations
equivalent). There is no partial result, so the workspace cannot report *which*
identifier failed.

Three consequences bind the UI:

1. The workspace must never call an evidence command with an empty identifier
   list. An empty request returns `EmptyRequest` as an error, not an empty
   result. A tab with no associations renders its empty state without issuing a
   command.
2. The workspace must de-duplicate identifiers before requesting them. A
   repeated identifier returns `DuplicateId` and fails the whole request. An
   incident whose associations legitimately repeat an evidence identifier across
   two triggers would otherwise make the tab permanently unavailable.
3. A tab has four distinct states, and they must not be collapsed: loading;
   empty because the incident has no associations of that kind; unavailable
   because resolution failed; and populated. "Empty" and "unavailable" mean
   completely different things during a retrospective investigation.

The unavailable state distinguishes its causes by error, which the commands do
report: unknown identifier, cross-scope identifier, and unverified redaction.
Only the specific identifiers stay unknown.

## 12. Validation and errors

Results below are the outcomes observed at the IPC boundary. Domain and service
layers may name an error more narrowly on the way out.

| Condition | Result |
| --- | --- |
| Comment body empty, over 4000 characters, or unsafe | `InvalidRequest` |
| Nil actor or request id | `InvalidRequest` |
| Incident unknown or in another workspace | `NotFound` |
| Comment request id replayed with identical content | stored event returned |
| Comment request id replayed with different content | `IdempotencyConflict` |
| Sequence contention after three retries | write-contention error |
| Lifecycle mutation with a stale version | `VersionConflict` |

### 12.1 Version conflict behaviour in the UI

Version-carrying mutations — status, severity, disposition and roles — are not
rendered optimistically. The workspace waits for the result. On
`VersionConflict` it reloads the incident automatically and reports that the
incident changed, naming the actor and time, and states that the caller's
command was not applied.

The workspace does not resubmit automatically. The responder's intent may no
longer hold once the underlying status has moved.

Comments are rendered optimistically, because they carry no version and only
append.

## 13. Safety and policy

### 13.1 The clipboard is an egress channel

Copying the Incident Summary Card places incident content outside the application, where
any local process can read it and its destination is unbounded. Sprint 18
supplies classification and redaction; until it lands, the copy is restricted by
an explicit field allowlist rather than by rendering whatever is on screen.

Copyable: incident id, summary, severity, derived severity, status, disposition,
created and updated timestamps.

Not copyable: evidence excerpts, comment bodies, trigger payloads, role
assignments, timeline reasons.

### 13.2 The summary is an accepted leak

`summary` is free text written by a responder or derived from an alert, and it
can contain a secret as easily as a log excerpt can. It is on the allowlist
because the card is close to useless without it. This is an accepted risk taken
knowingly, not a field that has been shown to be safe.

Sprint 18 must therefore route incident summaries through redaction, not only
evidence payloads. Recorded here so the requirement is not lost when that sprint
is scoped.

### 13.3 This allowlist is not an egress contract

Sprint 23 sends incident content into Jira, Slack, Discord and PagerDuty. The
field list above is the safe set for a clipboard on the responder's own machine;
it is not a decision about what may leave for an external system, where the
audience is an entire organisation and the content is retained by a third party.
Sprint 23 must derive its own allowlist from the Sprint 18 classification model.
Reusing this one unexamined would silently promote the accepted summary leak in
section 13.2 from a local risk into a published one.

## 14. Known limitations and debts

1. **Comments require full incident management rights.** Every incident write
   shares `Capability::IncidentWrite` with `Permission::ManageIncident`, so any
   principal permitted to comment is equally permitted to close the incident,
   change its severity and reassign roles. A `Stakeholder` — a role Sprint 15
   defines as non-exclusive and expected to be held by several people — can
   therefore either do everything or nothing. This is sharper than it first
   looks: `docs/requirements/system-requirements.md`, line 23, names the
   engineering manager and incident stakeholder as the people who *consume*
   Incident Cards and status updates, and under this decision those people
   cannot even leave a comment without being granted the right to close the
   incident.

   The debt has three owners, not one. Sprint 20 (Policy Center) owns the
   permission model and is where a comment permission would be introduced.
   Sprint 24 adds roles, resource scopes and shared comments, which is where the
   gap becomes user-visible. Sprint 25 revisits IPC capability restrictions. A
   fix in Sprint 20 alone is not sufficient if Sprint 24 does not adopt it.
2. **Comment immutability will be tested by Sprint 24.** Decision 4 makes
   comments permanently uneditable and undeletable. That is defensible for a
   single-user audit timeline. Sprint 24 delivers shared incidents and comments,
   and multi-user comment surfaces normally require deletion or redaction — the
   obvious case being a responder who pastes a credential into a comment. Under
   this design the only remedies are dismantling the append-only timeline
   contract or accepting that such a comment is permanent. Sprint 24 should
   decide this deliberately rather than discover it.
3. **Associations are frozen.** A responder who realises another alert belongs
   to the incident cannot attach it. The workspace is exactly where that need
   arises, so this will be felt.
4. **Failed evidence resolution cannot name the identifier.** Section 11. The
   fix belongs with the Sprint 18 evidence and redaction work.
5. **No Evidence Tide Line.** Section 3.2.

## 15. Internationalisation

Every user-visible string is defined in both `ui/src/locales/en.ts` and
`ui/src/locales/th.ts` in the commit that introduces it. A test asserts key
parity between the two files. Thai is not allowed to lag: a partially translated
surface this large becomes permanent debt.

Narrative rendering is a formatted record — timestamp, actor, what changed,
reason — rather than generated sentences. Template sentences would require
authoring the grammar twice for two languages whose word order differs, and
Sprint 19 would rewrite them anyway.

## 16. Testing

| Layer | Coverage |
| --- | --- |
| `thalassa-domain` | `add_comment` validation, payload round-trip, event kind wire stability |
| `thalassa-ipc` | `incident.add_comment` descriptor, capability and permission |
| `src-tauri` repository | in-transaction sequence allocation, retry bound, two concurrent comments, comment racing a transition |
| `src-tauri` service | comment replay, unknown incident, cross-workspace incident |
| `ui` panels | each panel rendered in isolation from fixtures |
| `ui` shell | selection, pagination, version-conflict reload behaviour |
| `ui` acceptance | a responder creates, comments, assigns, transitions and resolves one incident end to end |
| `ui` locales | en/th key parity |

Fixtures use the shared fixture day 2026-08-28 so they stay valid alongside the
Sprint 13-15 replay corpus, and they assert non-empty results before any
assertion is built on top of them.
