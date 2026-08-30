# Sprint 15 Incident Domain and Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a canonical, local-first Incident write model that a permitted responder explicitly creates from six supported trigger kinds and advances through a validated, actor-attributed lifecycle with an immutable audit timeline.

**Architecture:** `thalassa-domain` owns the Incident aggregate and pure invariants; a focused `src-tauri/src/incident` module owns local source resolution, transactional SQLite persistence and application services. Capability-scoped Tauri commands authorize every read or mutation before lookup, while TypeScript receives only stable contracts for Sprint 16.

**Tech Stack:** Rust 2021, Tauri 2, Serde, Chrono, Uuid, SQLite through rusqlite, React 18 TypeScript contracts, Vite, Vitest and the existing ThalassaOps policy/IPC crates.

**Spec:** `docs/design/sprint-15-incident-domain-lifecycle.md`

## Global Constraints

- `thalassa_domain::Incident` is the only canonical Incident aggregate. Extend it; never introduce a parallel `CanonicalIncident`, `PersistedIncident`, provider Incident type or UI-only Incident model.
- Incident creation is explicit only. Signal replay, Operations Console projection and correlation-candidate construction never call `incident.create` and never write Incident rows.
- Supported trigger wire values are exactly `alert`, `anomaly`, `user_report`, `scheduled_health_check`, `vulnerability_finding` and `manual_report`. A correlation candidate is not a trigger; a client starting from one submits selected underlying Signal IDs.
- Source-backed triggers store validated provenance and `ConsoleEvidenceId` references, not full provider payloads. Change the placeholder Incident evidence collection from `Vec<EvidenceId>` to the established `Vec<ConsoleEvidenceId>` rather than inventing a third evidence identifier.
- The status graph is exactly `detected -> triage -> investigating -> mitigating -> monitoring -> resolved -> closed`, plus `monitoring|resolved|closed -> reopened -> investigating`. No skip, self-transition or automatic transition is permitted.
- Status and disposition are independent. Required dispositions are `duplicate`, `false_positive`, `suppressed` and `cancelled`; preserve existing `informational` compatibility. Setting a disposition never closes or merges an Incident.
- Current state and one or more ordered timeline events are written in the same SQLite transaction. Timeline UPDATE and DELETE are rejected. A failed transaction leaves neither partial current state nor orphan audit rows.
- Creation uses the `CommandEnvelope.request_id` as an idempotency key. Later writes require `expected_version`; a stale value returns a typed conflict and never overwrites current state.
- Every mutation records actor, server timestamp, request ID, reason where required, policy version and typed before/after data. Never accept an actor, timestamp or policy version supplied as authoritative payload data.
- `Permission::ManageIncident` is granted only to Owner, Administrator and Operator. Viewer and Auditor can read but cannot create or mutate Incidents.
- Read commands are exactly `incident.get`, `incident.list` and `incident.timeline` with `IncidentRead`/`Read`. Write commands are exactly `incident.create`, `incident.transition`, `incident.set_severity`, `incident.set_disposition` and `incident.assign_role` with `IncidentWrite`/`ManageIncident`.
- Preserve authorization order: descriptor/capability, unbounded envelope, active membership and Principal identity, workspace grant, permission, local-storage/audit or UI policy, request parsing, target/source lookup, domain validation, persistence, then response policy.
- User/manual report text is structured and bounded. Reject control characters, credential/token/private-key markers and sensitive account identifiers before persistence; do not mask them into an immutable timeline.
- No provider network request, credential read, AI call, action execution, remediation, notification, external integration write, background scheduler or full Sprint 16 Incident Workspace is allowed.
- All enum wire values are explicit. Rust and TypeScript JSON use snake_case except severity values `S1` through `S5`.
- No wall-clock call appears in pure domain or repository tests. Inject server time into aggregate and repository methods.
- OMP is the sole implementation owner through Orca orchestration. The coordinator reviews every task commit before dispatching the next task. OMP must not spawn nested workers or edit outside the active task.
- Run `npm ci` before frontend gates if `node_modules` is absent. A gate that cannot run is blocked, not passed.
- The exact exit criterion is: "Incidents can be created from alerts, anomalies, user reports, scheduled health checks, vulnerability findings and manual reports, then progress through a validated state machine."

## File Map and Review Sequence

Task 1 freezes source, impact and severity contracts. Task 2 builds the aggregate state machine on those contracts. Task 3 freezes IPC and TypeScript wire contracts. Task 4 adds persistence. Task 5 adds explicit trigger resolution and creation. Task 6 adds mutations and reads. Task 7 exposes secured Tauri handlers. Task 8 proves acceptance and runs all release gates.

OMP completes and commits one task, sends `worker_done`, then stops. The coordinator reviews the task diff against this plan and the design. Fixes return to the same OMP terminal as a new Orca task; the next numbered task starts only after review passes.

---

### Task 1: Reconcile Incident, Business Impact, Source and Severity Contracts

**Files:**

- Modify: `crates/thalassa-domain/src/lib.rs` — reconcile the placeholder Incident model; add trigger, impact-dimension, severity-decision, role and bounded-input contracts; add stable Serde wire values.
- Create: `crates/thalassa-domain/tests/incident_contracts.rs` — JSON wires, impact derivation, safety floors, text bounds and source-kind consistency.
- Modify: `crates/thalassa-domain/tests/contracts.rs` — update the early Incident constructor assertion without weakening it.
- Modify: `crates/thalassa-domain/tests/operations_contracts.rs` — supply typed Business Impact dimensions to existing Operations Console fixtures.
- Modify: `src-tauri/src/operations/aggregate.rs` — populate typed Business Impact dimensions wherever the compact projection is built.
- Modify: `src-tauri/src/topology/fixtures.rs` — populate typed dimensions in the topology Incident queue fixture.

**Interfaces:**

- Consumes: existing `SignalId`, `ConsoleEvidenceId`, `PrincipalId`, `TeamId`, `ResourceScope`, `ImpactLevel`, `ImpactTrajectory`, `IncidentSeverity`, `IncidentStatus` and `IncidentDisposition`.
- Produces: `IncidentTriggerId`, `IncidentSourceKind`, `ImpactDimensions`, `IncidentSeverityOverride`, `IncidentRole`, `IncidentRoleAssignment`, `IncidentTrigger`, reconciled `BusinessImpact` and reconciled `Incident`; `BusinessImpact::derive_severity() -> Result<IncidentSeverity, IncidentError>`; `validate_incident_text(value: &str, maximum: usize) -> Result<(), IncidentError>`.

- [ ] **Step 1: Write failing wire and severity tests**

```rust
// crates/thalassa-domain/tests/incident_contracts.rs
use serde_json::json;
use thalassa_domain::{
    BusinessImpact, ImpactDimensions, ImpactLevel, ImpactTrajectory, IncidentSourceKind,
    IncidentSeverity,
};

fn impact(dimensions: ImpactDimensions) -> BusinessImpact {
    BusinessImpact {
        level: dimensions.highest_level(),
        summary: "Checkout unavailable".into(),
        customer_scope: "production customers".into(),
        service_criticality: "tier_0".into(),
        trajectory: dimensions.trajectory,
        dimensions,
        evidence_ids: vec!["evidence-checkout-alert".into()],
    }
}

#[test]
fn incident_source_kinds_have_exact_wire_values() {
    for (kind, wire) in [
        (IncidentSourceKind::Alert, "alert"),
        (IncidentSourceKind::Anomaly, "anomaly"),
        (IncidentSourceKind::UserReport, "user_report"),
        (IncidentSourceKind::ScheduledHealthCheck, "scheduled_health_check"),
        (IncidentSourceKind::VulnerabilityFinding, "vulnerability_finding"),
        (IncidentSourceKind::ManualReport, "manual_report"),
    ] {
        assert_eq!(serde_json::to_value(kind).unwrap(), json!(wire));
    }
}

#[test]
fn highest_impact_dimension_derives_initial_severity() {
    let dimensions = ImpactDimensions {
        availability: ImpactLevel::High,
        customer_reach: ImpactLevel::Medium,
        business_criticality: ImpactLevel::High,
        data_integrity: ImpactLevel::None,
        security_privacy: ImpactLevel::None,
        financial_contractual: ImpactLevel::Low,
        trajectory: ImpactTrajectory::Stable,
        production: true,
    };
    assert_eq!(impact(dimensions).derive_severity().unwrap(), IncidentSeverity::S2);
}

#[test]
fn rapidly_expanding_unknown_production_scope_is_at_least_s2() {
    let dimensions = ImpactDimensions {
        availability: ImpactLevel::Unknown,
        customer_reach: ImpactLevel::Unknown,
        business_criticality: ImpactLevel::High,
        data_integrity: ImpactLevel::None,
        security_privacy: ImpactLevel::None,
        financial_contractual: ImpactLevel::None,
        trajectory: ImpactTrajectory::Expanding,
        production: true,
    };
    assert_eq!(impact(dimensions).derive_severity().unwrap(), IncidentSeverity::S2);
}
```

- [ ] **Step 2: Run the test and confirm contract symbols are missing**

Run: `cargo test -p thalassa-domain --test incident_contracts`

Expected: FAIL with unresolved imports for the Sprint 15 contracts.

- [ ] **Step 3: Add exact source and impact contracts**

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IncidentSourceKind {
    #[serde(rename = "alert")]
    Alert,
    #[serde(rename = "anomaly")]
    Anomaly,
    #[serde(rename = "user_report")]
    UserReport,
    #[serde(rename = "scheduled_health_check")]
    ScheduledHealthCheck,
    #[serde(rename = "vulnerability_finding")]
    VulnerabilityFinding,
    #[serde(rename = "manual_report")]
    ManualReport,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ImpactDimensions {
    pub availability: ImpactLevel,
    pub customer_reach: ImpactLevel,
    pub business_criticality: ImpactLevel,
    pub data_integrity: ImpactLevel,
    pub security_privacy: ImpactLevel,
    pub financial_contractual: ImpactLevel,
    pub trajectory: ImpactTrajectory,
    pub production: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BusinessImpact {
    pub level: ImpactLevel,
    pub summary: String,
    pub customer_scope: String,
    pub service_criticality: String,
    pub trajectory: ImpactTrajectory,
    pub dimensions: ImpactDimensions,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}
```

Implement `ImpactDimensions::highest_level` with an explicit impact rank where Critical wins over High, High over Medium, Medium over Low, Low over None and Unknown contributes no confirmed impact. Do not use derived enum `Ord`, whose declaration order is not the business ranking. Map Critical/High/Medium/Low/None to S1/S2/S3/S4/S5. Map all-Unknown to S5 unless `production && trajectory == Expanding`, in which case enforce S2. Require `BusinessImpact.level == dimensions.highest_level()` and require non-empty safe summary, customer scope, service criticality and evidence IDs.

- [ ] **Step 4: Reconcile the Incident identity fields**

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentSeverityOverride {
    pub derived: IncidentSeverity,
    pub selected: IncidentSeverity,
    pub actor_id: PrincipalId,
    pub reason: String,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IncidentRole {
    #[serde(rename = "owner")]
    Owner,
    #[serde(rename = "incident_commander")]
    IncidentCommander,
    #[serde(rename = "technical_lead")]
    TechnicalLead,
    #[serde(rename = "communications_lead")]
    CommunicationsLead,
    #[serde(rename = "approver")]
    Approver,
    #[serde(rename = "change_owner")]
    ChangeOwner,
    #[serde(rename = "stakeholder")]
    Stakeholder,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentRoleAssignment {
    pub role: IncidentRole,
    pub principal_id: PrincipalId,
    pub assigned_by: PrincipalId,
    pub assigned_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentReport {
    pub reporter_id: Option<PrincipalId>,
    pub summary: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentTrigger {
    pub id: IncidentTriggerId,
    pub source_kind: IncidentSourceKind,
    pub source_id: String,
    pub source_record_digest: Option<String>,
    pub scope: ResourceScope,
    pub observed_at: DateTime<Utc>,
    pub signal_id: Option<SignalId>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub report: Option<IncidentReport>,
}
```

Add `pub type IncidentTriggerId = Uuid`. Reconcile `Incident` to keep `id`, `summary`, `scope`, `signal_ids`, `hypothesis_ids`, `action_ids`, `created_at` and `updated_at`, change `evidence_ids` to `Vec<ConsoleEvidenceId>`, and add `trigger_ids: Vec<IncidentTriggerId>`, `owning_team_id`, `business_impact`, `derived_severity`, optional `severity_override`, `duplicate_of_incident_id`, active `roles` and `version`. Add explicit Serde renames to status, severity and disposition enums.

- [ ] **Step 5: Add safe bounded text and collection validation tests**

```rust
#[test]
fn incident_text_rejects_secrets_controls_and_oversize_input() {
    assert!(thalassa_domain::validate_incident_text("ok\nline", 64).is_err());
    assert!(thalassa_domain::validate_incident_text("authorization: bearer abc", 64).is_err());
    assert!(thalassa_domain::validate_incident_text(&"x".repeat(65), 64).is_err());
    assert!(thalassa_domain::validate_incident_text("bounded safe summary", 64).is_ok());
}
```

Use maximum lengths 200 characters for summary, 4,000 for notes/reasons, 200 for source IDs and 1,000 for impact summaries. Sort and deduplicate Signal/evidence IDs before aggregate construction; reject nil UUIDs and unsafe/empty evidence IDs.

- [ ] **Step 6: Update every existing BusinessImpact construction and run domain regression**

Run: `cargo test -p thalassa-domain`

Expected: PASS, including Operations Console contracts after all constructors supply dimensions and evidence IDs.

- [ ] **Step 7: Commit Task 1**

```bash
git add crates/thalassa-domain src-tauri/src/operations/aggregate.rs src-tauri/src/topology/fixtures.rs
git commit -m "feat(incident): define canonical incident contracts"
```

---

### Task 2: Implement the Pure Incident Aggregate and State Machine

**Files:**

- Modify: `crates/thalassa-domain/src/lib.rs` — add aggregate commands, typed transition contexts, disposition/role operations, timeline payloads and `IncidentError`.
- Create: `crates/thalassa-domain/tests/incident_lifecycle.rs` — every allowed/rejected edge, required context, reopen, disposition, role cardinality, version and event attribution.

**Interfaces:**

- Consumes: Task 1 contracts.
- Produces: `IncidentCreateCommand`, `IncidentTransition`, `IncidentDispositionCommand`, `IncidentRoleCommand`, `IncidentSeverityCommand`, `IncidentTimelineEvent`, `IncidentMutation`; `Incident::create(command, actor_id, request_id, policy_version, now) -> Result<IncidentMutation, IncidentError>` and matching pure mutation methods.

- [ ] **Step 1: Write the failing lifecycle matrix test**

```rust
// crates/thalassa-domain/tests/incident_lifecycle.rs
use chrono::{TimeZone, Utc};
use thalassa_domain::{IncidentStatus, IncidentTransition};

#[test]
fn lifecycle_accepts_only_canonical_edges() {
    let allowed = [
        (IncidentStatus::Detected, IncidentStatus::Triage),
        (IncidentStatus::Triage, IncidentStatus::Investigating),
        (IncidentStatus::Investigating, IncidentStatus::Mitigating),
        (IncidentStatus::Mitigating, IncidentStatus::Monitoring),
        (IncidentStatus::Monitoring, IncidentStatus::Resolved),
        (IncidentStatus::Resolved, IncidentStatus::Closed),
        (IncidentStatus::Monitoring, IncidentStatus::Reopened),
        (IncidentStatus::Resolved, IncidentStatus::Reopened),
        (IncidentStatus::Closed, IncidentStatus::Reopened),
        (IncidentStatus::Reopened, IncidentStatus::Investigating),
    ];
    for (from, to) in allowed {
        assert!(IncidentTransition::edge_allowed(from, to), "{from:?} -> {to:?}");
    }
    assert!(!IncidentTransition::edge_allowed(IncidentStatus::Detected, IncidentStatus::Resolved));
    assert!(!IncidentTransition::edge_allowed(IncidentStatus::Triage, IncidentStatus::Triage));
    assert!(!IncidentTransition::edge_allowed(IncidentStatus::Detected, IncidentStatus::Reopened));
    let _fixed = Utc.with_ymd_and_hms(2026, 8, 30, 9, 0, 0).unwrap();
}
```

- [ ] **Step 2: Run the test and confirm the transition API is missing**

Run: `cargo test -p thalassa-domain --test incident_lifecycle`

Expected: FAIL with unresolved `IncidentTransition`.

- [ ] **Step 3: Add typed transition contexts**

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "target", content = "context")]
pub enum IncidentTransition {
    #[serde(rename = "triage")]
    Triage(TriageContext),
    #[serde(rename = "investigating")]
    Investigating(InvestigatingContext),
    #[serde(rename = "mitigating")]
    Mitigating(MitigatingContext),
    #[serde(rename = "monitoring")]
    Monitoring(MonitoringContext),
    #[serde(rename = "resolved")]
    Resolved(ResolvedContext),
    #[serde(rename = "closed")]
    Closed(ClosedContext),
    #[serde(rename = "reopened")]
    Reopened(ReopenedContext),
}
```

Define context structs with exact required values from the spec: Triage carries confirmed Business Impact, Owner and `duplicate_checked: bool`; Investigating carries a safe note and non-empty evidence IDs; Mitigating carries safe action description, executor and expected impact; Monitoring carries positive verification seconds capped at 86,400, success criteria and watch owner; Resolved carries resolution summary, non-empty evidence IDs and impact end time; Closed carries closure notes and follow-up references; Reopened carries reason plus new evidence IDs or recurrence Signal ID.

The create command consumes only resolved trigger values:

```rust
pub struct IncidentCreateCommand {
    pub summary: String,
    pub scope: ResourceScope,
    pub owning_team_id: TeamId,
    pub triggers: Vec<IncidentTrigger>,
    pub business_impact: BusinessImpact,
    pub initial_roles: Vec<IncidentRoleAssignment>,
}
```

- [ ] **Step 4: Write failing creation, attribution and version tests**

```rust
#[test]
fn creation_starts_detected_at_version_one_and_attributes_event() {
    let fixture = incident_create_fixture();
    let result = thalassa_domain::Incident::create(
        fixture.command,
        fixture.actor_id,
        fixture.request_id,
        7,
        fixture.now,
    )
    .unwrap();
    assert_eq!(result.incident.status, IncidentStatus::Detected);
    assert_eq!(result.incident.version, 1);
    assert_eq!(result.events[0].sequence, 1);
    assert_eq!(result.events[0].actor_id, fixture.actor_id);
    assert_eq!(result.events[0].policy_version, 7);
    assert_eq!(result.events[0].request_id, fixture.request_id);
}
```

Add a local fixture builder in the test file that creates one manual-report trigger, fixed UUIDs, valid Business Impact, Owner assignment and fixed UTC timestamp. Do not use `Utc::now()`.

- [ ] **Step 5: Implement pure aggregate operations**

`IncidentMutation` contains the new Incident plus non-empty ordered events. Each accepted operation increments version exactly once. `assign_role` permits many Stakeholders but rejects a second active exclusive role unless the operation is Replace. Duplicate disposition requires a different Incident ID; other dispositions require no duplicate ID. `set_severity` recalculates from changed impact or validates an explicit actor-attributed override.

Use these event and mutation envelopes so persistence never guesses audit metadata:

```rust
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IncidentEventKind {
    #[serde(rename = "incident_created")]
    IncidentCreated,
    #[serde(rename = "triggers_attached")]
    TriggersAttached,
    #[serde(rename = "status_transitioned")]
    StatusTransitioned,
    #[serde(rename = "severity_changed")]
    SeverityChanged,
    #[serde(rename = "disposition_changed")]
    DispositionChanged,
    #[serde(rename = "role_changed")]
    RoleChanged,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentTimelineEvent {
    pub id: Uuid,
    pub incident_id: IncidentId,
    pub sequence: u64,
    pub kind: IncidentEventKind,
    pub actor_id: PrincipalId,
    pub reason: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub request_id: Uuid,
    pub policy_version: u64,
    pub payload: IncidentTimelinePayload,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentMutation {
    pub incident: Incident,
    pub events: Vec<IncidentTimelineEvent>,
}
```

`IncidentTimelinePayload` is a tagged enum with typed `Created`, `TriggersAttached`, `StatusTransitioned`, `SeverityChanged`, `DispositionChanged` and `RoleChanged` variants. Each variant carries only its relevant typed before/after values and IDs; it never contains arbitrary JSON or a copied source payload.

Return typed `IncidentError` variants for invalid text, impact, trigger, scope, transition, transition context, severity override, disposition, duplicate reference, role and version. Do not return strings from pure domain methods.

- [ ] **Step 6: Run the complete lifecycle suite**

Run: `cargo test -p thalassa-domain --test incident_lifecycle`

Expected: PASS for all canonical edges, reopen sources, required-context failures, disposition independence, role cardinality and event sequence/version assertions.

- [ ] **Step 7: Run domain regression and commit Task 2**

Run: `cargo test -p thalassa-domain`

```bash
git add crates/thalassa-domain
git commit -m "feat(incident): validate lifecycle and audit events"
```

---

### Task 3: Freeze IPC Descriptors and TypeScript Contracts

**Files:**

- Modify: `crates/thalassa-domain/src/lib.rs` — add request/page response contracts used by both app and IPC tests.
- Modify: `crates/thalassa-ipc/src/lib.rs` — add `ManageIncident`-backed descriptors for all eight commands.
- Modify: `crates/thalassa-ipc/tests/contracts.rs` — assert exact names, capabilities, permissions and unbounded descriptor scopes.
- Modify: `ui/contracts/ipc.ts` — mirror Incident, trigger, impact, role, timeline, request/result and page contracts.
- Modify: `ui/contracts/guards.ts` — add strict runtime guards for Incident and timeline pages.
- Modify: `ui/src/operations/contractValidation.ts` — validate the expanded Business Impact dimensions and evidence closure.
- Modify: `ui/src/operations/OperationsConsole.test.tsx` — update the shared Business Impact fixture builder.
- Modify: `ui/src/topology/topology-fixtures.ts` and `ui/src/topology/operations-queue-fixtures.ts` — add deterministic Business Impact dimensions/evidence IDs to copied queue fixtures.
- Create: `ui/src/incident/incident-contracts.test.ts` — stable wire and guard tests only; no UI component.

**Interfaces:**

- Consumes: Tasks 1-2 domain types and existing `CommandEnvelope`.
- Produces: descriptor functions named `incident_create_descriptor`, `incident_get_descriptor`, `incident_list_descriptor`, `incident_timeline_descriptor`, `incident_transition_descriptor`, `incident_set_severity_descriptor`, `incident_set_disposition_descriptor`, `incident_assign_role_descriptor`; exact request/response/page structs mirrored in TypeScript.

- [ ] **Step 1: Write failing descriptor tests**

```rust
#[test]
fn incident_commands_separate_reads_from_writes() {
    for descriptor in [
        incident_get_descriptor(),
        incident_list_descriptor(),
        incident_timeline_descriptor(),
    ] {
        assert_eq!(descriptor.required_capability, Capability::IncidentRead);
        assert_eq!(descriptor.required_permission, Permission::Read);
        assert!(!descriptor.scope.is_bounded());
    }
    for descriptor in [
        incident_create_descriptor(),
        incident_transition_descriptor(),
        incident_set_severity_descriptor(),
        incident_set_disposition_descriptor(),
        incident_assign_role_descriptor(),
    ] {
        assert_eq!(descriptor.required_capability, Capability::IncidentWrite);
        assert_eq!(descriptor.required_permission, Permission::ManageIncident);
        assert!(!descriptor.scope.is_bounded());
    }
}
```

- [ ] **Step 2: Run IPC tests and add `ManageIncident` plus descriptors**

Run: `cargo test -p thalassa-ipc --test contracts`

Expected: FAIL before implementation, then PASS after adding the exact descriptors. Update `membership_role_grants_permission` later in Task 7; this task freezes metadata only.

- [ ] **Step 3: Add exact request and page contracts**

```rust
pub struct IncidentGetRequest { pub incident_id: IncidentId }
pub struct IncidentCreateRequest { pub summary: String, pub triggers: Vec<IncidentTriggerInput>, pub business_impact: BusinessImpact, pub initial_roles: Vec<IncidentRoleAssignmentInput> }
pub struct IncidentListRequest { pub cursor: Option<String>, pub limit: u16 }
pub struct IncidentTimelineRequest { pub incident_id: IncidentId, pub after_sequence: Option<u64>, pub limit: u16 }
pub struct IncidentTransitionRequest { pub incident_id: IncidentId, pub expected_version: u64, pub transition: IncidentTransition }
pub struct IncidentSeverityRequest { pub incident_id: IncidentId, pub expected_version: u64, pub command: IncidentSeverityCommand }
pub struct IncidentDispositionRequest { pub incident_id: IncidentId, pub expected_version: u64, pub command: IncidentDispositionCommand }
pub struct IncidentRoleRequest { pub incident_id: IncidentId, pub expected_version: u64, pub command: IncidentRoleCommand }
pub struct IncidentPage { pub items: Vec<Incident>, pub next_cursor: Option<String> }
pub struct IncidentTimelinePage { pub incident_id: IncidentId, pub events: Vec<IncidentTimelineEvent>, pub next_sequence: Option<u64> }
```

Define untrusted creation inputs separately from resolved domain triggers:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind")]
pub enum IncidentTriggerInput {
    #[serde(rename = "alert")]
    Alert { source_id: String },
    #[serde(rename = "anomaly")]
    Anomaly { source_id: String },
    #[serde(rename = "scheduled_health_check")]
    ScheduledHealthCheck { source_id: String },
    #[serde(rename = "vulnerability_finding")]
    VulnerabilityFinding { source_id: String },
    #[serde(rename = "user_report")]
    UserReport {
        reporter_id: PrincipalId,
        observed_at: DateTime<Utc>,
        summary: String,
        scope: ResourceScope,
    },
    #[serde(rename = "manual_report")]
    ManualReport {
        observed_at: DateTime<Utc>,
        summary: String,
        scope: ResourceScope,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentRoleAssignmentInput {
    pub role: IncidentRole,
    pub principal_id: PrincipalId,
}
```

`IncidentCreateRequest` is untrusted IPC input. Task 5 resolves it into Task 2's `IncidentCreateCommand`, whose triggers are validated `IncidentTrigger` values. Validate list/timeline limits in `1..=100`. Cursors are opaque safe strings; timeline uses numeric sequence and never timestamp-only paging.

- [ ] **Step 4: Mirror contracts and write strict guard tests**

```typescript
import { isIncident, isIncidentTimelinePage } from "../../contracts/guards";

test("incident guards reject unknown status and unordered timeline", () => {
  expect(isIncident({ ...incidentFixture, status: "acknowledged" })).toBe(false);
  expect(
    isIncidentTimelinePage({
      incident_id: incidentFixture.id,
      events: [timelineFixture.events[1], timelineFixture.events[0]],
      next_sequence: null
    })
  ).toBe(false);
});
```

The guard accepts only six trigger kinds, eight statuses, five dispositions, seven roles, non-nil UUIDs, finite versions, bounded safe display text and strictly increasing event sequence. It rejects unknown keys where existing guard helpers support exact-key validation.

- [ ] **Step 5: Run contract tests and commit Task 3**

Run: `cargo test -p thalassa-ipc --test contracts`

Run: `npm test -- ui/src/incident/incident-contracts.test.ts`

Run: `npm run typecheck`

```bash
git add crates/thalassa-domain crates/thalassa-ipc ui/contracts ui/src/incident
git commit -m "feat(ipc): define incident command contracts"
```

---

### Task 4: Persist Current State, Roles, Triggers and Immutable Timeline

**Files:**

- Create: `src-tauri/migrations/0006_incidents.sql` — four incident tables, indexes, uniqueness and timeline immutability triggers.
- Modify: `src-tauri/src/app/mod.rs` — embed/apply migration 0006 and record schema version 6.
- Create: `src-tauri/src/incident/mod.rs` — module exports.
- Create: `src-tauri/src/incident/repository.rs` — `SqliteIncidentRepository`, serialization helpers, idempotency and optimistic writes.
- Modify: `src-tauri/src/lib.rs` — export `incident`.
- Create: `src-tauri/tests/incident_repository.rs` — migration, create, get/list/timeline, rollback, idempotency, conflicts, cross-workspace isolation and immutability.

**Interfaces:**

- Consumes: `IncidentMutation` and page contracts from Tasks 2-3.
- Produces: `SqliteIncidentRepository::open(path)`, `create`, `get`, `list`, `timeline` and `apply_mutation`; `IncidentStoreError` with typed database/serialization/not-found/conflict/idempotency variants.

Repository creation receives one internal record, never raw IPC input:

```rust
pub struct IncidentCreationRecord {
    pub mutation: IncidentMutation,
    pub triggers: Vec<IncidentTrigger>,
    pub request_fingerprint: String,
}
```

`request_fingerprint` is the lowercase SHA-256 digest of canonical serialized `IncidentCreateCommand`; it contains no source payload or report text.

- [ ] **Step 1: Write the migration and repository tests first**

```rust
#[test]
fn create_is_atomic_idempotent_and_timeline_is_immutable() {
    let fixture = repository_fixture();
    let first = fixture.repository.create(fixture.creation.clone()).unwrap();
    let repeated = fixture.repository.create(fixture.creation).unwrap();
    assert_eq!(first, repeated);
    assert_eq!(fixture.repository.timeline(first.incident.id, None, 100).unwrap().events.len(), 1);
    let connection = rusqlite::Connection::open(fixture.database_path).unwrap();
    assert!(connection.execute("DELETE FROM incident_timeline_event", []).is_err());
}

#[test]
fn stale_version_does_not_append_or_overwrite() {
    let fixture = persisted_incident_fixture();
    let accepted = fixture.repository.apply_mutation(fixture.version_one_mutation).unwrap();
    let before = fixture.repository.timeline(accepted.incident.id, None, 100).unwrap();
    let error = fixture.repository.apply_mutation(fixture.stale_version_one_mutation).unwrap_err();
    assert!(matches!(error, IncidentStoreError::VersionConflict { .. }));
    assert_eq!(fixture.repository.timeline(accepted.incident.id, None, 100).unwrap(), before);
}
```

- [ ] **Step 2: Run and confirm missing migration/repository failure**

Run: `cargo test -p thalassaops --test incident_repository`

Expected: FAIL because migration 0006 and repository module do not exist.

- [ ] **Step 3: Create exact SQLite schema**

```sql
CREATE TABLE IF NOT EXISTS incident (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL,
    team_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    scope_json TEXT NOT NULL,
    summary TEXT NOT NULL,
    business_impact_json TEXT NOT NULL,
    severity TEXT NOT NULL,
    derived_severity TEXT NOT NULL,
    severity_override_json TEXT,
    status TEXT NOT NULL,
    disposition TEXT,
    duplicate_of_incident_id TEXT,
    signal_ids_json TEXT NOT NULL,
    evidence_ids_json TEXT NOT NULL,
    hypothesis_ids_json TEXT NOT NULL,
    action_ids_json TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    create_request_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS incident_workspace_updated_idx
    ON incident (workspace_id, updated_at DESC, id);

CREATE TABLE IF NOT EXISTS incident_trigger (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES incident(id),
    source_kind TEXT NOT NULL,
    source_id TEXT NOT NULL,
    source_record_digest TEXT,
    scope_json TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    evidence_ids_json TEXT NOT NULL,
    report_json TEXT,
    UNIQUE (incident_id, source_kind, source_id)
);

CREATE TABLE IF NOT EXISTS incident_role_assignment (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES incident(id),
    role TEXT NOT NULL,
    principal_id TEXT NOT NULL,
    assigned_by TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    released_by TEXT,
    released_at TEXT
);
CREATE INDEX IF NOT EXISTS incident_active_role_idx
    ON incident_role_assignment (incident_id, role, released_at);
CREATE UNIQUE INDEX IF NOT EXISTS incident_one_active_exclusive_role
    ON incident_role_assignment (incident_id, role)
    WHERE released_at IS NULL AND role <> 'stakeholder';
CREATE UNIQUE INDEX IF NOT EXISTS incident_one_active_stakeholder
    ON incident_role_assignment (incident_id, role, principal_id)
    WHERE released_at IS NULL AND role = 'stakeholder';

CREATE TABLE IF NOT EXISTS incident_timeline_event (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES incident(id),
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    event_kind TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    reason TEXT,
    occurred_at TEXT NOT NULL,
    request_id TEXT NOT NULL,
    policy_version INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    UNIQUE (incident_id, sequence),
    UNIQUE (incident_id, request_id, event_kind)
);
CREATE TRIGGER IF NOT EXISTS incident_timeline_no_update
BEFORE UPDATE ON incident_timeline_event BEGIN SELECT RAISE(ABORT, 'incident timeline is append-only'); END;
CREATE TRIGGER IF NOT EXISTS incident_timeline_no_delete
BEFORE DELETE ON incident_timeline_event BEGIN SELECT RAISE(ABORT, 'incident timeline is append-only'); END;
```

- [ ] **Step 4: Apply migration version 6 idempotently**

Embed the file beside migration 0005. `apply_migrations` executes it, checks `schema_migrations.version = 6`, and inserts version 6 with a server timestamp only when absent. Extend the in-memory migration test to assert all four tables and both triggers exist after two calls.

- [ ] **Step 5: Implement transactional repository writes**

Enable SQLite foreign keys on every repository connection. Use `Connection::transaction_with_behavior(TransactionBehavior::Immediate)`. Creation accepts `IncidentCreationRecord { mutation, triggers, request_fingerprint }` and checks `create_request_id`; identical stored request/result returns the existing Incident, while a reused request ID with a different fingerprint returns `IdempotencyConflict`. Later writes update with `WHERE id = ? AND workspace_id = ? AND version = ?`; affected row count 0 becomes NotFound or VersionConflict after a workspace-scoped lookup. Allocate event sequences from the loaded current maximum within the same immediate transaction. One command can append more than one event, but no command appends the same event kind twice for one request ID.

Serialize only typed structs with `serde_json`; parse enum strings through explicit match functions and reject unknown database values as typed corruption errors. Never use `unwrap` in repository production code.

- [ ] **Step 6: Run repository and app-state migration regressions**

Run: `cargo test -p thalassaops --test incident_repository`

Run: `cargo test -p thalassaops app::tests::migrations`

Expected: PASS with atomic rollback, workspace isolation, stale-version and immutability assertions.

- [ ] **Step 7: Commit Task 4**

```bash
git add src-tauri/migrations/0006_incidents.sql src-tauri/src/app/mod.rs src-tauri/src/incident src-tauri/src/lib.rs src-tauri/tests/incident_repository.rs
git commit -m "feat(incident): persist incident state and timeline"
```

---

### Task 5: Resolve Explicit Triggers and Create Incidents

**Files:**

- Create: `src-tauri/src/incident/source.rs` — `IncidentSourceResolver`, deterministic replay catalog and source-kind checks.
- Create: `src-tauri/src/incident/service.rs` — `IncidentService::create`, safe manual/user reports and repository orchestration.
- Modify: `src-tauri/src/incident/mod.rs` — export service/source contracts.
- Create: `docs/superpowers/fixtures/2026-08-30-incident/user-report.json` — safe attributed user report.
- Create: `docs/superpowers/fixtures/2026-08-30-incident/manual-report.json` — safe operator report.
- Create: `src-tauri/tests/incident_creation.rs` — six sources, multi-trigger candidate selection, mixed scope, secret rejection, no automatic writes and idempotency.

**Interfaces:**

- Consumes: Sprint 13 normalized `Signal`/source-record replay, Task 2 create command, Task 4 repository.
- Produces: `ResolvedIncidentTrigger`; `IncidentSourceResolver::resolve(kind, source_id, workspace_scope) -> Result<ResolvedIncidentTrigger, IncidentServiceError>`; `IncidentService::create(context, request) -> Result<IncidentMutation, IncidentServiceError>`.

- [ ] **Step 1: Write failing six-source creation tests**

```rust
#[test]
fn explicit_creation_resolves_all_six_source_kinds() {
    let fixture = creation_service_fixture();
    for request in fixture.requests_for_all_source_kinds() {
        let result = fixture.service.create(fixture.context(), request).unwrap();
        assert_eq!(result.incident.status, IncidentStatus::Detected);
        assert_eq!(result.incident.version, 1);
        assert!(!result.incident.evidence_ids.is_empty());
    }
    assert_eq!(fixture.repository.incident_count().unwrap(), 6);
}

#[test]
fn replay_and_candidate_projection_do_not_create_incidents() {
    let fixture = creation_service_fixture();
    fixture.replay_signals_and_build_candidates().unwrap();
    assert_eq!(fixture.repository.incident_count().unwrap(), 0);
}
```

- [ ] **Step 2: Run and confirm source/service modules are missing**

Run: `cargo test -p thalassaops --test incident_creation`

Expected: FAIL with unresolved incident source/service imports.

- [ ] **Step 3: Implement source-kind mapping and provenance**

```rust
pub fn source_kind_matches_signal(kind: IncidentSourceKind, signal: &Signal) -> bool {
    matches!(
        (kind, signal.kind),
        (IncidentSourceKind::Alert, SignalKind::Alert)
            | (IncidentSourceKind::Anomaly, SignalKind::Anomaly)
            | (IncidentSourceKind::ScheduledHealthCheck, SignalKind::HealthCheck)
            | (IncidentSourceKind::VulnerabilityFinding, SignalKind::SecurityFinding)
    )
}
```

The resolver indexes deterministic Sprint 13 replay Signals by UUID. It returns source-record digest, observed time, Signal ID, evidence IDs and exact scope. It rejects wrong kind, missing Signal, denied retention/evidence and workspace mismatch. It never calls a provider or stores the source payload.

User-report JSON contains reporter Principal ID, observed time, summary and ResourceScope. Manual-report JSON contains observed time, summary and ResourceScope; actor comes only from the authorized command context. Parse with `deny_unknown_fields`, apply bounds and reject sensitive markers before forming `ResolvedIncidentTrigger`.

- [ ] **Step 4: Implement all-or-nothing create flow**

`IncidentService::create` accepts `IncidentCommandContext { workspace_scope, actor_id, policy_version, request_id, now }`. It rejects empty triggers, resolves every source-backed trigger before opening the repository transaction, derives one contained incident scope, sorts/deduplicates triggers and evidence, calculates severity, calls `Incident::create`, then repository `create`. A failure in any trigger leaves the database unchanged.

- [ ] **Step 5: Add mixed-scope and secret tests**

```rust
#[test]
fn mixed_scope_or_sensitive_report_fails_without_partial_incident() {
    let fixture = creation_service_fixture();
    let mixed = fixture.mixed_workspace_request();
    assert!(matches!(fixture.service.create(fixture.context(), mixed), Err(IncidentServiceError::ScopeMismatch)));
    let secret = fixture.manual_request("token=sk-live-example");
    assert!(matches!(fixture.service.create(fixture.context(), secret), Err(IncidentServiceError::SensitiveContent)));
    assert_eq!(fixture.repository.incident_count().unwrap(), 0);
}
```

- [ ] **Step 6: Run creation and related replay regressions**

Run: `cargo test -p thalassaops --test incident_creation`

Run: `cargo test -p thalassaops --test signal_adapters --test signal_grouping`

Expected: PASS; replay/correlation tests remain read-only.

- [ ] **Step 7: Commit Task 5**

```bash
git add src-tauri/src/incident docs/superpowers/fixtures/2026-08-30-incident src-tauri/tests/incident_creation.rs
git commit -m "feat(incident): create incidents from explicit triggers"
```

---

### Task 6: Apply Validated Mutations and Bounded Reads

**Files:**

- Modify: `src-tauri/src/incident/service.rs` — transition, severity, disposition, role, get/list/timeline methods.
- Modify: `src-tauri/src/incident/repository.rs` — role history updates, mutation request idempotency and bounded read cursors.
- Create: `src-tauri/tests/incident_mutations.rs` — lifecycle, reopen, required context, disposition, role, concurrency, timeline order and pagination.

**Interfaces:**

- Consumes: aggregate methods and repository transaction contract.
- Produces: `IncidentService::transition`, `set_severity`, `set_disposition`, `assign_role`, `get`, `list`, `timeline`; every write returns `IncidentMutation`, every read is workspace constrained.

- [ ] **Step 1: Write failing full-lifecycle service test**

```rust
#[test]
fn service_progresses_full_lifecycle_and_persists_ordered_events() {
    let fixture = persisted_service_fixture();
    let incident = fixture.create_incident();
    let incident = fixture.transition(incident, fixture.triage()).unwrap().incident;
    let incident = fixture.transition(incident, fixture.investigating()).unwrap().incident;
    let incident = fixture.transition(incident, fixture.mitigating()).unwrap().incident;
    let incident = fixture.transition(incident, fixture.monitoring()).unwrap().incident;
    let incident = fixture.transition(incident, fixture.resolved()).unwrap().incident;
    let incident = fixture.transition(incident, fixture.closed()).unwrap().incident;
    assert_eq!(incident.status, IncidentStatus::Closed);
    let timeline = fixture.service.timeline(fixture.read_context(), incident.id, None, 100).unwrap();
    assert!(timeline.events.windows(2).all(|pair| pair[0].sequence < pair[1].sequence));
}
```

- [ ] **Step 2: Run and confirm service mutation methods are missing**

Run: `cargo test -p thalassaops --test incident_mutations`

Expected: FAIL with missing service methods.

- [ ] **Step 3: Implement mutation orchestration**

Each method authorizes through an already constructed command context, loads the Incident in the workspace, checks `expected_version`, calls the corresponding pure aggregate method, and passes `IncidentMutation` to one immediate repository transaction. Map domain Store errors into `IncidentServiceError` without losing the stable reason.

Role Replace releases the old exclusive assignment and inserts the new assignment in the same transaction as current state and timeline. Role Release requires the named active assignment. Stakeholder Add/Release affects only the selected Principal.

- [ ] **Step 4: Implement stable bounded reads**

List order is `updated_at DESC, id ASC`; cursor encodes both values. Timeline order is sequence ASC and accepts `after_sequence`. Both limits are `1..=100`. `get`, `list` and `timeline` include current roles and triggers where their response contracts require them, but never full source payloads.

- [ ] **Step 5: Add reopen, disposition and version-conflict tests**

```rust
#[test]
fn closed_can_reopen_but_stale_writer_cannot_mutate() {
    let fixture = closed_incident_fixture();
    let closed = fixture.incident.clone();
    let reopened = fixture.service.transition(fixture.context(closed.version), fixture.reopen()).unwrap();
    assert_eq!(reopened.incident.status, IncidentStatus::Reopened);
    let stale = fixture.service.set_disposition(fixture.context(closed.version), fixture.false_positive());
    assert!(matches!(stale, Err(IncidentServiceError::VersionConflict { .. })));
}
```

- [ ] **Step 6: Run mutation/repository/domain regression**

Run: `cargo test -p thalassaops --test incident_mutations --test incident_repository`

Run: `cargo test -p thalassa-domain --test incident_lifecycle`

Expected: PASS.

- [ ] **Step 7: Commit Task 6**

```bash
git add src-tauri/src/incident src-tauri/tests/incident_mutations.rs
git commit -m "feat(incident): apply validated incident mutations"
```

---

### Task 7: Expose Secured Incident IPC

**Files:**

- Create: `src-tauri/src/app/incident.rs` — eight exact command handlers on `AppState`, authorization helpers, strict payload parsing and safe error mapping.
- Modify: `src-tauri/src/app/mod.rs` — declare incident app module, grant `ManageIncident` to allowed roles and expose database path safely to the incident service.
- Modify: `src-tauri/src/main.rs` — add eight Tauri command wrappers and register them.
- Create: `src-tauri/tests/incident_ipc.rs` — command/capability/scope/membership/role/policy order, exact keys, typed errors and successful paths.

**Interfaces:**

- Consumes: Task 3 descriptors and Task 5-6 service methods.
- Produces: `AppState` methods named `incident_create`, `incident_get`, `incident_list`, `incident_timeline`, `incident_transition`, `incident_set_severity`, `incident_set_disposition`, `incident_assign_role` returning `IpcResult<T>`.

- [ ] **Step 1: Write failing authorization-order tests**

```rust
#[test]
fn unauthorized_write_does_not_disclose_incident_existence() {
    let fixture = ipc_fixture_with_role(MembershipRole::Viewer);
    let missing_id = uuid::Uuid::new_v4();
    let result = fixture.transition_envelope(missing_id, 1);
    assert_eq!(result.error_code(), IpcErrorCode::PermissionDenied);
    assert!(!result.safe_details().contains(missing_id.to_string().as_str()));
}

#[test]
fn operator_can_write_but_auditor_and_viewer_cannot() {
    assert!(ipc_fixture_with_role(MembershipRole::Operator).create().is_ok());
    assert!(ipc_fixture_with_role(MembershipRole::Viewer).create().is_permission_denied());
    assert!(ipc_fixture_with_role(MembershipRole::Auditor).create().is_permission_denied());
}
```

- [ ] **Step 2: Run and confirm app incident module is missing**

Run: `cargo test -p thalassaops --test incident_ipc`

Expected: FAIL with missing `AppState` incident methods.

- [ ] **Step 3: Add authorization and policy boundary**

Add one private `authorize_incident` helper following correlation/change ordering. It validates exact descriptor/capability, unbounded envelope scope, active membership, Principal identity, workspace grant and role permission before payload parsing or target lookup. Write paths evaluate verified Internal data for local storage and audit retention. Read paths evaluate UI policy before serialization.

Extend role mapping exactly:

```rust
MembershipRole::Operator => matches!(
    permission,
    Permission::Read
        | Permission::Investigate
        | Permission::ManageIncident
        | Permission::RecommendAction
        | Permission::ExecuteAction
),
MembershipRole::Viewer => matches!(permission, Permission::Read | Permission::Investigate),
MembershipRole::Auditor => matches!(permission, Permission::Read | Permission::AuditRead),
```

- [ ] **Step 4: Parse exact request shapes and map stable errors**

Use `#[serde(deny_unknown_fields)]` request structs. Validate all limits before opening SQLite. Map permission and policy failures to their existing IPC codes; map missing scoped targets to NotFound; map malformed input, transition/context, idempotency and version conflicts to InvalidRequest with a safe `reason` such as `incident_version_conflict`; map unavailable SQLite to InternalError without database text.

- [ ] **Step 5: Register exact Tauri commands**

Add synchronous wrappers matching existing local-only change commands. Each wrapper receives `CommandEnvelope<Value>`, borrows `AppState` and returns the corresponding `IpcResult`. Add all eight names to `tauri::generate_handler!`; do not add aliases or generic `incident.update`.

- [ ] **Step 6: Run IPC, migration and security regression**

Run: `cargo test -p thalassaops --test incident_ipc --test incident_repository --test security_adapters`

Expected: PASS with permission-before-existence and policy-before-persistence assertions.

- [ ] **Step 7: Commit Task 7**

```bash
git add src-tauri/src/app/incident.rs src-tauri/src/app/mod.rs src-tauri/src/main.rs src-tauri/tests/incident_ipc.rs
git commit -m "feat(incident): expose secured incident IPC"
```

---

### Task 8: Prove Sprint Acceptance and Run Release Gates

**Files:**

- Create: `src-tauri/tests/incident_acceptance.rs` — six sources, explicit multi-trigger creation, full lifecycle, reopen, dispositions, roles, audit, policy failure and concurrency.
- Create: `docs/superpowers/reports/2026-08-30-sprint-15-verification.md` — commands, exact counts, acceptance evidence and remaining exclusions.
- Modify: `README.md` — point current status and latest approved design to Sprint 15 after verification passes.

**Interfaces:**

- Consumes: every Sprint 15 public contract and committed fixture.
- Produces: acceptance proof and verification report; no new production abstraction.

- [ ] **Step 1: Write the end-to-end acceptance test**

```rust
#[test]
fn sprint_15_exit_criterion_is_reachable_from_committed_fixtures() {
    let fixture = acceptance_fixture();
    let created = fixture.create_one_incident_from_each_source_kind();
    assert_eq!(created.len(), 6);
    assert!(created.iter().all(|incident| incident.status == IncidentStatus::Detected));

    let multi = fixture.create_from_selected_candidate_signals();
    assert!(multi.trigger_ids.len() >= 2);
    assert!(!multi.trigger_kinds().contains(&"correlation_candidate"));

    let closed = fixture.progress_to_closed(multi);
    assert_eq!(closed.status, IncidentStatus::Closed);
    let reopened = fixture.reopen_and_investigate(closed);
    assert_eq!(reopened.status, IncidentStatus::Investigating);

    let timeline = fixture.timeline(reopened.id);
    assert!(timeline.events.windows(2).all(|pair| pair[0].sequence < pair[1].sequence));
    assert!(timeline.events.iter().all(|event| !event.actor_id.is_nil()));
}
```

- [ ] **Step 2: Add disposition, role, policy and concurrency acceptance assertions**

Exercise Duplicate, False Positive, Suppressed, Cancelled and retained Informational. Verify Duplicate points to another same-workspace Incident without merging. Assign every S1/S2 responder role while allowing one Principal to hold multiple roles. Deny one write by audit/local-storage policy and prove no row/event count changes. Submit one stale version and prove current state/timeline remain unchanged.

- [ ] **Step 3: Run focused Sprint 15 tests**

Run: `cargo test -p thalassa-domain --test incident_contracts --test incident_lifecycle`

Run: `cargo test -p thalassa-ipc --test contracts`

Run: `cargo test -p thalassaops --test incident_repository --test incident_creation --test incident_mutations --test incident_ipc --test incident_acceptance`

Run: `npm test -- ui/src/incident/incident-contracts.test.ts`

Expected: PASS.

- [ ] **Step 4: Run all seven release gates without pipes**

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run format:check
npm run lint
npm run typecheck
npm test
```

Record command, exit code, Rust test count, frontend test count and focused acceptance facts in the verification report. Never label an unrun or interrupted command as passed.

- [ ] **Step 5: Update README only after all gates pass**

Change the status paragraph to include the canonical local-first Incident domain, explicit six-source creation, validated lifecycle and immutable timeline. Point latest approved design to `docs/design/sprint-15-incident-domain-lifecycle.md`. Do not claim the Sprint 16 Incident Workspace exists.

- [ ] **Step 6: Re-run formatting checks for documentation changes**

Run: `cargo fmt --all -- --check`

Run: `npm run format:check`

Expected: PASS.

- [ ] **Step 7: Commit Task 8**

```bash
git add src-tauri/tests/incident_acceptance.rs docs/superpowers/reports/2026-08-30-sprint-15-verification.md README.md
git commit -m "test(incident): verify sprint 15 lifecycle acceptance"
```

## Completion Gate

The implementation branch is ready for coordinator review only when Tasks 1-8 each have a focused commit, the worktree is clean, all seven release gates pass and the verification report contains observed counts. The coordinator independently reviews the complete design-to-diff mapping, checks documented standards and code smells, reruns proportionate tests, and returns defects to OMP. Jira SCRUM-23 moves to Done and the Sprint 15 sprint closes only after review passes and the accepted branch is merged.
