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

#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
thread_local! {
    static OPERATIONS_CATALOG_OVERRIDE: RefCell<Option<FixtureCatalog>> = const { RefCell::new(None) };
}

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
    ///
    /// This intentionally rebuilds the full snapshot because the source is an
    /// in-memory fixture catalog; it does not fetch or query an external source.
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
            return Err(IpcError::new(
                IpcErrorCode::PermissionDenied,
                "permission denied",
                serde_json::json!({ "required_command": descriptor.name.to_string() }),
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
        let catalog = self.operations_catalog();
        OperationsAggregator::from_fixture_catalog(catalog).snapshot_at(fixture_time())
    }

    fn operations_catalog(&self) -> FixtureCatalog {
        let scope = self.workspace_scope();
        #[cfg(test)]
        if let Some(catalog) = operations_catalog_override() {
            return catalog;
        }
        catalog_for_workspace(fixture_catalog(), scope)
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

fn aggregation_ipc_error(error: crate::operations::AggregationError) -> IpcError {
    match error {
        crate::operations::AggregationError::SnapshotInvalid => IpcError::new(
            IpcErrorCode::InternalError,
            "operations snapshot validation failed",
            serde_json::json!({}),
        ),
    }
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

// Stamping every fixture record with the caller's workspace scope is a
// fixture-only adapter for Sprint 11. Never apply it to a live source, where
// scope is authoritative data rather than a function of the caller's identity.
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
        change.drill_down_reference.scope = scope.clone();
    }
    for environment in &mut catalog.environments {
        environment.resource_count.drill_down_reference.scope = scope.clone();
    }
    for evidence in &mut catalog.evidence {
        evidence.scope = scope.clone();
    }
    catalog
}

#[cfg(test)]
// This override exists solely to keep cross-scope denial reachable while the
// source is a fixture catalog; it disappears when a live source supplies
// authoritative scope data.
fn operations_catalog_override() -> Option<FixtureCatalog> {
    OPERATIONS_CATALOG_OVERRIDE.with(|catalog| catalog.borrow().clone())
}

#[cfg(test)]
fn with_operations_catalog_for_test<T>(
    catalog: FixtureCatalog,
    operation: impl FnOnce() -> T,
) -> T {
    struct Restore(Option<FixtureCatalog>);

    impl Drop for Restore {
        fn drop(&mut self) {
            OPERATIONS_CATALOG_OVERRIDE.with(|catalog| {
                catalog.replace(self.0.take());
            });
        }
    }

    let previous = OPERATIONS_CATALOG_OVERRIDE.with(|slot| slot.replace(Some(catalog)));
    let _restore = Restore(previous);
    operation()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::InMemoryCredentialStore;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;
    use thalassa_ipc::{Capability, CommandEnvelope, CommandName, IpcErrorCode};
    use uuid::Uuid;

    fn test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempdir().unwrap();
        let state = AppState::open_with_credential_store(
            directory.path().join("thalassaops.sqlite"),
            Arc::new(InMemoryCredentialStore::default()),
        )
        .unwrap();
        (directory, state)
    }

    fn envelope(
        _state: &AppState,
        verb: &str,
        capability: Capability,
        payload: Value,
    ) -> CommandEnvelope<Value> {
        CommandEnvelope {
            request_id: Uuid::new_v4(),
            command: CommandName::new("operations", verb).unwrap(),
            capability,
            scope: ResourceScope::default(),
            payload,
        }
    }

    #[test]
    fn aggregation_ipc_error_maps_snapshot_validation_to_an_internal_error() {
        let error = aggregation_ipc_error(crate::operations::AggregationError::SnapshotInvalid);

        assert_eq!(error.code, IpcErrorCode::InternalError);
        assert_eq!(error.message, "operations snapshot validation failed");
    }

    #[test]
    fn evidence_command_denies_a_foreign_scope_id_through_the_command_path() {
        let (_directory, state) = test_state();
        let foreign_scope =
            ResourceScope::workspace(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let catalog = catalog_for_workspace(fixture_catalog(), foreign_scope);
        let evidence_id = catalog.evidence[0].id.clone();

        let result = with_operations_catalog_for_test(catalog, || {
            state.operations_evidence(envelope(
                &state,
                "evidence",
                Capability::ResourceRead,
                json!({ "evidence_ids": [evidence_id] }),
            ))
        });

        assert!(
            matches!(result, IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied)
        );
    }
}
