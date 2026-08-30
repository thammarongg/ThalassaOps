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

/// Canonical, source-preserving envelope for every normalized signal.
///
/// The typed fields are deliberately small.  Source-specific facts remain in
/// the retained source record addressed by [`SourceRecordRef`], which means a
/// normalization pass never has to replace the originating record with a
/// lossy projection.
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

impl Signal {
    /// Validate the signal before it crosses a serialization or IPC boundary.
    pub fn validate(&self) -> Result<(), CorrelationError> {
        if self.id.is_nil() {
            return Err(CorrelationError::InvalidId);
        }
        validate_optional_timestamp(self.observed_at.as_deref())?;
        validate_optional_timestamp(self.ingested_at.as_deref())?;
        self.source_record.validate()?;
        if self.source_record.source_kind != self.source {
            return Err(CorrelationError::SourceMismatch);
        }
        validate_targets(&self.targets)?;
        validate_correlation_evidence_ids(&self.evidence_ids)?;
        if !contains_all(&self.evidence_ids, &self.source_record.evidence_ids) {
            return Err(CorrelationError::EvidenceMissing);
        }
        self.suppression.validate()?;
        validate_evidence_drill_down_for_correlation(
            &self.drill_down,
            &self.drill_down_reference,
            &self.evidence_ids,
        )?;
        if !self.scope.contains(&self.drill_down_reference.scope) {
            return Err(CorrelationError::ScopeMismatch);
        }
        if let Some(key) = &self.dedup_key {
            validate_safe_identifier(key)?;
        }

        self.payload.validate_for(self.kind, self.source)?;
        if let SignalPayload::SecurityFinding { finding } = &self.payload {
            if !contains_all(&self.targets, std::slice::from_ref(&finding.asset.target)) {
                return Err(CorrelationError::TargetMismatch);
            }
            if !contains_all(&self.evidence_ids, &finding.evidence_ids) {
                return Err(CorrelationError::EvidenceMissing);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum IncidentSeverity {
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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IncidentStatus {
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
    #[serde(rename = "resolved")]
    Resolved,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "reopened")]
    Reopened,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IncidentDisposition {
    #[serde(rename = "duplicate")]
    Duplicate,
    #[serde(rename = "false_positive")]
    FalsePositive,
    #[serde(rename = "suppressed")]
    Suppressed,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "informational")]
    Informational,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Incident {
    pub id: IncidentId,
    pub summary: String,
    pub scope: ResourceScope,
    pub owning_team_id: TeamId,
    pub business_impact: BusinessImpact,
    pub derived_severity: IncidentSeverity,
    pub severity_override: Option<IncidentSeverityOverride>,
    pub status: IncidentStatus,
    pub disposition: Option<IncidentDisposition>,
    pub duplicate_of_incident_id: Option<IncidentId>,
    pub trigger_ids: Vec<IncidentTriggerId>,
    pub signal_ids: Vec<SignalId>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub hypothesis_ids: Vec<HypothesisId>,
    pub action_ids: Vec<ActionId>,
    pub roles: Vec<IncidentRoleAssignment>,
    pub version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl Incident {
    /// Builds an empty-lifecycle incident whose initial severity derives from
    /// the supplied business impact assessment.
    pub fn new(
        summary: impl Into<String>,
        owning_team_id: TeamId,
        scope: ResourceScope,
        business_impact: BusinessImpact,
        now: DateTime<Utc>,
    ) -> Result<Self, IncidentError> {
        let summary = summary.into();
        validate_incident_text(&summary, INCIDENT_SUMMARY_MAXIMUM)?;
        let derived_severity = business_impact.derive_severity()?;
        Ok(Self {
            id: Uuid::new_v4(),
            summary,
            scope,
            owning_team_id,
            business_impact,
            derived_severity,
            severity_override: None,
            status: IncidentStatus::Detected,
            disposition: None,
            duplicate_of_incident_id: None,
            trigger_ids: Vec::new(),
            signal_ids: Vec::new(),
            evidence_ids: Vec::new(),
            hypothesis_ids: Vec::new(),
            action_ids: Vec::new(),
            roles: Vec::new(),
            version: 1,
            created_at: now,
            updated_at: now,
        })
    }
}

/// The six explicit reasons an incident may be created.
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

/// Server-assigned identity of one incident trigger.
pub type IncidentTriggerId = Uuid;

/// Why an incident was created: source identity, scope and evidence references.
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

/// Structured bounded input behind user and manual report triggers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentReport {
    pub reporter_id: Option<PrincipalId>,
    pub summary: String,
}

/// A responder's explicit severity decision that departs from the derived value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentSeverityOverride {
    pub derived: IncidentSeverity,
    pub selected: IncidentSeverity,
    pub actor_id: PrincipalId,
    pub reason: String,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

/// Responder responsibility held by one principal for one incident.
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

/// One active responder-role assignment on an incident.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IncidentRoleAssignment {
    pub role: IncidentRole,
    pub principal_id: PrincipalId,
    pub assigned_by: PrincipalId,
    pub assigned_at: DateTime<Utc>,
}

/// Maximum characters for an incident summary or report.
pub const INCIDENT_SUMMARY_MAXIMUM: usize = 200;
/// Maximum characters for incident notes, reasons and transition context.
pub const INCIDENT_NOTE_MAXIMUM: usize = 4_000;
/// Maximum characters for incident source identifiers and record digests.
pub const INCIDENT_SOURCE_ID_MAXIMUM: usize = 200;
/// Maximum characters for business-impact summary, scope and criticality text.
pub const IMPACT_SUMMARY_MAXIMUM: usize = 1_000;

/// Typed validation failures for incident contracts and inputs.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IncidentError {
    #[error("incident text is empty, contains control characters or sensitive markers")]
    UnsafeText,
    #[error("incident text exceeds its maximum length of {maximum} characters")]
    TextTooLong { maximum: usize },
    #[error("business impact level and trajectory do not match the impact dimensions")]
    ImpactLevelMismatch,
    #[error("incident evidence references are missing, duplicated or unsafe")]
    InvalidEvidence,
    #[error("incident identifier is nil or duplicated")]
    InvalidId,
}

/// Rejects empty, control-bearing, sensitive or oversized incident text.
pub fn validate_incident_text(value: &str, maximum: usize) -> Result<(), IncidentError> {
    if value.trim().is_empty() {
        return Err(IncidentError::UnsafeText);
    }
    if value.chars().count() > maximum {
        return Err(IncidentError::TextTooLong { maximum });
    }
    if value.chars().any(|character| character.is_control()) || contains_sensitive_marker(value) {
        return Err(IncidentError::UnsafeText);
    }
    Ok(())
}

impl IncidentTrigger {
    /// Validates bounded provenance: safe source identity and evidence references.
    pub fn validate(&self) -> Result<(), IncidentError> {
        if self.id.is_nil() {
            return Err(IncidentError::InvalidId);
        }
        validate_incident_text(&self.source_id, INCIDENT_SOURCE_ID_MAXIMUM)?;
        if let Some(digest) = &self.source_record_digest {
            validate_incident_text(digest, INCIDENT_SOURCE_ID_MAXIMUM)?;
        }
        validate_incident_evidence_ids(&self.evidence_ids)
    }
}

impl IncidentReport {
    /// Validates the bounded report summary.
    pub fn validate(&self) -> Result<(), IncidentError> {
        validate_incident_text(&self.summary, INCIDENT_SUMMARY_MAXIMUM)
    }
}

impl IncidentSeverityOverride {
    /// Validates the bounded override reason and evidence references.
    pub fn validate(&self) -> Result<(), IncidentError> {
        validate_incident_text(&self.reason, INCIDENT_NOTE_MAXIMUM)?;
        validate_incident_evidence_ids(&self.evidence_ids)
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
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
    #[serde(rename = "trivy")]
    Trivy,
    #[serde(rename = "falco")]
    Falco,
    #[serde(rename = "kyverno")]
    Kyverno,
    #[serde(rename = "opa_gatekeeper")]
    OpaGatekeeper,
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "gitlab")]
    GitLab,
    #[serde(rename = "argo_cd")]
    ArgoCd,
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
    #[serde(rename = "topology")]
    Topology,
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

/// Structured business-impact ratings across the accepted impact dimensions.
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

impl ImpactDimensions {
    /// Highest confirmed impact dimension. Unknown contributes no confirmed
    /// impact, and the ranking is explicit because enum declaration order is
    /// not the business ranking.
    pub fn highest_level(&self) -> ImpactLevel {
        [
            self.availability,
            self.customer_reach,
            self.business_criticality,
            self.data_integrity,
            self.security_privacy,
            self.financial_contractual,
        ]
        .into_iter()
        .max_by_key(|level| impact_business_rank(*level))
        .expect("dimensions are non-empty")
    }

    /// Dimensions attributing all confirmed impact to one primary dimension,
    /// as used by compact Operations Console projections. An Unknown primary
    /// means the whole assessment is unconfirmed, so every dimension stays
    /// Unknown instead of asserting a confirmed None.
    pub fn single_dimension(primary: ImpactLevel, trajectory: ImpactTrajectory) -> Self {
        let unconfirmed = if primary == ImpactLevel::Unknown {
            ImpactLevel::Unknown
        } else {
            ImpactLevel::None
        };
        Self {
            availability: primary,
            customer_reach: unconfirmed,
            business_criticality: unconfirmed,
            data_integrity: unconfirmed,
            security_privacy: unconfirmed,
            financial_contractual: unconfirmed,
            trajectory,
            production: true,
        }
    }
}

/// Explicit business ranking: Critical outranks High through None; Unknown
/// confirms no impact.
fn impact_business_rank(level: ImpactLevel) -> u8 {
    match level {
        ImpactLevel::Critical => 5,
        ImpactLevel::High => 4,
        ImpactLevel::Medium => 3,
        ImpactLevel::Low => 2,
        ImpactLevel::None => 1,
        ImpactLevel::Unknown => 0,
    }
}

/// Explicit severity ranking where S1 is most severe; derived `Ord` is not the
/// business ranking.
fn severity_rank(severity: &IncidentSeverity) -> u8 {
    match severity {
        IncidentSeverity::S1 => 5,
        IncidentSeverity::S2 => 4,
        IncidentSeverity::S3 => 3,
        IncidentSeverity::S4 => 2,
        IncidentSeverity::S5 => 1,
    }
}

/// A compact, evidence-backed description of business impact.
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

impl BusinessImpact {
    /// Validates bounded safe text, consistency with the impact dimensions and
    /// evidence references.
    pub fn validate(&self) -> Result<(), IncidentError> {
        if self.level != self.dimensions.highest_level()
            || self.trajectory != self.dimensions.trajectory
        {
            return Err(IncidentError::ImpactLevelMismatch);
        }
        for text in [
            &self.summary,
            &self.customer_scope,
            &self.service_criticality,
        ] {
            validate_incident_text(text, IMPACT_SUMMARY_MAXIMUM)?;
        }
        if self.evidence_ids.is_empty() {
            return Err(IncidentError::InvalidEvidence);
        }
        validate_incident_evidence_ids(&self.evidence_ids)
    }

    /// Derives the initial severity from the highest confirmed impact
    /// dimension, enforcing the accepted safety floors.
    pub fn derive_severity(&self) -> Result<IncidentSeverity, IncidentError> {
        self.validate()?;
        let severity = match self.dimensions.highest_level() {
            ImpactLevel::Critical => IncidentSeverity::S1,
            ImpactLevel::High => IncidentSeverity::S2,
            ImpactLevel::Medium => IncidentSeverity::S3,
            ImpactLevel::Low => IncidentSeverity::S4,
            ImpactLevel::None | ImpactLevel::Unknown => IncidentSeverity::S5,
        };
        if self.dimensions.production
            && self.dimensions.trajectory == ImpactTrajectory::Expanding
            && self.dimensions.customer_reach == ImpactLevel::Unknown
            && severity_rank(&severity) < severity_rank(&IncidentSeverity::S2)
        {
            return Ok(IncidentSeverity::S2);
        }
        Ok(severity)
    }
}

/// Validates that every evidence reference is a safe, bounded, unique identifier.
fn validate_incident_evidence_ids(ids: &[ConsoleEvidenceId]) -> Result<(), IncidentError> {
    if ids.iter().any(|id| {
        validate_incident_text(id, INCIDENT_SOURCE_ID_MAXIMUM).is_err()
            || id.chars().any(char::is_whitespace)
    }) || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(IncidentError::InvalidEvidence);
    }
    Ok(())
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
    #[serde(rename = "code_commit")]
    CodeCommit,
    #[serde(rename = "code_merge")]
    CodeMerge,
    #[serde(rename = "sync")]
    Sync,
    #[serde(rename = "rollback")]
    Rollback,
}

/// Evidence-backed change shown in the recent change stream.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeStreamItem {
    pub id: String,
    pub source: EvidenceSourceKind,
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

/// Stable identifier for a normalized source-backed change.
pub type ChangeEventId = Uuid;

/// Outcome reported by a change source.
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

/// Safe category for the actor reported by a change source.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ChangeActorKind {
    #[serde(rename = "human")]
    Human,
    #[serde(rename = "automation")]
    Automation,
    #[serde(rename = "unknown")]
    Unknown,
}

/// Source-scoped actor handle.  Display names and email addresses are not
/// retained in this contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeActor {
    pub kind: ChangeActorKind,
    pub handle: Option<String>,
}

impl ChangeActor {
    pub fn validate(&self) -> Result<(), ChangeError> {
        if self.kind == ChangeActorKind::Unknown && self.handle.is_some() {
            return Err(ChangeError::InvalidActor);
        }
        if let Some(handle) = &self.handle {
            validate_safe_identifier(handle).map_err(|_| ChangeError::UnsafeIdentity)?;
            if looks_like_email(handle) {
                return Err(ChangeError::UnsafeIdentity);
            }
        }
        Ok(())
    }
}

/// Source revision identifiers associated with a change.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRevision {
    pub id: String,
    pub short_id: Option<String>,
    pub parent_ids: Vec<String>,
}

impl ChangeRevision {
    fn validate(&self) -> Result<(), ChangeError> {
        validate_safe_identifier(&self.id).map_err(|_| ChangeError::InvalidRevision)?;
        if let Some(short_id) = &self.short_id {
            validate_safe_identifier(short_id).map_err(|_| ChangeError::InvalidRevision)?;
        }
        if self.parent_ids.iter().any(|id| {
            validate_safe_identifier(id).is_err()
                || self
                    .parent_ids
                    .iter()
                    .filter(|candidate| *candidate == id)
                    .count()
                    > 1
        }) {
            return Err(ChangeError::InvalidRevision);
        }
        Ok(())
    }
}

/// Typed repository identity.  A repository is not represented as a URL.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRepositoryRef {
    pub host: String,
    pub namespace: String,
    pub name: String,
    pub reference: Option<String>,
}

impl ChangeRepositoryRef {
    fn validate(&self) -> Result<(), ChangeError> {
        for value in [&self.host, &self.namespace, &self.name] {
            validate_safe_identifier(value).map_err(|_| ChangeError::InvalidRepository)?;
        }
        if let Some(reference) = &self.reference {
            validate_safe_identifier(reference).map_err(|_| ChangeError::InvalidRepository)?;
        }
        Ok(())
    }
}

/// Finite, non-negative changed-file statistics.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeDiffStat {
    pub files_changed: f64,
    pub insertions: f64,
    pub deletions: f64,
    pub unit: NumberUnit,
}

impl ChangeDiffStat {
    pub fn validate(&self) -> Result<(), ChangeError> {
        if self.unit != NumberUnit::Count {
            return Err(ChangeError::InvalidUnit);
        }
        for value in [self.files_changed, self.insertions, self.deletions] {
            if !value.is_finite() {
                return Err(ChangeError::NonFiniteNumber);
            }
            if value < 0.0 {
                return Err(ChangeError::NegativeNumber);
            }
        }
        Ok(())
    }
}

/// Native source link categories supported by change providers.
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

/// A validated native source URL.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeSourceLink {
    pub kind: ChangeLinkKind,
    pub url: String,
}

impl ChangeSourceLink {
    pub fn validate(&self, source: EvidenceSourceKind) -> Result<(), ChangeError> {
        let host = parse_https_host(&self.url).ok_or(ChangeError::InvalidLink)?;
        let host_is_allowed = match source {
            EvidenceSourceKind::GitHub => host == "github.com",
            EvidenceSourceKind::GitLab => host == "gitlab.com",
            EvidenceSourceKind::ArgoCd => !host.is_empty(),
            _ => false,
        };
        if host_is_allowed {
            Ok(())
        } else {
            Err(ChangeError::InvalidLink)
        }
    }
}

/// Canonical, source-preserving normalized change record.
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

impl ChangeEvent {
    pub fn validate(&self) -> Result<(), ChangeError> {
        if self.id.is_nil() {
            return Err(ChangeError::InvalidId);
        }
        parse_change_timestamp(&self.occurred_at)?;
        if let Some(ingested_at) = &self.ingested_at {
            parse_change_timestamp(ingested_at)?;
        }
        validate_targets(&self.targets).map_err(|_| ChangeError::InvalidTarget)?;
        self.actor.validate()?;
        if let Some(revision) = &self.revision {
            revision.validate()?;
        }
        if let Some(repository) = &self.repository {
            repository.validate()?;
        }
        if let Some(environment) = &self.environment {
            validate_safe_identifier(environment).map_err(|_| ChangeError::InvalidId)?;
        }
        if let Some(diff_stat) = &self.diff_stat {
            diff_stat.validate()?;
        }
        for path in &self.changed_paths {
            validate_change_path(path)?;
        }
        if let Some(link) = &self.source_link {
            link.validate(self.source)?;
        }
        self.source_record
            .validate()
            .map_err(|_| ChangeError::InvalidSourceRecord)?;
        if self.source_record.source_kind != self.source {
            return Err(ChangeError::SourceMismatch);
        }
        validate_change_evidence_ids(&self.evidence_ids)?;
        if !contains_all(&self.evidence_ids, &self.source_record.evidence_ids) {
            return Err(ChangeError::EvidenceMissing);
        }
        validate_change_drill_down(
            &self.drill_down,
            &self.drill_down_reference,
            &self.evidence_ids,
            &self.scope,
        )?;
        Ok(())
    }
}

/// Bounded, deterministic ordering of change event IDs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeTimeline {
    pub window: TimeWindow,
    pub entry_ids: Vec<ChangeEventId>,
    pub truncated: bool,
}

impl ChangeTimeline {
    fn validate(&self) -> Result<(), ChangeError> {
        self.window
            .validate()
            .map_err(|_| ChangeError::InvalidWindow)?;
        if self.entry_ids.iter().any(Uuid::is_nil)
            || self.entry_ids.iter().collect::<BTreeSet<_>>().len() != self.entry_ids.len()
        {
            return Err(ChangeError::DuplicateId);
        }
        Ok(())
    }
}

/// Structural context connecting a change to a correlation candidate.
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

impl ChangeAssociation {
    fn validate(&self) -> Result<(), ChangeError> {
        if self.change_id.is_nil() {
            return Err(ChangeError::InvalidId);
        }
        validate_safe_identifier(&self.candidate_id).map_err(|_| ChangeError::InvalidId)?;
        if self.qualification != CorrelationQualification::ProbableStructural {
            return Err(ChangeError::InvalidAssociation);
        }
        if !self.lead_time_seconds.is_finite() {
            return Err(ChangeError::NonFiniteNumber);
        }
        if self.lead_time_seconds < 0.0 {
            return Err(ChangeError::NegativeNumber);
        }
        if let Some(target) = &self.target {
            target.validate().map_err(|_| ChangeError::InvalidTarget)?;
        }
        validate_sorted_change_identifiers(&self.topology_path_ids)?;
        validate_change_evidence_ids(&self.evidence_ids)
    }
}

/// Metric keys emitted by a change snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ChangeMetricKey {
    #[serde(rename = "changes_in_window")]
    ChangesInWindow,
    #[serde(rename = "associated_changes")]
    AssociatedChanges,
    #[serde(rename = "changes_by_source")]
    ChangesBySource,
}

/// Finite, evidence-backed change metric.
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

impl ChangeMetric {
    fn validate(&self, scope: &ResourceScope) -> Result<(), ChangeError> {
        if !self.value.is_finite() {
            return Err(ChangeError::NonFiniteNumber);
        }
        if self.value < 0.0 {
            return Err(ChangeError::NegativeNumber);
        }
        if self.unit != NumberUnit::Count {
            return Err(ChangeError::InvalidUnit);
        }
        if (self.key == ChangeMetricKey::ChangesBySource) != self.source.is_some() {
            return Err(ChangeError::InvalidMetric);
        }
        validate_change_drill_down(
            &self.drill_down,
            &self.drill_down_reference,
            &self.evidence_ids,
            scope,
        )
    }
}

/// Request for one explicit, deterministic change snapshot evaluation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeRequest {
    pub window: TimeWindow,
    pub evaluated_at: String,
    pub lookback_seconds: u64,
    pub limit: u64,
}

impl ChangeRequest {
    pub fn validate(&self) -> Result<(), ChangeError> {
        self.window
            .validate()
            .map_err(|_| ChangeError::InvalidWindow)?;
        let evaluated_at = parse_change_timestamp(&self.evaluated_at)?;
        let start = parse_change_timestamp(&self.window.start)?;
        if evaluated_at < start {
            return Err(ChangeError::InvalidWindow);
        }
        validate_change_lookback(self.lookback_seconds)?;
        if self.limit == 0 || self.limit > MAX_CHANGE_LIMIT {
            return Err(ChangeError::InvalidLimit);
        }
        Ok(())
    }
}

/// Request for evidence IDs already admitted to a change snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ChangeEvidenceRequest {
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

impl ChangeEvidenceRequest {
    pub fn validate(&self) -> Result<(), ChangeError> {
        validate_change_evidence_ids(&self.evidence_ids)
    }
}

/// Complete deterministic read-only change projection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ChangeSnapshot {
    pub generated_at: String,
    pub scope: ResourceScope,
    pub request_window: TimeWindow,
    pub lookback_seconds: u64,
    pub events: Vec<ChangeEvent>,
    pub timeline: ChangeTimeline,
    pub associations: Vec<ChangeAssociation>,
    pub metrics: Vec<ChangeMetric>,
    pub source_statuses: Vec<SourceStatus>,
}

impl ChangeSnapshot {
    pub fn validate(&self) -> Result<(), ChangeError> {
        parse_change_timestamp(&self.generated_at)?;
        self.request_window
            .validate()
            .map_err(|_| ChangeError::InvalidWindow)?;
        validate_change_lookback(self.lookback_seconds)?;
        self.timeline.validate()?;
        if self.timeline.window != self.request_window {
            return Err(ChangeError::WindowMismatch);
        }

        let mut event_ids = BTreeSet::new();
        for event in &self.events {
            event.validate()?;
            if !event_ids.insert(event.id) {
                return Err(ChangeError::DuplicateId);
            }
            if !self.scope.contains(&event.scope) {
                return Err(ChangeError::ScopeMismatch);
            }
        }
        for entry_id in &self.timeline.entry_ids {
            let event = self
                .events
                .iter()
                .find(|event| event.id == *entry_id)
                .ok_or(ChangeError::CandidateReferenceMissing)?;
            let occurred_at = parse_change_timestamp(&event.occurred_at)?;
            let start = parse_change_timestamp(&self.timeline.window.start)?;
            let end = parse_change_timestamp(&self.timeline.window.end)?;
            if occurred_at < start || occurred_at >= end {
                return Err(ChangeError::InvalidWindow);
            }
        }
        for pair in self.timeline.entry_ids.windows(2) {
            let left = self
                .events
                .iter()
                .find(|event| event.id == pair[0])
                .ok_or(ChangeError::CandidateReferenceMissing)?;
            let right = self
                .events
                .iter()
                .find(|event| event.id == pair[1])
                .ok_or(ChangeError::CandidateReferenceMissing)?;
            let left_key = (parse_change_timestamp(&left.occurred_at)?, left.id);
            let right_key = (parse_change_timestamp(&right.occurred_at)?, right.id);
            if left_key > right_key {
                return Err(ChangeError::InvalidTimeline);
            }
        }

        let mut association_ids = BTreeSet::new();
        for association in &self.associations {
            association.validate()?;
            if !association_ids.insert((association.candidate_id.as_str(), association.change_id)) {
                return Err(ChangeError::DuplicateId);
            }
            if !event_ids.contains(&association.change_id) {
                return Err(ChangeError::CandidateReferenceMissing);
            }
        }

        let mut metric_keys = BTreeSet::new();
        for metric in &self.metrics {
            metric.validate(&self.scope)?;
            let metric_identity = (metric.key, metric.source);
            if !metric_keys.insert(metric_identity) {
                return Err(ChangeError::DuplicateId);
            }
        }
        let mut status_keys = BTreeSet::new();
        for status in &self.source_statuses {
            validate_safe_identifier(&status.source_key).map_err(|_| ChangeError::InvalidId)?;
            if !status_keys.insert(status.source_key.as_str()) {
                return Err(ChangeError::DuplicateId);
            }
            if let Some(observed_at) = &status.observed_at {
                parse_change_timestamp(observed_at)?;
            }
            validate_change_evidence_ids_allow_empty(&status.evidence_ids)?;
        }
        self.validate_evidence_closure()
    }

    /// Verify that all evidence references resolve through the event records
    /// included in this snapshot.
    pub fn validate_evidence_closure(&self) -> Result<(), ChangeError> {
        let known_ids = self
            .events
            .iter()
            .flat_map(|event| event.source_record.evidence_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        for event in &self.events {
            ensure_change_known_evidence(&event.evidence_ids, &known_ids)?;
            ensure_change_known_evidence(&event.source_record.evidence_ids, &known_ids)?;
            ensure_change_known_evidence(&event.drill_down.evidence_ids, &known_ids)?;
            ensure_change_known_evidence(&event.drill_down_reference.evidence_ids, &known_ids)?;
        }
        for association in &self.associations {
            ensure_change_known_evidence(&association.evidence_ids, &known_ids)?;
        }
        for metric in &self.metrics {
            ensure_change_known_evidence(&metric.evidence_ids, &known_ids)?;
            ensure_change_known_evidence(&metric.drill_down.evidence_ids, &known_ids)?;
            ensure_change_known_evidence(&metric.drill_down_reference.evidence_ids, &known_ids)?;
        }
        for status in &self.source_statuses {
            ensure_change_known_evidence(&status.evidence_ids, &known_ids)?;
        }
        Ok(())
    }
}

/// Typed validation failures for change contracts and replay inputs.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ChangeError {
    #[error("change identifier is empty, unsafe or duplicated")]
    InvalidId,
    #[error("change timestamp is invalid")]
    InvalidTimestamp,
    #[error("change timestamp is missing")]
    MissingTimestamp,
    #[error("change window is invalid")]
    InvalidWindow,
    #[error("change window and timeline do not agree")]
    WindowMismatch,
    #[error("change lookback is outside 0.0..=86400.0")]
    InvalidLookback,
    #[error("change limit is outside the permitted range")]
    InvalidLimit,
    #[error("change number is not finite")]
    NonFiniteNumber,
    #[error("change number cannot be negative")]
    NegativeNumber,
    #[error("change metric or diff statistic uses an invalid unit")]
    InvalidUnit,
    #[error("change source link is invalid")]
    InvalidLink,
    #[error("change actor is invalid")]
    InvalidActor,
    #[error("change actor identity is unsafe")]
    UnsafeIdentity,
    #[error("change revision is invalid")]
    InvalidRevision,
    #[error("change repository is invalid")]
    InvalidRepository,
    #[error("change path is invalid")]
    InvalidPath,
    #[error("change target is invalid")]
    InvalidTarget,
    #[error("change source record is invalid")]
    InvalidSourceRecord,
    #[error("change source does not match its source record")]
    SourceMismatch,
    #[error("change evidence is missing or invalid")]
    EvidenceMissing,
    #[error("change evidence is invalid")]
    InvalidEvidence,
    #[error("change scope does not contain a child value")]
    ScopeMismatch,
    #[error("change association is invalid")]
    InvalidAssociation,
    #[error("change metric is invalid")]
    InvalidMetric,
    #[error("change timeline is invalid")]
    InvalidTimeline,
    #[error("change reference is missing")]
    CandidateReferenceMissing,
    #[error("change identifier is duplicated")]
    DuplicateId,
    #[error("change request payload is malformed")]
    MalformedPayload,
    #[error("change source status is invalid")]
    InvalidSourceStatus,
    #[error("change policy denied the source record")]
    PolicyDenied,
}

pub const MAX_CHANGE_LOOKBACK_SECONDS: u64 = 86_400;
pub const DEFAULT_CHANGE_LOOKBACK_SECONDS: u64 = 3_600;
pub const MAX_CHANGE_LIMIT: u64 = 1_000;

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

/// Typed kind of a normalized signal.
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

/// Lifecycle state of a normalized signal.
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

/// Exact target categories that can participate in correlation grouping.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

/// A safe, canonical target identity carried by a Signal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SignalTarget {
    pub kind: SignalTargetKind,
    pub id: String,
}

impl SignalTarget {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_safe_identifier(&self.id)
    }
}

/// Reference to the complete post-policy source record retained by the core.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceRecordRef {
    pub source_kind: EvidenceSourceKind,
    pub native_id: Option<String>,
    pub revision: Option<String>,
    pub content_digest: String,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

impl SourceRecordRef {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_safe_identifier(&self.content_digest)?;
        if let Some(native_id) = &self.native_id {
            validate_safe_identifier(native_id)?;
        }
        if let Some(revision) = &self.revision {
            validate_safe_identifier(revision)?;
        }
        validate_correlation_evidence_ids(&self.evidence_ids)
    }
}

/// Provider-neutral payload for each Signal kind.
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

impl SignalPayload {
    pub fn kind(&self) -> SignalKind {
        match self {
            Self::Alert => SignalKind::Alert,
            Self::Anomaly { .. } => SignalKind::Anomaly,
            Self::SecurityFinding { .. } => SignalKind::SecurityFinding,
            Self::HealthCheck { .. } => SignalKind::HealthCheck,
        }
    }

    pub fn validate_for(
        &self,
        expected_kind: SignalKind,
        source: EvidenceSourceKind,
    ) -> Result<(), CorrelationError> {
        if self.kind() != expected_kind {
            return Err(CorrelationError::PayloadKindMismatch);
        }
        match self {
            Self::Alert => Ok(()),
            Self::Anomaly {
                observed_value,
                comparison_value,
                condition,
            } => {
                validate_finite(*observed_value, CorrelationNumberField::ObservedValue)?;
                validate_finite(*comparison_value, CorrelationNumberField::ComparisonValue)?;
                condition
                    .validate()
                    .map_err(|_| CorrelationError::InvalidPayload)
            }
            Self::SecurityFinding { finding } => {
                finding.validate()?;
                if finding.source != source {
                    return Err(CorrelationError::SourceMismatch);
                }
                Ok(())
            }
            Self::HealthCheck { .. } => Ok(()),
        }
    }
}

/// Asset categories used by a normalized vulnerability/security finding.
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

/// Safe target and optional source-provided display/artifact metadata.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FindingAsset {
    pub kind: FindingAssetKind,
    pub target: SignalTarget,
    pub display_name: Option<String>,
    pub artifact_digest: Option<String>,
}

impl FindingAsset {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        self.target.validate()?;
        if let Some(display_name) = &self.display_name {
            validate_safe_text(display_name)?;
        }
        if let Some(artifact_digest) = &self.artifact_digest {
            validate_safe_identifier(artifact_digest)?;
        }
        Ok(())
    }
}

/// Source severity of a vulnerability or security finding.
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

/// Source-provided exploitability classification.
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

/// Provider-neutral vulnerability/security finding nested in a Signal.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VulnerabilityFinding {
    pub source: EvidenceSourceKind,
    pub asset: FindingAsset,
    pub severity: Option<FindingSeverity>,
    pub exploitability: Option<Exploitability>,
    pub cvss_score: Option<f64>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

impl VulnerabilityFinding {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        if !self.source.is_security_source() {
            return Err(CorrelationError::UnsupportedFindingSource);
        }
        self.asset.validate()?;
        if let Some(cvss_score) = self.cvss_score {
            validate_finite(cvss_score, CorrelationNumberField::CvssScore)?;
            if !(0.0..=10.0).contains(&cvss_score) {
                return Err(CorrelationError::CvssOutOfRange);
            }
        }
        validate_correlation_evidence_ids(&self.evidence_ids)
    }
}

impl EvidenceSourceKind {
    /// Whether this source is one of the initial security finding adapters.
    pub fn is_security_source(self) -> bool {
        matches!(
            self,
            Self::Trivy | Self::Falco | Self::Kyverno | Self::OpaGatekeeper
        )
    }
}

/// Explicit request for one deterministic event-time correlation evaluation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationRequest {
    pub window: TimeWindow,
    pub evaluated_at: String,
    pub allowed_lateness_seconds: u64,
}

impl CorrelationRequest {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        self.window.validate()?;
        if self.allowed_lateness_seconds > MAX_CORRELATION_LATENESS_SECONDS {
            return Err(CorrelationError::LatenessOutOfRange);
        }
        let start = parse_correlation_timestamp(&self.window.start)?;
        let evaluated_at = parse_correlation_timestamp(&self.evaluated_at)?;
        if evaluated_at < start {
            return Err(CorrelationError::InvalidWindow);
        }
        Ok(())
    }
}

/// Lifecycle state of an event-time correlation window.
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

/// Explicit event-time window and watermark used for one snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationWindow {
    pub range: TimeWindow,
    pub evaluated_at: String,
    pub watermark: String,
    pub allowed_lateness_seconds: u64,
    pub state: CorrelationWindowState,
}

impl CorrelationWindow {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        self.range.validate()?;
        if self.allowed_lateness_seconds > MAX_CORRELATION_LATENESS_SECONDS {
            return Err(CorrelationError::LatenessOutOfRange);
        }
        let start = parse_correlation_timestamp(&self.range.start)?;
        let end = parse_correlation_timestamp(&self.range.end)?;
        let evaluated_at = parse_correlation_timestamp(&self.evaluated_at)?;
        let watermark = parse_correlation_timestamp(&self.watermark)?;
        let expected_watermark = evaluated_at
            .checked_sub_signed(chrono::Duration::seconds(
                self.allowed_lateness_seconds as i64,
            ))
            .ok_or(CorrelationError::InvalidWindow)?;
        if watermark != expected_watermark {
            return Err(CorrelationError::InvalidWindow);
        }
        let finalization_at = end
            .checked_add_signed(chrono::Duration::seconds(
                self.allowed_lateness_seconds as i64,
            ))
            .ok_or(CorrelationError::InvalidWindow)?;
        let expected_state = if evaluated_at < end {
            CorrelationWindowState::Open
        } else if evaluated_at < finalization_at {
            CorrelationWindowState::ReadyToFinalize
        } else {
            CorrelationWindowState::Finalized
        };
        if self.state == CorrelationWindowState::Reopened {
            if evaluated_at < finalization_at {
                return Err(CorrelationError::InvalidWindow);
            }
        } else if self.state != expected_state {
            return Err(CorrelationError::InvalidWindow);
        }
        if evaluated_at < start {
            return Err(CorrelationError::InvalidWindow);
        }
        Ok(())
    }
}

/// Explainable structural association categories used by correlation.
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
    #[serde(rename = "preceding_change")]
    PrecedingChange,
}

/// Confidence vocabulary for an association, deliberately excluding causality.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CorrelationQualification {
    #[serde(rename = "exact_association")]
    ExactAssociation,
    #[serde(rename = "probable_structural")]
    ProbableStructural,
}

/// Evidence-backed reason for connecting a set of Signals.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationReason {
    pub kind: CorrelationReasonKind,
    pub qualification: CorrelationQualification,
    pub signal_ids: Vec<SignalId>,
    pub target: Option<SignalTarget>,
    pub topology_path_ids: Vec<String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

impl CorrelationReason {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_signal_ids(&self.signal_ids, true)?;
        validate_correlation_evidence_ids(&self.evidence_ids)?;
        if let Some(target) = &self.target {
            target.validate()?;
        }
        match self.kind {
            CorrelationReasonKind::SharedResource
            | CorrelationReasonKind::SharedService
            | CorrelationReasonKind::SharedDeployment => {
                let target = self
                    .target
                    .as_ref()
                    .ok_or(CorrelationError::InvalidReason)?;
                let expected_target_kind = match self.kind {
                    CorrelationReasonKind::SharedResource => SignalTargetKind::Resource,
                    CorrelationReasonKind::SharedService => SignalTargetKind::Service,
                    CorrelationReasonKind::SharedDeployment => SignalTargetKind::Deployment,
                    CorrelationReasonKind::TopologyRelation
                    | CorrelationReasonKind::PrecedingChange => unreachable!(),
                };
                if target.kind != expected_target_kind
                    || !self.topology_path_ids.is_empty()
                    || self.qualification != CorrelationQualification::ExactAssociation
                {
                    return Err(CorrelationError::InvalidReason);
                }
            }
            CorrelationReasonKind::TopologyRelation => {
                if self.target.is_some()
                    || self.topology_path_ids.is_empty()
                    || validate_sorted_identifiers(&self.topology_path_ids).is_err()
                    || self.qualification != CorrelationQualification::ProbableStructural
                {
                    return Err(CorrelationError::InvalidReason);
                }
            }
            CorrelationReasonKind::PrecedingChange => {
                if self.qualification != CorrelationQualification::ProbableStructural {
                    return Err(CorrelationError::InvalidReason);
                }
            }
        }
        Ok(())
    }
}

/// Eligibility state of a correlation candidate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CandidateStatus {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "provisional")]
    Provisional,
    #[serde(rename = "suppressed")]
    Suppressed,
}

/// Deterministic, read-only grouping of source-preserving Signals.
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

impl CorrelationCandidate {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_safe_identifier(&self.id)?;
        self.window.validate()?;
        validate_signal_ids(&self.signal_ids, true)?;
        if self.signal_ids.len() < 2 {
            return Err(CorrelationError::CandidateTooSmall);
        }
        validate_signal_ids(&self.late_signal_ids, false)?;
        if !contains_all(&self.signal_ids, &self.late_signal_ids) {
            return Err(CorrelationError::CandidateReferenceMissing);
        }
        validate_targets(&self.grouping_targets)?;
        validate_correlation_evidence_ids(&self.evidence_ids)?;
        if self.reasons.is_empty() {
            return Err(CorrelationError::InvalidReason);
        }
        let mut explained_signal_ids = BTreeSet::new();
        for reason in &self.reasons {
            reason.validate()?;
            if !contains_all(&self.signal_ids, &reason.signal_ids)
                || !contains_all(&self.evidence_ids, &reason.evidence_ids)
            {
                return Err(CorrelationError::CandidateReferenceMissing);
            }
            explained_signal_ids.extend(reason.signal_ids.iter().copied());
        }
        if explained_signal_ids.len() != self.signal_ids.len()
            || self
                .signal_ids
                .iter()
                .any(|signal_id| !explained_signal_ids.contains(signal_id))
        {
            return Err(CorrelationError::InvalidReason);
        }
        validate_evidence_drill_down_for_correlation(
            &self.drill_down,
            &self.drill_down_reference,
            &self.evidence_ids,
        )?;
        if !self.scope.contains(&self.drill_down_reference.scope) {
            return Err(CorrelationError::ScopeMismatch);
        }
        Ok(())
    }
}

/// Metric keys emitted by a correlation snapshot.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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

/// Finite evidence-backed correlation count.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CorrelationMetric {
    pub key: CorrelationMetricKey,
    pub value: f64,
    pub unit: NumberUnit,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}

impl CorrelationMetric {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_finite(self.value, CorrelationNumberField::MetricValue)?;
        if self.value < 0.0 {
            return Err(CorrelationError::MetricValueOutOfRange);
        }
        if self.unit != NumberUnit::Count {
            return Err(CorrelationError::MetricUnitMismatch);
        }
        validate_correlation_evidence_ids(&self.evidence_ids)?;
        validate_evidence_drill_down_for_correlation(
            &self.drill_down,
            &self.drill_down_reference,
            &self.evidence_ids,
        )
    }
}

/// Collection of all emitted correlation metrics.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CorrelationSummary {
    pub metrics: Vec<CorrelationMetric>,
}

impl CorrelationSummary {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        let mut keys = BTreeSet::new();
        for metric in &self.metrics {
            metric.validate()?;
            if !keys.insert(metric.key) {
                return Err(CorrelationError::DuplicateId);
            }
        }
        Ok(())
    }
}

/// Complete deterministic read-only correlation projection.
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

impl CorrelationSnapshot {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        parse_correlation_timestamp(&self.generated_at)?;
        self.request.validate()?;
        self.window.validate()?;
        if self.request.window != self.window.range
            || self.request.evaluated_at != self.window.evaluated_at
            || self.request.allowed_lateness_seconds != self.window.allowed_lateness_seconds
        {
            return Err(CorrelationError::WindowMismatch);
        }
        self.summary.validate()?;

        let evidence_ids = self
            .evidence
            .iter()
            .map(|item| item.id.clone())
            .collect::<BTreeSet<_>>();
        if evidence_ids.len() != self.evidence.len()
            || self
                .evidence
                .iter()
                .any(|item| validate_safe_identifier(&item.id).is_err())
        {
            return Err(CorrelationError::DuplicateId);
        }
        for evidence in &self.evidence {
            if !evidence.redaction.classification_verified
                || !evidence.redaction.redaction_verified
                || (evidence.redaction.unparsed && evidence.redaction.masked)
            {
                return Err(CorrelationError::InvalidEvidence);
            }
            if !self.scope.contains(&evidence.scope) {
                return Err(CorrelationError::ScopeMismatch);
            }
        }

        let mut signal_ids = BTreeSet::new();
        for signal in &self.signals {
            signal.validate()?;
            if !signal_ids.insert(signal.id) {
                return Err(CorrelationError::DuplicateId);
            }
            if !self.scope.contains(&signal.scope) {
                return Err(CorrelationError::ScopeMismatch);
            }
            ensure_known_evidence(&signal.evidence_ids, &evidence_ids)?;
            ensure_known_evidence(&signal.source_record.evidence_ids, &evidence_ids)?;
            ensure_known_evidence(&signal.drill_down.evidence_ids, &evidence_ids)?;
            ensure_known_evidence(&signal.drill_down_reference.evidence_ids, &evidence_ids)?;
            for evidence_id in signal
                .evidence_ids
                .iter()
                .chain(signal.source_record.evidence_ids.iter())
            {
                let evidence = self
                    .evidence
                    .iter()
                    .find(|evidence| evidence.id == *evidence_id)
                    .ok_or(CorrelationError::EvidenceMissing)?;
                if evidence.source_kind != signal.source || !signal.scope.contains(&evidence.scope)
                {
                    return Err(if evidence.source_kind != signal.source {
                        CorrelationError::SourceMismatch
                    } else {
                        CorrelationError::ScopeMismatch
                    });
                }
            }
        }

        let mut path_ids = BTreeSet::new();
        for path in &self.topology_paths {
            path.validate()
                .map_err(|_| CorrelationError::InvalidTopologyPath)?;
            if !path_ids.insert(path.id.clone()) {
                return Err(CorrelationError::DuplicateId);
            }
            ensure_known_evidence(&path.evidence_ids, &evidence_ids)?;
            ensure_known_evidence(&path.drill_down.evidence_ids, &evidence_ids)?;
        }

        let mut candidate_ids = BTreeSet::new();
        for candidate in &self.candidates {
            candidate.validate()?;
            if !candidate_ids.insert(candidate.id.clone()) {
                return Err(CorrelationError::DuplicateId);
            }
            if candidate.scope != self.scope
                || candidate.window != self.window
                || !candidate
                    .signal_ids
                    .iter()
                    .all(|signal_id| signal_ids.contains(signal_id))
            {
                return Err(CorrelationError::CandidateReferenceMissing);
            }
            let all_suppressed = candidate.signal_ids.iter().all(|signal_id| {
                self.signals
                    .iter()
                    .find(|signal| signal.id == *signal_id)
                    .is_some_and(|signal| signal.suppression.kind != SuppressionKind::NotSuppressed)
            });
            let expected_status = if all_suppressed {
                CandidateStatus::Suppressed
            } else if !candidate.late_signal_ids.is_empty()
                || self.window.state == CorrelationWindowState::Reopened
            {
                CandidateStatus::Provisional
            } else {
                CandidateStatus::Active
            };
            if candidate.status != expected_status {
                return Err(CorrelationError::CandidateStatusMismatch);
            }
            ensure_known_evidence(&candidate.evidence_ids, &evidence_ids)?;
            ensure_known_evidence(&candidate.drill_down.evidence_ids, &evidence_ids)?;
            ensure_known_evidence(&candidate.drill_down_reference.evidence_ids, &evidence_ids)?;
            for reason in &candidate.reasons {
                if !reason
                    .topology_path_ids
                    .iter()
                    .all(|path_id| path_ids.contains(path_id))
                {
                    return Err(CorrelationError::CandidateReferenceMissing);
                }
                for signal_id in &reason.signal_ids {
                    let signal = self
                        .signals
                        .iter()
                        .find(|signal| signal.id == *signal_id)
                        .ok_or(CorrelationError::CandidateReferenceMissing)?;
                    if let Some(target) = &reason.target {
                        if !candidate.grouping_targets.contains(target)
                            || !signal.targets.contains(target)
                        {
                            return Err(CorrelationError::InvalidReason);
                        }
                    } else if !reason.topology_path_ids.iter().any(|path_id| {
                        self.topology_paths
                            .iter()
                            .find(|path| path.id == *path_id)
                            .is_some_and(|path| {
                                signal
                                    .targets
                                    .iter()
                                    .any(|target| path.node_ids.contains(&target.id))
                            })
                    }) {
                        return Err(CorrelationError::InvalidReason);
                    }
                    if !contains_all(&reason.evidence_ids, &signal.evidence_ids) {
                        return Err(CorrelationError::EvidenceMissing);
                    }
                }
                for path_id in &reason.topology_path_ids {
                    let path = self
                        .topology_paths
                        .iter()
                        .find(|path| path.id == *path_id)
                        .ok_or(CorrelationError::CandidateReferenceMissing)?;
                    if !contains_all(&reason.evidence_ids, &path.evidence_ids) {
                        return Err(CorrelationError::EvidenceMissing);
                    }
                }
            }
            if candidate.grouping_targets.iter().any(|target| {
                !candidate
                    .reasons
                    .iter()
                    .any(|reason| reason.target.as_ref() == Some(target))
            }) {
                return Err(CorrelationError::CandidateReferenceMissing);
            }
            for signal_id in &candidate.signal_ids {
                let signal = self
                    .signals
                    .iter()
                    .find(|signal| signal.id == *signal_id)
                    .ok_or(CorrelationError::CandidateReferenceMissing)?;
                if !contains_all(&candidate.evidence_ids, &signal.evidence_ids) {
                    return Err(CorrelationError::EvidenceMissing);
                }
            }
        }

        for metric in &self.summary.metrics {
            ensure_known_evidence(&metric.evidence_ids, &evidence_ids)?;
            ensure_known_evidence(&metric.drill_down.evidence_ids, &evidence_ids)?;
            ensure_known_evidence(&metric.drill_down_reference.evidence_ids, &evidence_ids)?;
            if !self.scope.contains(&metric.drill_down_reference.scope) {
                return Err(CorrelationError::ScopeMismatch);
            }
        }
        for status in &self.source_status {
            if status.source_key.trim().is_empty() {
                return Err(CorrelationError::InvalidId);
            }
            ensure_known_evidence(&status.evidence_ids, &evidence_ids)?;
        }
        if self
            .source_status
            .iter()
            .map(|status| status.source_key.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != self.source_status.len()
        {
            return Err(CorrelationError::DuplicateId);
        }
        Ok(())
    }
}

/// Evidence IDs already admitted to a correlation snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CorrelationEvidenceRequest {
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

impl CorrelationEvidenceRequest {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_correlation_evidence_ids(&self.evidence_ids)
    }
}

/// How a Signal was suppressed for an evaluation.
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

/// Complete, retained suppression decision for one Signal.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuppressionState {
    pub kind: SuppressionKind,
    pub rule_ids: Vec<String>,
    pub maintenance_window_ids: Vec<String>,
    pub evaluated_at: String,
    pub policy_version: u64,
}

impl SuppressionState {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        parse_correlation_timestamp(&self.evaluated_at)?;
        validate_sorted_identifiers(&self.rule_ids)?;
        validate_sorted_identifiers(&self.maintenance_window_ids)?;
        let has_rules = !self.rule_ids.is_empty();
        let has_windows = !self.maintenance_window_ids.is_empty();
        let expected_kind = match (has_rules, has_windows) {
            (false, false) => SuppressionKind::NotSuppressed,
            (true, false) => SuppressionKind::Rule,
            (false, true) => SuppressionKind::MaintenanceWindow,
            (true, true) => SuppressionKind::RuleAndMaintenanceWindow,
        };
        if self.kind != expected_kind {
            return Err(CorrelationError::SuppressionMismatch);
        }
        Ok(())
    }
}

/// Internal policy selector for Signals.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SuppressionRule {
    pub id: String,
    pub enabled: bool,
    pub scope: ResourceScope,
    pub source: Option<EvidenceSourceKind>,
    pub signal_kind: Option<SignalKind>,
    pub target: Option<SignalTarget>,
}

impl SuppressionRule {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_safe_identifier(&self.id)?;
        if let Some(target) = &self.target {
            target.validate()?;
        }
        Ok(())
    }
}

/// Reason attached to a maintenance window.
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

/// Internal policy maintenance interval used during suppression evaluation.
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

impl MaintenanceWindow {
    pub fn validate(&self) -> Result<(), CorrelationError> {
        validate_safe_identifier(&self.id)?;
        self.window.validate()?;
        if let Some(target) = &self.target {
            target.validate()?;
        }
        Ok(())
    }
}

/// Numeric fields that receive finite-value validation before serialization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CorrelationNumberField {
    ObservedValue,
    ComparisonValue,
    CvssScore,
    MetricValue,
}

/// Typed validation failures for source-preserving signal/correlation values.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CorrelationError {
    #[error("correlation identifier is empty or unsafe")]
    InvalidId,
    #[error("correlation timestamp is invalid")]
    InvalidTimestamp,
    #[error("correlation window is invalid")]
    InvalidWindow,
    #[error("correlation window exceeds the allowed bound")]
    WindowOutOfRange,
    #[error("correlation allowed lateness exceeds the allowed bound")]
    LatenessOutOfRange,
    #[error("correlation number is not finite: {0:?}")]
    NonFiniteNumber(CorrelationNumberField),
    #[error("CVSS score is outside 0.0..=10.0")]
    CvssOutOfRange,
    #[error("correlation evidence is missing")]
    EvidenceMissing,
    #[error("correlation evidence is invalid")]
    InvalidEvidence,
    #[error("signal payload and kind do not agree")]
    PayloadKindMismatch,
    #[error("signal source and source record/finding source do not agree")]
    SourceMismatch,
    #[error("finding source is not an initial security source")]
    UnsupportedFindingSource,
    #[error("correlation target is invalid or does not match its finding")]
    TargetMismatch,
    #[error("correlation reason is invalid")]
    InvalidReason,
    #[error("correlation candidate has fewer than two signals")]
    CandidateTooSmall,
    #[error("correlation candidate reference is missing")]
    CandidateReferenceMissing,
    #[error("correlation candidate status is inconsistent")]
    CandidateStatusMismatch,
    #[error("correlation snapshot scope does not contain a child value")]
    ScopeMismatch,
    #[error("correlation snapshot window and request do not agree")]
    WindowMismatch,
    #[error("correlation metric must use count units")]
    MetricUnitMismatch,
    #[error("correlation metric value cannot be negative")]
    MetricValueOutOfRange,
    #[error("correlation suppression lists do not agree with kind")]
    SuppressionMismatch,
    #[error("correlation value is malformed")]
    InvalidPayload,
    #[error("topology path is invalid")]
    InvalidTopologyPath,
    #[error("correlation identifier is duplicated")]
    DuplicateId,
}

pub const MAX_CORRELATION_WINDOW_SECONDS: i64 = 86_400;
pub const MAX_CORRELATION_LATENESS_SECONDS: u64 = 21_600;

impl TimeWindow {
    /// Validate an RFC3339 half-open range and its 24-hour bound.
    pub fn validate(&self) -> Result<(), CorrelationError> {
        let start = parse_correlation_timestamp(&self.start)?;
        let end = parse_correlation_timestamp(&self.end)?;
        if start >= end {
            return Err(CorrelationError::InvalidWindow);
        }
        if end - start > chrono::Duration::seconds(MAX_CORRELATION_WINDOW_SECONDS) {
            return Err(CorrelationError::WindowOutOfRange);
        }
        Ok(())
    }
}

fn parse_correlation_timestamp(value: &str) -> Result<DateTime<Utc>, CorrelationError> {
    if value.trim().is_empty() {
        return Err(CorrelationError::InvalidTimestamp);
    }
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| CorrelationError::InvalidTimestamp)
}

fn parse_change_timestamp(value: &str) -> Result<DateTime<Utc>, ChangeError> {
    if value.trim().is_empty() {
        return Err(ChangeError::InvalidTimestamp);
    }
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|_| ChangeError::InvalidTimestamp)
}

fn validate_change_lookback(value: u64) -> Result<(), ChangeError> {
    if value > MAX_CHANGE_LOOKBACK_SECONDS {
        return Err(ChangeError::InvalidLookback);
    }
    Ok(())
}

fn looks_like_email(value: &str) -> bool {
    value.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty() && !domain.is_empty() && !domain.chars().any(char::is_whitespace)
    })
}

/// Parse only the URL properties that the change contract admits.  This is
/// intentionally small and strict: a query or fragment is never accepted,
/// because provider URLs commonly place credentials and cursors there.
fn parse_https_host(value: &str) -> Option<String> {
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
    {
        return None;
    }
    let (scheme, remainder) = value.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") || remainder.is_empty() {
        return None;
    }
    if remainder.contains('?') || remainder.contains('#') {
        return None;
    }

    let authority_end = remainder.find('/').unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return None;
    }

    let host = if let Some(stripped) = authority.strip_prefix('[') {
        let end = stripped.find(']')?;
        let host = &stripped[..end];
        let suffix = &stripped[end + 1..];
        if host.is_empty()
            || (!suffix.is_empty()
                && (!suffix.starts_with(':')
                    || suffix[1..].is_empty()
                    || !suffix[1..]
                        .chars()
                        .all(|character| character.is_ascii_digit())))
        {
            return None;
        }
        host
    } else {
        let colon_count = authority
            .chars()
            .filter(|character| *character == ':')
            .count();
        if colon_count > 1 {
            return None;
        }
        if let Some((host, port)) = authority.split_once(':') {
            if port.is_empty() || !port.chars().all(|character| character.is_ascii_digit()) {
                return None;
            }
            host
        } else {
            authority
        }
    };
    if host.is_empty() || host.contains('.') && host.starts_with('.') {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

fn validate_change_path(path: &str) -> Result<(), ChangeError> {
    validate_safe_identifier(path).map_err(|_| ChangeError::InvalidPath)?;
    if path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(ChangeError::InvalidPath);
    }
    Ok(())
}

fn validate_change_evidence_ids(ids: &[ConsoleEvidenceId]) -> Result<(), ChangeError> {
    if ids.is_empty() {
        return Err(ChangeError::EvidenceMissing);
    }
    validate_change_evidence_ids_allow_empty(ids)
}

fn validate_change_evidence_ids_allow_empty(ids: &[ConsoleEvidenceId]) -> Result<(), ChangeError> {
    if ids.iter().any(|id| validate_safe_identifier(id).is_err())
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
        || ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(ChangeError::InvalidEvidence);
    }
    Ok(())
}

fn validate_sorted_change_identifiers(ids: &[String]) -> Result<(), ChangeError> {
    if ids.iter().any(|id| validate_safe_identifier(id).is_err())
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
        || ids.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(ChangeError::InvalidId);
    }
    Ok(())
}

fn ensure_change_known_evidence(
    ids: &[ConsoleEvidenceId],
    known_ids: &BTreeSet<ConsoleEvidenceId>,
) -> Result<(), ChangeError> {
    if ids.iter().any(|id| !known_ids.contains(id)) {
        Err(ChangeError::EvidenceMissing)
    } else {
        Ok(())
    }
}

fn validate_change_drill_down(
    drill_down: &DrillDownTarget,
    reference: &DrillDownReference,
    evidence_ids: &[ConsoleEvidenceId],
    scope: &ResourceScope,
) -> Result<(), ChangeError> {
    if drill_down.destination != DrillDownDestination::Evidence
        || drill_down.evidence_ids.is_empty()
        || reference.source_query.trim().is_empty()
        || reference.source_query.chars().any(char::is_control)
        || reference.evidence_ids.is_empty()
    {
        return Err(ChangeError::InvalidEvidence);
    }
    validate_change_evidence_ids(&drill_down.evidence_ids)?;
    validate_change_evidence_ids(&reference.evidence_ids)?;
    if !contains_all(evidence_ids, &drill_down.evidence_ids)
        || !contains_all(evidence_ids, &reference.evidence_ids)
    {
        return Err(ChangeError::EvidenceMissing);
    }
    if !scope.contains(&reference.scope) {
        return Err(ChangeError::ScopeMismatch);
    }
    if let Some(filter_key) = &drill_down.filter_key {
        validate_safe_identifier(filter_key).map_err(|_| ChangeError::InvalidId)?;
    }
    if let Some(time_window) = &reference.time_window {
        time_window
            .validate()
            .map_err(|_| ChangeError::InvalidWindow)?;
    }
    Ok(())
}

fn validate_optional_timestamp(value: Option<&str>) -> Result<(), CorrelationError> {
    if let Some(value) = value {
        parse_correlation_timestamp(value)?;
    }
    Ok(())
}

fn validate_finite(value: f64, field: CorrelationNumberField) -> Result<(), CorrelationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(CorrelationError::NonFiniteNumber(field))
    }
}

fn validate_non_empty_text(value: &str) -> Result<(), CorrelationError> {
    if value.trim().is_empty() {
        Err(CorrelationError::InvalidId)
    } else {
        Ok(())
    }
}

fn validate_safe_text(value: &str) -> Result<(), CorrelationError> {
    validate_non_empty_text(value)?;
    if value.chars().any(|character| character.is_control()) || contains_sensitive_marker(value) {
        return Err(CorrelationError::InvalidId);
    }
    Ok(())
}

fn validate_safe_identifier(value: &str) -> Result<(), CorrelationError> {
    validate_safe_text(value)?;
    if value.chars().any(char::is_whitespace) {
        return Err(CorrelationError::InvalidId);
    }
    Ok(())
}

fn contains_sensitive_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "credential",
        "authorization",
        "bearer",
        "api_key",
        "access_key",
        "private_key",
        "arn:",
        "/subscriptions/",
        "subscription_id",
        "account_id",
        "pagination_cursor",
        "next_link",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
        || contains_sensitive_account_id(&lower)
}

fn contains_sensitive_account_id(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if lower.contains("sha256:") || lower.contains("dedup:v1:") || lower.contains("candidate:v1:") {
        return false;
    }
    if looks_like_uuid(value) {
        return false;
    }
    let mut run_length = 0usize;
    for character in value.chars() {
        if character.is_ascii_digit() {
            run_length = run_length.saturating_add(1);
        } else {
            if run_length >= 12 {
                return true;
            }
            run_length = 0;
        }
    }
    run_length >= 12
}

fn looks_like_uuid(value: &str) -> bool {
    let parts = value.split('-').collect::<Vec<_>>();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(parts.iter())
            .all(|(length, part)| {
                part.len() == *length && part.chars().all(|c| c.is_ascii_hexdigit())
            })
}

fn validate_correlation_evidence_ids(ids: &[ConsoleEvidenceId]) -> Result<(), CorrelationError> {
    if ids.is_empty()
        || ids.iter().any(|id| validate_safe_identifier(id).is_err())
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(CorrelationError::EvidenceMissing);
    }
    if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CorrelationError::DuplicateId);
    }
    Ok(())
}

fn validate_signal_ids(ids: &[SignalId], require_two: bool) -> Result<(), CorrelationError> {
    if require_two && ids.len() < 2 {
        return Err(CorrelationError::CandidateTooSmall);
    }
    if ids.iter().any(Uuid::is_nil) || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(CorrelationError::InvalidId);
    }
    if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(CorrelationError::DuplicateId);
    }
    Ok(())
}

fn validate_targets(targets: &[SignalTarget]) -> Result<(), CorrelationError> {
    let mut identities = BTreeSet::new();
    for target in targets {
        target.validate()?;
        if !identities.insert((target.kind, target.id.as_str())) {
            return Err(CorrelationError::DuplicateId);
        }
    }
    Ok(())
}

fn validate_sorted_identifiers(ids: &[String]) -> Result<(), CorrelationError> {
    let mut previous: Option<&str> = None;
    for id in ids {
        validate_safe_identifier(id)?;
        if previous.is_some_and(|previous| previous >= id.as_str()) {
            return Err(CorrelationError::DuplicateId);
        }
        previous = Some(id);
    }
    Ok(())
}

fn contains_all<T: PartialEq>(haystack: &[T], needles: &[T]) -> bool {
    needles.iter().all(|needle| haystack.contains(needle))
}

fn ensure_known_evidence(
    ids: &[ConsoleEvidenceId],
    known_ids: &BTreeSet<String>,
) -> Result<(), CorrelationError> {
    if ids.iter().any(|id| !known_ids.contains(id)) {
        Err(CorrelationError::EvidenceMissing)
    } else {
        Ok(())
    }
}

fn validate_evidence_drill_down_for_correlation(
    drill_down: &DrillDownTarget,
    reference: &DrillDownReference,
    evidence_ids: &[ConsoleEvidenceId],
) -> Result<(), CorrelationError> {
    if drill_down.destination != DrillDownDestination::Evidence
        || drill_down.evidence_ids.is_empty()
        || reference.source_query.trim().is_empty()
        || reference.evidence_ids.is_empty()
        || !contains_all(evidence_ids, &drill_down.evidence_ids)
        || !contains_all(evidence_ids, &reference.evidence_ids)
    {
        return Err(CorrelationError::EvidenceMissing);
    }
    Ok(())
}

/// Closed, provider-neutral node kinds in the resource and service topology.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TopologyNodeKind {
    #[serde(rename = "environment")]
    Environment,
    #[serde(rename = "cluster")]
    Cluster,
    #[serde(rename = "namespace")]
    Namespace,
    #[serde(rename = "workload")]
    Workload,
    #[serde(rename = "service")]
    Service,
    #[serde(rename = "pod")]
    Pod,
    #[serde(rename = "node")]
    Node,
    #[serde(rename = "cloud_resource")]
    CloudResource,
    #[serde(rename = "observability_target")]
    ObservabilityTarget,
}

/// Source used to resolve a topology node's owning team.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TopologyOwnershipSource {
    #[serde(rename = "explicit_label")]
    ExplicitLabel,
    #[serde(rename = "resource_scope")]
    ResourceScope,
    #[serde(rename = "environment_default")]
    EnvironmentDefault,
    #[serde(rename = "fixture")]
    Fixture,
    #[serde(rename = "unassigned")]
    Unassigned,
}

/// Resolved owner reference for a topology node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyOwnership {
    pub team_id: Option<TeamId>,
    pub team_name: Option<String>,
    pub source: TopologyOwnershipSource,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

impl TopologyOwnership {
    /// Validates the explicit unassigned state and canonical team pairing.
    pub fn validate(&self) -> Result<(), TopologyError> {
        match self.source {
            TopologyOwnershipSource::Unassigned => {
                if self.team_id.is_some() || self.team_name.is_some() {
                    return Err(TopologyError::InvalidRequest);
                }
            }
            _ => {
                if self.team_id.is_none()
                    || self
                        .team_name
                        .as_deref()
                        .is_none_or(|name| name.trim().is_empty())
                {
                    return Err(TopologyError::InvalidRequest);
                }
            }
        }
        if self
            .evidence_ids
            .iter()
            .any(|evidence_id| evidence_id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        if self.source != TopologyOwnershipSource::Unassigned && self.evidence_ids.is_empty() {
            return Err(TopologyError::EvidenceMissing);
        }
        Ok(())
    }
}

/// Evidence-backed numeric value used by topology nodes and summaries.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologyMetric {
    pub key: String,
    pub value: f64,
    pub unit: NumberUnit,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
    pub drill_down_reference: DrillDownReference,
}

impl TopologyMetric {
    /// Validates finite values and the evidence navigation behind the metric.
    pub fn validate(&self) -> Result<(), TopologyError> {
        self.validate_with_destination(DrillDownDestination::Topology)
    }

    /// Validates a summary metric whose drill-down opens its evidence set.
    pub fn validate_summary(&self) -> Result<(), TopologyError> {
        self.validate_with_destination(DrillDownDestination::Evidence)
    }

    fn validate_with_destination(
        &self,
        destination: DrillDownDestination,
    ) -> Result<(), TopologyError> {
        if !self.value.is_finite() {
            return Err(TopologyError::NonFiniteNumber(
                TopologyNumberField::MetricValue,
            ));
        }
        if self.unit == NumberUnit::Count && self.value < 0.0 {
            return Err(TopologyError::InvalidRequest);
        }
        if self.key.trim().is_empty() {
            return Err(TopologyError::InvalidRequest);
        }
        if self.drill_down_reference.source_query.trim().is_empty() {
            return Err(TopologyError::InvalidRequest);
        }
        if destination == DrillDownDestination::Evidence
            && self.value == 0.0
            && self.evidence_ids.is_empty()
        {
            if self.drill_down.destination != DrillDownDestination::Evidence
                || !self.drill_down.evidence_ids.is_empty()
                || self.drill_down.filter_key.is_some()
                || !self.drill_down_reference.evidence_ids.is_empty()
            {
                return Err(TopologyError::InvalidRequest);
            }
            return Ok(());
        }
        if self.evidence_ids.is_empty()
            || self
                .evidence_ids
                .iter()
                .any(|evidence_id| evidence_id.trim().is_empty())
        {
            return Err(TopologyError::EvidenceMissing);
        }
        validate_drill_down(&self.drill_down, &self.evidence_ids, destination)?;
        if self.drill_down_reference.evidence_ids.is_empty()
            || !shares_evidence(&self.evidence_ids, &self.drill_down_reference.evidence_ids)
        {
            return Err(TopologyError::InvalidRequest);
        }
        Ok(())
    }
}

/// Evidence-backed node in the provider-neutral topology graph.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologyNode {
    pub id: String,
    pub kind: TopologyNodeKind,
    pub name: String,
    pub native_kind: Option<String>,
    pub native_id: Option<String>,
    pub environment_id: Option<String>,
    pub provider: Option<String>,
    pub scope: ResourceScope,
    pub status: ConsoleHealthState,
    pub labels: BTreeMap<String, String>,
    pub ownership: TopologyOwnership,
    pub metric: Option<TopologyMetric>,
    pub affected_by_incident: bool,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
}

impl TopologyNode {
    /// Validates identity, ownership, optional source fields and evidence.
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.id.trim().is_empty()
            || self.name.trim().is_empty()
            || self
                .native_kind
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .native_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .environment_id
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            || self
                .provider
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        self.ownership.validate()?;
        if let Some(metric) = &self.metric {
            metric.validate()?;
        }
        if self.evidence_ids.is_empty()
            || self
                .evidence_ids
                .iter()
                .any(|evidence_id| evidence_id.trim().is_empty())
        {
            return Err(TopologyError::EvidenceMissing);
        }
        validate_topology_node_drill_down(&self.drill_down, &self.evidence_ids, &self.id)
    }
}

/// Relationship vocabulary for directed topology edges.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TopologyEdgeKind {
    #[serde(rename = "contains")]
    Contains,
    #[serde(rename = "owns")]
    Owns,
    #[serde(rename = "selects")]
    Selects,
    #[serde(rename = "routes_to")]
    RoutesTo,
    #[serde(rename = "runs_on")]
    RunsOn,
    #[serde(rename = "depends_on")]
    DependsOn,
}

/// Provider-neutral source category that produced a topology edge.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TopologySourceKind {
    #[serde(rename = "kubernetes")]
    Kubernetes,
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "observability")]
    Observability,
    #[serde(rename = "fixture")]
    Fixture,
}

/// Source key and observation time for one edge provenance record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyEdgeProvenance {
    pub source: TopologySourceKind,
    pub source_key: String,
    pub observed_at: Option<String>,
}

impl TopologyEdgeProvenance {
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.source_key.trim().is_empty()
            || self
                .observed_at
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(TopologyError::MalformedSource);
        }
        Ok(())
    }
}

/// Directed, evidence-backed relationship between an upstream and downstream node.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologyEdge {
    pub id: String,
    pub upstream_node_id: String,
    pub downstream_node_id: String,
    pub kind: TopologyEdgeKind,
    pub provenance: Vec<TopologyEdgeProvenance>,
    pub confidence: f64,
    pub metadata: BTreeMap<String, String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
}

impl TopologyEdge {
    /// Validates an edge without requiring the graph's node index.
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.id.trim().is_empty()
            || self.upstream_node_id.trim().is_empty()
            || self.downstream_node_id.trim().is_empty()
            || self.upstream_node_id == self.downstream_node_id
        {
            return Err(TopologyError::InvalidRequest);
        }
        validate_confidence(self.confidence, TopologyNumberField::EdgeConfidence)?;
        if self.provenance.is_empty() {
            return Err(TopologyError::MalformedSource);
        }
        let mut provenance_identity = BTreeSet::new();
        for provenance in &self.provenance {
            provenance.validate()?;
            if !provenance_identity.insert((provenance.source, provenance.source_key.clone())) {
                return Err(TopologyError::MalformedSource);
            }
        }
        if self.evidence_ids.is_empty()
            || self
                .evidence_ids
                .iter()
                .any(|evidence_id| evidence_id.trim().is_empty())
        {
            return Err(TopologyError::EvidenceMissing);
        }
        validate_evidence_drill_down(&self.drill_down, &self.evidence_ids)
    }

    /// Validates that both edge endpoints were emitted by the current graph.
    pub fn validate_against_nodes(&self, node_ids: &BTreeSet<String>) -> Result<(), TopologyError> {
        self.validate()?;
        if !node_ids.contains(&self.upstream_node_id)
            || !node_ids.contains(&self.downstream_node_id)
        {
            return Err(TopologyError::NodeNotFound);
        }
        Ok(())
    }
}

/// Direction used when traversing topology relationships.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologyDirection {
    #[serde(rename = "upstream")]
    Upstream,
    #[serde(rename = "downstream")]
    Downstream,
    #[serde(rename = "both")]
    Both,
}

/// Qualification for a topology path; no causal qualification is exposed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologyPathKind {
    #[serde(rename = "probable_structural")]
    ProbableStructural,
}

/// Reason a bounded topology path ended.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologyPathTermination {
    #[serde(rename = "leaf")]
    Leaf,
    #[serde(rename = "cycle_detected")]
    CycleDetected,
    #[serde(rename = "depth_limit")]
    DepthLimit,
}

/// Evidence-backed path returned from a bounded topology traversal.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologyPath {
    pub id: String,
    pub root_node_id: String,
    pub terminal_node_id: String,
    pub node_ids: Vec<String>,
    pub edge_ids: Vec<String>,
    pub direction: TopologyDirection,
    pub depth: u16,
    pub confidence: f64,
    pub kind: TopologyPathKind,
    pub termination: TopologyPathTermination,
    pub cycle_edge_id: Option<String>,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
    pub drill_down: DrillDownTarget,
}

impl TopologyPath {
    /// Validates simple-path shape, bounded depth and evidence navigation.
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.id.trim().is_empty()
            || self.root_node_id.trim().is_empty()
            || self.terminal_node_id.trim().is_empty()
            || self.node_ids.is_empty()
            || self.node_ids.first() != Some(&self.root_node_id)
            || self.node_ids.last() != Some(&self.terminal_node_id)
            || self.node_ids.len() != self.edge_ids.len().saturating_add(1)
            || self.edge_ids.len() != usize::from(self.depth)
            || self.depth > 8
            || self
                .node_ids
                .iter()
                .any(|node_id| node_id.trim().is_empty())
            || self
                .edge_ids
                .iter()
                .any(|edge_id| edge_id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        let unique_nodes: BTreeSet<_> = self.node_ids.iter().collect();
        if unique_nodes.len() != self.node_ids.len() {
            return Err(TopologyError::InvalidRequest);
        }
        validate_confidence(self.confidence, TopologyNumberField::PathConfidence)?;
        match (&self.termination, &self.cycle_edge_id) {
            (TopologyPathTermination::CycleDetected, Some(edge_id))
                if !edge_id.trim().is_empty() => {}
            (TopologyPathTermination::CycleDetected, _) => {
                return Err(TopologyError::InvalidRequest)
            }
            (_, Some(_)) => return Err(TopologyError::InvalidRequest),
            (_, None) => {}
        }
        if self.evidence_ids.is_empty()
            || self
                .evidence_ids
                .iter()
                .any(|evidence_id| evidence_id.trim().is_empty())
        {
            return Err(TopologyError::EvidenceMissing);
        }
        validate_evidence_drill_down(&self.drill_down, &self.evidence_ids)
    }

    /// Validates that path nodes and edges belong to the current graph.
    pub fn validate_against_graph(
        &self,
        node_ids: &BTreeSet<String>,
        edges: &BTreeMap<String, TopologyEdge>,
    ) -> Result<(), TopologyError> {
        self.validate()?;
        if self
            .node_ids
            .iter()
            .any(|node_id| !node_ids.contains(node_id))
        {
            return Err(TopologyError::NodeNotFound);
        }
        if self
            .edge_ids
            .iter()
            .chain(self.cycle_edge_id.iter())
            .any(|edge_id| !edges.contains_key(edge_id))
        {
            return Err(TopologyError::InvalidRequest);
        }
        if self
            .cycle_edge_id
            .as_ref()
            .is_some_and(|edge_id| self.edge_ids.contains(edge_id))
        {
            return Err(TopologyError::InvalidRequest);
        }

        let mut expected_confidence: f64 = 1.0;
        for edge_id in self.edge_ids.iter().chain(self.cycle_edge_id.iter()) {
            let edge = edges.get(edge_id).ok_or(TopologyError::InvalidRequest)?;
            expected_confidence = expected_confidence.min(edge.confidence);
        }
        if self.confidence != expected_confidence {
            return Err(TopologyError::InvalidRequest);
        }

        for (edge_id, node_pair) in self.edge_ids.iter().zip(self.node_ids.windows(2)) {
            let edge = edges.get(edge_id).ok_or(TopologyError::InvalidRequest)?;
            let follows_direction = match self.direction {
                TopologyDirection::Upstream => {
                    edge.downstream_node_id == node_pair[0] && edge.upstream_node_id == node_pair[1]
                }
                TopologyDirection::Downstream => {
                    edge.upstream_node_id == node_pair[0] && edge.downstream_node_id == node_pair[1]
                }
                TopologyDirection::Both => {
                    (edge.upstream_node_id == node_pair[0]
                        && edge.downstream_node_id == node_pair[1])
                        || (edge.downstream_node_id == node_pair[0]
                            && edge.upstream_node_id == node_pair[1])
                }
            };
            if !follows_direction {
                return Err(TopologyError::InvalidRequest);
            }
        }

        if let Some(cycle_edge_id) = &self.cycle_edge_id {
            let cycle_edge = edges
                .get(cycle_edge_id)
                .ok_or(TopologyError::InvalidRequest)?;
            let closes_cycle = match self.direction {
                TopologyDirection::Upstream => {
                    cycle_edge.downstream_node_id == self.terminal_node_id
                        && self.node_ids.contains(&cycle_edge.upstream_node_id)
                }
                TopologyDirection::Downstream => {
                    cycle_edge.upstream_node_id == self.terminal_node_id
                        && self.node_ids.contains(&cycle_edge.downstream_node_id)
                }
                TopologyDirection::Both => {
                    (cycle_edge.downstream_node_id == self.terminal_node_id
                        && self.node_ids.contains(&cycle_edge.upstream_node_id))
                        || (cycle_edge.upstream_node_id == self.terminal_node_id
                            && self.node_ids.contains(&cycle_edge.downstream_node_id))
                }
            };
            if !closes_cycle {
                return Err(TopologyError::InvalidRequest);
            }
        }
        Ok(())
    }
}

/// Bounded upstream/downstream traversal request.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyTraversal {
    pub direction: TopologyDirection,
    pub max_depth: u16,
}

impl TopologyTraversal {
    /// Validates the inclusive Sprint 12 depth bound.
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.max_depth > 8 {
            return Err(TopologyError::InvalidRequest);
        }
        Ok(())
    }
}

/// Environment, team and Sprint 11 incident-queue filter dimensions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyFilter {
    pub environment_ids: Vec<String>,
    pub team_ids: Vec<TeamId>,
    /// Sprint 11 IncidentQueueItem.id; this is not IncidentId.
    pub incident_id: Option<String>,
}

impl TopologyFilter {
    /// Validates explicit absent values, identifiers and duplicate dimensions.
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self
            .environment_ids
            .iter()
            .any(|environment_id| environment_id.trim().is_empty())
            || self
                .incident_id
                .as_deref()
                .is_some_and(|incident_id| incident_id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        let environments: BTreeSet<_> = self.environment_ids.iter().collect();
        let teams: BTreeSet<_> = self.team_ids.iter().collect();
        if environments.len() != self.environment_ids.len() || teams.len() != self.team_ids.len() {
            return Err(TopologyError::InvalidRequest);
        }
        Ok(())
    }

    /// Validates filter IDs against the current workspace graph and queue projection.
    pub fn validate_against(
        &self,
        environment_ids: &BTreeSet<String>,
        team_ids: &BTreeSet<TeamId>,
        incident_ids: &BTreeSet<String>,
    ) -> Result<(), TopologyError> {
        self.validate()?;
        if self
            .environment_ids
            .iter()
            .any(|environment_id| !environment_ids.contains(environment_id))
            || self
                .team_ids
                .iter()
                .any(|team_id| !team_ids.contains(team_id))
        {
            return Err(TopologyError::InvalidRequest);
        }
        if self
            .incident_id
            .as_ref()
            .is_some_and(|incident_id| !incident_ids.contains(incident_id))
        {
            return Err(TopologyError::IncidentNotFound);
        }
        Ok(())
    }
}

/// Complete topology selection and bounded traversal request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyRequest {
    pub filter: TopologyFilter,
    pub focus_node_id: Option<String>,
    pub traversal: TopologyTraversal,
}

impl TopologyRequest {
    /// Validates request shape before graph work.
    pub fn validate(&self) -> Result<(), TopologyError> {
        self.filter.validate()?;
        self.traversal.validate()?;
        if self
            .focus_node_id
            .as_deref()
            .is_some_and(|node_id| node_id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        Ok(())
    }

    /// Validates request IDs against a current workspace graph and queue projection.
    pub fn validate_against(
        &self,
        node_ids: &BTreeSet<String>,
        environment_ids: &BTreeSet<String>,
        team_ids: &BTreeSet<TeamId>,
        incident_ids: &BTreeSet<String>,
    ) -> Result<(), TopologyError> {
        self.validate()?;
        if self
            .focus_node_id
            .as_ref()
            .is_some_and(|node_id| !node_ids.contains(node_id))
        {
            return Err(TopologyError::NodeNotFound);
        }
        self.filter
            .validate_against(environment_ids, team_ids, incident_ids)
    }
}

/// Evidence-backed counts for the visible topology projection.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologySummary {
    pub visible_nodes: TopologyMetric,
    pub visible_edges: TopologyMetric,
    pub affected_nodes: TopologyMetric,
    pub probable_paths: TopologyMetric,
}

impl TopologySummary {
    pub fn validate(&self) -> Result<(), TopologyError> {
        self.visible_nodes.validate_summary()?;
        self.visible_edges.validate_summary()?;
        self.affected_nodes.validate_summary()?;
        self.probable_paths.validate_summary()
    }
}

/// Complete read-only topology graph, paths, source statuses and admitted evidence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TopologySnapshot {
    pub generated_at: String,
    pub scope: ResourceScope,
    pub filter: TopologyFilter,
    pub focus_node_id: Option<String>,
    pub traversal: TopologyTraversal,
    pub summary: TopologySummary,
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub paths: Vec<TopologyPath>,
    pub source_status: Vec<SourceStatus>,
    pub evidence: Vec<EvidenceRef>,
}

impl TopologySnapshot {
    /// Validates graph references, evidence navigation and all finite numbers.
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.generated_at.trim().is_empty() {
            return Err(TopologyError::InvalidRequest);
        }
        self.filter.validate()?;
        self.traversal.validate()?;

        let node_ids: BTreeSet<_> = self.nodes.iter().map(|node| node.id.clone()).collect();
        if node_ids.len() != self.nodes.len()
            || self.nodes.iter().any(|node| node.id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        for node in &self.nodes {
            node.validate()?;
        }
        if self
            .focus_node_id
            .as_ref()
            .is_some_and(|node_id| !node_ids.contains(node_id))
        {
            return Err(TopologyError::NodeNotFound);
        }

        let edges: BTreeMap<_, _> = self
            .edges
            .iter()
            .map(|edge| (edge.id.clone(), edge.clone()))
            .collect();
        if edges.len() != self.edges.len()
            || self.edges.iter().any(|edge| edge.id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        for edge in &self.edges {
            edge.validate_against_nodes(&node_ids)?;
        }

        let path_ids: BTreeSet<_> = self.paths.iter().map(|path| path.id.clone()).collect();
        if path_ids.len() != self.paths.len()
            || self.paths.iter().any(|path| path.id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        for path in &self.paths {
            path.validate_against_graph(&node_ids, &edges)?;
        }
        for (metric, expected) in [
            (&self.summary.visible_nodes, self.nodes.len()),
            (&self.summary.visible_edges, self.edges.len()),
            (
                &self.summary.affected_nodes,
                self.nodes
                    .iter()
                    .filter(|node| node.affected_by_incident)
                    .count(),
            ),
            (&self.summary.probable_paths, self.paths.len()),
        ] {
            if metric.value != expected as f64 {
                return Err(TopologyError::InvalidRequest);
            }
        }
        self.summary.validate()?;

        let evidence_ids: BTreeSet<_> = self
            .evidence
            .iter()
            .map(|evidence| evidence.id.clone())
            .collect();
        if evidence_ids.len() != self.evidence.len()
            || self
                .evidence
                .iter()
                .any(|evidence| evidence.id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        if self.evidence.iter().any(|evidence| {
            !evidence.redaction.classification_verified
                || !evidence.redaction.redaction_verified
                || (evidence.redaction.unparsed && evidence.redaction.masked)
        }) {
            return Err(TopologyError::EvidenceUnverified);
        }
        for node in &self.nodes {
            validate_evidence_ids(&node.evidence_ids, &evidence_ids)?;
            validate_evidence_ids(&node.ownership.evidence_ids, &evidence_ids)?;
            if let Some(metric) = &node.metric {
                validate_evidence_ids(&metric.evidence_ids, &evidence_ids)?;
                validate_evidence_ids(&metric.drill_down.evidence_ids, &evidence_ids)?;
                validate_evidence_ids(&metric.drill_down_reference.evidence_ids, &evidence_ids)?;
            }
            validate_evidence_ids(&node.drill_down.evidence_ids, &evidence_ids)?;
        }
        for edge in &self.edges {
            validate_evidence_ids(&edge.evidence_ids, &evidence_ids)?;
            validate_evidence_ids(&edge.drill_down.evidence_ids, &evidence_ids)?;
        }
        for path in &self.paths {
            validate_evidence_ids(&path.evidence_ids, &evidence_ids)?;
            validate_evidence_ids(&path.drill_down.evidence_ids, &evidence_ids)?;
            validate_path_evidence(path, &self.nodes, &edges)?;
        }
        for metric in [
            &self.summary.visible_nodes,
            &self.summary.visible_edges,
            &self.summary.affected_nodes,
            &self.summary.probable_paths,
        ] {
            validate_evidence_ids(&metric.evidence_ids, &evidence_ids)?;
            validate_evidence_ids(&metric.drill_down.evidence_ids, &evidence_ids)?;
            validate_evidence_ids(&metric.drill_down_reference.evidence_ids, &evidence_ids)?;
        }
        let source_status_keys: BTreeSet<_> = self
            .source_status
            .iter()
            .map(|status| status.source_key.clone())
            .collect();
        if source_status_keys.len() != self.source_status.len()
            || self
                .source_status
                .iter()
                .any(|status| status.source_key.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        for status in &self.source_status {
            validate_evidence_ids(&status.evidence_ids, &evidence_ids)?;
        }
        Ok(())
    }
}

/// Request for evidence IDs previously emitted by a topology snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyEvidenceRequest {
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

impl TopologyEvidenceRequest {
    /// Validates non-empty, unique evidence IDs before lookup.
    pub fn validate(&self) -> Result<(), TopologyError> {
        if self.evidence_ids.is_empty()
            || self
                .evidence_ids
                .iter()
                .any(|evidence_id| evidence_id.trim().is_empty())
            || self.evidence_ids.iter().collect::<BTreeSet<_>>().len() != self.evidence_ids.len()
        {
            return Err(TopologyError::InvalidRequest);
        }
        Ok(())
    }

    /// Validates that every requested ID was emitted and verified by a snapshot.
    pub fn validate_against(&self, emitted_ids: &BTreeSet<String>) -> Result<(), TopologyError> {
        self.validate()?;
        if self
            .evidence_ids
            .iter()
            .any(|evidence_id| !emitted_ids.contains(evidence_id))
        {
            return Err(TopologyError::EvidenceMissing);
        }
        Ok(())
    }
}

/// Internal selector used by deterministic ownership adapters and fixtures.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TopologyOwnershipSelector {
    #[serde(rename = "node_id")]
    NodeId { node_id: String },
    #[serde(rename = "label")]
    Label { key: String, value: String },
    #[serde(rename = "environment")]
    Environment { environment_id: String },
}

impl TopologyOwnershipSelector {
    pub fn validate(&self) -> Result<(), TopologyError> {
        let valid = match self {
            Self::NodeId { node_id } => !node_id.trim().is_empty(),
            Self::Label { key, value } => !key.trim().is_empty() && !value.trim().is_empty(),
            Self::Environment { environment_id } => !environment_id.trim().is_empty(),
        };
        if valid {
            Ok(())
        } else {
            Err(TopologyError::InvalidRequest)
        }
    }
}

/// Deterministic, non-IPC mapping rule from a node or scope to a team.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TopologyOwnershipRule {
    pub selector: TopologyOwnershipSelector,
    pub team_id: TeamId,
    pub team_name: String,
    pub source: TopologyOwnershipSource,
    pub evidence_ids: Vec<ConsoleEvidenceId>,
}

impl TopologyOwnershipRule {
    /// Validates rule selectors and canonical team display data.
    pub fn validate(&self) -> Result<(), TopologyError> {
        self.selector.validate()?;
        if self.team_name.trim().is_empty() {
            return Err(TopologyError::InvalidRequest);
        }
        if self
            .evidence_ids
            .iter()
            .any(|evidence_id| evidence_id.trim().is_empty())
        {
            return Err(TopologyError::InvalidRequest);
        }
        Ok(())
    }
}

/// Number-bearing field used in typed non-finite topology errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TopologyNumberField {
    MetricValue,
    EdgeConfidence,
    PathConfidence,
}

/// Typed validation failures for topology requests, graph records and evidence.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TopologyError {
    #[error("invalid topology request")]
    InvalidRequest,
    #[error("topology node was not found")]
    NodeNotFound,
    #[error("incident queue item was not found")]
    IncidentNotFound,
    #[error("topology scope is not allowed")]
    ScopeDenied,
    #[error("topology evidence is not verified")]
    EvidenceUnverified,
    #[error("topology evidence is missing")]
    EvidenceMissing,
    #[error("topology number is not finite")]
    NonFiniteNumber(TopologyNumberField),
    #[error("topology confidence is outside the allowed range")]
    ConfidenceOutOfRange,
    #[error("topology source is malformed")]
    MalformedSource,
}

fn validate_confidence(value: f64, field: TopologyNumberField) -> Result<(), TopologyError> {
    if !value.is_finite() {
        return Err(TopologyError::NonFiniteNumber(field));
    }
    if !(0.0..=1.0).contains(&value) {
        return Err(TopologyError::ConfidenceOutOfRange);
    }
    Ok(())
}

fn validate_topology_node_drill_down(
    drill_down: &DrillDownTarget,
    evidence_ids: &[ConsoleEvidenceId],
    node_id: &str,
) -> Result<(), TopologyError> {
    if drill_down.filter_key.as_deref() != Some(node_id) {
        return Err(TopologyError::InvalidRequest);
    }
    validate_drill_down(drill_down, evidence_ids, DrillDownDestination::Topology)
}

fn validate_evidence_drill_down(
    drill_down: &DrillDownTarget,
    evidence_ids: &[ConsoleEvidenceId],
) -> Result<(), TopologyError> {
    validate_drill_down(drill_down, evidence_ids, DrillDownDestination::Evidence)
}

fn validate_drill_down(
    drill_down: &DrillDownTarget,
    evidence_ids: &[ConsoleEvidenceId],
    destination: DrillDownDestination,
) -> Result<(), TopologyError> {
    if drill_down.destination != destination
        || drill_down.evidence_ids.is_empty()
        || !shares_evidence(evidence_ids, &drill_down.evidence_ids)
    {
        return Err(TopologyError::InvalidRequest);
    }
    Ok(())
}

fn shares_evidence(left: &[ConsoleEvidenceId], right: &[ConsoleEvidenceId]) -> bool {
    left.iter().any(|id| right.contains(id))
}

fn validate_evidence_ids(
    ids: &[ConsoleEvidenceId],
    known_ids: &BTreeSet<String>,
) -> Result<(), TopologyError> {
    if ids.iter().any(|id| !known_ids.contains(id)) {
        return Err(TopologyError::EvidenceMissing);
    }
    Ok(())
}

fn validate_path_evidence(
    path: &TopologyPath,
    nodes: &[TopologyNode],
    edges: &BTreeMap<String, TopologyEdge>,
) -> Result<(), TopologyError> {
    let mut expected = BTreeSet::new();
    for node_id in &path.node_ids {
        let node = nodes
            .iter()
            .find(|node| node.id == *node_id)
            .ok_or(TopologyError::NodeNotFound)?;
        expected.extend(node.evidence_ids.iter().cloned());
    }
    for edge_id in path.edge_ids.iter().chain(path.cycle_edge_id.iter()) {
        let edge = edges.get(edge_id).ok_or(TopologyError::InvalidRequest)?;
        expected.extend(edge.evidence_ids.iter().cloned());
    }

    let actual: BTreeSet<_> = path.evidence_ids.iter().cloned().collect();
    if actual.len() != path.evidence_ids.len()
        || actual != expected
        || path.evidence_ids != expected.iter().cloned().collect::<Vec<_>>()
    {
        return Err(TopologyError::InvalidRequest);
    }
    Ok(())
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
        let mut evidence_content = BTreeSet::new();
        for evidence in &self.evidence {
            if evidence.redaction.unparsed && evidence.redaction.masked {
                return Err(OperationsSnapshotError::Validation(
                    "unparsed evidence cannot be marked masked".into(),
                ));
            }
            let content = serde_json::to_string(&(
                evidence.source_kind,
                &evidence.connector_id,
                &evidence.scope,
                &evidence.endpoint,
                &evidence.query,
                &evidence.observed_at,
                &evidence.excerpt,
                &evidence.native_url,
                &evidence.redaction,
            ))
            .map_err(|_| {
                OperationsSnapshotError::Validation(
                    "evidence content could not be validated".into(),
                )
            })?;
            if !evidence_content.insert(content) {
                return Err(OperationsSnapshotError::Validation(
                    "evidence content must be unique".into(),
                ));
            }
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
            .chain(self.incident_queue.iter().flat_map(|item| {
                [
                    &item.evidence_ids,
                    &item.drill_down.evidence_ids,
                    &item.drill_down_reference.evidence_ids,
                ]
            }))
            .chain(
                self.changes
                    .iter()
                    .flat_map(|change| [&change.evidence_ids, &change.drill_down.evidence_ids]),
            )
            .chain(self.environments.iter().flat_map(|environment| {
                [
                    &environment.evidence_ids,
                    &environment.drill_down.evidence_ids,
                ]
            }))
        {
            if ids.iter().any(|id| !evidence_ids.contains(id.as_str())) {
                return Err(OperationsSnapshotError::Validation(
                    "projection references unknown evidence".into(),
                ));
            }
        }

        for item in &self.incident_queue {
            for ids in [
                &item.evidence_ids,
                &item.drill_down.evidence_ids,
                &item.drill_down_reference.evidence_ids,
            ] {
                if ids.is_empty() {
                    return Err(OperationsSnapshotError::Validation(
                        "queue items require evidence".into(),
                    ));
                }
            }
            if !item
                .evidence_ids
                .iter()
                .any(|id| item.drill_down.evidence_ids.contains(id))
                || !item
                    .evidence_ids
                    .iter()
                    .any(|id| item.drill_down_reference.evidence_ids.contains(id))
            {
                return Err(OperationsSnapshotError::Validation(
                    "queue drill-down must reference its evidence".into(),
                ));
            }
        }
        for change in &self.changes {
            if change.evidence_ids.is_empty()
                || change.drill_down.evidence_ids.is_empty()
                || !change
                    .evidence_ids
                    .iter()
                    .any(|id| change.drill_down.evidence_ids.contains(id))
            {
                return Err(OperationsSnapshotError::Validation(
                    "changes require evidence".into(),
                ));
            }
        }
        for environment in &self.environments {
            if environment.evidence_ids.is_empty()
                || environment.drill_down.evidence_ids.is_empty()
                || !environment
                    .evidence_ids
                    .iter()
                    .any(|id| environment.drill_down.evidence_ids.contains(id))
            {
                return Err(OperationsSnapshotError::Validation(
                    "environments require evidence".into(),
                ));
            }
        }
        for contributing_scope in &self.health_summary.contributing_scopes {
            if contributing_scope.evidence_ids.is_empty() {
                return Err(OperationsSnapshotError::Validation(
                    "contributing scopes require evidence".into(),
                ));
            }
            if contributing_scope
                .evidence_ids
                .iter()
                .any(|id| !evidence_ids.contains(id.as_str()))
            {
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
