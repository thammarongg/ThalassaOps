//! Incident application services.
//!
//! Creation is explicit and all-or-nothing: every trigger is resolved and
//! screened before the repository transaction opens, so a rejected trigger
//! leaves no incident, no provenance row and no audit event behind.  Audit
//! values (actor, server time, request identifier, policy version) come only
//! from the authorized command context, never from request payload data.

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use thalassa_domain::{
    validate_incident_text, ConsoleEvidenceId, CorrelationError, Incident, IncidentCreateCommand,
    IncidentCreateRequest, IncidentDispositionRequest, IncidentError, IncidentId, IncidentMutation,
    IncidentPage, IncidentReport, IncidentRoleAssignment, IncidentRoleRequest,
    IncidentSeverityRequest, IncidentSourceKind, IncidentTimelinePage, IncidentTransitionRequest,
    IncidentTriggerId, IncidentTriggerInput, PrincipalId, ResourceScope, INCIDENT_SUMMARY_MAXIMUM,
};
use uuid::Uuid;

use crate::correlation::adapters::SignalAdapterError;

use super::repository::{IncidentCreationRecord, IncidentStoreError, SqliteIncidentRepository};
use super::source::{IncidentSourceResolver, ResolvedIncidentTrigger};

/// The authorized context one incident command executes under.  Sprint 15's
/// IPC layer builds this only after descriptor, membership, workspace grant
/// and permission checks have passed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IncidentCommandContext {
    pub workspace_scope: ResourceScope,
    pub actor_id: PrincipalId,
    pub policy_version: u64,
    pub request_id: Uuid,
    pub now: DateTime<Utc>,
}

/// Typed application failures.  No variant carries source payloads, report
/// text or credentials.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum IncidentServiceError {
    #[error("the incident request is not a valid command")]
    InvalidRequest,
    #[error("the referenced source could not be found locally")]
    UnknownSource,
    #[error("the referenced source is not of the requested trigger kind")]
    SourceKindMismatch,
    #[error("the referenced source cannot be resolved into a trigger")]
    UnresolvableSource,
    #[error("the referenced source carries no admitted evidence")]
    EvidenceMissing,
    #[error("the request mixes scopes or leaves the authorized workspace")]
    ScopeMismatch,
    #[error("the submitted text contains sensitive or unsafe content")]
    SensitiveContent,
    #[error("the creation request identifier was reused with different content")]
    IdempotencyConflict,
    #[error("the incident has been changed by another writer")]
    VersionConflict { expected: u64, actual: u64 },
    #[error("the incident was not found in this workspace")]
    NotFound,
    #[error("incident domain validation failed")]
    Domain(#[source] IncidentError),
    #[error("local incident storage failed")]
    Store(#[source] IncidentStoreError),
    #[error("deterministic source replay failed")]
    Replay(#[source] SignalAdapterError),
    #[error("a local contract rejected the replayed source")]
    Contract(#[source] CorrelationError),
}

impl From<IncidentStoreError> for IncidentServiceError {
    fn from(error: IncidentStoreError) -> Self {
        match error {
            IncidentStoreError::NotFound => Self::NotFound,
            IncidentStoreError::IdempotencyConflict => Self::IdempotencyConflict,
            IncidentStoreError::VersionConflict { expected, actual } => {
                Self::VersionConflict { expected, actual }
            }
            other => Self::Store(other),
        }
    }
}

impl From<IncidentError> for IncidentServiceError {
    fn from(error: IncidentError) -> Self {
        match error {
            IncidentError::UnsafeText => Self::SensitiveContent,
            IncidentError::InvalidScope => Self::ScopeMismatch,
            other => Self::Domain(other),
        }
    }
}

/// Application service over the incident aggregate, the local trigger
/// resolver and the local store.
pub struct IncidentService {
    resolver: IncidentSourceResolver,
    repository: SqliteIncidentRepository,
}

impl IncidentService {
    pub fn new(resolver: IncidentSourceResolver, repository: SqliteIncidentRepository) -> Self {
        Self {
            resolver,
            repository,
        }
    }

    /// The local trigger index backing this service.
    pub fn resolver(&self) -> &IncidentSourceResolver {
        &self.resolver
    }

    /// Total stored incidents.  Used to prove that a rejected command, a
    /// replay or a projection wrote nothing.
    pub fn incident_count(&self) -> Result<u64, IncidentServiceError> {
        Ok(self.repository.incident_count()?)
    }

    /// Creates one incident from an explicit, fully resolved command.
    pub fn create(
        &mut self,
        context: &IncidentCommandContext,
        request: IncidentCreateRequest,
    ) -> Result<IncidentMutation, IncidentServiceError> {
        if request.triggers.is_empty() {
            return Err(IncidentServiceError::InvalidRequest);
        }
        if context.request_id.is_nil() || context.actor_id.is_nil() {
            return Err(IncidentServiceError::InvalidRequest);
        }
        if context.workspace_scope.workspace_id.is_none()
            || context.workspace_scope.team_id.is_none()
            || context.workspace_scope.organization_id.is_none()
        {
            return Err(IncidentServiceError::ScopeMismatch);
        }
        // The fingerprint digests the untrusted request, not the resolved
        // command: a retry of the same request must produce the same value so
        // idempotent creation returns the stored incident instead of a
        // conflict.
        let request_fingerprint = fingerprint(&request)?;

        // Every trigger resolves before the write transaction opens.
        let mut triggers = Vec::with_capacity(request.triggers.len());
        for input in &request.triggers {
            let resolved = self.resolve_input(context, input)?;
            let id = trigger_id(
                context.request_id,
                resolved.source_kind,
                &resolved.source_id,
            );
            triggers.push(resolved.into_trigger(id));
        }

        let owning_team_id = context
            .workspace_scope
            .team_id
            .ok_or(IncidentServiceError::ScopeMismatch)?;
        let initial_roles: Vec<IncidentRoleAssignment> = request
            .initial_roles
            .iter()
            .map(|role| IncidentRoleAssignment {
                role: role.role,
                principal_id: role.principal_id,
                assigned_by: context.actor_id,
                assigned_at: context.now,
            })
            .collect();

        let command = IncidentCreateCommand {
            summary: request.summary.clone(),
            scope: context.workspace_scope.clone(),
            owning_team_id,
            triggers: triggers.clone(),
            business_impact: request.business_impact.clone(),
            initial_roles,
        };
        let mutation = Incident::create(
            command,
            context.actor_id,
            context.request_id,
            context.policy_version,
            context.now,
        )?;

        Ok(self.repository.create(IncidentCreationRecord {
            mutation,
            triggers,
            request_fingerprint,
        })?)
    }

    /// Applies one validated lifecycle transition.
    pub fn transition(
        &mut self,
        context: &IncidentCommandContext,
        request: IncidentTransitionRequest,
    ) -> Result<IncidentMutation, IncidentServiceError> {
        let (incident, first_event_sequence) =
            self.load_for_write(context, request.incident_id, request.expected_version)?;
        let mutation = incident.transition(
            request.expected_version,
            first_event_sequence,
            request.transition,
            context.actor_id,
            context.request_id,
            context.policy_version,
            context.now,
        )?;
        Ok(self.repository.apply_mutation(mutation)?)
    }

    /// Reassesses severity from a changed impact assessment, or records an
    /// explicit attributed override.
    pub fn set_severity(
        &mut self,
        context: &IncidentCommandContext,
        request: IncidentSeverityRequest,
    ) -> Result<IncidentMutation, IncidentServiceError> {
        let (incident, first_event_sequence) =
            self.load_for_write(context, request.incident_id, request.expected_version)?;
        let mutation = incident.set_severity(
            request.expected_version,
            first_event_sequence,
            request.command,
            context.actor_id,
            context.request_id,
            context.policy_version,
            context.now,
        )?;
        Ok(self.repository.apply_mutation(mutation)?)
    }

    /// Sets or clears a disposition.  A disposition never transitions status
    /// and never merges or closes an incident.
    pub fn set_disposition(
        &mut self,
        context: &IncidentCommandContext,
        request: IncidentDispositionRequest,
    ) -> Result<IncidentMutation, IncidentServiceError> {
        let (incident, first_event_sequence) =
            self.load_for_write(context, request.incident_id, request.expected_version)?;
        if matches!(
            request.command.disposition,
            Some(thalassa_domain::IncidentDisposition::Duplicate)
        ) {
            if let Some(duplicate_of) = request.command.duplicate_of_incident_id {
                // Duplicate references are resolved in the caller workspace before the
                // aggregate can append the immutable disposition event.
                self.repository
                    .get(self.workspace(context)?, duplicate_of)?;
            }
        }
        let mutation = incident.set_disposition(
            request.expected_version,
            first_event_sequence,
            request.command,
            context.actor_id,
            context.request_id,
            context.policy_version,
            context.now,
        )?;
        Ok(self.repository.apply_mutation(mutation)?)
    }

    /// Assigns, replaces or releases one responder role.
    pub fn assign_role(
        &mut self,
        context: &IncidentCommandContext,
        request: IncidentRoleRequest,
    ) -> Result<IncidentMutation, IncidentServiceError> {
        let (incident, first_event_sequence) =
            self.load_for_write(context, request.incident_id, request.expected_version)?;
        let mutation = incident.assign_role(
            request.expected_version,
            first_event_sequence,
            request.command,
            context.actor_id,
            context.request_id,
            context.policy_version,
            context.now,
        )?;
        Ok(self.repository.apply_mutation(mutation)?)
    }

    /// Reads one incident inside the caller's workspace.
    pub fn get(
        &self,
        context: &IncidentCommandContext,
        incident_id: IncidentId,
    ) -> Result<Incident, IncidentServiceError> {
        Ok(self.repository.get(self.workspace(context)?, incident_id)?)
    }

    /// Reads one bounded page of workspace incidents, newest update first.
    pub fn list(
        &self,
        context: &IncidentCommandContext,
        cursor: Option<&str>,
        limit: u16,
    ) -> Result<IncidentPage, IncidentServiceError> {
        Ok(self
            .repository
            .list(self.workspace(context)?, cursor, limit)?)
    }

    /// Reads one bounded, ordered page of an incident's immutable timeline.
    pub fn timeline(
        &self,
        context: &IncidentCommandContext,
        incident_id: IncidentId,
        after_sequence: Option<u64>,
        limit: u16,
    ) -> Result<IncidentTimelinePage, IncidentServiceError> {
        Ok(self.repository.timeline(
            self.workspace(context)?,
            incident_id,
            after_sequence,
            limit,
        )?)
    }

    fn workspace(&self, context: &IncidentCommandContext) -> Result<Uuid, IncidentServiceError> {
        context
            .workspace_scope
            .workspace_id
            .filter(|id| !id.is_nil())
            .ok_or(IncidentServiceError::ScopeMismatch)
    }

    /// Loads the current aggregate for a write and allocates the sequence its
    /// first appended event will take.  The version is checked here so a stale
    /// writer is rejected before the aggregate builds any event.
    fn load_for_write(
        &self,
        context: &IncidentCommandContext,
        incident_id: IncidentId,
        expected_version: u64,
    ) -> Result<(Incident, u64), IncidentServiceError> {
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
        let highest = self
            .repository
            .highest_event_sequence(workspace_id, incident_id)?;
        let first_event_sequence = highest
            .checked_add(1)
            .ok_or(IncidentServiceError::InvalidRequest)?;
        Ok((incident, first_event_sequence))
    }

    fn resolve_input(
        &self,
        context: &IncidentCommandContext,
        input: &IncidentTriggerInput,
    ) -> Result<ResolvedIncidentTrigger, IncidentServiceError> {
        match input {
            IncidentTriggerInput::Alert { source_id } => self.resolver.resolve(
                IncidentSourceKind::Alert,
                source_id,
                &context.workspace_scope,
            ),
            IncidentTriggerInput::Anomaly { source_id } => self.resolver.resolve(
                IncidentSourceKind::Anomaly,
                source_id,
                &context.workspace_scope,
            ),
            IncidentTriggerInput::ScheduledHealthCheck { source_id } => self.resolver.resolve(
                IncidentSourceKind::ScheduledHealthCheck,
                source_id,
                &context.workspace_scope,
            ),
            IncidentTriggerInput::VulnerabilityFinding { source_id } => self.resolver.resolve(
                IncidentSourceKind::VulnerabilityFinding,
                source_id,
                &context.workspace_scope,
            ),
            IncidentTriggerInput::UserReport {
                reporter_id,
                observed_at,
                summary,
                scope,
            } => resolve_report(
                IncidentSourceKind::UserReport,
                *reporter_id,
                *observed_at,
                summary,
                scope,
                &context.workspace_scope,
            ),
            IncidentTriggerInput::ManualReport {
                observed_at,
                summary,
                scope,
            } => resolve_report(
                // A manual report is attributed to the responder who opened
                // the incident; the reporter is never taken from the payload.
                IncidentSourceKind::ManualReport,
                context.actor_id,
                *observed_at,
                summary,
                scope,
                &context.workspace_scope,
            ),
        }
    }
}

/// Screens and bounds one structured report, then derives its local identity.
fn resolve_report(
    kind: IncidentSourceKind,
    reporter_id: PrincipalId,
    observed_at: DateTime<Utc>,
    summary: &str,
    scope: &ResourceScope,
    workspace_scope: &ResourceScope,
) -> Result<ResolvedIncidentTrigger, IncidentServiceError> {
    if reporter_id.is_nil() {
        return Err(IncidentServiceError::InvalidRequest);
    }
    if !workspace_scope.contains(scope) {
        return Err(IncidentServiceError::ScopeMismatch);
    }
    // Control characters, credential markers and oversized text are rejected
    // here, before anything reaches the immutable timeline.
    validate_incident_text(summary, INCIDENT_SUMMARY_MAXIMUM)
        .map_err(|_| IncidentServiceError::SensitiveContent)?;

    let report = IncidentReport {
        reporter_id: Some(reporter_id),
        summary: summary.to_owned(),
    };
    // A report is its own evidence: the identifier addresses the sanitized
    // report stored on the trigger row, and is derived only from that content.
    let digest = digest_hex(&[
        source_kind_wire(kind).as_bytes(),
        reporter_id.as_bytes(),
        observed_at.to_rfc3339().as_bytes(),
        summary.as_bytes(),
    ]);
    let short = &digest[..16];
    let source_id = format!("{}-{short}", source_kind_wire(kind).replace('_', "-"));
    let evidence_id: ConsoleEvidenceId = format!("evidence-{source_id}");

    Ok(ResolvedIncidentTrigger {
        source_kind: kind,
        source_id,
        source_record_digest: None,
        scope: scope.clone(),
        observed_at,
        signal_id: None,
        evidence_ids: vec![evidence_id],
        report: Some(report),
    })
}

/// Deterministic trigger identity: the same request resolving the same source
/// always names the same trigger, so a retry cannot mint a second identity.
fn trigger_id(request_id: Uuid, kind: IncidentSourceKind, source_id: &str) -> IncidentTriggerId {
    let name = format!("{}|{source_id}", source_kind_wire(kind));
    Uuid::new_v5(&request_id, name.as_bytes())
}

/// Lowercase SHA-256 digest of the canonical serialized creation request.
fn fingerprint(request: &IncidentCreateRequest) -> Result<String, IncidentServiceError> {
    let canonical =
        serde_json::to_vec(request).map_err(|_| IncidentServiceError::InvalidRequest)?;
    Ok(format!("sha256:{}", digest_hex(&[&canonical])))
}

fn digest_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part);
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn source_kind_wire(kind: IncidentSourceKind) -> &'static str {
    match kind {
        IncidentSourceKind::Alert => "alert",
        IncidentSourceKind::Anomaly => "anomaly",
        IncidentSourceKind::UserReport => "user_report",
        IncidentSourceKind::ScheduledHealthCheck => "scheduled_health_check",
        IncidentSourceKind::VulnerabilityFinding => "vulnerability_finding",
        IncidentSourceKind::ManualReport => "manual_report",
    }
}
