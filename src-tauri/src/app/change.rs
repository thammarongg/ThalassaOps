//! Capability-scoped, read-only change intelligence IPC commands.
//!
//! Both commands are projections over committed replay fixtures. There is no
//! ingest, adapter trigger, provider query or change mutation at this
//! boundary: a change is context a responder reads, never state the app
//! writes.

use super::*;
use crate::change::{adapters, association, fixtures as change_fixtures, metrics, timeline};
use crate::correlation::{SourceRecordError, SourceRecordStore};
use crate::topology::{topology_fixture_input, TopologyBuilder};
use chrono::DateTime;
use rusqlite::Connection;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use thalassa_domain::{
    ChangeError, ChangeEvent, ChangeEvidenceRequest, ChangeRequest, ChangeSnapshot, EvidenceRef,
    MembershipStatus, ResourceScope, SourceState, SourceStatus, StatusReason,
};
use thalassa_ipc::{
    change_evidence_descriptor, change_snapshot_descriptor, CommandDescriptor, CommandEnvelope,
};
use thalassa_policy::{DataClass, EgressDestination, EgressRequest};

impl AppState {
    /// Return the deterministic, source-preserving change projection.
    pub fn change_snapshot(&self, envelope: CommandEnvelope<Value>) -> IpcResult<ChangeSnapshot> {
        let descriptor = change_snapshot_descriptor();
        if let Err(error) = self.authorize_change(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let request = match parse_change_request(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = self.authorize_change_source_policy() {
            return IpcResult::Err { ok: false, error };
        }
        if let Err(error) = self.authorize_change_audit_retention() {
            return IpcResult::Err { ok: false, error };
        }

        let snapshot = match self.build_change_snapshot(&request) {
            Ok(snapshot) => snapshot,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = self.authorize_change_ui_egress() {
            return IpcResult::Err { ok: false, error };
        }
        IpcResult::Ok {
            ok: true,
            value: snapshot,
        }
    }

    /// Resolve only backend-issued evidence IDs present in the current change
    /// snapshot. This is not a native record retrieval path.
    pub fn change_evidence(&self, envelope: CommandEnvelope<Value>) -> IpcResult<Vec<EvidenceRef>> {
        let descriptor = change_evidence_descriptor();
        if let Err(error) = self.authorize_change(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let request = match parse_change_evidence_request(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = self.authorize_change_source_policy() {
            return IpcResult::Err { ok: false, error };
        }
        if let Err(error) = self.authorize_change_audit_retention() {
            return IpcResult::Err { ok: false, error };
        }

        // Rebuild and validate the same deterministic projection before
        // resolving any ID, so evidence lookup stays closed over the current
        // snapshot instead of becoming a source-record query.
        let (snapshot, evidence_by_id) = match self.change_evidence_index() {
            Ok(indexed) => indexed,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let mut evidence = Vec::with_capacity(request.evidence_ids.len());
        for evidence_id in &request.evidence_ids {
            let Some(reference) = evidence_by_id.get(evidence_id) else {
                return IpcResult::Err {
                    ok: false,
                    error: change_evidence_not_found(),
                };
            };
            if !snapshot.scope.contains(&reference.scope) {
                return IpcResult::Err {
                    ok: false,
                    error: change_evidence_scope_denied(),
                };
            }
            if !reference.redaction.classification_verified
                || !reference.redaction.redaction_verified
                || (reference.redaction.unparsed && reference.redaction.masked)
            {
                return IpcResult::Err {
                    ok: false,
                    error: change_evidence_policy_denied(),
                };
            }
            evidence.push(reference.clone());
        }
        if let Err(error) = self.authorize_change_ui_egress() {
            return IpcResult::Err { ok: false, error };
        }
        IpcResult::Ok {
            ok: true,
            value: evidence,
        }
    }

    fn authorize_change(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<(), IpcError> {
        let workspace_scope = self.change_workspace_scope();
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
            return Err(IpcError::new(
                IpcErrorCode::PermissionDenied,
                "permission denied",
                serde_json::json!({ "required_command": descriptor.name.to_string() }),
            ));
        }
        Ok(())
    }

    fn authorize_change_source_policy(&self) -> Result<(), IpcError> {
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
                "change local source retention policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn authorize_change_audit_retention(&self) -> Result<(), IpcError> {
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
                "change audit retention policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn authorize_change_ui_egress(&self) -> Result<(), IpcError> {
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
                "change UI egress policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn change_workspace_scope(&self) -> ResourceScope {
        ResourceScope::workspace(
            self.bootstrap.workspace.id,
            self.bootstrap.team.id,
            self.bootstrap.organization.id,
        )
    }

    fn change_evidence_index(
        &self,
    ) -> Result<(ChangeSnapshot, BTreeMap<String, EvidenceRef>), IpcError> {
        // Evidence requests carry no window, so the snapshot is rebuilt from
        // the same explicit request the read-only workspace view uses.
        let request = default_change_request();
        let scope = self.change_workspace_scope();
        let (snapshot, records) = self.build_change_snapshot_with_records(&request)?;
        let evidence_by_id = records
            .evidence_refs()
            .filter(|evidence| scope.contains(&evidence.scope))
            .cloned()
            .map(|evidence| (evidence.id.clone(), evidence))
            .collect::<BTreeMap<_, _>>();
        let known_ids = snapshot
            .events
            .iter()
            .flat_map(|event| event.source_record.evidence_ids.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>();
        let evidence_by_id = evidence_by_id
            .into_iter()
            .filter(|(id, _)| known_ids.contains(id))
            .collect();
        Ok((snapshot, evidence_by_id))
    }

    fn build_change_snapshot(&self, request: &ChangeRequest) -> Result<ChangeSnapshot, IpcError> {
        self.build_change_snapshot_with_records(request)
            .map(|(snapshot, _)| snapshot)
    }

    fn build_change_snapshot_with_records(
        &self,
        request: &ChangeRequest,
    ) -> Result<(ChangeSnapshot, SourceRecordStore), IpcError> {
        let scope = self.change_workspace_scope();
        let connection = Connection::open(&self.database_path).map_err(|error| {
            source_record_change_error(SourceRecordError::Database(error.to_string()))
        })?;
        let mut records = SourceRecordStore::with_connection_and_scope_and_policy(
            connection,
            scope.clone(),
            self.policy.clone(),
        )
        .map_err(source_record_change_error)?;

        let clock = change_fixtures::fixture_clock();
        let mut events = Vec::new();
        let mut source_statuses = Vec::new();
        for fixture in change_fixtures::catalog() {
            // Replaying one fixture at a time keeps a denied or malformed
            // record from erasing the healthy ones: it is omitted and reported
            // through a typed source status instead.
            match adapters::replay_from(vec![fixture], &mut records, &scope, clock) {
                Ok(output) => {
                    events.extend(output.events);
                    source_statuses.extend(output.statuses);
                }
                Err(error) => source_statuses.push(change_source_status(&fixture, &error)),
            }
        }
        events.sort_by(|left, right| {
            change_timestamp(&left.occurred_at)
                .cmp(&change_timestamp(&right.occurred_at))
                .then_with(|| left.id.cmp(&right.id))
        });

        let timeline = timeline::build(&events, &request.window, request.limit as usize)
            .map_err(change_ipc_error)?;
        let in_window = timeline
            .entry_ids
            .iter()
            .filter_map(|entry_id| events.iter().find(|event| event.id == *entry_id))
            .cloned()
            .collect::<Vec<ChangeEvent>>();

        let correlation = self.correlation_context()?;
        let topology = TopologyBuilder::from_input(topology_fixture_input(scope.clone()));
        let associations = association::associate(
            &in_window,
            &correlation.candidates,
            &correlation.signals,
            request.lookback_seconds as f64,
            &topology,
        )
        .map_err(change_ipc_error)?;
        let metrics = metrics::build(&in_window, &associations, &scope);

        let snapshot = ChangeSnapshot {
            generated_at: request.evaluated_at.clone(),
            scope,
            request_window: request.window.clone(),
            lookback_seconds: request.lookback_seconds,
            events,
            timeline,
            associations,
            metrics,
            source_statuses,
        };
        snapshot.validate().map_err(change_ipc_error)?;
        Ok((snapshot, records))
    }
}

/// The explicit fixture request used when no window is supplied.
fn default_change_request() -> ChangeRequest {
    ChangeRequest {
        window: thalassa_domain::TimeWindow {
            start: "2026-08-28T08:00:00Z".into(),
            end: "2026-08-28T09:00:00Z".into(),
        },
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        lookback_seconds: 3_600,
        limit: 50,
    }
}

fn change_timestamp(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
}

fn change_source_status(
    fixture: &crate::change::fixtures::ChangeFixture,
    error: &ChangeError,
) -> SourceStatus {
    // The typed failure is preserved as a safe, non-payload detail. React maps
    // the state and reason enums to localized copy and does not render this
    // backend message.
    let detail = Some(change_ipc_error(error.clone()).message);
    let (state, reason) = match error {
        ChangeError::PolicyDenied | ChangeError::UnsafeIdentity => {
            (SourceState::Unverified, StatusReason::PolicyDenied)
        }
        ChangeError::ScopeMismatch | ChangeError::InvalidEvidence => {
            (SourceState::Unverified, StatusReason::PolicyDenied)
        }
        ChangeError::MalformedPayload | ChangeError::InvalidSourceRecord => {
            (SourceState::Unverified, StatusReason::Unknown)
        }
        _ => (SourceState::Unverified, StatusReason::Unknown),
    };
    SourceStatus {
        source_key: format!("change-source-{}", safe_status_component(fixture.path)),
        state,
        reason: Some(reason),
        detail,
        observed_at: None,
        evidence_ids: Vec::new(),
    }
}

fn safe_status_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn parse_change_request(payload: Value) -> Result<ChangeRequest, IpcError> {
    let Value::Object(fields) = payload else {
        return Err(invalid_change_request());
    };
    if !has_exact_change_keys(
        &fields,
        ["window", "evaluated_at", "lookback_seconds", "limit"],
    ) {
        return Err(invalid_change_request());
    }
    let Some(Value::Object(window)) = fields.get("window") else {
        return Err(invalid_change_request());
    };
    if !has_exact_change_keys(window, ["start", "end"]) {
        return Err(invalid_change_request());
    }
    let request: ChangeRequest =
        serde_json::from_value(Value::Object(fields)).map_err(|_| invalid_change_request())?;
    request.validate().map_err(change_ipc_error)?;
    Ok(request)
}

fn parse_change_evidence_request(payload: Value) -> Result<ChangeEvidenceRequest, IpcError> {
    let Value::Object(fields) = payload else {
        return Err(invalid_change_evidence_request());
    };
    if !has_exact_change_keys(&fields, ["evidence_ids"]) {
        return Err(invalid_change_evidence_request());
    }
    let request: ChangeEvidenceRequest = serde_json::from_value(Value::Object(fields))
        .map_err(|_| invalid_change_evidence_request())?;
    request
        .validate()
        .map_err(|_| invalid_change_evidence_request())?;
    Ok(request)
}

fn has_exact_change_keys<const N: usize>(fields: &Map<String, Value>, expected: [&str; N]) -> bool {
    fields.len() == N && expected.iter().all(|key| fields.contains_key(*key))
}

fn invalid_change_request() -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "change request payload is malformed",
        serde_json::json!({}),
    )
}

fn invalid_change_evidence_request() -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "change evidence request payload is malformed",
        serde_json::json!({}),
    )
}

fn change_evidence_not_found() -> IpcError {
    IpcError::new(
        IpcErrorCode::NotFound,
        "change evidence was not emitted by the snapshot",
        serde_json::json!({}),
    )
}

fn change_evidence_scope_denied() -> IpcError {
    IpcError::new(
        IpcErrorCode::PermissionDenied,
        "change evidence is outside the workspace scope",
        serde_json::json!({}),
    )
}

fn change_evidence_policy_denied() -> IpcError {
    IpcError::new(
        IpcErrorCode::PolicyDenied,
        "change evidence failed verification",
        serde_json::json!({}),
    )
}

fn source_record_change_error(error: SourceRecordError) -> IpcError {
    let (code, message) = match error {
        SourceRecordError::InvalidScope | SourceRecordError::ScopeMismatch => (
            IpcErrorCode::PermissionDenied,
            "change source record is outside the workspace scope",
        ),
        SourceRecordError::PolicyDenied | SourceRecordError::UnsafeIdentity => (
            IpcErrorCode::PolicyDenied,
            "change source record failed retention policy",
        ),
        SourceRecordError::InvalidPayload | SourceRecordError::SourceMismatch => (
            IpcErrorCode::MalformedResponse,
            "change source record is inconsistent",
        ),
        _ => (
            IpcErrorCode::InternalError,
            "change source record store is unavailable",
        ),
    };
    IpcError::new(code, message, serde_json::json!({}))
}

fn change_ipc_error(error: ChangeError) -> IpcError {
    let (code, message) = match error {
        ChangeError::InvalidWindow | ChangeError::WindowMismatch => {
            (IpcErrorCode::InvalidRequest, "change window is invalid")
        }
        ChangeError::InvalidLookback => (
            IpcErrorCode::InvalidRequest,
            "change lookback exceeds the allowed range",
        ),
        ChangeError::InvalidLimit => (
            IpcErrorCode::InvalidRequest,
            "change limit exceeds the allowed range",
        ),
        ChangeError::InvalidTimestamp | ChangeError::MissingTimestamp => {
            (IpcErrorCode::InvalidRequest, "change timestamp is invalid")
        }
        ChangeError::InvalidId | ChangeError::DuplicateId => {
            (IpcErrorCode::InvalidRequest, "change identifier is invalid")
        }
        ChangeError::NonFiniteNumber
        | ChangeError::NegativeNumber
        | ChangeError::InvalidUnit
        | ChangeError::InvalidMetric => (
            IpcErrorCode::InvalidRequest,
            "change metric value is invalid",
        ),
        ChangeError::PolicyDenied => (
            IpcErrorCode::PolicyDenied,
            "change source policy denied the record",
        ),
        ChangeError::UnsafeIdentity
        | ChangeError::InvalidActor
        | ChangeError::InvalidLink
        | ChangeError::InvalidPath
        | ChangeError::InvalidRepository
        | ChangeError::InvalidRevision
        | ChangeError::InvalidTarget => (
            IpcErrorCode::PolicyDenied,
            "change record failed identity safety policy",
        ),
        ChangeError::ScopeMismatch => (
            IpcErrorCode::PermissionDenied,
            "change record is outside the workspace scope",
        ),
        ChangeError::MalformedPayload
        | ChangeError::InvalidSourceRecord
        | ChangeError::SourceMismatch
        | ChangeError::InvalidSourceStatus => (
            IpcErrorCode::MalformedResponse,
            "change fixture payload does not match the contract",
        ),
        ChangeError::EvidenceMissing
        | ChangeError::InvalidEvidence
        | ChangeError::CandidateReferenceMissing
        | ChangeError::InvalidAssociation
        | ChangeError::InvalidTimeline => (
            IpcErrorCode::InternalError,
            "change snapshot failed its evidence or ordering invariant",
        ),
    };
    IpcError::new(code, message, serde_json::json!({}))
}
