# Sprint 13 Signal Normalization, Security Findings and Correlation Design

**Status:** Design specification
**Date:** 2026-08-28
**Sprint:** 13 — Signal normalization, security findings and correlation

## Goal

Normalize operational and security observations into one source-preserving
Signal contract, then correlate those Signals into explainable, read-only
candidate projections. The projection gives an operator one place to see the
alerts, anomalies and security findings that overlap in time and scope while
keeping every originating record, source identity and evidence reference
reachable.

The Sprint 13 exit criterion is:

> "Alerts, anomalies and normalized vulnerability findings can be correlated into explainable candidates without losing original source references."

The word **candidate** is deliberate. A candidate is a read model for a
possible operational relationship; it is not a canonical Incident, does not
enter an incident lifecycle and cannot perform an action.

## Hard constraint: preserve the originating record

The common envelope is a typed index over a source record, not a replacement
for it. Normalization must never discard, flatten or paraphrase the originating
record. For every admitted Signal:

- the immutable local source-record ledger keeps the complete post-policy,
  structurally faithful JSON record, including fields not understood by the
  adapter;
- `Signal.source_record` identifies the source kind, optional safe native
  identity, revision, deterministic content digest and all evidence IDs for
  that record;
- the normalized fields add stable kind, state, scope, targets and typed facts
  needed for correlation; they do not replace source fields;
- the `CorrelationSnapshot.signals` array contains every admitted Signal used
  by a candidate, and `CorrelationCandidate.signal_ids` must resolve to those
  Signals; and
- a candidate's evidence is the sorted union of every contributing Signal's
  evidence and every topology path evidence reference used by a reason.

The UI can therefore move from a candidate to each contributing Signal, from
that Signal to its source record reference, and from the source reference to
the exact verified EvidenceRef or trusted native source link. A source record
that cannot pass classification, redaction, scope or identity validation is
not silently summarized as a healthy or empty result: it is retained only in
the safe local rejection/audit path and its source status explains why it was
not admitted.

The original record retained by the ledger is the post-policy record. The
existing immutable Restricted-data guard and recursive sensitive-key masking
run before local retention. This preserves the record's shape and unknown
fields without allowing a credential or unmasked Restricted value into a
normalized Signal, finding, reason, log or committed fixture.

## Scope and boundaries

Sprint 13 adds:

- the existing `thalassa_domain::Signal` as the one common normalized
  envelope, with a source-record reference and typed payload facts;
- a typed vulnerability/security finding envelope with source, asset, source
  severity, exploitability, optional finite CVSS score and evidence IDs;
- replayable, committed Trivy, Falco, Kyverno and OPA Gatekeeper fixture
  payloads and adapters that normalize those payloads;
- deterministic deduplication keys that identify the same logical source
  observation without deleting source records;
- explicit event-time correlation windows with half-open boundaries,
  watermarks and late-arrival handling;
- exact grouping by Resource, Service, Deployment and relationships returned by
  the Sprint 12 topology engine;
- structured, evidence-backed correlation reasons that describe association
  rather than causation;
- suppression rules and maintenance windows that silence presentation while
  retaining normalized records, evidence and audit metadata; and
- a read-only correlation snapshot/evidence surface for the Operations Console
  and a localized candidate detail view.

The following remain outside this sprint:

- provisioning infrastructure, running Terraform or OpenTofu, capturing new
  live cloud or cluster data, or adding an outbound network integration;
- live Trivy, Falco, Kyverno or Gatekeeper execution; every initial adapter
  consumes a recorded fixture or an already supplied provider-neutral value;
- creating, updating or transitioning an Incident, incident assignment,
  responder roles, comments, notifications or incident actions; Sprint 15 and
  later own that lifecycle, and the Sprint 11 queue remains a read-only
  projection;
- reimplementing topology traversal, node identity or ownership resolution;
  topology grouping consumes `src-tauri/src/topology/` through its existing
  interface;
- change intelligence, deployment history, AI investigation, model calls,
  mutation proposals, terminal execution or remediation; and
- source-specific query construction or arbitrary source-record lookup from
  React.

## Contract rules carried from Sprints 10–12

These rules apply to every new Rust and TypeScript contract:

1. **One type per concept.** The pre-existing `Signal` is the canonical Signal
   envelope. Do not add `SignalEnvelope`, `NormalizedSignal`, source-specific
   Signal aliases or a second candidate model. Reuse `ResourceScope`,
   `EvidenceRef`, `EvidenceRedaction`, `DrillDownTarget`,
   `DrillDownReference`, `TimeWindow`, `NumberUnit`, `SourceStatus`,
   `ConsoleSeverity`, `HealthCheckOutcome` and the Sprint 12 topology types.
2. **Numbers stay numeric.** Every new numeric value is `f64` in Rust and
   `number` in TypeScript. Non-finite values are rejected with a typed error
   before IPC serialization. CVSS scores are constrained to `0.0..=10.0`.
   Correlation metrics use `f64`; they do not reuse the Sprint 11
   string-valued `CriticalNumber` for new values.
3. **User-visible vocabulary is typed.** Signal kinds, states, finding
   severities, exploitability, candidate status, suppression state, window
   state and correlation reasons are enums. React maps their stable wire
   values to English and Thai i18n keys; Rust never emits user-facing English
   sentences.
4. **Absence is explicit.** Source identity, observed/ingested time, target,
   source severity, exploitability, CVSS score, asset detail and native links
   use `Option`/`null` when the source did not provide them. Empty strings are
   not placeholders and fabricated defaults are not allowed.
5. **Evidence is structural.** Every admitted Signal, finding, candidate,
   correlation reason and displayed metric has verified evidence IDs and a
   typed drill-down reference. A candidate cannot contain a Signal ID that is
   absent from the same snapshot, and a source reference cannot point to
   evidence absent from the admitted evidence set.
6. **Source fidelity is independent from normalized fields.** Unknown source
   fields stay in the local source-record ledger. The common envelope contains
   a reference/digest and typed facts, not a lossy text summary.
7. **Errors remain typed.** Adapter, window, correlation, suppression and
   evidence errors map to distinct existing `IpcErrorCode` values. Error
   details contain fixed safe keys only; source payloads and dynamic query
   text never enter an error.
8. **Sensitive data is rejected or masked before admission.** No credential,
   token, ARN, account ID, subscription ID or pagination cursor enters a
   normalized Signal, finding, correlation reason, log or committed fixture.

## Architecture

```text
Recorded Trivy / Falco / Kyverno / Gatekeeper fixtures
Existing Sprint 11 alerts and anomalies
Existing health-check results and provider-neutral evidence
                         │
                         ▼
             Source adapter contract
      parse → scope → mask/classify → retain source record
                         │
                         ▼
                 Canonical Signal values
       typed facts + SourceRecordRef + EvidenceRef IDs
                         │
              suppression / maintenance evaluation
                         │
                 deterministic dedup index
                         │
       explicit event-time window + late-arrival policy
                         │
          exact target grouping + topology resolver
                         │
                         ▼
               CorrelationCandidate projection
        structural reasons + source IDs + evidence IDs
                         │
          correlation.snapshot / correlation.evidence
                         │
             Operations Console candidate view
```

The source adapter is a deep module: callers provide a replayable payload,
workspace scope, verified evidence and a source-record store; the adapter
returns typed Signals or a fixed typed error. Adapter internals own parsing,
source-specific identity extraction, redaction admission and source-specific
field mapping. They do not expose provider HTTP clients, credentials or
provider queries to the correlation module.

The correlation module is another deep module. Its interface accepts a list of
already normalized Signals, a `CorrelationRequest`, suppression definitions,
and a topology resolver that satisfies the existing Sprint 12 interface. It
returns one deterministic `CorrelationSnapshot`. The implementation owns
sorting, key construction, window membership, grouping, reason construction,
candidate validation and evidence closure.

### Module layout

```text
crates/thalassa-domain/
  src/lib.rs                         # Signal envelope and correlation contracts
  tests/signal_correlation_contracts.rs

crates/thalassa-ipc/
  src/lib.rs                         # correlation.snapshot/evidence descriptors
  tests/contracts.rs

src-tauri/src/correlation/
  mod.rs                             # public orchestration and exports
  fixtures.rs                        # deterministic input catalog and fixture clock
  source_records.rs                  # local-only immutable source-record ledger
  adapters/
    mod.rs                           # SignalAdapter seam and shared admission rules
    trivy.rs                         # Trivy replay adapter
    falco.rs                         # Falco replay adapter
    kyverno.rs                       # Kyverno replay adapter
    gatekeeper.rs                    # OPA Gatekeeper replay adapter
  dedup.rs                            # source-aware logical identity keys
  window.rs                           # event-time membership and late arrivals
  grouping.rs                         # exact target groups and candidate reasons
  suppression.rs                      # rules and maintenance-window matching
  aggregate.rs                        # CorrelationSnapshot projection/validation
  evidence.rs                         # workspace-scoped issued-ID lookup

src-tauri/src/app/correlation.rs      # capability-scoped read IPC handlers
src-tauri/migrations/0003_signal_records.sql
src-tauri/tests/
  signal_adapters.rs                 # common and four-source mappings
  signal_dedup.rs                    # key construction and retention
  signal_windows.rs                  # boundaries and late arrivals
  signal_grouping.rs                 # target/topology grouping and reasons
  signal_suppression.rs              # suppression and maintenance semantics
  signal_ipc.rs                      # authorization, policy and leak scans

docs/superpowers/fixtures/2026-08-28-capture/security/
  trivy.json
  falco.json
  kyverno.json
  gatekeeper.json

ui/contracts/ipc.ts                  # exact TypeScript mirror
ui/src/correlation/
  CorrelationWorkspace.tsx            # request lifecycle and composition
  CandidateList.tsx                   # typed candidate rows
  CandidateDetails.tsx                # reasons, Signals and suppression
  CorrelationEvidencePanel.tsx        # issued-ID evidence lookup
  correlation-fixtures.ts             # copied contract fixture
  correlation-contracts.test.ts
  CorrelationWorkspace.test.tsx
  correlation.acceptance.test.tsx
ui/src/OperationsConsole.tsx           # candidate summary/entry point
ui/src/shell.tsx                       # correlation navigation if needed
ui/src/locales/en.ts
ui/src/locales/th.ts
ui/src/styles.css
```

The domain crate owns the wire model. `src-tauri/src/correlation` re-exports
those types and owns only internal adapters, stores and pure projections; it
does not define a second Rust model. The topology engine remains the only
owner of topology graph traversal and ownership resolution.

## Data model

### Common Signal envelope

The existing `thalassa_domain::Signal` becomes the common envelope. The name
is intentionally unchanged so later Incident work can continue to refer to
the canonical `Signal` concept. Its normalized facts are small and stable;
the source record reference carries everything that is source-specific.

`EvidenceSourceKind` remains the one source-kind enum used by Signals,
findings, EvidenceRef and SourceStatus. Sprint 13 adds these explicit wire
values to that existing enum: `trivy`, `falco`, `kyverno` and
`opa_gatekeeper`. The existing `alertmanager`, `prometheus`, `kubernetes`,
`cloud`, `health_check` and `fixture` values remain unchanged. No adapter
introduces a private source enum or a stringly typed source field.

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Signal {
    pub id: SignalId,
    pub kind: SignalKind,
    pub source: EvidenceSourceKind,
    pub state: SignalState,
    pub observed_at: Option<String>,
    pub ingested_at: Option<String>,
    pub scope: ResourceScope,
    pub targets: Vec<SignalTarget>,
    pub business_severity: Option<ConsoleSeverity>,
    pub payload: SignalPayload,
    pub source_record: SourceRecordRef,
    pub dedup_key: Option<String>,
    pub suppression: SuppressionState,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SignalKind {
    #[serde(rename = "alert")]
    Alert,
    #[serde(rename = "anomaly")]
    Anomaly,
    #[serde(rename = "security_finding")]
    SecurityFinding,
    #[serde(rename = "health_check")]
    HealthCheck,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SignalState {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "cleared")]
    Cleared,
    #[serde(rename = "observed")]
    Observed,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SignalTargetKind {
    #[serde(rename = "resource")]
    Resource,
    #[serde(rename = "service")]
    Service,
    #[serde(rename = "deployment")]
    Deployment,
    #[serde(rename = "topology")]
    Topology,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignalTarget {
    pub kind: SignalTargetKind,
    pub id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceRecordRef {
    pub source_kind: EvidenceSourceKind,
    pub native_id: Option<String>,
    pub revision: Option<String>,
    pub content_digest: String,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum SignalPayload {
    #[serde(rename = "alert")]
    Alert,
    #[serde(rename = "anomaly")]
    Anomaly {
        observed_value: f64,
        comparison_value: f64,
        condition: AnomalyCondition,
    },
    #[serde(rename = "security_finding")]
    SecurityFinding { finding: VulnerabilityFinding },
    #[serde(rename = "health_check")]
    HealthCheck { outcome: HealthCheckOutcome },
}
```

`Signal.id` is deterministic for the source record identity and revision. It
is not a random UUID. The adapter derives it from the source kind, stable
native identity or content digest, and revision using the project's fixed
stable-ID helper. `observed_at` is event time; `ingested_at` is the time the
record entered the local store. Neither is fabricated when absent.

The `payload` variant and `kind` must agree. Alert and health-check details
remain in the source record; anomaly numeric facts are copied as finite
`f64`; a security finding carries the typed finding envelope below. The
parent `evidence_ids` is the sorted union of source-record evidence and
payload evidence. `drill_down.destination` is `evidence` and its IDs must
overlap the Signal evidence set.

#### Source-record fidelity and local storage

`SourceRecordRef` is the only source identity bridge in the domain contract.
It is not a raw-payload field. The internal `SourceRecordStore` writes an
append-only row to the existing local SQLite state using the following
columns:

```text
source_kind, native_id, revision, content_digest, scope,
observed_at, ingested_at, redacted_payload_json, evidence_ids, retained_at
```

`redacted_payload_json` preserves the source JSON object/array structure and
unknown fields after the existing recursive masking and immutable restricted
guard. Rows are keyed by `(source_kind, content_digest, revision)` and are
never overwritten by a later normalization pass. When a native identity is
present, a secondary `(source_kind, native_id, revision)` index rejects a
different content digest as a typed `AmbiguousSourceIdentity` error; the
adapter does not choose one by arrival order. A repeated byte-identical
record may be indexed as a duplicate, but its source reference and evidence
remain reachable.

The store is internal to Rust. React cannot submit a native ID, digest,
source selector, query or URL to retrieve arbitrary records. Evidence lookup
accepts only IDs already issued in a valid snapshot. That keeps the source
record's provenance available without turning IPC into an unrestricted raw
payload reader.

### Mapping existing operational signals

The adapters for existing Sprint 11 producers consume provider-neutral values
directly; they do not re-query Alertmanager or Prometheus.

| Existing value         | Common mapping                                                                                                                                                                                | Retained source facts and evidence                                                                                                                      | Honest missing-data behavior                                                                                                                   |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `NormalizedAlert`      | `kind = Alert`, `source = Alertmanager`, `state` maps firing/resolved to `Active`/`Cleared`, `observed_at = starts_at`, `business_severity` from an existing typed label mapping when present | Alert fingerprint as safe `native_id`, complete masked alert record, labels/annotations/generator reference in the source ledger, Alertmanager evidence | Unresolved `ResourceReference` produces no target and no grouping claim; absent severity/time remains `None` and the source status is retained |
| `AnomalySignal`        | `kind = Anomaly`, `source = Prometheus`, `state = Active`, `observed_at`, finite `observed_value`/`comparison_value` and `condition` in `payload`, `business_severity = signal.severity`      | Rule ID, metric key, source query metadata and metric evidence remain in the source ledger/evidence refs                                                | A missing or ambiguous target is still a normalized Signal with no target; it cannot form a resource/service/deployment/topology group         |
| `HealthCheckResult`    | `kind = HealthCheck`, `source = HealthCheck`, `state` maps healthy/degraded/unavailable/timed-out to `Observed`/`Active`/`Unknown`, payload preserves typed outcome                           | Schedule/run identifiers, audit metadata and check evidence remain source references                                                                    | Skipped outcomes are retained as source records but do not create active candidates                                                            |
| `VulnerabilityFinding` | `kind = SecurityFinding`, `source` is one of Trivy/Falco/Kyverno/OpaGatekeeper, `state` is source status mapping, finding fields are carried in `payload`                                     | Complete masked scanner/policy record, source native identity/digest and evidence refs are retained                                                     | Missing asset or unverified evidence rejects the normalized finding with typed source status; no fabricated asset or severity is emitted       |

Labels, annotations, query strings and provider-specific status text are not
copied into user-facing reason text. They remain source evidence or safe
structured metadata. The UI receives typed enums and localized keys.

### Vulnerability/security finding envelope

The finding envelope is a payload of `SignalPayload::SecurityFinding`; it is
not a second Signal type and cannot exist without its parent source reference.

```rust
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VulnerabilityFinding {
    pub source: EvidenceSourceKind,
    pub asset: FindingAsset,
    pub severity: Option<FindingSeverity>,
    pub exploitability: Option<Exploitability>,
    pub cvss_score: Option<f64>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FindingAssetKind {
    #[serde(rename = "container_image")]
    ContainerImage,
    #[serde(rename = "runtime_resource")]
    RuntimeResource,
    #[serde(rename = "kubernetes_resource")]
    KubernetesResource,
    #[serde(rename = "host")]
    Host,
    #[serde(rename = "policy_subject")]
    PolicySubject,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FindingAsset {
    pub kind: FindingAssetKind,
    pub target: SignalTarget,
    pub display_name: Option<String>,
    pub artifact_digest: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FindingSeverity {
    #[serde(rename = "critical")]
    Critical,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "negligible")]
    Negligible,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Exploitability {
    #[serde(rename = "exploited")]
    Exploited,
    #[serde(rename = "known_exploit")]
    KnownExploit,
    #[serde(rename = "probable")]
    Probable,
    #[serde(rename = "possible")]
    Possible,
    #[serde(rename = "unlikely")]
    Unlikely,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "unknown")]
    Unknown,
}
```

`source` must match the parent Signal and, for this sprint, must be one of
the four initial security sources. A source that does not publish severity,
exploitability or CVSS leaves that field `None`; an explicit source value of
unknown becomes `Some(Unknown)`. A finding is not promoted to an incident
severity. If a business-impact mapping is needed by a later workflow, that
workflow derives it under its own policy and retains this source severity.

`FindingAsset.target.id` is a canonical safe resource/service/deployment ID
or a backend-issued topology node ID. A provider account, subscription,
project, ARN or credential reference is never used as an asset ID. An image
digest is allowed only as a validated artifact digest, not as an account or
registry credential locator. `cvss_score` is finite and within
`0.0..=10.0`; an invalid value rejects the finding rather than serializing
`null`.

### Initial security adapters and their contributions

Every adapter implements the same internal seam:

```rust
pub trait SignalAdapter {
    fn source_kind(&self) -> EvidenceSourceKind;

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError>;
}
```

`ReplayableSignalFixture` is an internal Rust input containing a fixture key,
source kind, workspace scope, recorded `serde_json::Value`, optional event and
ingest times, and already admitted evidence. It is not accepted from React.
The adapter first retains the masked source record, then maps typed facts. A
fixture cannot cause a network request, provider CLI invocation, credential
lookup or recursive Tauri command.

| Adapter        | Source record identity                                                                        | Asset and normalized facts                                                                                         | Severity/exploitability                                                                                                | Evidence behavior                                                                                                                                    |
| -------------- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Trivy          | `VulnerabilityID` plus package/path and image identity; observed scan revision is separate    | `ContainerImage` target for the exact image or resolved deployment, finding identity remains source-qualified      | Maps explicit scanner severity and finite CVSS; exploitability is present only when the payload explicitly provides it | One EvidenceRef per admitted scan result or stable result group; all unknown scan fields remain in the source record                                 |
| Falco          | Event/rule identity plus exact runtime target; event timestamp is not part of the logical key | `RuntimeResource` target for an exact pod/workload/host reference; runtime rule and event facts stay source-backed | Maps an explicit security priority only through the fixed adapter table; absent exploitability remains absent          | Event evidence retains rule, output fields and timestamp after masking; ambiguous target retains a safe source record/status but rejects the finding |
| Kyverno        | Policy, rule, resource identity and violation path                                            | `PolicySubject`/`KubernetesResource` target from exact namespace/kind/name                                         | Uses explicit policy-report severity; exploitability is absent unless a source field says otherwise                    | Evidence points to the policy-report result and retains all report fields, including unknown fields                                                  |
| OPA Gatekeeper | Constraint identity, template, resource identity and violation path                           | `PolicySubject`/`KubernetesResource` target from exact namespace/kind/name                                         | Uses an explicit source severity annotation when safe; exploitability is otherwise absent                              | Evidence points to the violation record and preserves the complete masked constraint payload                                                         |

The initial committed fixture files contain only stable synthetic names, safe
source IDs, timestamps, policy/rule names, package/image names and evidence
references. They contain no credential, token, ARN, account ID, subscription
ID or pagination cursor. A fixture-shape test scans both keys and values for
the forbidden vocabulary before any adapter test runs.

### Deduplication keys

Deduplication identifies the same logical source observation; it is not a
retention or evidence-deletion operation. The wire field is optional because a
source without a safe stable identity cannot honestly be assigned one.

```text
dedup:v1:<source-kind>:<signal-kind>:<stable-identity-digest>
```

`stable-identity-digest` is computed from a canonical, field-labelled tuple
after masking. It excludes observed time, ingest time, evidence IDs, severity,
state and free-form message text so a source update remains the same logical
observation. It includes the source-native identity when present and the
exact safe target identity when the source requires it. The digest is used as
an opaque key and never exposes the tuple to React.

| Source             | Identity tuple (event time and evidence excluded)                         |
| ------------------ | ------------------------------------------------------------------------- |
| Alertmanager       | Alertmanager fingerprint; if absent, safe source digest plus exact target |
| Prometheus anomaly | Rule ID, metric key, condition and exact target                           |
| Trivy              | Vulnerability ID, image/asset identity, package and vulnerable path       |
| Falco              | Rule identity, exact runtime target and source event fingerprint          |
| Kyverno            | Policy/rule identity, exact resource identity and violation path          |
| OPA Gatekeeper     | Constraint/template identity, exact resource identity and violation path  |
| Health check       | Schedule identity and probe key                                           |

Two records are the same logical Signal only when source kind, Signal kind and
the complete source-aware identity tuple produce the same key within the same
workspace. Equal CVE text on two scanners is not a duplicate: Trivy and a
future scanner have different source kinds. Two records with the same key but
different content digests remain separate source revisions and are both
retained. Byte-identical repeated records may share one logical index entry,
but their source evidence remains reachable from the retained record.

If a source has neither a safe native identity nor a safe target, its
`dedup_key` is `None`. The record is still normalized when its evidence and
scope are valid, but it does not participate in deduplication or grouping by
guesswork. No key is formed from a secret-like value, provider account value,
URL credential, pagination cursor or unbounded raw message.

Candidate construction keeps every admitted Signal ID. The dedup index only
prevents one source revision from creating duplicate grouping edges or
duplicate reasons. It never removes a source reference from a candidate.

### Correlation window semantics

The request uses the existing `TimeWindow` contract and adds an explicit
evaluation time and allowed lateness:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationRequest {
    pub window: TimeWindow,
    pub evaluated_at: String,
    pub allowed_lateness_seconds: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CorrelationWindowState {
    #[serde(rename = "open")]
    Open,
    #[serde(rename = "ready_to_finalize")]
    ReadyToFinalize,
    #[serde(rename = "finalized")]
    Finalized,
    #[serde(rename = "reopened")]
    Reopened,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationWindow {
    pub range: TimeWindow,
    pub evaluated_at: String,
    pub watermark: String,
    pub allowed_lateness_seconds: u64,
    pub state: CorrelationWindowState,
}
```

The interval is half-open: `[window.start, window.end)`. A Signal observed
exactly at `start` is included; one observed exactly at `end` belongs to a
later window. `evaluated_at` is explicit and must not come from the wall
clock. The request validator uses fixed bounds of at most 86,400 seconds
(24 hours) for the window and at most 21,600 seconds (6 hours) for allowed
lateness; the initial fixture uses a bounded minutes-scale range. These are
validation limits, not source defaults, and a caller that exceeds either
limit receives a typed invalid-request error.

The watermark is `evaluated_at - allowed_lateness_seconds`. Signals with an
observed time in the range are eligible even when their `ingested_at` is
later than the first evaluation. The window state is:

1. `Open` while `evaluated_at < window.end`;
2. `ReadyToFinalize` while `window.end <= evaluated_at < window.end +
allowed_lateness`;
3. `Finalized` once `evaluated_at >= window.end + allowed_lateness`; and
4. `Reopened` when a later evaluation admits a Signal whose observed time is
   inside a finalized range but whose ingestion arrived after finalization.

The late-arrival policy is **reopen and recompute**. A late Signal inside the
same range is added to the existing candidate component, the candidate keeps
its stable anchor ID, `late_signal_ids` names the late source Signals, and the
window is marked `Reopened`. The candidate is never silently frozen without
the late source. A Signal observed outside the range is not forced into the
candidate; it remains a normalized, evidence-backed Signal in the local store
and is eligible for its correct window. A Signal with no observed time is
retained with explicit absence but cannot be correlated. A future observed
time at or after the exclusive end is rejected from this window, not rewritten
to the evaluation time.

All window membership and late-arrival behavior is deterministic from
`window`, `evaluated_at`, `allowed_lateness_seconds`, `observed_at` and
`ingested_at`. The UI never supplies a provider query or source timestamp
override to alter the source record.

### Grouping by Resource, Service, Deployment and topology

The correlator first admits only Signals whose scope is inside the current
workspace and whose observed time is in the requested window. It then builds
connected components from exact, evidence-backed association edges:

- **Resource:** two Signals share the same `SignalTarget { kind: Resource,
id }`;
- **Service:** two Signals share the same exact Service target ID;
- **Deployment:** two Signals share the same exact Deployment target ID; and
- **Topology:** the existing Sprint 12 topology engine returns a bounded
  `TopologyPath` connecting the Signals' backend-issued node IDs.

Targets are compared as `(kind, id)` values inside the same workspace. Name
similarity, shared labels, equal timestamps, common connector IDs and source
kind alone never create a group. Time overlap is a prerequisite for every
edge, but time proximity alone is not a reason. A component must contain at
least two distinct admitted Signal IDs and at least one association reason;
duplicate records with only one logical key do not create a candidate.

Topology grouping has one seam:

```rust
pub trait TopologyCorrelationResolver {
    fn relation(
        &self,
        left: &SignalTarget,
        right: &SignalTarget,
        window: &CorrelationWindow,
    ) -> Result<Option<TopologyPath>, TopologyError>;
}
```

The production adapter delegates this call to the Sprint 12 topology engine
and passes through its `TopologyPath` ID, direction, termination, confidence
and evidence. The correlation module does not walk adjacency lists, resolve
ownership, infer a new edge or copy a topology graph. If the topology engine
cannot return a verified bounded path, the two Signals remain ungrouped by
topology and the source status records the limitation.

Candidate components are sorted by their smallest stable grouping key. A
candidate ID is a deterministic digest of the window range, sorted grouping
keys and the smallest stable dedup key (or smallest Signal ID when no key
exists). Adding a late Signal to an existing grouping key therefore keeps the
candidate ID stable. Candidate `signal_ids`, grouping targets, reasons,
topology path IDs and evidence IDs are all sorted before validation.

### Explainable correlation reasons

Reasons are structured records, not Rust-produced sentences:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CorrelationReasonKind {
    #[serde(rename = "shared_resource")]
    SharedResource,
    #[serde(rename = "shared_service")]
    SharedService,
    #[serde(rename = "shared_deployment")]
    SharedDeployment,
    #[serde(rename = "topology_relation")]
    TopologyRelation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CorrelationQualification {
    #[serde(rename = "exact_association")]
    ExactAssociation,
    #[serde(rename = "probable_structural")]
    ProbableStructural,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationReason {
    pub kind: CorrelationReasonKind,
    pub qualification: CorrelationQualification,
    pub signal_ids: Vec<SignalId>,
    pub target: Option<SignalTarget>,
    pub topology_path_ids: Vec<String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}
```

`SharedResource`, `SharedService` and `SharedDeployment` require an exact
target and use `ExactAssociation`. `TopologyRelation` requires one or more
Sprint 12 `TopologyPath` IDs and uses `ProbableStructural`. A reason's
evidence IDs include the contributing Signal evidence and the topology path
evidence when present. Its Signal IDs must be a subset of the candidate's
Signal IDs.

There is intentionally no `ProvenCausal`, `RootCause`, `CausedBy` or
probability field. A reason states the structural or exact relationship that
caused grouping, not why an operational failure happened. React maps reason
and qualification enums to localized labels such as “shared resource” and
“probable structural relationship”; target names and source references are
rendered as separate evidence-backed values. Missing or contradictory source
facts appear as typed source status, not an invented explanation.

### Candidate and correlation snapshot

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CandidateStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "provisional")]
    Provisional,
    #[serde(rename = "suppressed")]
    Suppressed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationCandidate {
    pub id: String,
    pub scope: ResourceScope,
    pub window: CorrelationWindow,
    pub signal_ids: Vec<SignalId>,
    pub grouping_targets: Vec<SignalTarget>,
    pub reasons: Vec<CorrelationReason>,
    pub status: CandidateStatus,
    pub late_signal_ids: Vec<SignalId>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CorrelationMetricKey {
    #[serde(rename = "normalized_signals")]
    NormalizedSignals,
    #[serde(rename = "active_candidates")]
    ActiveCandidates,
    #[serde(rename = "suppressed_candidates")]
    SuppressedCandidates,
    #[serde(rename = "uncorrelated_signals")]
    UncorrelatedSignals,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CorrelationMetric {
    pub key: CorrelationMetricKey,
    pub value: f64,
    pub unit: NumberUnit,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CorrelationSummary {
    pub metrics: Vec<CorrelationMetric>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CorrelationSnapshot {
    pub generated_at: String,
    pub scope: ResourceScope,
    pub request: CorrelationRequest,
    pub window: CorrelationWindow,
    pub summary: CorrelationSummary,
    pub signals: Vec<Signal>,
    pub candidates: Vec<CorrelationCandidate>,
    pub topology_paths: Vec<TopologyPath>,
    pub source_status: Vec<SourceStatus>,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationEvidenceRequest {
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}
```

`CorrelationCandidate` has no incident ID, IncidentStatus, disposition,
owner, action or write timestamp. `signal_ids` is the complete contributing
set, not only one representative per dedup key. The candidate evidence set
must include every Signal's evidence and every reason's evidence. Its
drill-down opens the evidence panel and never executes a provider query.

`CorrelationMetric.value` is a finite `f64` with `NumberUnit::Count`; its
evidence IDs are the sorted union of exactly the records counted. A missing
source yields an omitted metric plus a typed SourceStatus, never a guessed
zero. The summary uses a vector keyed by `CorrelationMetricKey` so the UI
cannot infer a value from an absent field.

### Suppression and maintenance windows

Suppression is a presentation and candidate-eligibility decision, not data
deletion and not an Incident disposition. Definitions are internal policy
inputs in this sprint; no UI command creates or edits them.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SuppressionKind {
    #[serde(rename = "not_suppressed")]
    NotSuppressed,
    #[serde(rename = "rule")]
    Rule,
    #[serde(rename = "maintenance_window")]
    MaintenanceWindow,
    #[serde(rename = "rule_and_maintenance_window")]
    RuleAndMaintenanceWindow,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuppressionState {
    pub kind: SuppressionKind,
    pub rule_ids: Vec<String>,
    pub maintenance_window_ids: Vec<String>,
    pub evaluated_at: String,
    pub policy_version: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuppressionRule {
    pub id: String,
    pub enabled: bool,
    pub scope: ResourceScope,
    pub source: Option<EvidenceSourceKind>,
    pub signal_kind: Option<SignalKind>,
    pub target: Option<SignalTarget>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MaintenanceWindowReason {
    #[serde(rename = "planned_change")]
    PlannedChange,
    #[serde(rename = "routine_maintenance")]
    RoutineMaintenance,
    #[serde(rename = "security_testing")]
    SecurityTesting,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MaintenanceWindow {
    pub id: String,
    pub enabled: bool,
    pub scope: ResourceScope,
    pub target: Option<SignalTarget>,
    pub window: TimeWindow,
    pub reason: MaintenanceWindowReason,
    pub policy_version: u64,
}
```

A rule matches when it is enabled, its scope contains the Signal scope, each
present source/kind selector equals the Signal, and its present target equals
one of the Signal targets. A maintenance window additionally requires the
Signal observed time to be inside `[window.start, window.end)`. A null target
matches every target in the scoped Signal; a target is never inferred from a
name. Multiple matching rules and windows are all retained in sorted ID order
and produce `RuleAndMaintenanceWindow` when both categories match.

Suppression is evaluated after source admission and before grouping. A
suppressed Signal still records its Signal ID, source kind, native identity,
content digest, observed/ingested times, scope, targets, typed payload,
deduplication key, all EvidenceRef IDs, matching rule/window IDs, evaluation
time, policy version and candidate membership. If every Signal in an eligible
component is suppressed, the component is returned as a `Suppressed`
candidate so the operator can explain the silence. A mixed component remains
`Active` (or `Provisional` when late) and retains each suppressed Signal as
context. A singleton suppressed Signal is present in `signals` but does not
become a candidate because correlation requires an association between at
least two Signals.

Candidate status precedence is deterministic: all-suppressed →
`Suppressed`; otherwise any late Signal in an open/reopened window →
`Provisional`; otherwise → `Active`. Suppression never becomes
`IncidentDisposition::Suppressed`, never changes source severity and never
authorizes a mutation. Audit retention stores only typed policy IDs/version,
scope and outcome; it contains no raw source payload or credentials.

## Adapter fixture catalog

The four fixture payloads are committed under
`docs/superpowers/fixtures/2026-08-28-capture/security/`. They are deterministic
synthetic records evaluated at the shared Sprint 13 fixture clock
`2026-08-28T09:00:00Z`:

- `trivy.json` contains one container image result with a stable vulnerability
  ID, package, installed version, high severity, finite CVSS score and a
  deployment/image target;
- `falco.json` contains one runtime rule event with a stable rule identity,
  exact pod target, typed priority and event timestamp;
- `kyverno.json` contains one failed policy report with policy/rule identity,
  exact namespace/kind/name target and explicit severity; and
- `gatekeeper.json` contains one constraint violation with template/constraint
  identity, exact resource target and a safe violation path.

Each fixture includes a source evidence ID and a safe fixture endpoint. The
fixture catalog also includes the Sprint 11 Alertmanager alert and Prometheus
anomaly values, a late-arrival record, an exact shared-service pair, an exact
deployment pair, a topology relation returned by the Sprint 12 engine, one
active maintenance window and one rule-suppressed Signal. Fixture order is
intentionally shuffled in tests to prove output ordering is independent of
input order.

Fixtures do not contain live provider responses, credentials, authorization
headers, ARNs, account/subscription IDs, pagination cursors or new cloud/cluster
captures. A forbidden-data scan runs on the fixture bytes and serialized
outputs.

## Data flow and deterministic correlation algorithm

The aggregator runs these phases in order:

1. **Validate request and workspace.** Parse an exact
   `CorrelationRequest`, verify RFC3339 times, `start < end`, bounded window
   length, `evaluated_at >= start`, allowed lateness limits and current
   workspace scope. Reject duplicate request fields or unknown source
   selectors before adapter work.
2. **Normalize source inputs.** Run the four replay adapters plus the
   provider-neutral Alertmanager, Prometheus and health-check adapters. Each
   adapter checks scope, masks/classifies the record, writes the safe source
   record, validates evidence, derives a deterministic Signal ID and returns
   a typed Signal. Invalid records create SourceStatus and do not become
   healthy zeroes or candidate evidence.
3. **Evaluate suppression.** Match all rules and maintenance windows against
   each admitted Signal. Store the complete `SuppressionState`; do not remove
   the Signal or its source evidence.
4. **Build the dedup index.** Compute source-aware keys, sort Signals by
   observed time/source/content digest/Signal ID, and collapse only repeated
   association edges. The full Signal set remains in the snapshot and local
   source-record ledger.
5. **Assign window membership.** Include only Signals in the half-open range
   with an observed timestamp. Mark late members and determine the window
   state from the explicit evaluation time. Missing/future/out-of-range
   records remain retained but cannot be forced into this candidate set.
6. **Build exact association edges.** Connect Signals sharing an exact
   Resource, Service or Deployment target and eligible event window. Ask the
   Sprint 12 topology resolver for bounded relationships between unresolved
   target kinds. A failed resolver call does not create a fallback edge.
7. **Emit components and reasons.** Keep components with at least two
   distinct Signal IDs and one evidence-backed reason. Build one structured
   `CorrelationReason` per unique association, use `ProbableStructural` only
   for topology relations, calculate status/late IDs, and derive a stable
   candidate ID from the component anchor.
8. **Close evidence.** Intern only verified EvidenceRef values. Validate every
   Signal, finding, reason, topology path, candidate and metric reference,
   including candidate-to-Signal and reason-to-candidate subsets. Reject an
   invariant violation as `INTERNAL_ERROR`; never serialize a partial
   candidate.
9. **Apply egress policy and serialize.** Retain audit metadata only after
   `AuditLog` policy approval, then evaluate `Ui` egress with verified
   `Internal` data before returning `IpcResult::Ok`.

Input sorting and candidate construction are pure functions. Equal fixture
inputs, request and policy version produce byte-identical snapshots. No
background scheduler, sleep, provider request or wall-clock call is allowed.

## Trust, capability and policy boundary

### New IPC commands

Only these two read commands cross the Tauri boundary:

| Tauri function         | Envelope command       | Capability      | Permission | Scope                                                                     | Payload/return                                 |
| ---------------------- | ---------------------- | --------------- | ---------- | ------------------------------------------------------------------------- | ---------------------------------------------- |
| `correlation_snapshot` | `correlation.snapshot` | `WorkspaceRead` | `Read`     | Unbounded envelope resolved to current workspace                          | `CorrelationRequest` → `CorrelationSnapshot`   |
| `correlation_evidence` | `correlation.evidence` | `ResourceRead`  | `Read`     | Unbounded envelope resolved to current workspace; IDs checked server-side | `CorrelationEvidenceRequest` → `EvidenceRef[]` |

There is no `signal.ingest`, `finding.write`, `correlation.write`,
`correlation.act`, `incident.write`, provider query, adapter trigger or
maintenance-window mutation command in Sprint 13. Adapters are internal Rust
functions over committed fixtures. Suppression and maintenance definitions
are supplied by the local policy/fixture layer, not React.

Both handlers use the established authorization order:

1. construct the exact `CommandDescriptor` from `thalassa-ipc` and compare
   command name and capability;
2. reject a bounded/unexpected envelope scope, inactive membership, principal
   mismatch, missing current-workspace grant or a role without `Read`;
3. parse the exact request shape and validate timestamps, ranges, limits and
   IDs before source or evidence work;
4. evaluate source/local-storage policy before retaining or normalizing any
   record;
5. run adapters, suppression, deduplication, windows, topology delegation and
   snapshot validation;
6. resolve evidence IDs only from the current validated snapshot for
   `correlation.evidence`; and
7. evaluate `EgressDestination::Ui` with verified `Internal` data before
   serializing the response. Audit metadata also requires verified
   `Internal` data at `EgressDestination::AuditLog`.

If a later live source is added, it must use that source connector's existing
read capability and transport policy, including
`EgressDestination::ExternalIntegration` with verified data. The correlation
capability does not grant connector access, credential resolution or a new
HTTP path.

### Masking, redaction and source-reference policy

The existing policy and masking modules remain authoritative:

- JSON source records pass through the existing recursive sensitive-key
  masking path before the source-record store or EvidenceRef admission;
- unparsed content is marked `unparsed` and is never marked `masked`; the UI
  displays that distinction explicitly;
- source adapter values such as `password`, `secret`, `token`, `key`,
  `credential`, authorization headers, cookies and private keys are masked or
  rejected before they can enter normalized fields;
- values matching credential-like token patterns, ARNs, cloud account or
  subscription identifiers, pagination cursor fields or connector credential
  references are rejected from Signals, findings, reasons, logs and fixtures;
- source/native IDs and asset display names pass a safe-identifier/text guard;
  an unsafe identity causes an explicit source rejection instead of a blank
  label or fabricated key;
- `EvidenceRef` is admitted only when both classification and redaction are
  verified and when its scope is inside the current workspace;
- the local source ledger uses `EgressDestination::LocalStorage` with verified
  `Internal` data. A Restricted or unverified record fails closed and is not
  normalized; and
- UI, AuditLog and any future ExternalIntegration policy denials return a
  typed `POLICY_DENIED` result. A denial never downgrades a source to an
  unattributed candidate.

Evidence IDs are backend-issued opaque strings. `correlation.evidence`
rejects empty, duplicate, unknown, cross-workspace or unverified IDs and
resolves the complete request before returning anything. Native links are
copied only from admitted EvidenceRef values and opened through the existing
HTTPS/trusted-source shell guard; React cannot construct one from a target ID.

### Error mapping

The correlation module exposes typed internal errors with fixed safe
diagnostic variants. AppState maps them as follows:

| Internal condition                                                       | `IpcErrorCode`                          | UI behavior                                     |
| ------------------------------------------------------------------------ | --------------------------------------- | ----------------------------------------------- |
| malformed request, duplicate IDs, invalid range or non-finite input      | `INVALID_REQUEST`                       | localized invalid-request state                 |
| unknown evidence, unknown backend-issued Signal/topology reference       | `NOT_FOUND`                             | localized unavailable reference                 |
| command/capability/scope/membership/role failure                         | `PERMISSION_DENIED`                     | localized access-denied state                   |
| unverified data, restricted record, redaction/storage/UI/AuditLog denial | `POLICY_DENIED`                         | localized policy-blocked state                  |
| malformed replay payload or unsupported source schema                    | `MALFORMED_RESPONSE`                    | localized source-malformed state                |
| valid source failure that leaves a partial snapshot                      | snapshot `Ok` plus typed `SourceStatus` | preserve healthy sources and show source status |
| candidate/evidence invariant failure after valid input                   | `INTERNAL_ERROR`                        | fail closed; never serialize partial output     |

No error variant includes a raw provider body, query, authorization header,
credential reference or source payload. Source-level failures are visible in
`SourceStatus` when the rest of the snapshot remains valid; they never become
an inferred healthy state.

## React interaction contract

The correlation surface is a read-only candidate view reachable from the
Operations Console's signal/correlation area. It may be a dedicated
`Correlation` navigation area if the shell needs a full workspace, but it
does not create an Incident route or an incident detail model.

The UI behavior is:

1. send a complete `CorrelationRequest` with an explicit UTC `TimeWindow`,
   evaluation time and allowed lateness through `correlation.snapshot`;
2. render candidate status, source kinds, Signal IDs, source native identity
   when present, target kinds, typed reasons, qualification and suppression
   state; all labels come from locale keys;
3. show each candidate's contributing Signal as an expandable evidence-backed
   row. The UI obtains IDs only from `Signal.evidence_ids`,
   `CorrelationReason.evidence_ids`, `CorrelationCandidate.evidence_ids` or
   returned Sprint 12 topology paths;
4. call `correlation.evidence` only with backend-issued IDs. The evidence panel
   reuses source, endpoint, query, observed time, excerpt and masking state,
   and opens only an existing trusted HTTPS native link;
5. display `ProbableStructural` as “probable structural relationship” and
   never render “root cause”, “caused by”, “confirmed dependency” or an
   equivalent causal claim;
6. show suppressed Signals and candidates with typed rule/window IDs and
   policy version, while making clear that suppression hides attention but
   does not delete the source record; and
7. render empty, stale, unavailable, unverified, malformed, late/reopened
   and policy-denied states through typed localized copy. Status and severity
   use text and accessible indicators in addition to color.

Summary counts use `CorrelationMetric.value` as `number`; the UI formats the
number at render time and never changes it to a string in the contract. A
missing metric is rendered as unavailable, not as zero. Keyboard focus and
screen-reader labels expose every candidate, Signal, reason and evidence
control.

## Verification and acceptance

Rust contract, adapter and projection tests must cover:

- explicit JSON values for every new enum, the additive security evidence
  source kinds and the two IPC descriptors;
- round-trip field-name/nullability parity between Rust and TypeScript,
  including `Option`/`null`, `f64`/`number` and `SignalPayload` variants;
- common Alertmanager, Prometheus anomaly and health-check mapping, including
  source-record digest/reference, unknown-field retention and unresolved
  targets;
- each Trivy, Falco, Kyverno and OPA Gatekeeper fixture adapter, including
  source, asset kind/target, severity, exploitability absence/presence,
  finite CVSS, evidence IDs and malformed/unsupported payloads;
- scope, classification, redaction, forbidden-data and safe-identity
  admission, with no credential/ARN/account/subscription/cursor in a Signal,
  finding, reason, fixture, log or serialized result;
- deduplication tuple construction, exclusion of time/evidence/severity,
  cross-source non-deduplication, missing-identity `None`, conflicting native
  identity and retention of every source reference;
- correlation range start-inclusive/end-exclusive boundaries, missing/future
  timestamps, explicit evaluation time, watermark states and reopen-and-
  recompute late arrival behavior;
- exact Resource/Service/Deployment grouping, no time-only/shared-label
  grouping, deterministic candidate IDs/order, complete Signal ID sets and
  exact reason/evidence subsets;
- topology grouping through the Sprint 12 resolver, path evidence passthrough,
  bounded traversal delegation and probable-structural qualification without
  any causal/root-cause field;
- suppression-rule matching, maintenance-window boundaries, multiple-match
  retention, mixed/all-suppressed candidate status and preserved source
  evidence/audit metadata;
- finite `CorrelationMetric` values and typed rejection of NaN/infinite CVSS,
  metric or topology values; and
- `correlation.snapshot` and `correlation.evidence` command, capability,
  scope, membership, role, payload, source-policy, UI-policy and AuditLog
  policy failures with distinct `IpcErrorCode` mapping.

React tests must cover:

- the copied four-source fixture rendering before live adapters exist;
- the explicit request shape and deterministic candidate list;
- each candidate expanding to every contributing source Signal and evidence
  reference;
- shared Resource/Service/Deployment reasons and a topology reason labelled
  probable structural, never causal;
- late/reopened, suppressed, maintenance-window and mixed-candidate states;
- f64/number metric rendering, omitted metrics and no fabricated zero;
- source/query/time-window/excerpt/masked/unparsed evidence presentation and
  trusted HTTPS native-link handling;
- keyboard navigation, focus, screen-reader labels and status text
  independent of color; and
- identical English/Thai locale object structure.

The fixture acceptance journey must show at least one Alertmanager alert, one
Prometheus anomaly and one normalized security finding in a candidate, open
each contributing source evidence reference, show an exact grouping reason,
show a topology relation as probable structural, and show a suppressed
Signal whose source record remains available. It must make no network call,
provisioning call, provider CLI call, Incident write or mutation.

The sprint is accepted only when the following statement is observable in the
validated fixture snapshot and its evidence controls:

> "Alerts, anomalies and normalized vulnerability findings can be correlated into explainable candidates without losing original source references."
