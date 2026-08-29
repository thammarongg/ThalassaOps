# Sprint 14 — Change intelligence design

## Goal

Connect operational problems to the changes that preceded them. Sprint 14
normalizes GitHub, GitLab and Argo CD change records into one canonical,
source-preserving `ChangeEvent` contract, orders them into a deterministic
change timeline, and attaches them to Sprint 13 correlation candidates as
explainable structural context.

The sprint exit criterion is exactly: "A user can identify what changed before
an incident and inspect the supporting source/diff."

## Hard constraint: a change is context, not a cause

Sprint 13 established that correlation reasons state structural or exact
relationships, never causal ones. Change intelligence is where that rule is
easiest to break, because a deployment that lands four minutes before an alert
looks like an explanation. It is not one.

Three rules enforce this:

1. A `ChangeEvent` is never a member of a `CorrelationCandidate`. Candidate
   membership continues to mean "these signals belong together". Changes are
   attached as a separate association list.
2. Temporal precedence alone never produces an association. A change must also
   share an exact target with the candidate or connect to it through a
   Sprint 12 topology path.
3. The vocabulary is `preceding_change` and `probable_structural`. There is no
   `root_cause`, `caused_by`, `triggered_by`, `blast_radius` or probability
   score in any contract, wire value, locale key or rendered string.

The user decides whether a change explains a failure. The product shows what
changed, when, by whom, against which target, and links to the source.

## Scope and boundaries

In scope:

- Replayable GitHub, GitLab and Argo CD adapters over committed synthetic
  fixtures.
- The canonical `ChangeEvent` contract and its append-only source-record
  retention.
- A bounded, deterministic change timeline.
- Change-to-candidate association with explainable structural reasons.
- Changed-file metadata, diff statistics and native source links.
- A read-only, localized change view in the Operations Console.

Out of scope, deliberately:

- Live provider calls, credential storage, token scopes, rate limiting,
  webhooks, polling and capability discovery. Sprint 14 ships no outbound
  network path and no provider CLI invocation. A later sprint adds live
  ingestion behind the existing connector capability and transport policy.
- Diff bodies. Changed-file paths and counts are retained; hunk content is not
  stored, not returned over IPC and not rendered. The user follows the native
  link to read the diff at the source. See "Diff statistics and changed paths".
- Any write path: no change ingestion command, no revert, no redeploy, no
  Argo CD sync trigger, no incident entity, no candidate mutation.
- Reimplementing topology traversal. Association delegates to the Sprint 12
  engine in `src-tauri/src/topology/`.

## Contract rules carried from Sprints 10-13

These are unchanged and bind every task in this sprint:

- One type per concept. There is no second change model, no UI-only change
  type and no provider-specific change struct.
- One source enum. `EvidenceSourceKind` gains `github`, `gitlab` and `argo_cd`.
  No adapter introduces a private source enum or a stringly typed source field.
- Rust numeric fields are `f64`, TypeScript numeric fields are `number`, and
  NaN and both infinities are rejected with typed errors before serialization.
- Absent source data is `Option`/`null`, an explicit unavailable
  `SourceStatus`, or an omitted record. Empty strings are never placeholders.
  Fabricated timestamps, actors, revisions, targets and links are forbidden.
- The complete post-policy source record, unknown fields included, is retained
  in the append-only ledger. Normalized fields are typed indexes over that
  record, never a lossy replacement.
- Every displayed value carries verified evidence IDs and a typed drill-down
  reference. No returned ID may resolve outside the current workspace.
- No credential, token, ARN, account ID, subscription ID, authorization header,
  cookie, pagination cursor or credential reference may enter a normalized
  change, association, log, committed fixture, retained record or serialized
  result. Safe identity validation rejects unsafe values rather than blanking
  them.
- Rust never emits user-facing English sentences. React maps stable wire values
  to English and Thai locale keys.
- Identical inputs and policy version produce byte-identical output. No
  wall-clock dependence, no input-order dependence, no background scheduler.

## Reconciliation with the Sprint 11 change stream

Sprint 11 already defines `ChangeKind` and `ChangeStreamItem`
(`crates/thalassa-domain/src/lib.rs`), produced by
`src-tauri/src/operations/fixtures.rs` and consumed by the Operations Console
aggregate and `ui/src/operations`. Sprint 14 does not add a parallel type
beside it. Instead:

- `ChangeEvent` becomes the canonical record. `ChangeStreamItem` stays as the
  console's summary projection and is derived from `ChangeEvent` values rather
  than invented in the operations fixture module.
- `ChangeStreamItem.source` changes from `Option<String>` to
  `EvidenceSourceKind`, closing the last stringly typed source field in the
  console contract.
- `ChangeKind` is extended, not replaced. The existing `deployment`,
  `configuration`, `maintenance` and `connector` wire values are unchanged;
  `code_commit`, `code_merge`, `sync` and `rollback` are added.
- The console change-stream widget, its contract validation and its tests move
  to the typed source without changing their layout or locale keys.

This is the "improve the code you are working in" case: it removes a duplicate
concept instead of adding one, and it is a required change, not optional
refactoring.

## Architecture

```text
committed fixtures (github/gitlab/argocd JSON)
        │
        ▼
  change adapters ──► post-policy source record ──► append-only ledger (SQLite)
        │                                                   │
        ▼                                                   │
  normalized ChangeEvent ◄──────────── typed index over ────┘
        │
        ├──► change timeline (bounded window, deterministic order)
        │
        └──► association engine
                 │  requires temporal precedence AND
                 │  (exact shared target OR Sprint 12 topology path)
                 ▼
          ChangeAssociation attached to CorrelationCandidate
                 │
                 ▼
      change.snapshot / change.evidence (read-only, capability scoped)
                 │
                 ▼
        Operations Console change view (en/th)
```

### Module layout

```text
crates/thalassa-domain/src/lib.rs        # ChangeEvent and association contracts
crates/thalassa-ipc/src/lib.rs           # change.snapshot / change.evidence descriptors
src-tauri/migrations/0005_change_records.sql     # change records only; evidence reuses 0004
src-tauri/src/change/mod.rs              # module surface, no second model
src-tauri/src/change/fixtures.rs         # replay catalog and fixture clock
src-tauri/src/change/adapters/github.rs
src-tauri/src/change/adapters/gitlab.rs
src-tauri/src/change/adapters/argocd.rs
src-tauri/src/change/records.rs          # retention over the Sprint 13 ledger
src-tauri/src/change/timeline.rs         # bounded window ordering
src-tauri/src/change/association.rs      # change-to-candidate association
src-tauri/src/app/change.rs              # IPC handlers
ui/contracts/ipc.ts                      # mirrored wire contracts
ui/src/change/                           # timeline, detail, candidate section
```

## Data model

### Change source kinds

`EvidenceSourceKind` gains exactly three values:

```rust
#[serde(rename = "github")]
GitHub,
#[serde(rename = "gitlab")]
GitLab,
#[serde(rename = "argo_cd")]
ArgoCd,
```

### The ChangeEvent envelope

```rust
pub type ChangeEventId = Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeEvent {
    pub id: ChangeEventId,
    pub source: EvidenceSourceKind,
    pub kind: ChangeKind,
    pub outcome: ChangeOutcome,
    pub occurred_at: String,
    pub ingested_at: Option<String>,
    pub scope: ResourceScope,
    pub targets: Vec<SignalTarget>,
    pub revision: Option<ChangeRevision>,
    pub actor: ChangeActor,
    pub repository: Option<ChangeRepositoryRef>,
    pub environment: Option<String>,
    pub diff_stat: Option<ChangeDiffStat>,
    pub changed_paths: Vec<String>,
    pub source_link: Option<ChangeSourceLink>,
    pub source_record: SourceRecordRef,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}
```

`targets` reuses the Sprint 13 `SignalTarget` verbatim. This is the mechanism
that makes association exact: a change and a signal that name the same
deployment produce byte-identical target values, so the association is a
comparison, not a heuristic string match.

`occurred_at` is required because a change without a source-supplied timestamp
cannot participate in a timeline or a precedence test. An adapter that finds no
timestamp rejects the record with a typed error rather than substituting
ingestion time.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeOutcome {
    #[serde(rename = "succeeded")]
    Succeeded,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "reverted")]
    Reverted,
    #[serde(rename = "unknown")]
    Unknown,
}
```

`Unknown` is reserved for a source that genuinely reports no outcome. It is not
a fallback for an unparsed field; an unparsed outcome is a typed adapter error.

### Actor and identity safety

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeActorKind {
    #[serde(rename = "human")]
    Human,
    #[serde(rename = "automation")]
    Automation,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeActor {
    pub kind: ChangeActorKind,
    pub handle: Option<String>,
}
```

`handle` holds a source-scoped account handle and nothing else. Email
addresses, display names carrying personal data, tokens and bot credentials are
rejected by safe identity validation, not blanked. A rejected handle yields
`ChangeActorKind::Unknown` with `handle: None` and a typed source status, so the
UI shows an explicit unknown actor rather than an invented one.

### Revision and repository

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRevision {
    pub id: String,
    pub short_id: Option<String>,
    pub parent_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRepositoryRef {
    pub host: String,
    pub namespace: String,
    pub name: String,
    pub reference: Option<String>,
}
```

Repository identity is split into typed parts rather than a single URL string,
so policy classification and masking apply to each part and no credential can
hide in a userinfo component. `reference` is a branch or tag name.

### Diff statistics and changed paths

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeDiffStat {
    pub files_changed: f64,
    pub insertions: f64,
    pub deletions: f64,
    pub unit: NumberUnit,
}
```

`unit` is always `NumberUnit::Count`. All three values are finite and
non-negative; NaN, infinity and negative counts are typed validation errors.

`changed_paths` holds repository-relative paths only. Diff hunk content is
never parsed into a contract, never retained, never serialized and never
rendered. This is the deliberate answer to the fact that a diff body is
unstructured text where field-level masking guarantees do not hold: the safest
handling of secret-bearing content is not to carry it. The exit criterion is met
through the native link, which takes the user to the source diff.

The source record ledger still retains the complete post-policy source payload
for the core, and adapters therefore drop diff-body fields before the record is
admitted, rather than after.

Retention splits across two tables by design: migration `0005` adds
`change_source_record` for change payloads, while evidence rows continue to use
the existing `source_record_evidence` table from migration `0004`. There is one
evidence store, one evidence ID format and one lookup path shared with Sprint 13
correlation evidence.

### Native source links

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeLinkKind {
    #[serde(rename = "commit")]
    Commit,
    #[serde(rename = "pull_request")]
    PullRequest,
    #[serde(rename = "compare")]
    Compare,
    #[serde(rename = "deployment")]
    Deployment,
    #[serde(rename = "application")]
    Application,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeSourceLink {
    pub kind: ChangeLinkKind,
    pub url: String,
}
```

A link is admitted only when it parses as absolute `https`, its host matches the
allowlist for its `EvidenceSourceKind`, it carries no userinfo component, no
query string and no fragment. A query string is rejected outright because that is
where tokens and pagination cursors travel. A link that fails validation is
omitted and reported as a typed source status; it is never emitted unvalidated.

### Change timeline

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeTimeline {
    pub window: TimeWindow,
    pub entry_ids: Vec<ChangeEventId>,
    pub truncated: bool,
}
```

The timeline is a half-open window `[start, end)`, matching the Sprint 13
window convention. Entries are ordered by `(occurred_at, id)` ascending, so ties
resolve deterministically. When the entry count exceeds the request limit the
oldest entries are dropped and `truncated` is `true`; the UI renders an explicit
truncation notice rather than silently showing a partial history.

### Change-to-candidate association

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeAssociation {
    pub change_id: ChangeEventId,
    pub candidate_id: String,
    pub qualification: CorrelationQualification,
    pub lead_time_seconds: f64,
    pub target: Option<SignalTarget>,
    pub topology_path_ids: Vec<String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}
```

`candidate_id` is the existing `CorrelationCandidate.id` string; Sprint 14 does
not change that field's type.

`CorrelationReasonKind` gains one value:

```rust
#[serde(rename = "preceding_change")]
PrecedingChange,
```

`PrecedingChange` always carries `CorrelationQualification::ProbableStructural`,
never `ExactAssociation`, even when the shared target is exact. The target match
is exact; the relevance of the change to the failure is not, and the
qualification describes the latter.

`lead_time_seconds` is the finite, non-negative distance from the change's
`occurred_at` to the earliest `observed_at` among the candidate's signals. It is
a measured interval, not a score.

The association rule, evaluated per candidate:

1. **Precedence.** `change.occurred_at < earliest_signal.observed_at`, and
   `lead_time_seconds <= lookback_seconds`. The interval is half-open on the
   later boundary: a change at exactly the lookback horizon is included, a
   change at exactly the signal observation time is not.
2. **Structure.** The change shares an exact `SignalTarget` with the candidate,
   or the Sprint 12 topology engine returns at least one path between the
   change's target and a candidate target.
3. Both conditions are required. A change that satisfies only precedence is
   present in the timeline and absent from every association list.

`lookback_seconds` defaults to 3,600 and is validated against a maximum of
86,400, matching the Sprint 13 correlation window bound. A candidate whose
signals carry no `observed_at` produces no associations rather than falling back
to ingestion time.

### Change snapshot

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeSnapshot {
    pub generated_at: String,
    pub scope: ResourceScope,
    pub request_window: TimeWindow,
    pub lookback_seconds: f64,
    pub events: Vec<ChangeEvent>,
    pub timeline: ChangeTimeline,
    pub associations: Vec<ChangeAssociation>,
    pub metrics: Vec<ChangeMetric>,
    pub source_statuses: Vec<SourceStatus>,
}
```

The snapshot is evidence-closed: every evidence ID referenced by an event, an
association or a metric resolves inside this snapshot. Every `change_id` in
`associations` appears in `events`, and every `candidate_id` resolves against
the correlation snapshot computed for the same request evaluation time.

`metrics` mirrors the Sprint 13 `CorrelationMetric` shape with its own key
enum, so a change metric can never be confused with a correlation metric:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeMetricKey {
    #[serde(rename = "changes_in_window")]
    ChangesInWindow,
    #[serde(rename = "associated_changes")]
    AssociatedChanges,
    #[serde(rename = "changes_by_source")]
    ChangesBySource,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeMetric {
    pub key: ChangeMetricKey,
    pub source: Option<EvidenceSourceKind>,
    pub value: f64,
    pub unit: NumberUnit,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}
```

Values are finite and `unit` is always `NumberUnit::Count`. `source` is set only
for `ChangesBySource`.

## Adapter fixture catalog

Fixtures live under `docs/superpowers/fixtures/2026-08-29-change/` and are
committed synthetic payloads modeled on the documented provider response
shapes:

| File | Source | Covers |
| --- | --- | --- |
| `github/push.json` | GitHub | commit push, multiple commits, changed-file stats |
| `github/pull-request-merged.json` | GitHub | merged pull request, merge commit, actor handle |
| `github/deployment-status.json` | GitHub | deployment with environment and outcome |
| `gitlab/push.json` | GitLab | commit push, namespace/project split |
| `gitlab/merge-request-merged.json` | GitLab | merged merge request |
| `gitlab/pipeline-deployment.json` | GitLab | deployment outcome including a failed run |
| `argocd/sync-succeeded.json` | Argo CD | application sync with target revision |
| `argocd/sync-failed.json` | Argo CD | failed sync, degraded application |
| `argocd/rollback.json` | Argo CD | rollback to a previous revision |

Every fixture is written to exercise the safety paths as well as the happy
path: at least one record carries an email-shaped actor field that must be
rejected using the reserved-TLD marker
`sprint14-fixture-actor@example.invalid`, which is the only email-shaped string
permitted in any committed fixture; one carries a URL with a query string that
must be dropped, one carries a diff body that must never reach a contract, and one carries unknown
fields that must survive in the retained record.

## Data flow and determinism

For one `change.snapshot` request:

1. Validate the request: scope, window bounds, `lookback_seconds`, limits.
2. Load fixture payloads through the injected fixture clock.
3. Evaluate source and local-storage policy, then admit each post-policy record
   to the append-only ledger with a content digest.
4. Normalize each record into a `ChangeEvent`, rejecting unsafe identities,
   unparsable timestamps and invalid links with typed errors and source
   statuses.
5. Build the timeline over the half-open window with `(occurred_at, id)`
   ordering.
6. Compute the Sprint 13 correlation snapshot for the same evaluation time,
   then evaluate associations against it.
7. Validate evidence closure over the assembled snapshot.
8. Evaluate `EgressDestination::Ui` policy, then serialize.

Determinism obligations: an injected clock, an explicit request evaluation
time, sorted IDs, stable digests and no dependence on fixture file order. The
acceptance suite asserts byte-identical snapshots across repeated runs and
across shuffled fixture input order.

## Trust, capability and policy boundary

### New IPC commands

| Tauri function | Envelope command | Capability | Permission | Payload and return |
| --- | --- | --- | --- | --- |
| `change_snapshot` | `change.snapshot` | `WorkspaceRead` | `Read` | `ChangeRequest` to `ChangeSnapshot` |
| `change_evidence` | `change.evidence` | `ResourceRead` | `Read` | `ChangeEvidenceRequest` to `EvidenceRef[]` |

There is no `change.ingest`, `change.write`, `change.sync`, `change.revert`,
adapter trigger or provider query command. Adapters are internal Rust functions
over committed fixtures.

Both handlers follow the established authorization order: exact
`CommandDescriptor` and capability comparison; envelope scope, membership,
principal, workspace grant and role permission; request parsing and limit
validation; source and local-storage policy; adapter, timeline and association
work; evidence-ID validation against the current snapshot; then
`EgressDestination::Ui` egress policy with verified `Internal` data before
serialization, with audit metadata gated at `EgressDestination::AuditLog`.

`change.evidence` resolves only backend-issued evidence IDs present in the
current snapshot, matching the Sprint 13 `correlation.evidence` contract. It is
not a native record retrieval path.

### Masking, redaction and link policy

Existing recursive masking and policy classification remain authoritative.
Unparsed evidence is not marked masked. Restricted or unverified data fails
closed. A policy denial never degrades a change to an unattributed or healthy
record; it produces a typed source status and omits the record.

Repository host, namespace, name, branch reference, actor handle and changed
paths are classified before retention. A changed path that fails safe-path
validation is dropped from `changed_paths` and reported through source status,
because a path can itself carry a secret-looking token in some repositories.

### Error mapping

| Condition | `IpcErrorCode` |
| --- | --- |
| Malformed request, bad window, out-of-range lookback or limit | `INVALID_REQUEST` |
| Unknown workspace, unknown candidate, unresolvable evidence ID | `NOT_FOUND` |
| Missing capability, grant, membership or role permission | `PERMISSION_DENIED` |
| Source, storage or egress policy denial | `POLICY_DENIED` |
| Fixture payload that cannot be parsed into the contract | `MALFORMED_RESPONSE` |
| Evidence closure or invariant violation | `INTERNAL_ERROR` |

`CONNECTOR_UNAVAILABLE` is not used this sprint; there is no connector to be
unavailable.

## React interaction contract

The Operations Console gains a read-only change view:

- **Change timeline lane.** Ordered entries with source, kind, outcome, actor,
  target and relative time. Truncation is explicit.
- **Change detail.** Revision, repository, environment, diff statistics,
  changed-file paths and the native link, which opens the source in the
  system browser. The panel states plainly that diff content is read at the
  source; it does not present an empty in-app diff viewer.
- **Recent changes on a candidate.** Associations rendered with the localized
  `preceding_change` label, the qualification label, the measured lead time and
  the shared target or topology path that justified the association.

All copy is keyed in `en` and `th`. Localized strings use precedence wording
("changed before", "preceded"), never causal wording. Numbers, timestamps and
lead times are formatted in React from typed values. Every rendered value
carries its evidence IDs and drill-down reference, and a value without verified
evidence is not rendered at all.

## Verification and acceptance

| Gate | Content |
| --- | --- |
| Domain contracts | Every enum wire value, JSON shape, `Option`/null behavior, finite-number validation, evidence and drill-down invariants for all new contracts, plus the extended `ChangeKind` and `EvidenceSourceKind` |
| Sprint 11 reconciliation | `ChangeStreamItem` derived from `ChangeEvent`, typed source field, unchanged locale keys and console layout |
| Adapter replay | Each of the nine fixtures normalizes to the expected event; unknown fields survive in the retained record; diff bodies never reach a contract |
| Identity safety | Email-shaped actors, credentialed URLs, query-string links and unsafe paths are rejected rather than blanked, with typed source statuses |
| Timeline | Half-open window boundaries, `(occurred_at, id)` tie-breaking, truncation flag |
| Association | Precedence boundaries at both edges, lookback cap, exact-target and topology paths, and the negative case: temporal proximity with no structural relationship yields no association |
| Determinism | Byte-identical snapshots across repeated runs and shuffled fixture order |
| Policy and egress | Authorization order, `Ui` and `AuditLog` egress, policy denial paths, evidence closure |
| Secret leak | No credential, token, email or diff body in any fixture, retained record, log line or serialized snapshot |
| UI | Contract validation, en/th locale coverage, no causal copy, truncation and empty states |
| Acceptance | The exit criterion end to end: from a correlation candidate, identify what changed before it and reach the supporting source through the native link |

Release gates are the standard seven, run unpiped with real exit codes:
`npm run format:check`, `npm run lint`, `npm run typecheck`, `npm test`,
`cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`.
