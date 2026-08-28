// SPDX-License-Identifier: Apache-2.0

//! Provider-neutral domain contracts shared by the Rust core and adapters.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub type OrganizationId = Uuid;
pub type TeamId = Uuid;
pub type WorkspaceId = Uuid;
pub type EnvironmentId = Uuid;
pub type ResourceId = Uuid;
pub type SignalId = Uuid;
pub type IncidentId = Uuid;
pub type EvidenceId = Uuid;
pub type HypothesisId = Uuid;
pub type ActionId = Uuid;
pub type PolicyId = Uuid;
pub type AuditId = Uuid;
pub type PrincipalId = Uuid;
pub type LocalPrincipal = Principal;

fn now() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResourceScope {
    pub organization_id: Option<OrganizationId>,
    pub team_id: Option<TeamId>,
    pub workspace_id: Option<WorkspaceId>,
    pub environment_id: Option<EnvironmentId>,
    pub resource_ids: Vec<ResourceId>,
}

impl ResourceScope {
    pub fn organization(id: OrganizationId) -> Self {
        Self {
            organization_id: Some(id),
            ..Default::default()
        }
    }
    pub fn team(id: TeamId, organization_id: OrganizationId) -> Self {
        Self {
            organization_id: Some(organization_id),
            team_id: Some(id),
            ..Default::default()
        }
    }
    pub fn workspace(id: WorkspaceId, team_id: TeamId, organization_id: OrganizationId) -> Self {
        Self {
            organization_id: Some(organization_id),
            team_id: Some(team_id),
            workspace_id: Some(id),
            ..Default::default()
        }
    }
    pub fn environment(
        id: EnvironmentId,
        workspace_id: WorkspaceId,
        team_id: TeamId,
        organization_id: OrganizationId,
    ) -> Self {
        Self {
            organization_id: Some(organization_id),
            team_id: Some(team_id),
            workspace_id: Some(workspace_id),
            environment_id: Some(id),
            ..Default::default()
        }
    }
    pub fn resource(mut self, id: ResourceId) -> Self {
        self.resource_ids.push(id);
        self
    }
    pub fn is_bounded(&self) -> bool {
        self.organization_id.is_some()
            || self.team_id.is_some()
            || self.workspace_id.is_some()
            || self.environment_id.is_some()
            || !self.resource_ids.is_empty()
    }

    /// Returns true when `candidate` is equal to or narrower than this scope.
    pub fn contains(&self, candidate: &Self) -> bool {
        (self.organization_id.is_none() || candidate.organization_id == self.organization_id)
            && (self.team_id.is_none() || candidate.team_id == self.team_id)
            && (self.workspace_id.is_none() || candidate.workspace_id == self.workspace_id)
            && (self.environment_id.is_none() || candidate.environment_id == self.environment_id)
            && (self.resource_ids.is_empty()
                || (!candidate.resource_ids.is_empty()
                    && candidate
                        .resource_ids
                        .iter()
                        .all(|id| self.resource_ids.contains(id))))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Organization {
    pub id: OrganizationId,
    pub name: String,
    pub created_at: DateTime<Utc>,
}
impl Organization {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            created_at: now(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Team {
    pub id: TeamId,
    pub organization_id: OrganizationId,
    pub name: String,
    pub created_at: DateTime<Utc>,
}
impl Team {
    pub fn new(organization_id: OrganizationId, name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            organization_id,
            name: name.into(),
            created_at: now(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub team_id: TeamId,
    pub name: String,
    pub created_at: DateTime<Utc>,
}
impl Workspace {
    pub fn new(team_id: TeamId, name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            team_id,
            name: name.into(),
            created_at: now(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum EnvironmentKind {
    Kubernetes,
    VirtualMachines,
    BareMetal,
    CloudAccount,
    Serverless,
    Network,
    Other(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Environment {
    pub id: EnvironmentId,
    pub workspace_id: WorkspaceId,
    pub name: String,
    pub kind: EnvironmentKind,
    pub provider: Option<String>,
    pub created_at: DateTime<Utc>,
}
impl Environment {
    pub fn new(workspace_id: WorkspaceId, name: impl Into<String>, kind: EnvironmentKind) -> Self {
        Self {
            id: Uuid::new_v4(),
            workspace_id,
            name: name.into(),
            kind,
            provider: None,
            created_at: now(),
        }
    }
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum PrincipalKind {
    Local,
    Human,
    Service,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnterpriseIdentity {
    pub issuer: Option<String>,
    pub subject: String,
    pub provider: Option<String>,
    pub external_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Principal {
    pub id: PrincipalId,
    pub kind: PrincipalKind,
    pub display_name: String,
    pub identity: EnterpriseIdentity,
    pub created_at: DateTime<Utc>,
}
impl Principal {
    pub fn local(subject: impl Into<String>, display_name: impl Into<String>) -> Self {
        let subject = subject.into();
        Self {
            id: Uuid::new_v4(),
            kind: PrincipalKind::Local,
            display_name: display_name.into(),
            identity: EnterpriseIdentity {
                subject,
                ..Default::default()
            },
            created_at: now(),
        }
    }
    pub fn enterprise(
        kind: PrincipalKind,
        issuer: impl Into<String>,
        subject: impl Into<String>,
        display_name: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            kind,
            display_name: display_name.into(),
            identity: EnterpriseIdentity {
                issuer: Some(issuer.into()),
                subject: subject.into(),
                ..Default::default()
            },
            created_at: now(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MembershipRole {
    Owner,
    Administrator,
    Operator,
    Viewer,
    Auditor,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MembershipStatus {
    Active,
    Suspended,
    Revoked,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Membership {
    pub principal_id: PrincipalId,
    pub role: MembershipRole,
    pub status: MembershipStatus,
    pub scope: ResourceScope,
    pub created_at: DateTime<Utc>,
}
impl Membership {
    pub fn workspace_owner(principal_id: PrincipalId, workspace_id: WorkspaceId) -> Self {
        Self {
            principal_id,
            role: MembershipRole::Owner,
            status: MembershipStatus::Active,
            scope: ResourceScope {
                workspace_id: Some(workspace_id),
                ..Default::default()
            },
            created_at: now(),
        }
    }
    pub fn grants(&self, scope: &ResourceScope) -> bool {
        self.status == MembershipStatus::Active && self.scope.contains(scope)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Resource {
    pub id: ResourceId,
    pub environment_id: EnvironmentId,
    pub scope: ResourceScope,
    pub kind: String,
    pub name: String,
    pub provider: Option<String>,
    pub native_id: Option<String>,
    pub labels: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
}
impl Resource {
    pub fn new(
        environment_id: EnvironmentId,
        scope: ResourceScope,
        kind: impl Into<String>,
        name: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            environment_id,
            scope,
            kind: kind.into(),
            name: name.into(),
            provider: None,
            native_id: None,
            labels: BTreeMap::new(),
            created_at: now(),
        }
    }
    pub fn with_labels(mut self, labels: BTreeMap<String, String>) -> Self {
        self.labels = labels;
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Signal {
    pub id: SignalId,
    pub source: String,
    pub kind: String,
    pub observed_at: DateTime<Utc>,
    pub resource_ids: Vec<ResourceId>,
    pub payload: Value,
}
impl Signal {
    pub fn new(
        source: impl Into<String>,
        kind: impl Into<String>,
        resource_ids: Vec<ResourceId>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source: source.into(),
            kind: kind.into(),
            observed_at: now(),
            resource_ids,
            payload: Value::Null,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum IncidentSeverity {
    S1,
    S2,
    S3,
    S4,
    S5,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IncidentStatus {
    Detected,
    Triage,
    Investigating,
    Mitigating,
    Monitoring,
    Resolved,
    Closed,
    Reopened,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IncidentDisposition {
    Duplicate,
    FalsePositive,
    Suppressed,
    Cancelled,
    Informational,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Incident {
    pub id: IncidentId,
    pub summary: String,
    pub severity: IncidentSeverity,
    pub status: IncidentStatus,
    pub disposition: Option<IncidentDisposition>,
    pub scope: ResourceScope,
    pub signal_ids: Vec<SignalId>,
    pub evidence_ids: Vec<EvidenceId>,
    pub hypothesis_ids: Vec<HypothesisId>,
    pub action_ids: Vec<ActionId>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl Incident {
    pub fn new(
        summary: impl Into<String>,
        severity: IncidentSeverity,
        scope: ResourceScope,
    ) -> Self {
        let timestamp = now();
        Self {
            id: Uuid::new_v4(),
            summary: summary.into(),
            severity,
            status: IncidentStatus::Detected,
            disposition: None,
            scope,
            signal_ids: vec![],
            evidence_ids: vec![],
            hypothesis_ids: vec![],
            action_ids: vec![],
            created_at: timestamp,
            updated_at: timestamp,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Evidence {
    pub id: EvidenceId,
    pub source: String,
    pub resource_scope: ResourceScope,
    pub observed_at: DateTime<Utc>,
    pub time_range: Option<TimeRange>,
    pub query: String,
    pub excerpt: String,
    pub structured_result: Option<Value>,
}
impl Evidence {
    pub fn new(
        source: impl Into<String>,
        resource_scope: ResourceScope,
        query: impl Into<String>,
        excerpt: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source: source.into(),
            resource_scope,
            observed_at: now(),
            time_range: None,
            query: query.into(),
            excerpt: excerpt.into(),
            structured_result: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub statement: String,
    pub confidence: f32,
    pub evidence_ids: Vec<EvidenceId>,
    pub created_at: DateTime<Utc>,
}
impl Hypothesis {
    pub fn new(
        statement: impl Into<String>,
        confidence: f32,
        evidence_ids: Vec<EvidenceId>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            statement: statement.into(),
            confidence: confidence.clamp(0.0, 1.0),
            evidence_ids,
            created_at: now(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ActionRiskClass {
    #[serde(rename = "READ-ONLY")]
    ReadOnly,
    #[serde(rename = "MUTATING")]
    Mutating,
    #[serde(rename = "BLOCKED")]
    Blocked,
    #[serde(rename = "REQUIRES APPROVAL")]
    RequiresApproval,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ExecutionMode {
    #[serde(rename = "OBSERVE")]
    Observe,
    #[serde(rename = "RECOMMEND")]
    Recommend,
    #[serde(rename = "APPROVAL")]
    Approval,
    #[serde(rename = "POLICY_AUTO")]
    PolicyAuto,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ActionStatus {
    Proposed,
    PendingApproval,
    Approved,
    Executed,
    Rejected,
    Failed,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Action {
    pub id: ActionId,
    pub description: String,
    pub risk_class: ActionRiskClass,
    pub execution_mode: ExecutionMode,
    pub scope: ResourceScope,
    pub target_resource_ids: Vec<ResourceId>,
    pub expected_impact: Option<String>,
    pub rollback_plan: Option<String>,
    pub verification_plan: Option<String>,
    pub status: ActionStatus,
    pub created_at: DateTime<Utc>,
}
impl Action {
    pub fn new(
        description: impl Into<String>,
        risk_class: ActionRiskClass,
        execution_mode: ExecutionMode,
        scope: ResourceScope,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            description: description.into(),
            risk_class,
            execution_mode,
            scope,
            target_resource_ids: vec![],
            expected_impact: None,
            rollback_plan: None,
            verification_plan: None,
            status: ActionStatus::Proposed,
            created_at: now(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Policy {
    pub id: PolicyId,
    pub name: String,
    pub version: u64,
    pub scope: ResourceScope,
    pub created_at: DateTime<Utc>,
}
impl Policy {
    pub fn new(name: impl Into<String>, version: u64, scope: ResourceScope) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            version,
            scope,
            created_at: now(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Audit {
    pub id: AuditId,
    pub event_type: String,
    pub actor_id: Option<PrincipalId>,
    pub scope: ResourceScope,
    pub timestamp: DateTime<Utc>,
    pub outcome: String,
    pub details: Value,
    pub policy_version: Option<u64>,
}
impl Audit {
    pub fn new(event_type: impl Into<String>, scope: ResourceScope) -> Self {
        Self {
            id: Uuid::new_v4(),
            event_type: event_type.into(),
            actor_id: None,
            scope,
            timestamp: now(),
            outcome: "recorded".into(),
            details: Value::Null,
            policy_version: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Permission {
    Read,
    Investigate,
    RecommendAction,
    ExecuteAction,
    ManagePolicy,
    ManageMembership,
    AuditRead,
}

/// Stable identifier for evidence captured in an Operations Console snapshot.
pub type ConsoleEvidenceId = String;

/// Provider-neutral source that produced a console evidence reference.
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

/// Redaction and classification assertions attached to evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EvidenceRedaction {
    pub classification_verified: bool,
    pub redaction_verified: bool,
    pub masked: bool,
    pub unparsed: bool,
}

/// A redacted source-backed fact that can be displayed or opened natively.
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

/// Typed local destination for a console drill-down.
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

/// Evidence IDs and an optional local filter for a console drill-down.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrillDownTarget {
    pub destination: DrillDownDestination,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub filter_key: Option<String>,
}

/// Request payload for retrieving evidence already admitted to a snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationsEvidenceRequest {
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

/// Unit used to render a critical number at the IPC boundary.
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

/// A displayed number together with the evidence and typed destination that explain it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CriticalNumber {
    pub key: String,
    pub value: String,
    pub unit: NumberUnit,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}

/// Overall health posture for the business-impact-first console summary.
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

/// Business impact tier, ordered from greatest to least impact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

/// Business-impact severity used by console queue items and anomaly signals.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

/// Operational priority, independent from business-impact severity.
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

/// A compact, evidence-backed description of business impact.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BusinessImpact {
    pub level: ImpactLevel,
    pub summary: String,
    pub customer_scope: String,
    pub service_criticality: String,
    pub trajectory: ImpactTrajectory,
}

/// Direction in which observed business impact is moving.
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

/// Business-impact-first health summary and evidence-backed headline counts.
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

impl HealthSummary {
    /// Returns the overall posture represented by this summary.
    pub fn overall_posture(&self) -> ConsoleHealthState {
        self.state
    }

    /// Returns the highest impact tier represented by the headline.
    pub fn impact_tier(&self) -> ImpactLevel {
        self.headline.level
    }
}

/// A scope that contributes to the current business-impact posture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContributingScope {
    pub scope: ResourceScope,
    pub impact: ImpactLevel,
    pub summary: String,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

/// Source category for an independent active queue projection item.
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

/// Active queue lifecycle status shown by the read-only console.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

/// An independent alert, anomaly or scheduled-check item in the active queue.
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

/// Aggregated active alerts, anomalies and scheduled health-check counts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignalSummary {
    pub active_alerts: CriticalNumber,
    pub active_anomalies: CriticalNumber,
    pub checks_due: CriticalNumber,
    pub checks_timed_out: CriticalNumber,
    pub by_source: Vec<SignalCount>,
}

/// A source-specific count in the signal summary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignalCount {
    pub source_kind: QueueItemSourceKind,
    pub count: CriticalNumber,
}

/// Aggregated alert count for consumers that render alerts separately.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AlertSummary {
    pub active: CriticalNumber,
    pub by_source: Vec<SignalCount>,
}

/// Aggregated anomaly count for consumers that render anomalies separately.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AnomalySummary {
    pub active: CriticalNumber,
    pub by_severity: Vec<CriticalNumber>,
}

/// Threshold or rate-of-change rule evaluated against one metric fixture.
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

impl AnomalyRule {
    /// Validates identifiers, scope-independent metric selection and finite comparison values.
    pub fn validate(&self) -> Result<(), AnomalyRuleError> {
        if self.id.trim().is_empty() {
            return Err(AnomalyRuleError::Validation("id cannot be empty".into()));
        }
        if self.name.trim().is_empty() {
            return Err(AnomalyRuleError::Validation("name cannot be empty".into()));
        }
        if self.metric_key.trim().is_empty() {
            return Err(AnomalyRuleError::Validation(
                "metric_key cannot be empty".into(),
            ));
        }
        self.condition.validate()
    }

    /// Alias for [`AnomalyRule::validate`] useful to validation pipelines.
    pub fn is_valid(&self) -> Result<(), AnomalyRuleError> {
        self.validate()
    }
}

/// Validation errors for anomaly rule definitions.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum AnomalyRuleError {
    #[error("validation error: {0}")]
    Validation(String),
}

/// Comparison expression used by an anomaly rule.
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

impl AnomalyCondition {
    /// Validates finite decimal thresholds and a positive rate-comparison window.
    pub fn validate(&self) -> Result<(), AnomalyRuleError> {
        match self {
            Self::Threshold { threshold, .. } => {
                if is_finite_decimal(threshold) {
                    Ok(())
                } else {
                    Err(AnomalyRuleError::Validation(
                        "threshold must be a finite decimal".into(),
                    ))
                }
            }
            Self::RateOfChange {
                direction,
                threshold_per_second,
                window_seconds,
            } => {
                if !is_finite_decimal(threshold_per_second) {
                    return Err(AnomalyRuleError::Validation(
                        "threshold_per_second must be a finite decimal".into(),
                    ));
                }
                if *window_seconds == 0 {
                    return Err(AnomalyRuleError::Validation(
                        "window_seconds must be positive".into(),
                    ));
                }
                let threshold = threshold_per_second.parse::<f64>().map_err(|_| {
                    AnomalyRuleError::Validation(
                        "threshold_per_second must be a finite decimal".into(),
                    )
                })?;
                let direction_is_valid = match direction {
                    RateDirection::Increase => threshold > 0.0,
                    RateDirection::Decrease => threshold < 0.0,
                    RateDirection::Absolute => threshold >= 0.0,
                };
                if direction_is_valid {
                    Ok(())
                } else {
                    Err(AnomalyRuleError::Validation(
                        "rate threshold sign does not match direction".into(),
                    ))
                }
            }
        }
    }
}

fn is_finite_decimal(value: &str) -> bool {
    value
        .trim()
        .parse::<f64>()
        .is_ok_and(|number| number.is_finite())
}

/// Operator used by a threshold anomaly condition.
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

/// Direction used by a rate-of-change anomaly condition.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum RateDirection {
    #[serde(rename = "increase")]
    Increase,
    #[serde(rename = "decrease")]
    Decrease,
    #[serde(rename = "absolute")]
    Absolute,
}

/// Deterministic metric input to the anomaly evaluator.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MetricFixture {
    pub key: String,
    pub scope: ResourceScope,
    pub labels: BTreeMap<String, String>,
    pub samples: Vec<MetricFixtureSample>,
    pub source: MetricFixtureSource,
}

/// One timestamped decimal sample in a metric fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MetricFixtureSample {
    pub timestamp_seconds: i64,
    pub value: String,
}

/// Trusted source metadata for a metric fixture.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MetricFixtureSource {
    pub connector_id: String,
    pub query: String,
    pub endpoint: String,
}

/// Produced anomaly signal with its source scope and evidence reference.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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

/// Outcome of evaluating one anomaly rule.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AnomalyEvaluationStatus {
    #[serde(rename = "triggered")]
    Triggered,
    #[serde(rename = "not_triggered")]
    NotTriggered,
    #[serde(rename = "insufficient_data")]
    InsufficientData,
}

/// Rule evaluation result with an optional produced signal.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AnomalyEvaluation {
    pub rule_id: String,
    pub metric_key: String,
    pub status: AnomalyEvaluationStatus,
    pub signal: Option<AnomalySignal>,
}

/// Scheduled, explicit-clock health-check definition.
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

impl HealthCheckSchedule {
    /// Validates scheduling bounds, timestamps, scope and source identifiers.
    pub fn validate(&self) -> Result<(), HealthCheckScheduleError> {
        if self.id.trim().is_empty() {
            return Err(HealthCheckScheduleError::Validation(
                "id cannot be empty".into(),
            ));
        }
        if self.name.trim().is_empty() {
            return Err(HealthCheckScheduleError::Validation(
                "name cannot be empty".into(),
            ));
        }
        if self.interval_seconds == 0 {
            return Err(HealthCheckScheduleError::Validation(
                "interval_seconds must be positive".into(),
            ));
        }
        if self.timeout_ms == 0 {
            return Err(HealthCheckScheduleError::Validation(
                "timeout_ms must be positive".into(),
            ));
        }
        if !self.scope.is_bounded() {
            return Err(HealthCheckScheduleError::Validation(
                "scope must be bounded".into(),
            ));
        }
        self.source.validate()?;
        for timestamp in [self.last_run_at.as_deref(), self.last_signal_at.as_deref()]
            .into_iter()
            .flatten()
        {
            if !is_rfc3339_timestamp(timestamp) {
                return Err(HealthCheckScheduleError::Validation(
                    "timestamps must be RFC3339".into(),
                ));
            }
        }
        if let Some(timestamp) = self.defined_at.as_deref() {
            if !is_rfc3339_timestamp(timestamp) {
                return Err(HealthCheckScheduleError::Validation(
                    "defined_at must be RFC3339".into(),
                ));
            }
        }
        if self
            .defined_by
            .as_deref()
            .is_some_and(|defined_by| defined_by.trim().is_empty())
        {
            return Err(HealthCheckScheduleError::Validation(
                "defined_by cannot be empty".into(),
            ));
        }
        Ok(())
    }

    /// Alias for [`HealthCheckSchedule::validate`] useful to validation pipelines.
    pub fn is_valid(&self) -> Result<(), HealthCheckScheduleError> {
        self.validate()
    }
}

/// Validation errors for scheduled health-check definitions.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HealthCheckScheduleError {
    #[error("validation error: {0}")]
    Validation(String),
}

/// Fixture outcome used by a deterministic health-check evaluation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FixtureHealthCheck {
    pub outcome: HealthCheckOutcome,
    pub duration_ms: u64,
    pub evidence_id: Option<ConsoleEvidenceId>,
}

/// Provider-neutral source for a scheduled health check.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum HealthCheckSource {
    #[serde(rename = "connector")]
    Connector {
        connector_id: String,
        probe_key: String,
    },
    #[serde(rename = "kubernetes")]
    Kubernetes {
        connector_id: String,
        resource_key: String,
    },
    #[serde(rename = "observability")]
    Observability {
        connector_id: String,
        probe_key: String,
    },
    #[serde(rename = "fixture")]
    Fixture { fixture_key: String },
}

impl HealthCheckSource {
    /// Validates source identifiers without inspecting provider payloads.
    pub fn validate(&self) -> Result<(), HealthCheckScheduleError> {
        let valid = match self {
            Self::Connector {
                connector_id,
                probe_key,
            }
            | Self::Observability {
                connector_id,
                probe_key,
            } => !connector_id.trim().is_empty() && !probe_key.trim().is_empty(),
            Self::Kubernetes {
                connector_id,
                resource_key,
            } => !connector_id.trim().is_empty() && !resource_key.trim().is_empty(),
            Self::Fixture { fixture_key } => !fixture_key.trim().is_empty(),
        };
        if valid {
            Ok(())
        } else {
            Err(HealthCheckScheduleError::Validation(
                "source identifiers cannot be empty".into(),
            ))
        }
    }
}

/// Result status from one deterministic health-check run.
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

/// Audit metadata retained for a health-check run.
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

/// Result of evaluating a scheduled health check at an explicit timestamp.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HealthCheckResult {
    pub schedule_id: String,
    pub outcome: HealthCheckOutcome,
    pub observed_at: String,
    pub evidence_id: Option<ConsoleEvidenceId>,
    pub audit: HealthCheckAudit,
}

fn is_rfc3339_timestamp(value: &str) -> bool {
    DateTime::parse_from_rfc3339(value).is_ok()
}

/// Category of a recent operational change.
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

/// Evidence-backed change shown in the recent change stream.
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

/// Typed reason for a source or projection status that is not healthy/fresh.
///
/// The UI maps these stable wire values to localized copy.  Provider-specific
/// details belong in evidence and the optional `SourceStatus::detail` field,
/// never in this user-facing reason.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum StatusReason {
    #[serde(rename = "not_configured")]
    NotConfigured,
    #[serde(rename = "unreachable")]
    Unreachable,
    #[serde(rename = "timed_out")]
    TimedOut,
    #[serde(rename = "policy_denied")]
    PolicyDenied,
    #[serde(rename = "no_data_in_window")]
    NoDataInWindow,
    #[serde(rename = "unknown")]
    Unknown,
}

/// Availability state for the recent change widget.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeStreamState {
    #[serde(rename = "available")]
    Available,
    #[serde(rename = "empty")]
    Empty,
    #[serde(rename = "unavailable")]
    Unavailable,
}

/// Explicit state for the recent change stream, including honest empty data.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeStreamStatus {
    pub state: ChangeStreamState,
    pub reason: Option<StatusReason>,
    pub detail: Option<String>,
}

impl Default for ChangeStreamStatus {
    fn default() -> Self {
        Self {
            state: ChangeStreamState::Empty,
            reason: Some(StatusReason::NotConfigured),
            detail: None,
        }
    }
}

/// Provider-neutral environment health and resource count overview.
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

/// Snapshot source freshness and evidence status.
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

/// Status for one provider-neutral snapshot source.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceStatus {
    pub source_key: String,
    pub state: SourceState,
    #[serde(default)]
    pub reason: Option<StatusReason>,
    #[serde(default)]
    pub detail: Option<String>,
    pub observed_at: Option<String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

/// Complete read-only Operations Console projection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OperationsSnapshot {
    pub generated_at: String,
    pub scope: ResourceScope,
    pub source_status: Vec<SourceStatus>,
    pub health_summary: HealthSummary,
    pub incident_queue: Vec<IncidentQueueItem>,
    pub signal_summary: SignalSummary,
    pub changes: Vec<ChangeStreamItem>,
    #[serde(default)]
    pub change_stream_status: ChangeStreamStatus,
    pub environments: Vec<EnvironmentStatus>,
    pub evidence: Vec<EvidenceRef>,
    pub widget_registry: Vec<WidgetDefinition>,
}

/// Fixed curated widget identity for the Operations Console.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

/// Curated widget width used by the presentation contract.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WidgetSize {
    #[serde(rename = "compact")]
    Compact,
    #[serde(rename = "standard")]
    Standard,
    #[serde(rename = "wide")]
    Wide,
}

/// Server-owned widget registry entry and default presentation metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WidgetDefinition {
    pub id: WidgetId,
    pub title_key: String,
    pub default_order: u16,
    pub default_size: WidgetSize,
    pub required: bool,
}

/// Local presentation preference for a curated widget.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WidgetPreference {
    pub id: WidgetId,
    pub visible: bool,
    pub order: u16,
    pub size: WidgetSize,
    pub collapsed: bool,
}

/// Alias for the widget kind, which is fixed to the curated widget IDs.
pub type WidgetKind = WidgetId;

/// Per-widget options stored as JSON values and never interpreted as queries.
pub type WidgetOptions = BTreeMap<String, Value>;

/// User-local widget configuration for the curated dashboard.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WidgetConfig {
    pub id: WidgetId,
    pub kind: WidgetKind,
    pub visible: bool,
    pub order: u16,
    pub options: WidgetOptions,
}

/// Returns the deterministic five-widget Operations Console layout.
pub fn curated_default_layout() -> Vec<WidgetConfig> {
    [
        (WidgetId::HealthSummary, true),
        (WidgetId::IncidentQueue, true),
        (WidgetId::SignalSummary, true),
        (WidgetId::ChangeStream, true),
        (WidgetId::EnvironmentStatus, true),
    ]
    .into_iter()
    .enumerate()
    .map(|(order, (id, visible))| WidgetConfig {
        id,
        kind: id,
        visible,
        order: order as u16,
        options: BTreeMap::new(),
    })
    .collect()
}

/// Explicit string time window used by a drill-down reference.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TimeWindow {
    pub start: String,
    pub end: String,
}

/// Source query, scope, time window and evidence behind a displayed number.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DrillDownReference {
    pub source_query: String,
    pub scope: ResourceScope,
    pub time_window: Option<TimeWindow>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

/// Validation errors for an Operations Console snapshot.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum OperationsSnapshotError {
    #[error("validation error: {0}")]
    Validation(String),
}

impl CriticalNumber {
    /// Validates that a displayed number carries a usable evidence drill-down.
    pub fn validate(&self) -> Result<(), OperationsSnapshotError> {
        if self.key.trim().is_empty() {
            return Err(OperationsSnapshotError::Validation(
                "critical number key cannot be empty".into(),
            ));
        }
        if self.value.trim().is_empty() {
            return Err(OperationsSnapshotError::Validation(
                "critical number value cannot be empty".into(),
            ));
        }
        if self
            .value
            .trim()
            .parse::<f64>()
            .map_or(true, |value| !value.is_finite())
        {
            return Err(OperationsSnapshotError::Validation(
                "critical number value must be a finite decimal".into(),
            ));
        }
        if self.evidence_ids.is_empty() {
            return Err(OperationsSnapshotError::Validation(
                "critical number requires evidence".into(),
            ));
        }
        if self.drill_down.evidence_ids.is_empty() {
            return Err(OperationsSnapshotError::Validation(
                "critical number drill-down requires evidence".into(),
            ));
        }
        if !self
            .evidence_ids
            .iter()
            .any(|id| self.drill_down.evidence_ids.contains(id))
        {
            return Err(OperationsSnapshotError::Validation(
                "critical number drill-down must reference its evidence".into(),
            ));
        }
        if self.drill_down_reference.source_query.trim().is_empty() {
            return Err(OperationsSnapshotError::Validation(
                "critical number source query cannot be empty".into(),
            ));
        }
        if self.drill_down_reference.evidence_ids.is_empty()
            || !self
                .evidence_ids
                .iter()
                .any(|id| self.drill_down_reference.evidence_ids.contains(id))
        {
            return Err(OperationsSnapshotError::Validation(
                "critical number reference must identify its evidence".into(),
            ));
        }
        Ok(())
    }
}

impl OperationsSnapshot {
    /// Returns every number-bearing field that crosses the console boundary.
    pub fn critical_numbers(&self) -> Vec<&CriticalNumber> {
        let health = &self.health_summary;
        let signal = &self.signal_summary;
        let mut numbers = vec![
            &health.attention,
            &health.impacted_services,
            &signal.active_alerts,
            &signal.active_anomalies,
            &signal.checks_due,
            &signal.checks_timed_out,
        ];
        numbers.extend(health.active_by_severity.iter());
        numbers.extend(health.environments_by_state.iter());
        numbers.extend(signal.by_source.iter().map(|item| &item.count));
        numbers.extend(
            self.environments
                .iter()
                .map(|environment| &environment.resource_count),
        );
        numbers
    }

    /// Validates evidence references, uniqueness and critical-number invariants.
    pub fn validate(&self) -> Result<(), OperationsSnapshotError> {
        let evidence_ids: BTreeSet<_> = self
            .evidence
            .iter()
            .map(|evidence| evidence.id.as_str())
            .collect();
        if evidence_ids.len() != self.evidence.len()
            || self
                .evidence
                .iter()
                .any(|evidence| evidence.id.trim().is_empty())
        {
            return Err(OperationsSnapshotError::Validation(
                "evidence IDs must be unique and non-empty".into(),
            ));
        }

        for number in self.critical_numbers() {
            number.validate()?;
            for id in number
                .evidence_ids
                .iter()
                .chain(number.drill_down.evidence_ids.iter())
                .chain(number.drill_down_reference.evidence_ids.iter())
            {
                if !evidence_ids.contains(id.as_str()) {
                    return Err(OperationsSnapshotError::Validation(
                        "critical number references unknown evidence".into(),
                    ));
                }
            }
        }

        for ids in self
            .source_status
            .iter()
            .map(|status| &status.evidence_ids)
            .chain(self.incident_queue.iter().map(|item| &item.evidence_ids))
            .chain(self.changes.iter().map(|change| &change.evidence_ids))
            .chain(
                self.environments
                    .iter()
                    .map(|environment| &environment.evidence_ids),
            )
        {
            if ids.iter().any(|id| !evidence_ids.contains(id.as_str())) {
                return Err(OperationsSnapshotError::Validation(
                    "projection references unknown evidence".into(),
                ));
            }
        }

        let queue_ids: BTreeSet<_> = self
            .incident_queue
            .iter()
            .map(|item| item.id.as_str())
            .collect();
        if queue_ids.len() != self.incident_queue.len()
            || self
                .incident_queue
                .iter()
                .any(|item| item.id.trim().is_empty())
        {
            return Err(OperationsSnapshotError::Validation(
                "queue IDs must be unique and non-empty".into(),
            ));
        }

        let change_ids: BTreeSet<_> = self
            .changes
            .iter()
            .map(|change| change.id.as_str())
            .collect();
        if change_ids.len() != self.changes.len()
            || self
                .changes
                .iter()
                .any(|change| change.id.trim().is_empty())
        {
            return Err(OperationsSnapshotError::Validation(
                "change IDs must be unique and non-empty".into(),
            ));
        }

        let widget_ids: BTreeSet<_> = self
            .widget_registry
            .iter()
            .map(|widget| widget.id)
            .collect();
        if widget_ids.len() != self.widget_registry.len() {
            return Err(OperationsSnapshotError::Validation(
                "widget IDs must be unique".into(),
            ));
        }
        if self.change_streams_status_is_invalid() {
            return Err(OperationsSnapshotError::Validation(
                "change stream status is invalid".into(),
            ));
        }
        Ok(())
    }

    fn change_streams_status_is_invalid(&self) -> bool {
        match self.change_stream_status.state {
            ChangeStreamState::Available => self.change_stream_status.reason.is_some(),
            ChangeStreamState::Empty | ChangeStreamState::Unavailable => {
                self.change_stream_status.reason.is_none()
            }
        }
    }
}
