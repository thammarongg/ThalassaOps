# Sprint 11 Operations Console Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Build a deterministic, read-only Operations Console projection that lets a user identify business impact, active attention and the next evidence-backed drill-down within 30 seconds.

**Architecture:** Add provider-neutral operations contracts, deterministic anomaly and scheduled-health-check producers, and an aggregation layer that projects existing connector, observability, cloud and Kubernetes fixture shapes into one OperationsSnapshot. Expose only capability-scoped operations.snapshot and operations.evidence commands, then render the snapshot through curated React widgets whose local preferences cannot hide critical attention.

**Tech Stack:** Rust 2021, Tauri 2, Tokio, Serde, SQLite/local-first state already in the repository, React 18, TypeScript, Vite, Vitest, Testing Library, existing connector/observability/cloud fixture contracts.

**Spec:** docs/design/sprint-11-operations-console.md.

## Global Constraints

- Producers are fixture-driven and deterministic in this sprint; every evaluation receives an explicit timestamp and never reads the wall clock or sleeps to simulate a timeout.
- Do not provision cloud or Kubernetes infrastructure, run Terraform or OpenTofu, capture new live cloud fixtures, or add a new external network integration.
- Reuse the existing connector, observability and cloud modules as data sources behind provider-neutral aggregation adapters; do not reimplement credential resolution, provider URLs, HTTP policy or masking.
- Signal normalization, deduplication, correlation windows, suppression and explainable correlation reasons belong to Sprint 13 and are not implemented here.
- Service/resource topology, dependency paths and blast-radius inference belong to Sprint 12 and are not implemented here.
- The console projection does not create or mutate canonical incidents; the canonical incident lifecycle belongs to Sprint 15 and later.
- Every critical number is a CriticalNumber with non-empty evidence IDs and a typed DrillDownTarget; no bare critical count or percentage may cross IPC.
- Every enum crossing IPC declares explicit Serde wire values and has a Rust serialization test whose shape is copied into TypeScript contract fixtures.
- New IPC commands are exactly operations.snapshot (WorkspaceRead, Read) and operations.evidence (ResourceRead, Read); producers remain internal Rust services.
- Command authorization checks command name, capability, unbounded envelope scope, active membership, principal identity, workspace grant and role permission before operation work; UI egress is checked before return.
- Any future non-fixture source read evaluates EgressDestination::ExternalIntegration with verified Internal data before delegating to an existing connector; local evidence passes Ui and, when retained, AuditLog policy checks.
- Existing observability/Kubernetes recursive sensitive-field masking, unparsed-line warnings, transient cloud credentials, sanitized connector errors and immutable Restricted-data fail-closed policy remain in force.
- The UI cannot accept arbitrary provider URLs, source queries, widget code, rule definitions or mutation commands; native links are trusted source references opened through existing shell permission.
- English and Thai locale objects remain structurally identical, status never relies on color alone, and keyboard focus styles remain usable.
- Run npm ci before any frontend gate. A gate that cannot run is blocked and must be reported; it is not a passing gate.
- The exact exit criterion is: "A user can open the application and understand what needs attention within 30 seconds."

---

### Task 2: Define Operations Console contracts and deterministic fixture catalog

**Files:**
- Create: src-tauri/src/operations/mod.rs
- Create: src-tauri/src/operations/model.rs
- Create: src-tauri/src/operations/fixtures.rs
- Create: src-tauri/tests/operations_contracts.rs
- Modify: src-tauri/src/lib.rs
- Modify: ui/contracts/ipc.ts
- Create: ui/src/operations/contract-fixtures.test.ts

**Interfaces:**
- Consumes: existing ResourceScope, MetricSeries, NormalizedAlert, CloudEnvironment, CloudResource, KubernetesInventory and connector summaries.
- Produces: OperationsSnapshot, HealthSummary, CriticalNumber, EvidenceRef, EvidenceRedaction, DrillDownTarget, IncidentQueueItem, SignalSummary, ChangeStreamItem, EnvironmentStatus, AnomalyRule, MetricFixture, AnomalySignal, HealthCheckSchedule, HealthCheckResult, HealthCheckAudit, WidgetDefinition, WidgetId, WidgetPreference, FixtureCatalog and every explicit wire enum in the design.

**Tests to add:**

- wire_enums_serialize_to_the_documented_values asserts literal JSON for every operation enum.
- fixture_catalog_is_stable_and_contains_each_sprint_signal asserts fixed IDs/time, alert, threshold rule, rate rule, due/timeout/cooldown checks, environments and changes.
- critical_numbers_reference_existing_evidence_and_drill_downs recursively checks every number-bearing field.
- typescript_fixture_matches_the_rust_wire_shape checks copied TypeScript fixture keys and union values.

- [ ] **Step 1: Write failing contract tests**

Create Rust tests with literal expectations:

~~~rust
#[test]
fn wire_enums_serialize_to_the_documented_values() {
    assert_eq!(serde_json::to_value(EvidenceSourceKind::Alertmanager).unwrap(), json!("alertmanager"));
    assert_eq!(serde_json::to_value(DrillDownDestination::EnvironmentStatus).unwrap(), json!("environment_status"));
    assert_eq!(serde_json::to_value(NumberUnit::Percentage).unwrap(), json!("percentage"));
    assert_eq!(serde_json::to_value(ConsoleHealthState::Critical).unwrap(), json!("critical"));
    assert_eq!(serde_json::to_value(ConsoleSeverity::S1).unwrap(), json!("S1"));
    assert_eq!(serde_json::to_value(QueueItemSourceKind::ScheduledHealthCheck).unwrap(), json!("scheduled_health_check"));
    assert_eq!(serde_json::to_value(HealthCheckOutcome::TimedOut).unwrap(), json!("timed_out"));
    assert_eq!(serde_json::to_value(WidgetId::HealthSummary).unwrap(), json!("health_summary"));
}
~~~

- [ ] **Step 2: Run focused tests**

Run: cargo test -p thalassaops --test operations_contracts

Expected: FAIL because the operations module, contracts and fixture catalog do not exist.

- [ ] **Step 3: Implement model contracts and validation**

Copy the model shapes from the design specification with snake-case fields and explicit serde rename variants. Include EvidenceRef.scope so evidence lookup can enforce workspace scope. Define ConsoleEvidenceId as String and CriticalNumber.value as a decimal string. Implement OperationsSnapshot::critical_numbers() and OperationsSnapshot::validate(); validation rejects empty/unknown evidence, missing shared drill-down evidence, duplicate evidence content, duplicate queue/change IDs and duplicate/unknown widget IDs. Errors use fixed safe messages and never interpolate source payloads.

- [ ] **Step 4: Implement the fixed fixture catalog**

Define FixtureCatalog with alerts, metrics, anomaly rules, health checks, fixture check results, changes, environments and evidence. Expose fixture_catalog() and fixture_time().

Use stable time 2026-08-28T09:00:00Z; alert alert-checkout-s1; metric metric-cpu-prod with 70 then 92; metric metric-error-rate-prod with 0.010 then 0.080; rules rule-cpu-threshold (gt 90) and rule-error-rate-rise (increase 0.0005/second, 60-second window); schedules check-api-health (interval 300, timeout 1000, cooldown 0), check-db-health (interval 60, timeout 250, cooldown 600, prior signal at 08:59:30Z) and check-worker-timeout (interval 30, timeout 100, cooldown 0, fixture duration 250); environments env-aws-prod and env-gcp-staging; changes change-payment-deploy and change-db-config. Give every metric an explicit ResourceScope and every source record a stable evidence ID.

- [ ] **Step 5: Export and mirror the contract**

Declare pub mod operations in src-tauri/src/lib.rs and re-export public contracts. Add exact TypeScript unions/object shapes to ui/contracts/ipc.ts:

~~~ts
export type OperationsSnapshotRequest = null;
export type OperationsEvidenceRequest = { evidence_ids: string[] };
~~~

The TypeScript mirror uses the same snake-case keys and strings asserted by Rust. Do not add a second UI-only representation of critical numbers.

- [ ] **Step 6: Add fixture-shape test and run suites**

In ui/src/operations/contract-fixtures.test.ts, load a literal copied from Rust serialization, assert health_summary.attention.evidence_ids.length > 0 and assert widget IDs belong to the five documented IDs. Run:

~~~bash
cargo test -p thalassaops --test operations_contracts
npm ci
npm test -- ui/src/operations/contract-fixtures.test.ts
npm run typecheck
~~~

Expected: PASS, with the TypeScript fixture copied from asserted Rust JSON.

- [ ] **Step 7: Commit**

~~~bash
git add src-tauri/src/operations src-tauri/src/lib.rs src-tauri/tests/operations_contracts.rs ui/contracts/ipc.ts ui/src/operations/contract-fixtures.test.ts
git commit -m "feat: define the operations console contracts and fixtures"
~~~

**Acceptance criteria:**

- Every operation enum serializes to the documented value.
- The default catalog is deterministic and contains alert, threshold, rate, health-check, environment and change records.
- Snapshot validation structurally requires evidence-backed critical numbers.
- Rust and TypeScript field names and enum values are identical.

---

### Task 3: Implement the threshold and rate-of-change anomaly producer

**Files:**
- Create: src-tauri/src/operations/anomaly.rs
- Modify: src-tauri/src/operations/mod.rs
- Modify: src-tauri/src/operations/model.rs
- Modify: src-tauri/src/operations/fixtures.rs
- Create: src-tauri/tests/operations_anomaly.rs

**Interfaces:**
- Consumes: AnomalyRule, MetricFixture, MetricFixtureSample, AnomalyCondition, ConsoleSeverity, ResourceScope and explicit evaluation time.
- Produces: AnomalyEvaluationStatus, AnomalyEvaluation, AnomalyError, evaluate_rule(rule, metric, evaluated_at) and evaluate_rules(rules, metrics, evaluated_at).

**Tests to add:**

- threshold gt/gte/lt/lte boundary tests;
- rate increase/decrease/absolute direction tests using exact seconds math;
- window filtering, insufficient data, zero/negative delta and non-finite value tests;
- invalid rule, duplicate ID, missing metric and ambiguous metric tests;
- scope mismatch test; and
- stable ID/evidence and no-credential serialization tests.

- [ ] **Step 1: Write failing anomaly test**

Use Task 2 fixtures:

~~~rust
#[test]
fn threshold_and_rate_rules_emit_distinct_deterministic_signals() {
    let catalog = fixture_catalog();
    let values = evaluate_rules(&catalog.anomaly_rules, &catalog.metrics, fixture_time()).unwrap();
    let signals: Vec<_> = values.into_iter().filter_map(|item| item.signal).collect();
    assert_eq!(signals.len(), 2);
    assert_eq!(signals[0].rule_id, "rule-cpu-threshold");
    assert_eq!(signals[0].observed_value, "92");
    assert_eq!(signals[1].rule_id, "rule-error-rate-rise");
    assert_eq!(signals[1].comparison_value, "0.0011666666666666667");
}
~~~

- [ ] **Step 2: Run focused test**

Run: cargo test -p thalassaops --test operations_anomaly

Expected: FAIL because the evaluator does not exist.

- [ ] **Step 3: Add evaluation and error types**

Define AnomalyEvaluationStatus with triggered, not_triggered and insufficient_data wire values, plus:

~~~rust
pub struct AnomalyEvaluation {
    pub rule_id: String,
    pub metric_key: String,
    pub status: AnomalyEvaluationStatus,
    pub signal: Option<AnomalySignal>,
}

pub enum AnomalyError {
    InvalidRule(String),
    DuplicateRuleId(String),
    MetricNotFound(String),
    AmbiguousMetric(String),
    ScopeMismatch(String),
    InvalidSample(String),
}
~~~

Insufficient data is an honest non-signal result. Error messages are fixed safe descriptions.

- [ ] **Step 4: Implement threshold evaluation**

Require one exact metric-key match, parse latest value and threshold as finite f64 values, apply gt/gte/lt/lte exactly, and emit only on a match. Copy original value/threshold strings, condition and source evidence ID into the signal.

- [ ] **Step 5: Implement rate evaluation**

Keep samples within the configured window relative to latest. Fewer than two samples, non-positive delta or non-finite values produce insufficient_data. Calculate (latest - first) / delta_seconds and compare increase/decrease/absolute without rounding before comparison. Store a stable decimal comparison string and derive signal ID from rule ID, metric key, condition and evaluation timestamp.

- [ ] **Step 6: Enforce ordering and scope**

Sort rules and metrics by stable ID/key, reject duplicate rules before evaluation, and require metric scope to be equal to or narrower than rule scope using ResourceScope::contains. Do not correlate or deduplicate signals.

- [ ] **Step 7: Run and commit**

Run:

~~~bash
cargo test -p thalassaops --test operations_anomaly
cargo test --workspace
cargo fmt --all -- --check
~~~

Expected: PASS with no HTTP call or new dependency.

~~~bash
git add src-tauri/src/operations/anomaly.rs src-tauri/src/operations/model.rs src-tauri/src/operations/fixtures.rs src-tauri/src/operations/mod.rs src-tauri/tests/operations_anomaly.rs
git commit -m "feat: add deterministic threshold and rate anomaly evaluation"
~~~

**Acceptance criteria:**

- Fixture threshold and rate rules emit the expected two signals with stable IDs/evidence.
- Comparison boundaries, directions, windows, invalid data and invalid configuration are tested.
- No signal is correlated, deduplicated or inferred from out-of-scope data.
- Equal inputs and timestamp produce byte-identical output.

---

### Task 4: Implement the scheduled health-check producer

**Files:**
- Create: src-tauri/src/operations/health_check.rs
- Modify: src-tauri/src/operations/mod.rs
- Modify: src-tauri/src/operations/model.rs
- Modify: src-tauri/src/operations/fixtures.rs
- Create: src-tauri/tests/operations_health_check.rs

**Interfaces:**
- Consumes: HealthCheckSchedule, FixtureHealthCheck, HealthCheckSource, HealthCheckOutcome, ResourceScope, explicit DateTime<Utc> and policy version.
- Produces: HealthCheckError, DueState, HealthCheckResult, is_due(schedule, now), run_due_checks(schedules, fixtures, now, policy_version) and audit_for(result).

**Tests to add:**

- interval due/not-due and disabled tests;
- scope preservation in result/audit;
- timeout test proving no sleep and exact timeout duration;
- cooldown suppression test;
- deterministic run ID and complete audit metadata test; and
- serialized audit/result scan for credentials, authorization headers and raw provider bodies.

- [ ] **Step 1: Write failing scheduler test**

Use fixed times:

~~~rust
#[test]
fn interval_timeout_cooldown_and_audit_are_deterministic() {
    let catalog = fixture_catalog();
    let runs = run_due_checks(
        &catalog.health_checks,
        &catalog.health_check_results,
        fixture_time(),
        7,
    ).unwrap();
    let api = runs.iter().find(|run| run.schedule_id == "check-api-health").unwrap();
    assert_eq!(api.outcome, HealthCheckOutcome::Healthy);
    assert_eq!(api.audit.triggered_by, "scheduler");
    assert_eq!(api.audit.policy_version, 7);

    let db = runs.iter().find(|run| run.schedule_id == "check-db-health").unwrap();
    assert_eq!(db.outcome, HealthCheckOutcome::SkippedCooldown);
    assert!(db.audit.cooldown_suppressed);
    assert_eq!(db.audit.duration_ms, 0);

    let timeout = runs.iter().find(|run| run.schedule_id == "check-worker-timeout").unwrap();
    assert_eq!(timeout.outcome, HealthCheckOutcome::TimedOut);
    assert_eq!(timeout.audit.duration_ms, 100);
}
~~~

- [ ] **Step 2: Run focused test**

Run: cargo test -p thalassaops --test operations_health_check

Expected: FAIL because scheduler functions do not exist.

- [ ] **Step 3: Add due-state and run types**

Define DueState with Disabled, NotDue, Cooldown and Due, plus the design's HealthCheckResult carrying schedule ID, outcome, observed time, optional evidence ID and HealthCheckAudit. Return one result per schedule, including disabled/not-due/cooldown outcomes. A missing fixture returns HealthCheckError::FixtureNotFound and never emits healthy.

- [ ] **Step 4: Implement due/cooldown rules**

Apply disabled → skipped_disabled; now < last_run_at + interval_seconds → skipped_not_due; a present last_signal_at with now < last_signal_at + cooldown_seconds → skipped_cooldown; otherwise run once. Zero cooldown never suppresses. Reject invalid timestamps, zero interval, zero timeout and an unbounded fixture source scope.

- [ ] **Step 5: Implement timeout and audit**

Compare fixture duration to timeout without sleeping. Greater-than-timeout becomes timed_out with audit duration equal to timeout; otherwise preserve fixture outcome/duration. Derive run ID from schedule ID plus now, set timestamps from explicit durations, set triggered_by to scheduler and copy scope/source/outcome/policy version. Never include a source payload or credential.

- [ ] **Step 6: Run and commit**

Run:

~~~bash
cargo test -p thalassaops --test operations_health_check
cargo test --workspace
cargo fmt --all -- --check
~~~

Expected: PASS with no timer, sleep, network call or provider CLI.

~~~bash
git add src-tauri/src/operations/health_check.rs src-tauri/src/operations/model.rs src-tauri/src/operations/fixtures.rs src-tauri/src/operations/mod.rs src-tauri/tests/operations_health_check.rs
git commit -m "feat: add deterministic scheduled health checks"
~~~

**Acceptance criteria:**

- Interval, scope, timeout and cooldown semantics are explicit and independently tested.
- Due, not-due, disabled and cooldown-suppressed runs retain audit metadata.
- Timeout classification is deterministic and non-blocking.
- Audit metadata records policy version and contains no secret/raw payload.

---

### Task 5: Build the aggregation layer and capability-scoped IPC commands

**Files:**
- Create: src-tauri/src/operations/aggregate.rs
- Create: src-tauri/src/operations/evidence.rs
- Create: src-tauri/src/app/operations.rs
- Create: src-tauri/tests/operations_aggregation.rs
- Modify: src-tauri/src/operations/mod.rs
- Modify: src-tauri/src/app/mod.rs
- Modify: src-tauri/src/main.rs
- Modify: ui/contracts/ipc.ts

**Interfaces:**
- Consumes: FixtureCatalog, anomaly/health-check producers, NormalizedAlert, CloudEnvironment, CloudResource, KubernetesInventory, connector summaries and AppState policy/membership helpers.
- Produces: AggregationInput, OperationsAggregator::from_fixture_catalog, OperationsAggregator::snapshot_at, OperationsEvidenceRequest, OperationsAggregator::evidence, AppState::operations_snapshot, AppState::operations_evidence and registered Tauri handlers.

**Tests to add:**

- count, business-impact and deterministic ordering tests;
- no-correlation and partial-source tests;
- critical-number validation test;
- operations.snapshot authorization table: wrong command/capability, bounded scope, inactive membership, principal mismatch, role denial, malformed payload and UI policy denial;
- operations.evidence unknown/duplicate/cross-scope/unverified/malformed/UI-policy tests; and
- successful serialization leak scan.

- [ ] **Step 1: Write failing aggregation test**

~~~rust
#[test]
fn snapshot_prioritizes_business_impact_and_keeps_source_items_uncorrelated() {
    let snapshot = OperationsAggregator::from_fixture_catalog(fixture_catalog())
        .snapshot_at(fixture_time()).unwrap();
    assert_eq!(snapshot.health_summary.state, ConsoleHealthState::Critical);
    assert_eq!(snapshot.health_summary.headline.level, ImpactLevel::Critical);
    assert_eq!(snapshot.incident_queue[0].severity, ConsoleSeverity::S1);
    assert!(snapshot.incident_queue.iter().any(|item| item.source_kind == QueueItemSourceKind::Anomaly));
    assert!(snapshot.incident_queue.iter().any(|item| item.source_kind == QueueItemSourceKind::Alert));
}
~~~

- [ ] **Step 2: Run focused test**

Run: cargo test -p thalassaops --test operations_aggregation

Expected: FAIL because aggregator and commands do not exist.

- [ ] **Step 3: Define aggregation seams**

Add:

~~~rust
pub struct AggregationInput {
    pub generated_at: DateTime<Utc>,
    pub source_status: Vec<SourceStatus>,
    pub alerts: Vec<NormalizedAlert>,
    pub metrics: Vec<MetricFixture>,
    pub anomaly_rules: Vec<AnomalyRule>,
    pub health_checks: Vec<HealthCheckSchedule>,
    pub health_check_results: BTreeMap<String, FixtureHealthCheck>,
    pub changes: Vec<ChangeStreamItem>,
    pub environments: Vec<EnvironmentStatus>,
    pub evidence: Vec<EvidenceRef>,
}

pub struct OperationsAggregator { catalog: FixtureCatalog }

impl OperationsAggregator {
    pub fn from_fixture_catalog(catalog: FixtureCatalog) -> Self;
    pub fn snapshot_at(&self, now: DateTime<Utc>) -> Result<OperationsSnapshot, AggregationError>;
}
~~~

Keep adapters provider-neutral. No provider-specific HTTP client is imported into aggregate.rs.

- [ ] **Step 4: Implement projection and ordering**

Run both producers with explicit now, convert alerts/anomalies and only degraded, unavailable or timed-out checks with verified evidence into independent queue items, intern evidence and calculate headline: S1/critical unavailable → Critical; attention/stale → Degraded; all healthy/no attention → Healthy; missing/unverified required evidence → Unknown. Keep healthy/not-due/cooldown/disabled check results in audit/summary only. Sort queue by severity/status/time/ID, changes by time/ID and environments by provider/name/ID.

Construct every number through a helper taking key, value, unit, evidence IDs, destination and filter key. Run OperationsSnapshot::validate() before returning. No count is calculated from a missing source, and no source is silently treated as healthy.

- [ ] **Step 5: Implement scoped evidence lookup**

Add EvidenceStore::from_snapshot(snapshot) and EvidenceStore::get_for_scope(ids, workspace_scope). Reject unknown/duplicate IDs, unverified evidence and entries outside the workspace. Requests never accept a query or URL; native links come only from trusted admitted source references.

- [ ] **Step 6: Add AppState commands**

Implement AppState::operations_snapshot and AppState::operations_evidence. Snapshot accepts null payload; evidence accepts { "evidence_ids": ["..."] }. Use Capability::WorkspaceRead/ResourceRead and Permission::Read as documented. Reject bounded envelopes, enforce active principal/workspace/role checks before work, evaluate verified Internal/Ui before fixture response, and retain health-check audit metadata only after verified Internal/AuditLog policy approval. Map errors through existing IpcResult/IpcError. Register both Tauri handlers and mirror all request/response types.

- [ ] **Step 7: Run and commit**

Run:

~~~bash
cargo test -p thalassaops --test operations_aggregation
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
~~~

Expected: PASS with no network call, recursive Tauri invocation or capability beyond the documented two.

~~~bash
git add src-tauri/src/operations src-tauri/src/app/operations.rs src-tauri/src/app/mod.rs src-tauri/src/main.rs src-tauri/tests/operations_aggregation.rs ui/contracts/ipc.ts
git commit -m "feat: aggregate deterministic operations console data"
~~~

**Acceptance criteria:**

- Snapshot presents business impact first, keeps source items independent and preserves failed sources.
- Every number passes evidence/drill-down validation and deterministic ordering.
- Only operations.snapshot and operations.evidence cross IPC with documented read capabilities.
- Evidence lookup is workspace-scoped and cannot execute a user query or URL.
- Serialized results contain no secret, credential reference, authorization header, raw provider error body or unverified excerpt.

---

### Task 6: Build the curated React Operations Console widgets

**Files:**
- Create: ui/src/OperationsConsole.tsx
- Create: ui/src/operations/WidgetFrame.tsx
- Create: ui/src/operations/HealthSummary.tsx
- Create: ui/src/operations/IncidentQueue.tsx
- Create: ui/src/operations/SignalSummary.tsx
- Create: ui/src/operations/ChangeStream.tsx
- Create: ui/src/operations/EnvironmentStatus.tsx
- Create: ui/src/operations/widgetConfig.ts
- Create: ui/src/OperationsConsole.test.tsx
- Modify: ui/src/shell.tsx
- Modify: ui/src/locales/en.ts
- Modify: ui/src/locales/th.ts
- Modify: ui/src/styles.css

**Interfaces:**
- Consumes: Invoke, OperationsSnapshot, CriticalNumber, WidgetDefinition, WidgetId, WidgetPreference and operations.snapshot.
- Produces: OperationsConsole({ invoke }), loadWidgetPreferences(storage), normalizeWidgetPreferences(preferences, registry) and five focused widget components.

**Tests to add:**

- initial route invocation with WorkspaceRead and unbounded scope;
- fixture journey showing impact headline, S1 item, alert/anomaly counts and environments;
- failed-source independence;
- preference order/size/collapse/optional visibility with required widgets forced visible;
- keyboard/focus and non-color-only status tests; and
- English/Thai locale-key shape parity.

- [ ] **Step 1: Write failing console journey test**

~~~tsx
test("opens on business impact and shows what needs attention", async () => {
  const invoke = vi.fn().mockResolvedValue({ ok: true, value: operationsSnapshotFixture });
  render(<I18nProvider><Shell invoke={invoke} /></I18nProvider>);
  await userEvent.setup().click(screen.getByRole("button", { name: "Command Center" }));
  expect(await screen.findByRole("heading", { name: /critical impact/i })).toBeInTheDocument();
  expect(screen.getByText(/checkout unavailable/i)).toBeInTheDocument();
  expect(screen.getByText(/gcp staging/i)).toBeInTheDocument();
  expect(invoke).toHaveBeenCalledWith("operations_snapshot", expect.objectContaining({
    envelope: expect.objectContaining({ capability: "WorkspaceRead" })
  }));
});
~~~

- [ ] **Step 2: Run focused test**

Run: npm ci then npm test -- ui/src/OperationsConsole.test.tsx

Expected: FAIL because commandCenter currently renders the unavailable route state.

- [ ] **Step 3: Add composition root and focused widgets**

Invoke operations_snapshot once on mount with request ID, command operations.snapshot, capability WorkspaceRead, unbounded scope and null payload. Render widgets in registry order, with HealthSummary and IncidentQueue first. Each widget receives typed data and no provider modules. Render severity/status/business-impact/source text, not only colors.

- [ ] **Step 4: Implement curated preferences**

Use storage key thalassaops.operations.widgets.v1. Define loadWidgetPreferences(storage) and normalizeWidgetPreferences(preferences, registry). Accept only five IDs, discard duplicates/unknown values, clamp non-negative integer order, accept compact/standard/wide sizes, force health_summary and incident_queue visible, and persist only order/size/collapsed/optional visibility. Do not persist queries, URLs, source selectors, rules or capabilities.

- [ ] **Step 5: Connect route, locale and styles**

Replace commandCenter empty state in ui/src/shell.tsx with OperationsConsole. Add every new visible string/state/control to both locale objects with identical nested keys. Add first-viewport, status-label, focus, widget-size and queue-row styles.

- [ ] **Step 6: Run and commit**

Run:

~~~bash
npm ci
npm test -- ui/src/OperationsConsole.test.tsx
npm run typecheck
npm run lint
npm run format:check
~~~

Expected: PASS.

~~~bash
git add ui/src/OperationsConsole.tsx ui/src/operations ui/src/shell.tsx ui/src/locales/en.ts ui/src/locales/th.ts ui/src/styles.css ui/src/OperationsConsole.test.tsx
git commit -m "feat: add the curated operations console widgets"
~~~

**Acceptance criteria:**

- Command Center requests one deterministic snapshot and renders health, incidents, signals, changes and environments in documented order.
- First viewport shows business-impact headline and first active queue item.
- Failed sources stay visible as unavailable/stale/unverified while healthy sources remain.
- Preferences configure optional layout without hiding health/incident attention.
- Copy is localized, keyboard reachable and status-labelled independently of color.

---

### Task 7: Wire evidence drill-downs and complete trust-boundary regression coverage

**Files:**
- Create: ui/src/operations/DrillDown.tsx
- Create: ui/src/operations/EvidencePanel.tsx
- Modify: ui/src/OperationsConsole.tsx
- Modify: ui/src/operations/HealthSummary.tsx
- Modify: ui/src/operations/SignalSummary.tsx
- Modify: ui/src/operations/EnvironmentStatus.tsx
- Modify: ui/src/operations/ChangeStream.tsx
- Modify: src-tauri/src/operations/evidence.rs
- Modify: src-tauri/src/app/operations.rs
- Modify: ui/src/OperationsConsole.test.tsx
- Create: src-tauri/tests/operations_security.rs

**Interfaces:**
- Consumes: CriticalNumber.drill_down, CriticalNumber.evidence_ids, EvidenceRef, OperationsEvidenceRequest and existing shell open.
- Produces: DrillDown({ target, onClose, invoke }), EvidencePanel({ evidence, onOpenNative }) and the completed security/authorization matrix.

**Tests to add:**

- every number-bearing widget sends only backend-issued evidence IDs;
- evidence source/query/time/excerpt and masked/unparsed indicators;
- trusted native-link guard;
- empty/unknown/unavailable evidence;
- backend unknown/duplicate/cross-scope/unverified evidence;
- Ui, ExternalIntegration and AuditLog policy denials;
- serialized secret scan for password/token/authorization/credential_reference/sk-live-/raw provider error; and
- command descriptor rejection of IncidentWrite, ConnectorAct, PolicyManage and mutation execution.

- [ ] **Step 1: Write failing drill-down journey test**

~~~tsx
const invoke = vi.fn().mockImplementation((name: string) => {
  if (name === "operations_snapshot") return Promise.resolve({ ok: true, value: operationsSnapshotFixture });
  if (name === "operations_evidence") return Promise.resolve({ ok: true, value: evidenceFixture });
  return Promise.resolve({ ok: true, value: {} });
});
await user.click(screen.getByRole("button", { name: /attention/i }));
expect(await screen.findByText(/source: prometheus/i)).toBeInTheDocument();
expect(screen.getByText(/query_range/i)).toBeInTheDocument();
expect(screen.getByText(/masked/i)).toBeInTheDocument();
expect(invoke).toHaveBeenLastCalledWith("operations_evidence", expect.objectContaining({
  envelope: expect.objectContaining({
    capability: "ResourceRead",
    payload: { evidence_ids: ["evidence:fixture:attention"] }
  })
}));
~~~

- [ ] **Step 2: Run focused test**

Run: npm ci then npm test -- ui/src/OperationsConsole.test.tsx

Expected: FAIL because critical-number controls do not open evidence.

- [ ] **Step 3: Implement typed drill-down controls**

Add a keyboard-focusable button for every CriticalNumber. DrillDown sends only backend-issued evidence IDs and selects the typed local destination. Do not interpolate filter_key into a provider query. Empty IDs or unknown destination render localized unavailable state and make no IPC call.

- [ ] **Step 4: Implement evidence panel and native-link guard**

Render source kind, connector, endpoint, query, observed time, excerpt and masked/unparsed state. When native_url exists, require an https URL and pass that exact URL to existing open. Never render raw authorization headers or credentials; unparsed excerpts remain visibly not masked.

- [ ] **Step 5: Add backend security tests**

Construct a test AppState and assert wrong command/capability/scope/membership/role, unknown evidence, UI policy denial and unverified evidence return IpcResult::Err. Serialize success and assert it contains none of password, token, authorization, credential_reference, sk-live-, arn:aws:iam or the raw fixture provider error. Assert IncidentWrite capability is rejected before aggregation.

- [ ] **Step 6: Run and commit**

Run:

~~~bash
cargo test -p thalassaops --test operations_security
cargo test --workspace
npm ci
npm test -- ui/src/OperationsConsole.test.tsx
npm run typecheck
npm run lint
npm run format:check
~~~

Expected: PASS with no unmasked evidence, arbitrary URL, external request or mutation path.

~~~bash
git add ui/src/operations/DrillDown.tsx ui/src/operations/EvidencePanel.tsx ui/src/OperationsConsole.tsx ui/src/operations/HealthSummary.tsx ui/src/operations/SignalSummary.tsx ui/src/operations/EnvironmentStatus.tsx ui/src/operations/ChangeStream.tsx src-tauri/src/operations/evidence.rs src-tauri/src/app/operations.rs ui/src/OperationsConsole.test.tsx src-tauri/tests/operations_security.rs
git commit -m "feat: add evidence-backed operations drill-downs"
~~~

**Acceptance criteria:**

- Every critical number opens typed evidence with exact backend-issued IDs.
- Source, query/time and redaction/unparsed state are visible; unparsed data is not labelled masked.
- Native links are trusted, HTTPS-only and opened through existing shell permission.
- Unknown/cross-scope/unverified evidence and capability/policy failures fail closed.
- Security tests find no credential, authorization header, credential reference or raw provider error body.

---

### Task 8: Run complete regression, fixture acceptance and release verification

**Files:**
- Create: docs/superpowers/reports/2026-08-28-sprint-11-verification.md
- Create: ui/src/operations/operations-console.acceptance.test.tsx
- Modify only for a minimal defect proven by a failing gate: files listed in Tasks 2–7.

**Interfaces:**
- Consumes: complete console, fixture catalog, Rust/TypeScript contracts and all Task 2–7 tests.
- Produces: verification report with actual command outcomes and a committed, unpushed branch.

**Tests to add:**

- operations-console.acceptance.test.tsx runs the complete deterministic fixture journey, including first-viewport attention, source independence, critical-number evidence and no external/mutation invocation; a defect fix adds its regression test in the owning task.

- [ ] **Step 1: Run Rust gates**

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
~~~

Expected: PASS with no warnings and no test count lower than the pre-Sprint 11 baseline.

- [ ] **Step 2: Install frontend dependencies**

Run: npm ci

Expected: exit code 0. If unavailable, report blocked with output; do not claim frontend gates passed.

- [ ] **Step 3: Run frontend gates**

~~~bash
npm run format:check
npm run lint
npm run typecheck
npm run build
npm test
~~~

Expected: PASS all five.

- [ ] **Step 4: Audit scope and serialized output**

Run:

~~~bash
git diff --check
git diff main...HEAD -- docs/design/sprint-11-operations-console.md docs/superpowers/plans/2026-08-28-sprint-11-operations-console.md
git diff main...HEAD -- ':!docs/design/sprint-11-operations-console.md' ':!docs/superpowers/plans/2026-08-28-sprint-11-operations-console.md'
~~~

Verify: no Terraform/OpenTofu run or live fixture capture; no new network integration; no IncidentWrite/mutation command; no Sprint 12 topology or Sprint 13 normalization/correlation; only the two documented IPC commands; existing masking/policy modules remain authoritative; no secret/credential reference/authorization header/raw provider error body; required widgets cannot be hidden.

- [ ] **Step 5: Add and execute the 30-second fixture acceptance test**

Create ui/src/operations/operations-console.acceptance.test.tsx with the fixed snapshot/evidence fixtures. Assert the snapshot loads, first viewport exposes the critical impact headline and first queue item, alert/anomaly/check counts are visible, one failed environment does not hide a healthy one, every critical number opens evidence, masking/unparsed state is honest and no provider/network/mutation call occurs. Run npm test -- ui/src/operations/operations-console.acceptance.test.tsx and record the exact test name/result.

- [ ] **Step 6: Write verification report**

Create the report with actual branch name, exact exit codes/results, fixture observations and:

~~~markdown
## Exit criterion

> "A user can open the application and understand what needs attention within 30 seconds."
~~~

Do not report PASS for a command that was not run.

- [ ] **Step 7: Commit without pushing or merging**

~~~bash
git add docs/superpowers/reports/2026-08-28-sprint-11-verification.md ui/src/operations/operations-console.acceptance.test.tsx
git commit -m "test: record sprint 11 operations console acceptance"
~~~

**Acceptance criteria:**

- All Rust/frontend gates pass after npm ci, including npm run format:check.
- Fixture journey demonstrates the quoted exit criterion without live infrastructure or mutation.
- Final diff stays within Sprint 11 boundaries and preserves trust/policy/masking boundaries.
- Verification report contains actual outcomes, coverage and acceptance observations.
- Branch is committed, unpushed and unmerged.
