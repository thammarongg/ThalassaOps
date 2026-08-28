//! Capability-scoped Operations Console IPC commands.
//!
//! The command boundary owns envelope, membership, role and policy checks.
//! Provider-neutral aggregation remains in `crate::operations` and receives
//! no user-supplied provider URL, query, connector selector or mutation.

use super::*;
use crate::operations::{
    fixture_catalog, fixture_time, EvidenceError, EvidenceStore, FixtureCatalog,
    OperationsAggregator, OperationsEvidenceRequest,
};
use serde_json::Value;
use thalassa_domain::{MembershipStatus, OperationsSnapshot, ResourceScope};
use thalassa_ipc::{
    operations_evidence_descriptor, operations_snapshot_descriptor, CommandDescriptor,
    CommandEnvelope,
};

impl AppState {
    /// Return the deterministic, redacted Operations Console snapshot.
    pub fn operations_snapshot(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<OperationsSnapshot> {
        let descriptor = operations_snapshot_descriptor();
        if let Err(error) = self.authorize_operations(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }
        if !envelope.payload.is_null() {
            return IpcResult::Err {
                ok: false,
                error: invalid_operations_request(),
            };
        }
        if let Err(error) = self.authorize_audit_retention() {
            return IpcResult::Err { ok: false, error };
        }

        let snapshot = match self.build_operations_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: aggregation_ipc_error(error),
                }
            }
        };
        if let Err(error) = self.authorize_ui_egress() {
            return IpcResult::Err { ok: false, error };
        }
        IpcResult::Ok {
            ok: true,
            value: snapshot,
        }
    }

    /// Return only evidence IDs emitted by the current workspace snapshot.
    pub fn operations_evidence(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<thalassa_domain::EvidenceRef>> {
        let descriptor = operations_evidence_descriptor();
        if let Err(error) = self.authorize_operations(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }
        let request = match parse_evidence_request(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = self.authorize_audit_retention() {
            return IpcResult::Err { ok: false, error };
        }

        let snapshot = match self.build_operations_snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: aggregation_ipc_error(error),
                }
            }
        };
        let store = EvidenceStore::from_snapshot(&snapshot);
        let evidence = match store.get_for_scope(&request.evidence_ids, &self.workspace_scope()) {
            Ok(evidence) => evidence,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: evidence_ipc_error(error),
                }
            }
        };
        if let Err(error) = self.authorize_ui_egress() {
            return IpcResult::Err { ok: false, error };
        }
        IpcResult::Ok {
            ok: true,
            value: evidence,
        }
    }

    fn authorize_operations(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<(), IpcError> {
        let workspace_scope = self.workspace_scope();
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
            return Err(IpcError::permission_denied(
                descriptor.name.to_string(),
                envelope.scope.clone(),
            ));
        }
        Ok(())
    }

    fn authorize_ui_egress(&self) -> Result<(), IpcError> {
        if self
            .policy
            .evaluate_egress(thalassa_policy::EgressRequest::verified(
                thalassa_policy::DataClass::Internal,
                thalassa_policy::EgressDestination::Ui,
            ))
            .is_allowed()
        {
            Ok(())
        } else {
            Err(IpcError::new(
                IpcErrorCode::PolicyDenied,
                "policy denied operations response",
                serde_json::json!({}),
            ))
        }
    }

    fn authorize_audit_retention(&self) -> Result<(), IpcError> {
        if self
            .policy
            .evaluate_egress(thalassa_policy::EgressRequest::verified(
                thalassa_policy::DataClass::Internal,
                thalassa_policy::EgressDestination::AuditLog,
            ))
            .is_allowed()
        {
            Ok(())
        } else {
            Err(IpcError::new(
                IpcErrorCode::PolicyDenied,
                "policy denied operations audit retention",
                serde_json::json!({}),
            ))
        }
    }

    fn workspace_scope(&self) -> ResourceScope {
        ResourceScope::workspace(
            self.bootstrap.workspace.id,
            self.bootstrap.team.id,
            self.bootstrap.organization.id,
        )
    }

    fn build_operations_snapshot(
        &self,
    ) -> Result<OperationsSnapshot, crate::operations::AggregationError> {
        let scope = self.workspace_scope();
        let catalog = catalog_for_workspace(fixture_catalog(), scope);
        OperationsAggregator::from_fixture_catalog(catalog).snapshot_at(fixture_time())
    }
}

fn parse_evidence_request(payload: Value) -> Result<OperationsEvidenceRequest, IpcError> {
    let Value::Object(fields) = payload else {
        return Err(invalid_operations_request());
    };
    if fields.len() != 1 || !fields.contains_key("evidence_ids") {
        return Err(invalid_operations_request());
    }
    serde_json::from_value(Value::Object(fields)).map_err(|_| invalid_operations_request())
}

fn invalid_operations_request() -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "invalid operations request payload",
        serde_json::json!({}),
    )
}

fn aggregation_ipc_error(_error: crate::operations::AggregationError) -> IpcError {
    IpcError::new(
        IpcErrorCode::MalformedResponse,
        "malformed operations response",
        serde_json::json!({}),
    )
}

fn evidence_ipc_error(error: EvidenceError) -> IpcError {
    let (code, message) = match error {
        EvidenceError::EmptyRequest | EvidenceError::DuplicateId => {
            (IpcErrorCode::InvalidRequest, "invalid evidence request")
        }
        EvidenceError::UnknownId => (IpcErrorCode::NotFound, "evidence not found"),
        EvidenceError::CrossScope => (IpcErrorCode::PermissionDenied, "evidence access denied"),
        EvidenceError::Unverified => (IpcErrorCode::PolicyDenied, "evidence policy denied"),
    };
    IpcError::new(code, message, serde_json::json!({}))
}

fn catalog_for_workspace(mut catalog: FixtureCatalog, scope: ResourceScope) -> FixtureCatalog {
    for metric in &mut catalog.metrics {
        metric.scope = scope.clone();
    }
    for rule in &mut catalog.anomaly_rules {
        rule.scope = scope.clone();
    }
    for schedule in &mut catalog.health_checks {
        schedule.scope = scope.clone();
    }
    for change in &mut catalog.changes {
        change.scope = scope.clone();
    }
    for environment in &mut catalog.environments {
        environment.resource_count.drill_down_reference.scope = scope.clone();
    }
    for evidence in &mut catalog.evidence {
        evidence.scope = scope.clone();
    }
    catalog
}
