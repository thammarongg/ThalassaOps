# Sprint 11 Operations Console Design

**Status:** Design specification
**Date:** 2026-08-28
**Sprint:** 11 — Operations Console

## Goal

Build the primary home experience as a business-impact-first operations
console. A responder should see the most important health, active attention,
signal, change and environment information in one local-first projection, and
every critical number must lead to the evidence that produced it.

The Sprint 11 exit criterion is:

> "A user can open the application and understand what needs attention within 30 seconds."

## Scope and boundaries

Sprint 11 adds a read-only Operations Console projection and the deterministic
producers needed to populate it:

- a business-impact-first health summary;
- an active incident queue projection;
- alert, anomaly and scheduled-check counts;
- a threshold and rate-of-change anomaly evaluator over metric fixtures;
- a scheduled health-check evaluator with interval, scope, timeout, cooldown
  and audit metadata;
- a recent change stream projection;
- an environment status projection;
- a curated dashboard widget registry with local presentation preferences; and
- a typed drill-down contract from each critical number to evidence.

Producers are fixture-driven and deterministic in this sprint. They consume
explicit fixture inputs and an explicit evaluation timestamp; they do not call
the wall clock, sleep to simulate a timeout, or infer missing data.

The aggregation layer reuses the existing connector, observability and cloud
modules as data sources behind provider-neutral source adapters. It does not
reimplement provider URLs, credential resolution, HTTP policy, or response
masking. The UI consumes the aggregation contract and never imports provider
modules.

The following are outside this sprint:

- provisioning cloud or Kubernetes infrastructure, running Terraform or
  OpenTofu, capturing new live cloud fixtures, or adding an external network
  integration;
- live producer scheduling, a background daemon, or a provider-side scheduler;
  opening the console evaluates due fixture schedules with an explicit clock;
- signal normalization, deduplication, correlation windows, suppression,
  maintenance windows or explainable correlation reasons, which belong to
  Sprint 13;
- service/resource topology, dependency paths or blast-radius inference, which
  belong to Sprint 12;
- the canonical incident lifecycle, incident writes, responder roles and
  incident actions, which belong to Sprint 15 and later;
- AI investigation, model calls, mutation proposals, terminal execution and
  remediation; and
- custom dashboard queries, arbitrary widget code, arbitrary provider URLs or
  user-configured rules.

The active queue is therefore a read-only console projection. Its items retain
their source identity and evidence, but Sprint 11 does not claim that two
items are correlated into one canonical Incident.

## Architecture

```text
React Operations Console
        │  operations.snapshot / operations.evidence
        ▼
Capability, membership, scope and policy checks in AppState
        │
        ▼
Operations aggregation layer
  ├── connector source adapter
  ├── observability source adapter
  ├── cloud/environment source adapter
  ├── Kubernetes source adapter
  ├── deterministic anomaly evaluator
  ├── deterministic scheduled-check evaluator
  ├── fixture change source
  └── projection + evidence/drill-down validator
        │
        ▼
OperationsSnapshot
  ├── health summary
  ├── active incident queue (uncorrelated projection)
  ├── alert/anomaly/check summary
  ├── recent changes
  ├── environment status
  ├── evidence summaries
  └── curated widget definitions
```

The aggregation layer is a boundary, not another provider. It accepts
provider-neutral source results such as `NormalizedAlert`, `MetricSeries`,
`CloudEnvironment`, `CloudResource`, connector summaries and Kubernetes
inventory. A source adapter may use an existing module's fixture implementation
in Sprint 11 and can later delegate to that module's live connector without
changing the console contract.

The aggregation layer performs projection only:

1. validate that each source result is in the requested workspace scope;
2. evaluate the anomaly rules and due health-check schedules against fixtures;
3. retain each alert, anomaly and check as an independent queue candidate;
4. calculate business-impact-first counts and health state;
5. attach or intern redacted evidence references;
6. validate every critical number's evidence and drill-down references; and
7. sort the result with deterministic tie-breakers before serializing it.

No source adapter may call another Tauri command recursively. Rust code calls
the existing provider-neutral functions directly, while the two new Tauri
commands perform authorization once at the boundary.

### Module layout

```text
src-tauri/src/
  operations/
    mod.rs             public operation contracts and module exports
    model.rs           wire enums, projections, evidence and widget types
    fixtures.rs        deterministic source, rule, schedule and change fixtures
    anomaly.rs         threshold and rate-of-change evaluator
    health_check.rs    due evaluation, timeout/cooldown and audit metadata
    aggregate.rs       source adapters and OperationsSnapshot projection
    evidence.rs        scoped evidence lookup and redaction validation
  app/
    operations.rs      capability-scoped operations.snapshot/evidence commands

ui/
  contracts/ipc.ts     TypeScript mirror of the Rust operations contract
  src/
    OperationsConsole.tsx
    operations/
      WidgetFrame.tsx
      HealthSummary.tsx
      IncidentQueue.tsx
      SignalSummary.tsx
      ChangeStream.tsx
      EnvironmentStatus.tsx
      DrillDown.tsx
      widgetConfig.ts
```

The projection types are exported from the canonical `thalassa-domain` crate
so Rust producers and future IPC adapters share one symmetric contract.
Producer implementations stay under `src-tauri/src/operations`; existing
`Signal`, `Incident`, `Evidence` and `Audit` types remain the long-lived
workflow contracts. Sprint 11's console types are read models that can be
replaced or mapped when Sprint 13 and Sprint 15 deliver their canonical
workflows.

## Data model

All types crossing IPC use the repository's stable JSON field names (snake
case) and explicit enum wire values. The snippets below are the contract
shape; implementation may use Rust type aliases where that does not change the
serialized form.

### Shared evidence and drill-down contract

Evidence is interned once per snapshot and referenced by ID. A source result
must provide a stable fixture key; the aggregator derives a stable evidence ID
from that key and the source query/window. IDs must not be random per refresh,
so a UI can retain a selected evidence item while the console refreshes.

```rust
pub type ConsoleEvidenceId = String;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EvidenceSourceKind {
    #[serde(rename = "alertmanager")]
    Alertmanager,
    #[serde(rename = "prometheus")]
    Prometheus,
    #[serde(rename = "kubernetes")]
    Kubernetes,
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "health_check")]
    HealthCheck,
    #[serde(rename = "fixture")]
    Fixture,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceRedaction {
    pub classification_verified: bool,
    pub redaction_verified: bool,
    pub masked: bool,
    pub unparsed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceRef {
    pub id: ConsoleEvidenceId,
    pub source_kind: EvidenceSourceKind,
    pub connector_id: Option<String>,
    pub scope: ResourceScope,
    pub endpoint: String,
    pub query: Option<String>,
    pub observed_at: String,
    pub excerpt: String,
    pub native_url: Option<String>,
    pub redaction: EvidenceRedaction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DrillDownDestination {
    #[serde(rename = "evidence")]
    Evidence,
    #[serde(rename = "incident_queue")]
    IncidentQueue,
    #[serde(rename = "signal_summary")]
    SignalSummary,
    #[serde(rename = "change_stream")]
    ChangeStream,
    #[serde(rename = "environment_status")]
    EnvironmentStatus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrillDownTarget {
    pub destination: DrillDownDestination,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub filter_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationsEvidenceRequest {
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CriticalNumber {
    pub key: String,
    pub value: String,
    pub unit: NumberUnit,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimeWindow {
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrillDownReference {
    pub source_query: String,
    pub scope: ResourceScope,
    pub time_window: Option<TimeWindow>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum NumberUnit {
    #[serde(rename = "count")]
    Count,
    #[serde(rename = "percentage")]
    Percentage,
    #[serde(rename = "milliseconds")]
    Milliseconds,
    #[serde(rename = "seconds")]
    Seconds,
}
```

`CriticalNumber.value` is a canonical decimal string so counts, percentages
and future high-precision values cannot lose precision at the Rust/TypeScript
boundary. The UI formats the value using `unit` and localized labels; the
backend does not send preformatted prose.

The aggregator rejects a snapshot when a critical number has no evidence IDs,
when an ID is not present in the snapshot evidence set, or when its
`drill_down.evidence_ids` or `drill_down_reference.evidence_ids` does not
identify at least one of the number's evidence sources. The reference also
retains the source query, scope and optional time window. This is an invariant,
not a convention. A number that has no source is omitted from the projection
and represented by a source-unavailable state rather than guessed.

The evidence endpoint accepts only IDs emitted by the aggregator and resolves
them within the current workspace. Its request never accepts a user-supplied
provider URL, query or arbitrary source selector. A native link is copied from
an existing trusted source reference and is opened only through the existing
shell permission.

### Business-impact-first health summary

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConsoleHealthState {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "degraded")]
    Degraded,
    #[serde(rename = "critical")]
    Critical,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ImpactLevel {
    #[serde(rename = "critical")]
    Critical,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "none")]
    None,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConsoleSeverity {
    #[serde(rename = "S1")]
    S1,
    #[serde(rename = "S2")]
    S2,
    #[serde(rename = "S3")]
    S3,
    #[serde(rename = "S4")]
    S4,
    #[serde(rename = "S5")]
    S5,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ConsolePriority {
    #[serde(rename = "P1")]
    P1,
    #[serde(rename = "P2")]
    P2,
    #[serde(rename = "P3")]
    P3,
    #[serde(rename = "P4")]
    P4,
    #[serde(rename = "P5")]
    P5,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BusinessImpact {
    pub level: ImpactLevel,
    pub summary: String,
    pub customer_scope: String,
    pub service_criticality: String,
    pub trajectory: ImpactTrajectory,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ImpactTrajectory {
    #[serde(rename = "expanding")]
    Expanding,
    #[serde(rename = "stable")]
    Stable,
    #[serde(rename = "improving")]
    Improving,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HealthSummary {
    pub state: ConsoleHealthState,
    pub headline: BusinessImpact,
    pub attention: CriticalNumber,
    pub impacted_services: CriticalNumber,
    pub active_by_severity: Vec<CriticalNumber>,
    pub environments_by_state: Vec<CriticalNumber>,
    pub contributing_scopes: Vec<ContributingScope>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContributingScope {
    pub scope: ResourceScope,
    pub impact: ImpactLevel,
    pub summary: String,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}
```

The headline is selected from the highest business impact represented in the
fixture set. Severity is based on the accepted S1–S5 business-impact baseline;
priority and urgency are not collapsed into severity. The console may show a
derived queue ordering value later, but Sprint 11 does not invent a new
priority policy. `active_by_severity` and `environments_by_state` are arrays of
`CriticalNumber`, not bare numeric maps, so their click targets remain
evidence-backed.

The initial state calculation is deterministic:

1. `Critical` when any active item is S1 or any critical-scope source reports
   an unavailable state with verified evidence;
2. `Degraded` when attention exists without the condition above, or a source is
   stale;
3. `Healthy` when all required sources are healthy and no active item requires
   attention; and
4. `Unknown` when required source evidence is unavailable or unverified.

No raw CPU, memory or provider status is allowed to outrank business impact by
itself. Such values can be evidence for a queue item or a drill-down, but they
do not become the headline without an impact fixture.

### Active incident queue projection

Sprint 11 uses a projection because the canonical Incident entity and lifecycle
are later deliverables. Queue items retain enough shape for triage while
preserving source identity and avoiding an implicit correlation claim.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum QueueItemSourceKind {
    #[serde(rename = "alert")]
    Alert,
    #[serde(rename = "anomaly")]
    Anomaly,
    #[serde(rename = "scheduled_health_check")]
    ScheduledHealthCheck,
    #[serde(rename = "fixture_incident")]
    FixtureIncident,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum QueueStatus {
    #[serde(rename = "detected")]
    Detected,
    #[serde(rename = "triage")]
    Triage,
    #[serde(rename = "investigating")]
    Investigating,
    #[serde(rename = "mitigating")]
    Mitigating,
    #[serde(rename = "monitoring")]
    Monitoring,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentQueueItem {
    pub id: String,
    pub title: String,
    pub source_kind: QueueItemSourceKind,
    pub source_id: String,
    pub severity: ConsoleSeverity,
    pub priority: Option<ConsolePriority>,
    pub status: QueueStatus,
    pub business_impact: BusinessImpact,
    pub scope: ResourceScope,
    pub detected_at: String,
    pub opened_at: String,
    pub last_update: String,
    pub affected_scope: ResourceScope,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}
```

An item is active when its status is one of the five queue statuses. Resolved,
closed and dispositions are not represented as active queue items. Each alert,
anomaly or scheduled check remains a separate item even when its labels or
scope look similar; Sprint 13 owns correlation. Queue ordering is severity
ascending (`S1` first), then status order (`detected` through `monitoring`),
then `detected_at` descending, then stable `id` ascending. `priority` is
optional source data and is never synthesized from severity when absent.

### Alert, anomaly and health-check summary

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignalSummary {
    pub active_alerts: CriticalNumber,
    pub active_anomalies: CriticalNumber,
    pub checks_due: CriticalNumber,
    pub checks_timed_out: CriticalNumber,
    pub by_source: Vec<SignalCount>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignalCount {
    pub source_kind: QueueItemSourceKind,
    pub count: CriticalNumber,
}
```

Counts are calculated from the source records included in this snapshot. A
record with insufficient metric data is not counted as an active anomaly; it
is retained in producer diagnostics and represented as source freshness when
needed. A timed-out health check is counted separately from checks due so a
user can distinguish pending work from a failed probe.

Only `degraded`, `unavailable` and `timed_out` health-check outcomes with a
verified evidence ID become active queue items. `healthy`, `skipped_not_due`,
`skipped_cooldown` and `skipped_disabled` results remain in the signal summary
and audit metadata but do not create attention without evidence.

### Rule-based anomaly model

Rules operate on explicit `MetricFixture` inputs shaped like the existing
Prometheus metric result. The evaluator never issues a PromQL request and does
not infer a series from labels.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnomalyRule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub scope: ResourceScope,
    pub metric_key: String,
    pub condition: AnomalyCondition,
    pub severity: ConsoleSeverity,
    pub cooldown_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AnomalyCondition {
    #[serde(rename = "threshold")]
    Threshold {
        operator: ThresholdOperator,
        threshold: String,
    },
    #[serde(rename = "rate_of_change")]
    RateOfChange {
        direction: RateDirection,
        threshold_per_second: String,
        window_seconds: u64,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ThresholdOperator {
    #[serde(rename = "gt")]
    GreaterThan,
    #[serde(rename = "gte")]
    GreaterThanOrEqual,
    #[serde(rename = "lt")]
    LessThan,
    #[serde(rename = "lte")]
    LessThanOrEqual,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RateDirection {
    #[serde(rename = "increase")]
    Increase,
    #[serde(rename = "decrease")]
    Decrease,
    #[serde(rename = "absolute")]
    Absolute,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MetricFixture {
    pub key: String,
    pub scope: ResourceScope,
    pub labels: std::collections::BTreeMap<String, String>,
    pub samples: Vec<MetricFixtureSample>,
    pub source: MetricFixtureSource,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MetricFixtureSample {
    pub timestamp_seconds: i64,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MetricFixtureSource {
    pub connector_id: String,
    pub query: String,
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnomalySignal {
    pub id: String,
    pub rule_id: String,
    pub metric_key: String,
    pub severity: ConsoleSeverity,
    pub observed_at: String,
    pub observed_value: f64,
    pub comparison_value: f64,
    pub condition: AnomalyCondition,
    pub scope: ResourceScope,
    pub evidence_id: ConsoleEvidenceId,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AnomalyEvaluationStatus {
    #[serde(rename = "triggered")]
    Triggered,
    #[serde(rename = "not_triggered")]
    NotTriggered,
    #[serde(rename = "insufficient_data")]
    InsufficientData,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnomalyEvaluation {
    pub rule_id: String,
    pub metric_key: String,
    pub status: AnomalyEvaluationStatus,
    pub signal: Option<AnomalySignal>,
}
```

The threshold evaluator parses the latest sample as a finite decimal and
applies the configured operator. The rate-of-change evaluator uses the first
and latest samples inside the rule's `window_seconds`:

```text
rate_per_second = (latest_value - first_value) /
                  (latest_timestamp_seconds - first_timestamp_seconds)
```

`increase` requires the rate to be at least the configured positive threshold,
`decrease` requires it to be at most the negative threshold, and `absolute`
compares the absolute rate. Fewer than two samples, a zero or negative time
delta, a non-finite number or a window with no samples yields
`insufficient_data`, not a signal. The rule's `cooldown_seconds` is carried in
the fixture state for deterministic suppression by the aggregator; the
evaluator itself remains a pure comparison.

Rules must be enabled, have a non-blank ID/name/metric key, use a positive
rate window, and contain finite decimal thresholds. A fixture metric key must
match exactly one series. Duplicate rule IDs are rejected. The signal ID is a
stable digest of `rule_id`, metric key, observed timestamp and condition, not a
random UUID.

### Scheduled health-check model

Health checks use the same explicit-clock pattern. Sprint 11 evaluates due
checks when the console snapshot is built; it does not install a timer or make
an external request.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HealthCheckSchedule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub scope: ResourceScope,
    pub source: HealthCheckSource,
    pub interval_seconds: u64,
    pub timeout_ms: u64,
    pub cooldown_seconds: u64,
    pub last_run_at: Option<String>,
    pub last_signal_at: Option<String>,
    pub defined_by: Option<String>,
    pub defined_at: Option<String>,
    pub last_outcome: Option<HealthCheckOutcome>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FixtureHealthCheck {
    pub outcome: HealthCheckOutcome,
    pub duration_ms: u64,
    pub evidence_id: Option<ConsoleEvidenceId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HealthCheckSource {
    #[serde(rename = "connector")]
    Connector { connector_id: String, probe_key: String },
    #[serde(rename = "kubernetes")]
    Kubernetes { connector_id: String, resource_key: String },
    #[serde(rename = "observability")]
    Observability { connector_id: String, probe_key: String },
    #[serde(rename = "fixture")]
    Fixture { fixture_key: String },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HealthCheckOutcome {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "degraded")]
    Degraded,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "timed_out")]
    TimedOut,
    #[serde(rename = "skipped_not_due")]
    SkippedNotDue,
    #[serde(rename = "skipped_cooldown")]
    SkippedCooldown,
    #[serde(rename = "skipped_disabled")]
    SkippedDisabled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HealthCheckAudit {
    pub run_id: String,
    pub schedule_id: String,
    pub triggered_by: String,
    pub started_at: String,
    pub completed_at: String,
    pub duration_ms: u64,
    pub scope: ResourceScope,
    pub source: HealthCheckSource,
    pub outcome: HealthCheckOutcome,
    pub cooldown_suppressed: bool,
    pub policy_version: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HealthCheckResult {
    pub schedule_id: String,
    pub outcome: HealthCheckOutcome,
    pub observed_at: String,
    pub evidence_id: Option<ConsoleEvidenceId>,
    pub audit: HealthCheckAudit,
}
```

The due and cooldown rules are explicit:

- a disabled schedule returns `skipped_disabled` and records no provider
  operation;
- a schedule is not due when `now < last_run_at + interval_seconds`;
- after a signal, `now < last_signal_at + cooldown_seconds` returns
  `skipped_cooldown` and does not re-emit the same signal; and
- otherwise the fixture probe runs once. Its fixture `duration_ms` is compared
  to `timeout_ms`; a duration greater than the timeout returns `timed_out`
  with `duration_ms == timeout_ms`, without sleeping.

`run_id` is deterministic from schedule ID and evaluation time. `triggered_by`
is `"scheduler"` for snapshot evaluation and `"operator"` only if a future
manual trigger is added. The audit record contains scope, source, timing,
outcome and policy version, but never credentials, authorization headers or
raw provider response bodies. A local audit record may be retained by the
existing local-first state layer; no external audit integration is added in
this sprint. `defined_by`, `defined_at` and `last_outcome` are optional
definition metadata; absent source data stays absent rather than being
represented by an empty string.

### Recent change stream

Sprint 11's changes are fixture records. GitHub, GitLab, Argo CD and other
delivery integrations remain future source adapters.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeKind {
    #[serde(rename = "deployment")]
    Deployment,
    #[serde(rename = "configuration")]
    Configuration,
    #[serde(rename = "maintenance")]
    Maintenance,
    #[serde(rename = "connector")]
    Connector,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeStreamItem {
    pub id: String,
    pub source: Option<String>,
    pub occurred_at: String,
    pub kind: ChangeKind,
    pub summary: String,
    pub actor: Option<String>,
    pub target_resource: Option<String>,
    pub native_link: Option<String>,
    pub scope: ResourceScope,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
}
```

Changes sort by `occurred_at` descending and stable ID ascending. The stream
does not assert that a change caused an alert or anomaly; it only presents
recent evidence for the responder to inspect. Change summaries and actors are
fixture data and are subject to the same masking and classification rules as
all other evidence.

### Environment status overview

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvironmentStatus {
    pub environment_id: String,
    pub name: String,
    pub provider: Option<String>,
    pub health: ConsoleHealthState,
    pub status_detail: String,
    pub resource_count: CriticalNumber,
    pub last_observed_at: String,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
}
```

The overview is provider-neutral and reuses the existing cloud and Kubernetes
resource results. A failed environment remains visible with `Unknown` or
`Unavailable` state and its remedy/source evidence; it does not blank other
environments. Provider-specific console links remain in evidence or the
existing cloud resource contract rather than being copied into a new UI-only
provider branch.

### Operations snapshot

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationsSnapshot {
    pub generated_at: String,
    pub scope: ResourceScope,
    pub source_status: Vec<SourceStatus>,
    pub health_summary: HealthSummary,
    pub incident_queue: Vec<IncidentQueueItem>,
    pub signal_summary: SignalSummary,
    pub changes: Vec<ChangeStreamItem>,
    pub environments: Vec<EnvironmentStatus>,
    pub evidence: Vec<EvidenceRef>,
    pub widget_registry: Vec<WidgetDefinition>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SourceState {
    #[serde(rename = "fresh")]
    Fresh,
    #[serde(rename = "stale")]
    Stale,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "unverified")]
    Unverified,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceStatus {
    pub source_key: String,
    pub state: SourceState,
    pub observed_at: Option<String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}
```

The snapshot is a read model, not an event log. `generated_at` is the explicit
fixture evaluation time. `source_status` makes stale, unavailable and
unverified inputs visible so the console never turns missing data into a
healthy claim.

## Curated dashboard widget model

Widget customization is presentation state. It is stored in the existing UI
local storage path under a versioned key; it is not policy data, an external
query, or a source of authorization. The backend owns the registry and the
critical-number/evidence invariants. The UI owns order, size, collapse state
and optional visibility within that registry.

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WidgetId {
    #[serde(rename = "health_summary")]
    HealthSummary,
    #[serde(rename = "incident_queue")]
    IncidentQueue,
    #[serde(rename = "signal_summary")]
    SignalSummary,
    #[serde(rename = "change_stream")]
    ChangeStream,
    #[serde(rename = "environment_status")]
    EnvironmentStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WidgetSize {
    #[serde(rename = "compact")]
    Compact,
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "wide")]
    Wide,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WidgetDefinition {
    pub id: WidgetId,
    pub title_key: String,
    pub default_order: u16,
    pub default_size: WidgetSize,
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WidgetPreference {
    pub id: WidgetId,
    pub visible: bool,
    pub order: u16,
    pub size: WidgetSize,
    pub collapsed: bool,
}

pub type WidgetOptions = std::collections::BTreeMap<String, serde_json::Value>;

pub type WidgetKind = WidgetId;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WidgetConfig {
    pub id: WidgetId,
    pub kind: WidgetKind,
    pub visible: bool,
    pub order: u16,
    pub options: WidgetOptions,
}

pub fn curated_default_layout() -> Vec<WidgetConfig>;
```

The registry is fixed to the five widgets above. `health_summary` and
`incident_queue` are required and cannot be hidden or moved below optional
widgets in a way that obscures critical attention. Unknown IDs, duplicate IDs,
negative/overflowing orders and invalid JSON are discarded and replaced with
the default preference. No preference can remove a critical number or alter
its source query.

## Data flow and freshness

```text
Fixture catalog / existing source contracts
        │
        ├── metric fixtures → AnomalyEngine
        ├── schedule fixtures + explicit now → HealthCheckScheduler
        ├── alert/environment/connector fixtures → source adapters
        └── change fixtures → change projection
                         │
                         ▼
                 AggregationInput
                         │
                         ▼
              deterministic OperationsSnapshot
```

Each source contributes a `SourceStatus`. A source with no fixture is
`unavailable`; a fixture older than the source freshness threshold is `stale`;
an item whose classification/redaction cannot be verified is `unverified`.
The aggregator may still return healthy source results alongside one failed
source, but the headline cannot claim `Healthy` when a required source is
unknown. Every displayed number is computed from the records actually present
and links to the evidence set used in that computation.

The default fixture set should demonstrate the entire home narrative in one
snapshot: one high-impact active alert, one threshold anomaly, one
rate-of-change anomaly, one due healthy check, one timed-out check, one
cooldown-suppressed check, at least two environments with different health
states, and at least two recent changes. The timeout is represented by a
separate `check-worker-timeout` schedule whose fixture duration exceeds its
timeout; the cooldown case is `check-db-health`. Stable timestamps are
supplied by the fixture clock so the acceptance test does not drift.

## Trust, capability and policy boundary

### New IPC commands

Sprint 11 exposes only the commands required by the home projection and its
evidence panel. The Tauri function names use underscores, while the command
envelope names continue to use lowercase `resource.verb` components.

| Tauri function | Envelope command | Capability | Permission | Scope | Purpose |
| --- | --- | --- | --- | --- | --- |
| `operations_snapshot` | `operations.snapshot` | `WorkspaceRead` | `Read` | Unbounded envelope resolved to the current workspace | Return the redacted, deterministic home projection. |
| `operations_evidence` | `operations.evidence` | `ResourceRead` | `Read` | Unbounded envelope resolved to the current workspace; evidence IDs are workspace-scoped server-side | Return evidence details for IDs already emitted by a snapshot. |

The anomaly and health-check producers are internal Rust services called by
the aggregation layer. They are not IPC commands and therefore cannot become
an accidental capability bypass. They perform no external mutation. A future
manual health-check trigger would require a separate design and capability
review.

Both commands use the established authorization order:

1. construct a `CommandDescriptor` with the exact command, capability and
   `Permission::Read`;
2. reject a mismatched command or capability, a bounded/unexpected envelope
   scope, an inactive membership, a principal mismatch, a membership grant
   outside the current workspace, or a role without `Read`;
3. resolve the request payload and reject malformed or unknown evidence IDs;
4. evaluate source-specific policy before any external connector access;
5. run the provider-neutral source/aggregation operation; and
6. evaluate the `Ui` egress policy before serializing the `IpcResult`.

`operations.snapshot` reads fixture data in this sprint, so it has no live
external request. Its source adapter still carries the policy seam: if an
existing connector is used for a future non-fixture source, the adapter must
evaluate `EgressDestination::ExternalIntegration` with verified `Internal`
data before delegating to that connector. It must not call a provider client
directly or accept a URL/query from React. `operations.evidence` reads the
already captured, redacted workspace evidence and performs no external
request.

Any local audit metadata emitted by a health-check run is evaluated against the
existing `AuditLog` destination with verified `Internal` data before local
retention. A policy denial fails closed; it does not downgrade the check to an
unattributed result.

No Sprint 11 command requires `IncidentWrite`, `ConnectorAct`,
`PolicyManage`, `ExecuteAction` or a new capability. The console cannot create
incidents, change a connector, execute Terraform, invoke a provider CLI,
mutate a resource or authorize an action.

### Connector capability reuse

The aggregation layer consumes existing connector contracts and their declared
capabilities:

- connector summaries use the existing `ConnectorRead` path;
- Kubernetes/environment data uses the existing read-only Kubernetes or cloud
  source capabilities and their existing membership/policy checks;
- Prometheus/Alertmanager data uses the existing observability `ResourceRead`
  commands and fixed GET adapters; and
- fixture sources implement the same provider-neutral result shapes without
  opening a network connection.

The aggregation layer does not infer that a connector declaration grants a
user permission. Connector manifests describe adapter capability; the
`AppState` command boundary remains the authorization boundary.

### Masking and redaction

The existing observability and Kubernetes masking behavior applies before any
data enters the aggregation layer:

- `observability::masking::sensitive_key` and
  `mask_json_object` remain the shared sensitive-field-name deny list;
- parsed JSON log/manifest fields are recursively masked before
  serialization, while unparsed lines are explicitly marked unparsed and are
  never labelled as masked;
- cloud credential resolution and signed/bearer authorization headers remain
  transient and are never included in connector summaries, evidence, audit
  metadata, fixtures or `IpcResult` values;
- provider failures retain only the existing sanitized status/service error,
  never a response body, authorization header or credential reference; and
- evidence excerpts are admitted only when `classification_verified` and
  `redaction_verified` are true. An excerpt with an immutable Restricted
  value is omitted and the source is marked `unverified`.

The Policy Runtime remains the authority for egress decisions. The baseline's
immutable Restricted-data rule, fail-closed behavior, separate UI,
local-storage, external-integration and audit destinations, and versioned
policy metadata all continue to apply. Sprint 11 does not add value-pattern
redaction or weaken the existing name-based masking; a later policy/redaction
change must update the shared masking and evidence admission path together.

The UI may display a local masked excerpt and a clear `masked`/`unparsed`
status, but it cannot mark raw data safe. The aggregator copies only source
references, verified excerpts and typed metadata into `EvidenceRef`.

### Error and partial-source behavior

Errors use the existing serializable `{ code, message, details }` shape. A
malformed fixture, missing evidence, denied policy or failed source is never
converted into a healthy zero. The snapshot can contain healthy results from
other sources, while `source_status` and the affected drill-down show the
failure. The UI renders a localized unavailable/stale/unverified state and
keeps unaffected environments and queue items visible.

## UI composition and interaction

The home route renders the widgets in this order by default:

1. `HealthSummary` — impact headline, attention count and environment/service
   health counts;
2. `IncidentQueue` — highest-impact active items with severity and status;
3. `SignalSummary` — active alerts, anomalies and scheduled-check state;
4. `ChangeStream` — recent deployment/configuration/maintenance evidence; and
5. `EnvironmentStatus` — provider-neutral environment health and access.

The first viewport must contain the health summary and the first queue item on
the default fixture size. The console is optimized for triage rather than
deep analysis: raw metrics, logs and traces remain in their existing routes.

Every numeric card renders a `CriticalNumber` and exposes a keyboard-focusable
drill-down control. The control opens the evidence panel or the typed local
destination, passes only the backend-issued evidence IDs, and shows source,
query, timestamp/window, excerpt and masking status. If a number has no
evidence, the component refuses to render it as a critical metric and shows a
localized unavailable state.

Status is never conveyed by color alone. Severity/state labels, text and
accessible status indicators remain visible. All new strings are added to the
English and Thai catalogs with identical object structure, and all controls
remain keyboard reachable with the existing focus styles.

## Verification and acceptance

Rust tests use deterministic fixture data and local-only code paths. They must
cover:

- exact JSON serialization for every new IPC enum and the Rust/TypeScript
  contract shape;
- threshold operators, rate-of-change direction/window math, insufficient
  data, invalid rules and stable anomaly IDs;
- interval due checks, scope validation, timeout classification,
  cooldown-suppressed runs and complete audit metadata without credentials;
- aggregation counts, business-impact-first state selection, deterministic
  ordering, partial-source behavior and the no-correlation invariant;
- rejection of any critical number without a valid evidence/drill-down
  reference;
- command name/capability/scope/membership/role failures, malformed payloads,
  unknown evidence IDs and UI/external/audit policy denials; and
- serialized-result scans proving no credential, authorization header,
  credential reference, unmasked Restricted value or raw provider error body
  enters the snapshot/evidence result.

React tests use fixtures copied from the Rust contract tests. They must cover:

- the command-center journey from a blank shell to a visible business impact
  headline, active queue and signal counts;
- independent rendering of a failed source alongside healthy environments;
- curated widget preferences (order, size, collapse and optional visibility)
  while required health/incident widgets remain visible;
- keyboard and screen-reader access to every critical number's drill-down;
- evidence source/query/time-window and masking state in the drill-down panel;
- honest stale, unavailable, unverified, empty and malformed states; and
- identical English/Thai locale object structure.

The sprint is accepted only when the fixture journey is deterministic, no
live infrastructure or new network integration is required, no mutation is
possible from the console, and the exact exit criterion quoted above is
observable in the UI behavior.
