// SPDX-License-Identifier: Apache-2.0

//! Provider-neutral domain contracts shared by the Rust core and adapters.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
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
