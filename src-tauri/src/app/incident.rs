//! Capability-scoped incident commands.
//!
//! Authorization runs in a fixed order before anything else happens:
//! descriptor and capability, unbounded envelope scope, active membership and
//! Principal identity, workspace grant, role permission, then local policy.
//! Only after all of that does a payload get parsed or a target looked up, so
//! a denied caller can never learn whether an incident exists.

use super::*;
use crate::correlation::SourceRecordStore;
use crate::incident::{
    IncidentCommandContext, IncidentService, IncidentServiceError, IncidentSourceResolver,
    SqliteIncidentRepository,
};
use chrono::Utc;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::Value;
use thalassa_domain::{
    BusinessImpact, Incident, IncidentCreateRequest, IncidentDispositionCommand, IncidentId,
    IncidentMutation, IncidentPage, IncidentRoleAssignmentInput, IncidentRoleCommand,
    IncidentSeverityCommand, IncidentTimelinePage, IncidentTransition, IncidentTriggerInput,
    MembershipStatus, ResourceScope,
};
use thalassa_ipc::{
    incident_assign_role_descriptor, incident_create_descriptor, incident_get_descriptor,
    incident_list_descriptor, incident_set_disposition_descriptor,
    incident_set_severity_descriptor, incident_timeline_descriptor, incident_transition_descriptor,
    CommandDescriptor, CommandEnvelope,
};
use thalassa_policy::{DataClass, EgressDestination, EgressRequest};

/// Exact `incident.create` payload keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreatePayload {
    summary: String,
    triggers: Vec<IncidentTriggerInput>,
    business_impact: BusinessImpact,
    initial_roles: Vec<IncidentRoleAssignmentInput>,
}

/// Exact `incident.get` payload keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GetPayload {
    incident_id: IncidentId,
}

/// Exact `incident.list` payload keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ListPayload {
    cursor: Option<String>,
    limit: u16,
}

/// Exact `incident.timeline` payload keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TimelinePayload {
    incident_id: IncidentId,
    after_sequence: Option<u64>,
    limit: u16,
}

/// Exact `incident.transition` payload keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionPayload {
    incident_id: IncidentId,
    expected_version: u64,
    transition: IncidentTransition,
}

/// Exact `incident.set_severity` payload keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeverityPayload {
    incident_id: IncidentId,
    expected_version: u64,
    command: IncidentSeverityCommand,
}

/// Exact `incident.set_disposition` payload keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DispositionPayload {
    incident_id: IncidentId,
    expected_version: u64,
    command: IncidentDispositionCommand,
}

/// Exact `incident.assign_role` payload keys.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RolePayload {
    incident_id: IncidentId,
    expected_version: u64,
    command: IncidentRoleCommand,
}

impl AppState {
    /// Creates one incident from an explicit, fully attributed command.
    pub fn incident_create(&self, envelope: CommandEnvelope<Value>) -> IpcResult<IncidentMutation> {
        let descriptor = incident_create_descriptor();
        let request_id = envelope.request_id;
        let payload = match self.begin_incident_write::<CreatePayload>(&envelope, &descriptor) {
            Ok(payload) => payload,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let mut service = match self.incident_service(true) {
            Ok(service) => service,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = service.create(
            &self.incident_context(request_id),
            IncidentCreateRequest {
                summary: payload.summary,
                triggers: payload.triggers,
                business_impact: payload.business_impact,
                initial_roles: payload.initial_roles,
            },
        );
        self.finish_incident_write(result)
    }

    /// Applies one validated lifecycle transition.
    pub fn incident_transition(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<IncidentMutation> {
        let descriptor = incident_transition_descriptor();
        let request_id = envelope.request_id;
        let payload = match self.begin_incident_write::<TransitionPayload>(&envelope, &descriptor) {
            Ok(payload) => payload,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let mut service = match self.incident_service(false) {
            Ok(service) => service,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = service.transition(
            &self.incident_context(request_id),
            thalassa_domain::IncidentTransitionRequest {
                incident_id: payload.incident_id,
                expected_version: payload.expected_version,
                transition: payload.transition,
            },
        );
        self.finish_incident_write(result)
    }

    /// Reassesses severity or records an explicit attributed override.
    pub fn incident_set_severity(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<IncidentMutation> {
        let descriptor = incident_set_severity_descriptor();
        let request_id = envelope.request_id;
        let payload = match self.begin_incident_write::<SeverityPayload>(&envelope, &descriptor) {
            Ok(payload) => payload,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let mut service = match self.incident_service(false) {
            Ok(service) => service,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = service.set_severity(
            &self.incident_context(request_id),
            thalassa_domain::IncidentSeverityRequest {
                incident_id: payload.incident_id,
                expected_version: payload.expected_version,
                command: payload.command,
            },
        );
        self.finish_incident_write(result)
    }

    /// Sets or clears a disposition without transitioning status.
    pub fn incident_set_disposition(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<IncidentMutation> {
        let descriptor = incident_set_disposition_descriptor();
        let request_id = envelope.request_id;
        let payload = match self.begin_incident_write::<DispositionPayload>(&envelope, &descriptor)
        {
            Ok(payload) => payload,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let mut service = match self.incident_service(false) {
            Ok(service) => service,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = service.set_disposition(
            &self.incident_context(request_id),
            thalassa_domain::IncidentDispositionRequest {
                incident_id: payload.incident_id,
                expected_version: payload.expected_version,
                command: payload.command,
            },
        );
        self.finish_incident_write(result)
    }

    /// Assigns, replaces or releases one responder role.
    pub fn incident_assign_role(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<IncidentMutation> {
        let descriptor = incident_assign_role_descriptor();
        let request_id = envelope.request_id;
        let payload = match self.begin_incident_write::<RolePayload>(&envelope, &descriptor) {
            Ok(payload) => payload,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let mut service = match self.incident_service(false) {
            Ok(service) => service,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = service.assign_role(
            &self.incident_context(request_id),
            thalassa_domain::IncidentRoleRequest {
                incident_id: payload.incident_id,
                expected_version: payload.expected_version,
                command: payload.command,
            },
        );
        self.finish_incident_write(result)
    }

    /// Reads one incident inside the caller's workspace.
    pub fn incident_get(&self, envelope: CommandEnvelope<Value>) -> IpcResult<Incident> {
        let descriptor = incident_get_descriptor();
        let request_id = envelope.request_id;
        let payload = match self.begin_incident_read::<GetPayload>(&envelope, &descriptor) {
            Ok(payload) => payload,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let service = match self.incident_service(false) {
            Ok(service) => service,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = service.get(&self.incident_context(request_id), payload.incident_id);
        self.finish_incident_read(result)
    }

    /// Reads one bounded page of workspace incidents.
    pub fn incident_list(&self, envelope: CommandEnvelope<Value>) -> IpcResult<IncidentPage> {
        let descriptor = incident_list_descriptor();
        let request_id = envelope.request_id;
        let payload = match self.begin_incident_read::<ListPayload>(&envelope, &descriptor) {
            Ok(payload) => payload,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let service = match self.incident_service(false) {
            Ok(service) => service,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = service.list(
            &self.incident_context(request_id),
            payload.cursor.as_deref(),
            payload.limit,
        );
        self.finish_incident_read(result)
    }

    /// Reads one bounded page of an incident's immutable timeline.
    pub fn incident_timeline(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<IncidentTimelinePage> {
        let descriptor = incident_timeline_descriptor();
        let request_id = envelope.request_id;
        let payload = match self.begin_incident_read::<TimelinePayload>(&envelope, &descriptor) {
            Ok(payload) => payload,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let service = match self.incident_service(false) {
            Ok(service) => service,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let result = service.timeline(
            &self.incident_context(request_id),
            payload.incident_id,
            payload.after_sequence,
            payload.limit,
        );
        self.finish_incident_read(result)
    }

    /// Authorization, write policy and strict parsing, in that order.
    fn begin_incident_write<T: serde::de::DeserializeOwned>(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<T, IpcError> {
        self.authorize_incident(envelope, descriptor)?;
        self.authorize_incident_local_storage()?;
        self.authorize_incident_audit_retention()?;
        parse_incident_payload(envelope.payload.clone())
    }

    /// Authorization, read policy and strict parsing, in that order.
    fn begin_incident_read<T: serde::de::DeserializeOwned>(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<T, IpcError> {
        self.authorize_incident(envelope, descriptor)?;
        self.authorize_incident_local_storage()?;
        parse_incident_payload(envelope.payload.clone())
    }

    /// Applies the response policy that guards everything leaving for the UI.
    fn finish_incident_write<T>(&self, result: Result<T, IncidentServiceError>) -> IpcResult<T> {
        self.finish_incident_read(result)
    }

    fn finish_incident_read<T>(&self, result: Result<T, IncidentServiceError>) -> IpcResult<T> {
        match result {
            Ok(value) => match self.authorize_incident_ui_egress() {
                Ok(()) => IpcResult::Ok { ok: true, value },
                Err(error) => IpcResult::Err { ok: false, error },
            },
            Err(error) => IpcResult::Err {
                ok: false,
                error: incident_service_error(error),
            },
        }
    }

    fn authorize_incident(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<(), IpcError> {
        let workspace_scope = self.incident_workspace_scope();
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
            || envelope.scope.is_bounded()
            || !descriptor.scope.contains(&envelope.scope)
            || self.bootstrap.membership.status != MembershipStatus::Active
            || self.bootstrap.membership.principal_id != self.bootstrap.principal.id
            || !self.bootstrap.membership.grants(&workspace_scope)
            || !membership_role_grants_permission(
                &self.bootstrap.membership.role,
                &descriptor.required_permission,
            )
        {
            // The denial names only the command that would have been required:
            // it never echoes payload data or the requested target.
            return Err(IpcError::new(
                IpcErrorCode::PermissionDenied,
                "permission denied",
                serde_json::json!({ "required_command": descriptor.name.to_string() }),
            ));
        }
        Ok(())
    }

    fn authorize_incident_local_storage(&self) -> Result<(), IpcError> {
        if self
            .policy
            .evaluate_egress(EgressRequest::verified(
                DataClass::Internal,
                EgressDestination::LocalStorage,
            ))
            .is_allowed()
        {
            Ok(())
        } else {
            Err(IpcError::new(
                IpcErrorCode::PolicyDenied,
                "incident local storage policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn authorize_incident_audit_retention(&self) -> Result<(), IpcError> {
        if self
            .policy
            .evaluate_egress(EgressRequest::verified(
                DataClass::Internal,
                EgressDestination::AuditLog,
            ))
            .is_allowed()
        {
            Ok(())
        } else {
            Err(IpcError::new(
                IpcErrorCode::PolicyDenied,
                "incident audit retention policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn authorize_incident_ui_egress(&self) -> Result<(), IpcError> {
        if self
            .policy
            .evaluate_egress(EgressRequest::verified(
                DataClass::Internal,
                EgressDestination::Ui,
            ))
            .is_allowed()
        {
            Ok(())
        } else {
            Err(IpcError::new(
                IpcErrorCode::PolicyDenied,
                "incident UI egress policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn incident_workspace_scope(&self) -> ResourceScope {
        ResourceScope::workspace(
            self.bootstrap.workspace.id,
            self.bootstrap.team.id,
            self.bootstrap.organization.id,
        )
    }

    /// Audit values come only from here: the acting Principal, the envelope's
    /// request identifier, the active policy version and the server clock.
    fn incident_context(&self, request_id: uuid::Uuid) -> IncidentCommandContext {
        IncidentCommandContext {
            workspace_scope: self.incident_workspace_scope(),
            actor_id: self.bootstrap.principal.id,
            policy_version: self.policy.version(),
            request_id,
            now: Utc::now(),
        }
    }

    /// Opens the local incident store.  The deterministic trigger index is
    /// built only for creation, which is the one command that cites a source.
    fn incident_service(&self, with_resolver: bool) -> Result<IncidentService, IpcError> {
        let repository = SqliteIncidentRepository::open(&self.database_path)
            .map_err(|_| incident_unavailable())?;
        if !with_resolver {
            return Ok(IncidentService::new(
                IncidentSourceResolver::default(),
                repository,
            ));
        }
        let scope = self.incident_workspace_scope();
        let connection =
            Connection::open(&self.database_path).map_err(|_| incident_unavailable())?;
        let mut records = SourceRecordStore::with_connection_and_scope_and_policy(
            connection,
            scope.clone(),
            self.policy.clone(),
        )
        .map_err(|_| incident_unavailable())?;
        let resolver = IncidentSourceResolver::replay(&scope, &mut records)
            .map_err(|_| incident_unavailable())?;
        Ok(IncidentService::new(resolver, repository))
    }
}

fn parse_incident_payload<T: serde::de::DeserializeOwned>(payload: Value) -> Result<T, IpcError> {
    serde_json::from_value(payload).map_err(|_| {
        IpcError::new(
            IpcErrorCode::InvalidRequest,
            "incident request payload is invalid",
            serde_json::json!({ "reason": "incident_invalid_payload" }),
        )
    })
}

fn incident_unavailable() -> IpcError {
    IpcError::new(
        IpcErrorCode::InternalError,
        "local incident storage is unavailable",
        serde_json::json!({}),
    )
}

fn invalid_incident_request(reason: &str) -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "incident request was rejected",
        serde_json::json!({ "reason": reason }),
    )
}

/// Maps service failures to stable codes and safe reasons.  No mapping copies
/// a database message, a source payload or report text into the response.
fn incident_service_error(error: IncidentServiceError) -> IpcError {
    use thalassa_domain::IncidentError;
    match error {
        IncidentServiceError::NotFound => IpcError::new(
            IpcErrorCode::NotFound,
            "incident was not found in this workspace",
            serde_json::json!({ "reason": "incident_not_found" }),
        ),
        IncidentServiceError::UnknownSource => IpcError::new(
            IpcErrorCode::NotFound,
            "the referenced source was not found locally",
            serde_json::json!({ "reason": "incident_source_not_found" }),
        ),
        IncidentServiceError::VersionConflict { .. } => {
            invalid_incident_request("incident_version_conflict")
        }
        IncidentServiceError::WriteContention {} => incident_unavailable(),
        IncidentServiceError::IdempotencyConflict => {
            invalid_incident_request("incident_idempotency_conflict")
        }
        IncidentServiceError::SourceKindMismatch => {
            invalid_incident_request("incident_source_kind_mismatch")
        }
        IncidentServiceError::UnresolvableSource => {
            invalid_incident_request("incident_source_unresolvable")
        }
        IncidentServiceError::EvidenceMissing => {
            invalid_incident_request("incident_evidence_missing")
        }
        IncidentServiceError::ScopeMismatch => invalid_incident_request("incident_scope_mismatch"),
        IncidentServiceError::SensitiveContent => {
            invalid_incident_request("incident_unsafe_content")
        }
        IncidentServiceError::InvalidRequest => {
            invalid_incident_request("incident_invalid_request")
        }
        IncidentServiceError::Domain(domain) => match domain {
            IncidentError::InvalidEventSequence => IpcError::new(
                IpcErrorCode::InvalidEventSequence,
                "incident event sequence is invalid",
                serde_json::json!({ "reason": "incident_invalid_event_sequence" }),
            ),
            IncidentError::InvalidSeverityOverride => IpcError::new(
                IpcErrorCode::InvalidSeverityOverride,
                "incident severity override is invalid",
                serde_json::json!({ "reason": "incident_invalid_severity_override" }),
            ),
            IncidentError::VersionConflict { .. } => {
                invalid_incident_request("incident_version_conflict")
            }
            other => invalid_incident_request(incident_domain_reason(&other)),
        },
        IncidentServiceError::Store(store) => match store {
            crate::incident::IncidentStoreError::InvalidPagination => {
                invalid_incident_request("incident_invalid_pagination")
            }
            crate::incident::IncidentStoreError::InvalidEventSequence { .. } => IpcError::new(
                IpcErrorCode::InvalidEventSequence,
                "incident event sequence is invalid",
                serde_json::json!({ "reason": "incident_invalid_event_sequence" }),
            ),
            _ => incident_unavailable(),
        },
        IncidentServiceError::Replay(_) | IncidentServiceError::Contract(_) => {
            incident_unavailable()
        }
    }
}

fn incident_domain_reason(error: &thalassa_domain::IncidentError) -> &'static str {
    use thalassa_domain::IncidentError;
    match error {
        IncidentError::UnsafeText => "incident_unsafe_content",
        IncidentError::TextTooLong { .. } => "incident_text_too_long",
        IncidentError::ImpactLevelMismatch => "incident_impact_level_mismatch",
        IncidentError::InvalidEvidence => "incident_invalid_evidence",
        IncidentError::InvalidId => "incident_invalid_id",
        IncidentError::InvalidTrigger => "incident_invalid_trigger",
        IncidentError::InvalidScope => "incident_scope_mismatch",
        IncidentError::InvalidTransition { .. } => "incident_invalid_transition",
        IncidentError::InvalidTransitionContext => "incident_invalid_transition_context",
        IncidentError::InvalidSeverityOverride => "incident_invalid_severity_override",
        IncidentError::InvalidDisposition => "incident_invalid_disposition",
        IncidentError::InvalidDuplicateReference => "incident_invalid_duplicate_reference",
        IncidentError::InvalidRole => "incident_invalid_role",
        IncidentError::VersionConflict { .. } => "incident_version_conflict",
        IncidentError::InvalidEventSequence => "incident_invalid_event_sequence",
        IncidentError::InvalidPagination => "incident_invalid_pagination",
    }
}
