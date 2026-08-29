//! Capability-scoped, read-only signal correlation IPC commands.
//!
//! The app boundary owns command, capability, workspace and policy checks.
//! Correlation itself remains a provider-neutral projection over deterministic
//! replay inputs; no provider URL, query, connector selector or mutation is
//! accepted at this boundary.

use super::*;
use crate::correlation::adapters::{normalize_operational, normalize_security};
use crate::correlation::{
    correlate_signals_with_records, correlation_fixture_catalog, fixture_time,
    CorrelationFixtureCatalog, CorrelationInput, SignalAdapterError, SourceRecordError,
    SourceRecordStore,
};
use crate::topology::{topology_fixture_input, TopologyBuilder};
use rusqlite::Connection;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use thalassa_domain::{
    CorrelationError, CorrelationEvidenceRequest, CorrelationRequest, CorrelationSnapshot,
    EvidenceRef, EvidenceSourceKind, MembershipStatus, ResourceScope, SourceState, SourceStatus,
    StatusReason,
};
use thalassa_ipc::{
    correlation_evidence_descriptor, correlation_snapshot_descriptor, CommandDescriptor,
    CommandEnvelope,
};
use thalassa_policy::{DataClass, EgressDestination, EgressRequest};

impl AppState {
    /// Return the deterministic, source-preserving correlation projection.
    pub fn correlation_snapshot(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<CorrelationSnapshot> {
        let descriptor = correlation_snapshot_descriptor();
        if let Err(error) = self.authorize_correlation(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let request = match parse_correlation_request(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = self.authorize_correlation_source_policy() {
            return IpcResult::Err { ok: false, error };
        }
        if let Err(error) = self.authorize_correlation_audit_retention() {
            return IpcResult::Err { ok: false, error };
        }

        let snapshot = match self.build_correlation_snapshot(&request) {
            Ok(snapshot) => snapshot,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = self.authorize_correlation_ui_egress() {
            return IpcResult::Err { ok: false, error };
        }
        IpcResult::Ok {
            ok: true,
            value: snapshot,
        }
    }

    /// Resolve only backend-issued evidence IDs from the current validated
    /// correlation snapshot. The complete snapshot is built and validated
    /// before any evidence is returned.
    pub fn correlation_evidence(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<EvidenceRef>> {
        let descriptor = correlation_evidence_descriptor();
        if let Err(error) = self.authorize_correlation(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let request = match parse_correlation_evidence_request(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = self.authorize_correlation_source_policy() {
            return IpcResult::Err { ok: false, error };
        }
        if let Err(error) = self.authorize_correlation_audit_retention() {
            return IpcResult::Err { ok: false, error };
        }

        // Rebuild the same deterministic source projection before resolving
        // IDs. This keeps evidence lookup closed over the current snapshot;
        // it cannot become an arbitrary source-record or native-ID lookup.
        let snapshot = match self.build_correlation_snapshot_for_evidence(&request) {
            Ok(snapshot) => snapshot,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        let evidence_by_id = snapshot
            .evidence
            .iter()
            .cloned()
            .map(|evidence| (evidence.id.clone(), evidence))
            .collect::<BTreeMap<_, _>>();
        let mut evidence = Vec::with_capacity(request.evidence_ids.len());
        for evidence_id in &request.evidence_ids {
            let Some(reference) = evidence_by_id.get(evidence_id) else {
                return IpcResult::Err {
                    ok: false,
                    error: correlation_evidence_not_found(),
                };
            };
            if !self
                .correlation_workspace_scope()
                .contains(&reference.scope)
            {
                return IpcResult::Err {
                    ok: false,
                    error: correlation_evidence_scope_denied(),
                };
            }
            if !reference.redaction.classification_verified
                || !reference.redaction.redaction_verified
                || (reference.redaction.unparsed && reference.redaction.masked)
            {
                return IpcResult::Err {
                    ok: false,
                    error: correlation_evidence_policy_denied(),
                };
            }
            evidence.push(reference.clone());
        }
        if let Err(error) = self.authorize_correlation_ui_egress() {
            return IpcResult::Err { ok: false, error };
        }
        IpcResult::Ok {
            ok: true,
            value: evidence,
        }
    }

    fn authorize_correlation(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<(), IpcError> {
        let workspace_scope = self.correlation_workspace_scope();
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

    fn authorize_correlation_source_policy(&self) -> Result<(), IpcError> {
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
                "correlation local source retention policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn authorize_correlation_audit_retention(&self) -> Result<(), IpcError> {
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
                "correlation audit retention policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn authorize_correlation_ui_egress(&self) -> Result<(), IpcError> {
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
                "correlation UI egress policy denied",
                serde_json::json!({}),
            ))
        }
    }

    fn correlation_workspace_scope(&self) -> ResourceScope {
        ResourceScope::workspace(
            self.bootstrap.workspace.id,
            self.bootstrap.team.id,
            self.bootstrap.organization.id,
        )
    }

    fn build_correlation_snapshot_for_evidence(
        &self,
        _request: &CorrelationEvidenceRequest,
    ) -> Result<CorrelationSnapshot, IpcError> {
        // Evidence requests do not carry a source query or window, so use the
        // same explicit fixture request used by the read-only workspace. The
        // IDs are still checked against this newly validated snapshot.
        self.build_correlation_snapshot(&default_correlation_request())
    }

    fn build_correlation_snapshot(
        &self,
        request: &CorrelationRequest,
    ) -> Result<CorrelationSnapshot, IpcError> {
        let scope = self.correlation_workspace_scope();
        let catalog = catalog_for_workspace(correlation_fixture_catalog(), scope.clone());
        if let Err(error) = catalog.validate() {
            return Err(correlation_ipc_error(error));
        }

        let connection = Connection::open(&self.database_path).map_err(|error| {
            source_record_ipc_error(SourceRecordError::Database(error.to_string()))
        })?;
        let mut records = SourceRecordStore::with_connection_and_scope_and_policy(
            connection,
            scope.clone(),
            self.policy.clone(),
        )
        .map_err(source_record_ipc_error)?;
        let mut signals = Vec::new();
        let mut source_status = Vec::new();
        for fixture in &catalog.fixtures {
            let normalized = match fixture.source_kind {
                EvidenceSourceKind::Alertmanager
                | EvidenceSourceKind::Prometheus
                | EvidenceSourceKind::HealthCheck => normalize_operational(fixture, &mut records),
                EvidenceSourceKind::Trivy
                | EvidenceSourceKind::Falco
                | EvidenceSourceKind::Kyverno
                | EvidenceSourceKind::OpaGatekeeper => normalize_security(fixture, &mut records),
                _ => Err(SignalAdapterError::UnsupportedSource),
            };
            match normalized {
                Ok(admitted) => signals.extend(admitted),
                Err(error) => source_status.push(source_status_for(fixture.source_kind, &error)),
            }
        }

        let evidence = records.evidence_refs().cloned().collect::<Vec<_>>();
        let topology = TopologyBuilder::from_input(topology_fixture_input(scope.clone()));
        let input = CorrelationInput {
            generated_at: fixture_time().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            scope,
            request: request.clone(),
            signals,
            source_status,
            evidence,
            prior_window: None,
            suppression_rules: catalog.suppression_rules,
            maintenance_windows: catalog.maintenance_windows,
            policy_version: self.policy.version(),
        };
        correlate_signals_with_records(input, &records, &topology).map_err(correlation_ipc_error)
    }
}

fn default_correlation_request() -> CorrelationRequest {
    CorrelationRequest {
        window: thalassa_domain::TimeWindow {
            start: "2026-08-28T08:55:00Z".into(),
            end: "2026-08-28T09:05:00Z".into(),
        },
        evaluated_at: "2026-08-28T09:00:00Z".into(),
        allowed_lateness_seconds: 300,
    }
}

fn catalog_for_workspace(
    mut catalog: CorrelationFixtureCatalog,
    scope: ResourceScope,
) -> CorrelationFixtureCatalog {
    for fixture in &mut catalog.fixtures {
        fixture.scope = scope.clone();
        for evidence in &mut fixture.evidence {
            evidence.scope = scope.clone();
        }
    }
    for rule in &mut catalog.suppression_rules {
        rule.scope = scope.clone();
    }
    for window in &mut catalog.maintenance_windows {
        window.scope = scope.clone();
    }
    catalog
}

fn parse_correlation_request(payload: Value) -> Result<CorrelationRequest, IpcError> {
    let Value::Object(fields) = payload else {
        return Err(invalid_correlation_request());
    };
    if !has_exact_keys(
        &fields,
        ["window", "evaluated_at", "allowed_lateness_seconds"],
    ) {
        return Err(invalid_correlation_request());
    }
    let Some(Value::Object(window)) = fields.get("window") else {
        return Err(invalid_correlation_request());
    };
    if !has_exact_keys(window, ["start", "end"]) {
        return Err(invalid_correlation_request());
    }
    let request: CorrelationRequest =
        serde_json::from_value(Value::Object(fields)).map_err(|_| invalid_correlation_request())?;
    request.validate().map_err(correlation_ipc_error)?;
    Ok(request)
}

fn parse_correlation_evidence_request(
    payload: Value,
) -> Result<CorrelationEvidenceRequest, IpcError> {
    let Value::Object(fields) = payload else {
        return Err(invalid_correlation_evidence_request());
    };
    if !has_exact_keys(&fields, ["evidence_ids"]) {
        return Err(invalid_correlation_evidence_request());
    }
    let request: CorrelationEvidenceRequest = serde_json::from_value(Value::Object(fields))
        .map_err(|_| invalid_correlation_evidence_request())?;
    request
        .validate()
        .map_err(|_| invalid_correlation_evidence_request())?;
    Ok(request)
}

fn has_exact_keys<const N: usize>(fields: &Map<String, Value>, expected: [&str; N]) -> bool {
    fields.len() == N && expected.iter().all(|key| fields.contains_key(*key))
}

fn invalid_correlation_request() -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "correlation request payload is malformed",
        serde_json::json!({}),
    )
}

fn invalid_correlation_evidence_request() -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "correlation evidence request payload is malformed",
        serde_json::json!({}),
    )
}

fn correlation_evidence_not_found() -> IpcError {
    IpcError::new(
        IpcErrorCode::NotFound,
        "correlation evidence was not emitted by the snapshot",
        serde_json::json!({}),
    )
}

fn correlation_evidence_scope_denied() -> IpcError {
    IpcError::new(
        IpcErrorCode::PermissionDenied,
        "correlation evidence scope is not accessible",
        serde_json::json!({}),
    )
}

fn correlation_evidence_policy_denied() -> IpcError {
    IpcError::new(
        IpcErrorCode::PolicyDenied,
        "correlation evidence verification policy denied",
        serde_json::json!({}),
    )
}

fn source_status_for(source: EvidenceSourceKind, error: &SignalAdapterError) -> SourceStatus {
    // Preserve the typed adapter/source failure as a safe, non-payload detail
    // for diagnostics. The UI maps the status/reason enums to localized copy
    // and intentionally does not render this backend message.
    let detail = Some(signal_adapter_ipc_error(error.clone()).message);
    let (state, reason) = match error {
        SignalAdapterError::UnsupportedSource | SignalAdapterError::UnsupportedSecuritySource => {
            (SourceState::Unavailable, StatusReason::NotConfigured)
        }
        SignalAdapterError::Source(SourceRecordError::PolicyDenied) => {
            (SourceState::Unverified, StatusReason::PolicyDenied)
        }
        SignalAdapterError::Source(SourceRecordError::ScopeMismatch)
        | SignalAdapterError::Source(SourceRecordError::InvalidEvidence)
        | SignalAdapterError::Source(SourceRecordError::SourceMismatch) => {
            (SourceState::Unverified, StatusReason::PolicyDenied)
        }
        _ => (SourceState::Unverified, StatusReason::Unknown),
    };
    SourceStatus {
        source_key: source_kind_wire(source).to_owned(),
        state,
        reason: Some(reason),
        detail,
        observed_at: None,
        evidence_ids: Vec::new(),
    }
}

fn source_kind_wire(source: EvidenceSourceKind) -> &'static str {
    match source {
        EvidenceSourceKind::Alertmanager => "alertmanager",
        EvidenceSourceKind::Prometheus => "prometheus",
        EvidenceSourceKind::Kubernetes => "kubernetes",
        EvidenceSourceKind::Cloud => "cloud",
        EvidenceSourceKind::HealthCheck => "health_check",
        EvidenceSourceKind::Fixture => "fixture",
        EvidenceSourceKind::Trivy => "trivy",
        EvidenceSourceKind::Falco => "falco",
        EvidenceSourceKind::Kyverno => "kyverno",
        EvidenceSourceKind::OpaGatekeeper => "opa_gatekeeper",
    }
}

fn correlation_ipc_error(error: CorrelationError) -> IpcError {
    let (code, message) = match error {
        CorrelationError::InvalidId => (
            IpcErrorCode::InvalidRequest,
            "correlation identifier is invalid",
        ),
        CorrelationError::InvalidTimestamp => (
            IpcErrorCode::InvalidRequest,
            "correlation timestamp is invalid",
        ),
        CorrelationError::InvalidWindow => (
            IpcErrorCode::InvalidRequest,
            "correlation window is invalid",
        ),
        CorrelationError::WindowOutOfRange => (
            IpcErrorCode::InvalidRequest,
            "correlation window exceeds the allowed range",
        ),
        CorrelationError::LatenessOutOfRange => (
            IpcErrorCode::InvalidRequest,
            "correlation lateness exceeds the allowed range",
        ),
        CorrelationError::NonFiniteNumber(field) => (
            IpcErrorCode::InvalidRequest,
            match field {
                thalassa_domain::CorrelationNumberField::ObservedValue => {
                    "correlation observed value is not finite"
                }
                thalassa_domain::CorrelationNumberField::ComparisonValue => {
                    "correlation comparison value is not finite"
                }
                thalassa_domain::CorrelationNumberField::CvssScore => {
                    "correlation CVSS score is not finite"
                }
                thalassa_domain::CorrelationNumberField::MetricValue => {
                    "correlation metric value is not finite"
                }
            },
        ),
        CorrelationError::CvssOutOfRange => (
            IpcErrorCode::InvalidRequest,
            "correlation CVSS score is outside the allowed range",
        ),
        CorrelationError::EvidenceMissing => (
            IpcErrorCode::NotFound,
            "correlation evidence reference is missing",
        ),
        CorrelationError::InvalidEvidence => (
            IpcErrorCode::PolicyDenied,
            "correlation evidence failed verification",
        ),
        CorrelationError::PayloadKindMismatch => (
            IpcErrorCode::MalformedResponse,
            "correlation signal payload kind is unsupported",
        ),
        CorrelationError::SourceMismatch => (
            IpcErrorCode::MalformedResponse,
            "correlation source identity is inconsistent",
        ),
        CorrelationError::UnsupportedFindingSource => (
            IpcErrorCode::MalformedResponse,
            "correlation finding source is unsupported",
        ),
        CorrelationError::TargetMismatch => (
            IpcErrorCode::MalformedResponse,
            "correlation target does not match its finding",
        ),
        CorrelationError::InvalidReason => (
            IpcErrorCode::InternalError,
            "correlation reason validation failed",
        ),
        CorrelationError::CandidateTooSmall => (
            IpcErrorCode::InternalError,
            "correlation candidate does not meet its minimum size",
        ),
        CorrelationError::CandidateReferenceMissing => (
            IpcErrorCode::InternalError,
            "correlation candidate reference is missing",
        ),
        CorrelationError::CandidateStatusMismatch => (
            IpcErrorCode::InternalError,
            "correlation candidate status is inconsistent",
        ),
        CorrelationError::ScopeMismatch => (
            IpcErrorCode::PermissionDenied,
            "correlation value is outside the workspace scope",
        ),
        CorrelationError::WindowMismatch => (
            IpcErrorCode::InternalError,
            "correlation window does not match its request",
        ),
        CorrelationError::MetricUnitMismatch => (
            IpcErrorCode::InternalError,
            "correlation metric unit is unsupported",
        ),
        CorrelationError::MetricValueOutOfRange => (
            IpcErrorCode::InternalError,
            "correlation metric value is outside the allowed range",
        ),
        CorrelationError::SuppressionMismatch => (
            IpcErrorCode::InternalError,
            "correlation suppression state is inconsistent",
        ),
        CorrelationError::InvalidPayload => (
            IpcErrorCode::MalformedResponse,
            "correlation source payload is malformed",
        ),
        CorrelationError::InvalidTopologyPath => (
            IpcErrorCode::InternalError,
            "correlation topology path validation failed",
        ),
        CorrelationError::DuplicateId => (
            IpcErrorCode::InvalidRequest,
            "correlation identifier is duplicated",
        ),
    };
    IpcError::new(code, message, serde_json::json!({}))
}

fn signal_adapter_ipc_error(error: SignalAdapterError) -> IpcError {
    match error {
        SignalAdapterError::Fixture(error) | SignalAdapterError::Signal(error) => {
            correlation_ipc_error(error)
        }
        SignalAdapterError::Source(error) => source_record_ipc_error(error),
        SignalAdapterError::SourceMismatch => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "correlation adapter source does not match its fixture",
            serde_json::json!({}),
        ),
        SignalAdapterError::UnsupportedSource => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "correlation adapter does not support this operational source",
            serde_json::json!({}),
        ),
        SignalAdapterError::UnsupportedSecuritySource => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "correlation adapter does not support this security source",
            serde_json::json!({}),
        ),
        SignalAdapterError::UnsupportedSchema => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "correlation adapter schema is unsupported",
            serde_json::json!({}),
        ),
        SignalAdapterError::AmbiguousTarget => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "correlation adapter target is ambiguous",
            serde_json::json!({}),
        ),
        SignalAdapterError::UnsafeIdentity => IpcError::new(
            IpcErrorCode::PolicyDenied,
            "correlation adapter identity failed safety policy",
            serde_json::json!({}),
        ),
        SignalAdapterError::InvalidSeverity => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "correlation adapter severity is malformed",
            serde_json::json!({}),
        ),
        SignalAdapterError::InvalidExploitability => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "correlation adapter exploitability is malformed",
            serde_json::json!({}),
        ),
        SignalAdapterError::CvssOutOfRange => IpcError::new(
            IpcErrorCode::InvalidRequest,
            "correlation adapter CVSS score is outside the allowed range",
            serde_json::json!({}),
        ),
        SignalAdapterError::MalformedPayload => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "correlation operational payload is malformed",
            serde_json::json!({}),
        ),
        SignalAdapterError::InvalidNumber => IpcError::new(
            IpcErrorCode::InvalidRequest,
            "correlation operational number is invalid",
            serde_json::json!({}),
        ),
        SignalAdapterError::InvalidTimestamp => IpcError::new(
            IpcErrorCode::InvalidRequest,
            "correlation operational timestamp is invalid",
            serde_json::json!({}),
        ),
    }
}

fn source_record_ipc_error(error: SourceRecordError) -> IpcError {
    let (code, message) = match error {
        SourceRecordError::InvalidScope => (
            IpcErrorCode::PermissionDenied,
            "correlation source record scope is invalid",
        ),
        SourceRecordError::ScopeMismatch => (
            IpcErrorCode::PermissionDenied,
            "correlation source record is outside the workspace scope",
        ),
        SourceRecordError::EvidenceMissing => (
            IpcErrorCode::NotFound,
            "correlation source record evidence is missing",
        ),
        SourceRecordError::InvalidEvidence => (
            IpcErrorCode::PolicyDenied,
            "correlation source record evidence failed verification",
        ),
        SourceRecordError::SourceMismatch => (
            IpcErrorCode::MalformedResponse,
            "correlation source record source is inconsistent",
        ),
        SourceRecordError::DuplicateEvidence => (
            IpcErrorCode::InvalidRequest,
            "correlation source record evidence ID is duplicated",
        ),
        SourceRecordError::UnsafeIdentity => (
            IpcErrorCode::PolicyDenied,
            "correlation source record identity failed safety policy",
        ),
        SourceRecordError::InvalidTimestamp => (
            IpcErrorCode::InvalidRequest,
            "correlation source record timestamp is invalid",
        ),
        SourceRecordError::InvalidPayload => (
            IpcErrorCode::MalformedResponse,
            "correlation source record payload is malformed",
        ),
        SourceRecordError::AmbiguousSourceIdentity => (
            IpcErrorCode::MalformedResponse,
            "correlation source record identity is ambiguous",
        ),
        SourceRecordError::PolicyDenied => (
            IpcErrorCode::PolicyDenied,
            "correlation source record retention policy denied",
        ),
        SourceRecordError::Contract(error) => return correlation_ipc_error(error),
        SourceRecordError::Database(_) => (
            IpcErrorCode::InternalError,
            "correlation source record storage failed",
        ),
    };
    IpcError::new(code, message, serde_json::json!({}))
}

#[allow(dead_code)]
fn topology_ipc_error(error: thalassa_domain::TopologyError) -> IpcError {
    let (code, message) = match error {
        thalassa_domain::TopologyError::InvalidRequest => (
            IpcErrorCode::InvalidRequest,
            "correlation topology request is invalid",
        ),
        thalassa_domain::TopologyError::NodeNotFound => (
            IpcErrorCode::NotFound,
            "correlation topology node was not found",
        ),
        thalassa_domain::TopologyError::IncidentNotFound => (
            IpcErrorCode::NotFound,
            "correlation topology incident was not found",
        ),
        thalassa_domain::TopologyError::ScopeDenied => (
            IpcErrorCode::PermissionDenied,
            "correlation topology scope was denied",
        ),
        thalassa_domain::TopologyError::EvidenceUnverified => (
            IpcErrorCode::PolicyDenied,
            "correlation topology evidence was not verified",
        ),
        thalassa_domain::TopologyError::EvidenceMissing => (
            IpcErrorCode::NotFound,
            "correlation topology evidence was not found",
        ),
        thalassa_domain::TopologyError::NonFiniteNumber(field) => (
            IpcErrorCode::InternalError,
            match field {
                thalassa_domain::TopologyNumberField::MetricValue => {
                    "correlation topology metric value is not finite"
                }
                thalassa_domain::TopologyNumberField::EdgeConfidence => {
                    "correlation topology edge confidence is not finite"
                }
                thalassa_domain::TopologyNumberField::PathConfidence => {
                    "correlation topology path confidence is not finite"
                }
            },
        ),
        thalassa_domain::TopologyError::ConfidenceOutOfRange => (
            IpcErrorCode::InternalError,
            "correlation topology confidence is outside the allowed range",
        ),
        thalassa_domain::TopologyError::MalformedSource => (
            IpcErrorCode::MalformedResponse,
            "correlation topology source projection is malformed",
        ),
    };
    IpcError::new(code, message, serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use thalassa_domain::{CorrelationNumberField, TopologyError, TopologyNumberField};

    #[test]
    fn every_correlation_error_variant_has_a_distinct_safe_message() {
        let errors = vec![
            CorrelationError::InvalidId,
            CorrelationError::InvalidTimestamp,
            CorrelationError::InvalidWindow,
            CorrelationError::WindowOutOfRange,
            CorrelationError::LatenessOutOfRange,
            CorrelationError::NonFiniteNumber(CorrelationNumberField::ObservedValue),
            CorrelationError::NonFiniteNumber(CorrelationNumberField::ComparisonValue),
            CorrelationError::NonFiniteNumber(CorrelationNumberField::CvssScore),
            CorrelationError::NonFiniteNumber(CorrelationNumberField::MetricValue),
            CorrelationError::CvssOutOfRange,
            CorrelationError::EvidenceMissing,
            CorrelationError::InvalidEvidence,
            CorrelationError::PayloadKindMismatch,
            CorrelationError::SourceMismatch,
            CorrelationError::UnsupportedFindingSource,
            CorrelationError::TargetMismatch,
            CorrelationError::InvalidReason,
            CorrelationError::CandidateTooSmall,
            CorrelationError::CandidateReferenceMissing,
            CorrelationError::CandidateStatusMismatch,
            CorrelationError::ScopeMismatch,
            CorrelationError::WindowMismatch,
            CorrelationError::MetricUnitMismatch,
            CorrelationError::MetricValueOutOfRange,
            CorrelationError::SuppressionMismatch,
            CorrelationError::InvalidPayload,
            CorrelationError::InvalidTopologyPath,
            CorrelationError::DuplicateId,
        ];
        let mapped = errors
            .into_iter()
            .map(correlation_ipc_error)
            .collect::<Vec<_>>();
        let messages = mapped
            .iter()
            .map(|error| error.message.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(messages.len(), mapped.len());
        assert!(mapped
            .iter()
            .all(|error| error.details == serde_json::json!({})));
    }

    #[test]
    fn source_and_adapter_errors_keep_distinct_categories() {
        let adapter = signal_adapter_ipc_error(SignalAdapterError::UnsupportedSchema);
        assert_eq!(adapter.code, IpcErrorCode::MalformedResponse);
        assert_eq!(adapter.message, "correlation adapter schema is unsupported");
        let source = source_record_ipc_error(SourceRecordError::PolicyDenied);
        assert_eq!(source.code, IpcErrorCode::PolicyDenied);
        assert_eq!(
            source.message,
            "correlation source record retention policy denied"
        );
    }

    #[test]
    fn topology_error_mapping_has_variant_specific_messages() {
        let node = topology_ipc_error(TopologyError::NodeNotFound);
        let evidence = topology_ipc_error(TopologyError::EvidenceMissing);
        let number = topology_ipc_error(TopologyError::NonFiniteNumber(
            TopologyNumberField::MetricValue,
        ));
        assert_ne!(node.message, evidence.message);
        assert_ne!(evidence.message, number.message);
    }
}
