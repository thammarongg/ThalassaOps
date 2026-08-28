//! Capability-scoped, read-only topology IPC commands.
//!
//! The command boundary owns envelope, membership, role and policy checks.
//! Provider-neutral graph construction remains in `crate::topology` and never
//! receives a provider URL, query, connector selector or mutation.

use super::*;
use crate::topology::evidence::TopologyEvidenceStore;
use crate::topology::{
    default_topology_request, topology_fixture_input, validate_topology_request, TopologyBuilder,
    TopologyInput,
};
use serde_json::{Map, Value};
use thalassa_domain::{
    MembershipStatus, ResourceScope, TopologyError, TopologyEvidenceRequest, TopologyRequest,
    TopologySnapshot,
};
use thalassa_ipc::{
    topology_evidence_descriptor, topology_snapshot_descriptor, CommandDescriptor, CommandEnvelope,
};

#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
thread_local! {
    static TOPOLOGY_INPUT_OVERRIDE: RefCell<Option<TopologyInput>> = const { RefCell::new(None) };
}

impl AppState {
    /// Return the deterministic, filtered and redacted topology projection.
    pub fn topology_snapshot(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<TopologySnapshot> {
        let descriptor = topology_snapshot_descriptor();
        if let Err(error) = self.authorize_topology(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let request = match parse_topology_request(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = request.validate() {
            return IpcResult::Err {
                ok: false,
                error: topology_ipc_error(error),
            };
        }
        if let Err(error) = self.authorize_topology_source_policy() {
            return IpcResult::Err { ok: false, error };
        }

        let input = self.topology_input();
        if !self.topology_workspace_scope().contains(&input.scope) {
            return IpcResult::Err {
                ok: false,
                error: topology_ipc_error(TopologyError::ScopeDenied),
            };
        }
        if let Err(error) = validate_topology_request(&request, &input) {
            return IpcResult::Err {
                ok: false,
                error: topology_ipc_error(error),
            };
        }
        let snapshot = match TopologyBuilder::from_input(input).snapshot_at(&request) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: topology_ipc_error(error),
                }
            }
        };
        if let Err(error) = self.authorize_topology_ui_egress() {
            return IpcResult::Err { ok: false, error };
        }
        IpcResult::Ok {
            ok: true,
            value: snapshot,
        }
    }

    /// Return only evidence IDs admitted by the current topology snapshot.
    pub fn topology_evidence(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<thalassa_domain::EvidenceRef>> {
        let descriptor = topology_evidence_descriptor();
        if let Err(error) = self.authorize_topology(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let request = match parse_topology_evidence_request(envelope.payload) {
            Ok(request) => request,
            Err(error) => return IpcResult::Err { ok: false, error },
        };
        if let Err(error) = request.validate() {
            return IpcResult::Err {
                ok: false,
                error: topology_ipc_error(error),
            };
        }
        if let Err(error) = self.authorize_topology_source_policy() {
            return IpcResult::Err { ok: false, error };
        }

        let input = self.topology_input();
        if !self.topology_workspace_scope().contains(&input.scope) {
            return IpcResult::Err {
                ok: false,
                error: topology_ipc_error(TopologyError::ScopeDenied),
            };
        }
        let snapshot =
            match TopologyBuilder::from_input(input).snapshot_at(&default_topology_request()) {
                Ok(snapshot) => snapshot,
                Err(error) => {
                    return IpcResult::Err {
                        ok: false,
                        error: topology_ipc_error(error),
                    }
                }
            };
        let store = TopologyEvidenceStore::from_snapshot(&snapshot);
        let evidence = match store.get_for_scope(&request, &self.topology_workspace_scope()) {
            Ok(evidence) => evidence,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: topology_ipc_error(error),
                }
            }
        };
        if let Err(error) = self.authorize_topology_ui_egress() {
            return IpcResult::Err { ok: false, error };
        }
        IpcResult::Ok {
            ok: true,
            value: evidence,
        }
    }

    fn authorize_topology(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<(), IpcError> {
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
        {
            return Err(topology_permission_denied(descriptor));
        }
        if envelope.scope.is_bounded() || !descriptor.scope.contains(&envelope.scope) {
            return Err(topology_permission_denied(descriptor));
        }
        if self.bootstrap.membership.status != MembershipStatus::Active
            || self.bootstrap.membership.principal_id != self.bootstrap.principal.id
        {
            return Err(topology_permission_denied(descriptor));
        }
        if !self
            .bootstrap
            .membership
            .grants(&self.topology_workspace_scope())
            || !membership_role_grants_permission(
                &self.bootstrap.membership.role,
                &descriptor.required_permission,
            )
        {
            return Err(topology_permission_denied(descriptor));
        }
        Ok(())
    }

    fn authorize_topology_source_policy(&self) -> Result<(), IpcError> {
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
                "policy denied topology source retention",
                serde_json::json!({}),
            ))
        }
    }

    fn authorize_topology_ui_egress(&self) -> Result<(), IpcError> {
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
                "policy denied topology response",
                serde_json::json!({}),
            ))
        }
    }

    fn topology_workspace_scope(&self) -> ResourceScope {
        ResourceScope::workspace(
            self.bootstrap.workspace.id,
            self.bootstrap.team.id,
            self.bootstrap.organization.id,
        )
    }

    fn topology_input(&self) -> TopologyInput {
        #[cfg(test)]
        if let Some(input) = topology_input_override() {
            return input;
        }
        topology_fixture_input(self.topology_workspace_scope())
    }
}

fn parse_topology_request(payload: Value) -> Result<TopologyRequest, IpcError> {
    let Value::Object(fields) = payload else {
        return Err(invalid_topology_request());
    };
    if !has_exact_keys(&fields, ["filter", "focus_node_id", "traversal"]) {
        return Err(invalid_topology_request());
    }
    let Some(Value::Object(filter)) = fields.get("filter") else {
        return Err(invalid_topology_request());
    };
    if !has_exact_keys(filter, ["environment_ids", "team_ids", "incident_id"]) {
        return Err(invalid_topology_request());
    }
    let Some(Value::Object(traversal)) = fields.get("traversal") else {
        return Err(invalid_topology_request());
    };
    if !has_exact_keys(traversal, ["direction", "max_depth"]) {
        return Err(invalid_topology_request());
    }
    serde_json::from_value(Value::Object(fields)).map_err(|_| invalid_topology_request())
}

fn parse_topology_evidence_request(payload: Value) -> Result<TopologyEvidenceRequest, IpcError> {
    let Value::Object(fields) = payload else {
        return Err(invalid_topology_request());
    };
    if !has_exact_keys(&fields, ["evidence_ids"]) {
        return Err(invalid_topology_request());
    }
    serde_json::from_value(Value::Object(fields)).map_err(|_| invalid_topology_request())
}

fn has_exact_keys<const N: usize>(fields: &Map<String, Value>, expected: [&str; N]) -> bool {
    fields.len() == N && expected.iter().all(|key| fields.contains_key(*key))
}

fn invalid_topology_request() -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "invalid topology request payload",
        serde_json::json!({}),
    )
}

fn topology_permission_denied(descriptor: &CommandDescriptor) -> IpcError {
    IpcError::new(
        IpcErrorCode::PermissionDenied,
        "permission denied",
        serde_json::json!({ "required_command": descriptor.name.to_string() }),
    )
}

fn topology_ipc_error(error: TopologyError) -> IpcError {
    let (code, message) = match error {
        TopologyError::InvalidRequest => (IpcErrorCode::InvalidRequest, "invalid topology request"),
        TopologyError::NodeNotFound => (IpcErrorCode::NotFound, "topology node not found"),
        TopologyError::IncidentNotFound => (
            IpcErrorCode::NotFound,
            "topology incident queue item not found",
        ),
        TopologyError::ScopeDenied => (IpcErrorCode::PermissionDenied, "topology scope denied"),
        TopologyError::EvidenceUnverified => (
            IpcErrorCode::PolicyDenied,
            "topology evidence is not verified",
        ),
        TopologyError::EvidenceMissing => (IpcErrorCode::NotFound, "topology evidence not found"),
        TopologyError::NonFiniteNumber(field) => (
            IpcErrorCode::InternalError,
            match field {
                thalassa_domain::TopologyNumberField::MetricValue => {
                    "topology metric value is not finite"
                }
                thalassa_domain::TopologyNumberField::EdgeConfidence => {
                    "topology edge confidence is not finite"
                }
                thalassa_domain::TopologyNumberField::PathConfidence => {
                    "topology path confidence is not finite"
                }
            },
        ),
        TopologyError::ConfidenceOutOfRange => (
            IpcErrorCode::InternalError,
            "topology confidence is outside the allowed range",
        ),
        TopologyError::MalformedSource => (
            IpcErrorCode::InternalError,
            "topology source projection is malformed",
        ),
    };
    IpcError::new(code, message, serde_json::json!({}))
}

#[cfg(test)]
fn topology_input_override() -> Option<TopologyInput> {
    TOPOLOGY_INPUT_OVERRIDE.with(|input| input.borrow().clone())
}

#[cfg(test)]
fn with_topology_input_for_test<T>(input: TopologyInput, operation: impl FnOnce() -> T) -> T {
    struct Restore(Option<TopologyInput>);

    impl Drop for Restore {
        fn drop(&mut self) {
            TOPOLOGY_INPUT_OVERRIDE.with(|input| {
                input.replace(self.0.take());
            });
        }
    }

    let previous = TOPOLOGY_INPUT_OVERRIDE.with(|slot| slot.replace(Some(input)));
    let _restore = Restore(previous);
    operation()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::InMemoryCredentialStore;
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use tempfile::tempdir;
    use thalassa_domain::{TopologyFilter, TopologyNumberField, TopologyTraversal};
    use thalassa_ipc::{Capability, CommandName};
    use thalassa_policy::{DataClass, PolicyDocument, PolicyRuntime};
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

    fn request_value() -> Value {
        serde_json::to_value(default_topology_request()).unwrap()
    }

    fn envelope(verb: &str, capability: Capability, payload: Value) -> CommandEnvelope<Value> {
        CommandEnvelope {
            request_id: Uuid::new_v4(),
            command: CommandName::new("topology", verb).unwrap(),
            capability,
            scope: ResourceScope::default(),
            payload,
        }
    }

    #[test]
    fn snapshot_command_returns_a_valid_workspace_projection() {
        let (_directory, state) = test_state();
        let result = state.topology_snapshot(envelope(
            "snapshot",
            Capability::WorkspaceRead,
            request_value(),
        ));
        let IpcResult::Ok { value, .. } = result else {
            panic!("topology snapshot should succeed")
        };
        assert_eq!(
            value.scope,
            ResourceScope::workspace(
                state.bootstrap.workspace.id,
                state.bootstrap.team.id,
                state.bootstrap.organization.id,
            )
        );
        assert!(value.validate().is_ok());
    }

    #[test]
    fn snapshot_command_rejects_a_foreign_topology_input_scope() {
        let (_directory, state) = test_state();
        let foreign_scope =
            ResourceScope::workspace(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let input = topology_fixture_input(foreign_scope);
        let result = with_topology_input_for_test(input, || {
            state.topology_snapshot(envelope(
                "snapshot",
                Capability::WorkspaceRead,
                request_value(),
            ))
        });

        assert!(matches!(
            result,
            IpcResult::Err { error, .. }
                if error.code == IpcErrorCode::PermissionDenied
                    && error.message == "topology scope denied"
        ));
    }

    #[test]
    fn evidence_foreign_scope_is_denied_through_the_command_path() {
        let (_directory, state) = test_state();
        let foreign_scope =
            ResourceScope::workspace(Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
        let input = topology_fixture_input(foreign_scope);
        let evidence_id = input.evidence[0].id.clone();
        let result = with_topology_input_for_test(input, || {
            state.topology_evidence(envelope(
                "evidence",
                Capability::ResourceRead,
                json!({ "evidence_ids": [evidence_id] }),
            ))
        });
        assert!(matches!(
            result,
            IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
        ));
    }

    #[test]
    fn topology_engine_errors_have_distinct_ipc_messages() {
        let errors = [
            topology_ipc_error(TopologyError::InvalidRequest),
            topology_ipc_error(TopologyError::NodeNotFound),
            topology_ipc_error(TopologyError::IncidentNotFound),
            topology_ipc_error(TopologyError::ScopeDenied),
            topology_ipc_error(TopologyError::EvidenceUnverified),
            topology_ipc_error(TopologyError::EvidenceMissing),
            topology_ipc_error(TopologyError::NonFiniteNumber(
                TopologyNumberField::MetricValue,
            )),
            topology_ipc_error(TopologyError::NonFiniteNumber(
                TopologyNumberField::EdgeConfidence,
            )),
            topology_ipc_error(TopologyError::NonFiniteNumber(
                TopologyNumberField::PathConfidence,
            )),
            topology_ipc_error(TopologyError::ConfidenceOutOfRange),
            topology_ipc_error(TopologyError::MalformedSource),
        ];
        let messages = errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(messages.len(), errors.len());
    }

    #[test]
    fn topology_engine_failures_preserve_distinct_ipc_error_categories() {
        let cases = [
            (TopologyError::InvalidRequest, IpcErrorCode::InvalidRequest),
            (TopologyError::NodeNotFound, IpcErrorCode::NotFound),
            (TopologyError::ScopeDenied, IpcErrorCode::PermissionDenied),
            (
                TopologyError::EvidenceUnverified,
                IpcErrorCode::PolicyDenied,
            ),
            (TopologyError::MalformedSource, IpcErrorCode::InternalError),
        ];
        for (engine_error, expected_code) in cases {
            assert_eq!(topology_ipc_error(engine_error).code, expected_code);
        }
    }

    #[test]
    fn rejected_evidence_is_not_returned_through_the_command_path() {
        let (_directory, state) = test_state();
        let mut input = topology_fixture_input(state.topology_workspace_scope());
        let evidence_id = input.evidence[0].id.clone();
        input.evidence[0].redaction.unparsed = true;
        input.evidence[0].redaction.masked = true;
        let result = with_topology_input_for_test(input, || {
            state.topology_evidence(envelope(
                "evidence",
                Capability::ResourceRead,
                json!({ "evidence_ids": [evidence_id] }),
            ))
        });
        assert!(matches!(
            result,
            IpcResult::Err { error, .. }
                if error.code == IpcErrorCode::NotFound
                    && error.message == "topology evidence not found"
        ));
    }

    #[test]
    fn topology_source_policy_is_checked_before_ui_egress() {
        let (_directory, mut state) = test_state();
        state.policy = PolicyRuntime::load(
            PolicyDocument::baseline(2).with_audit_log_data_classes(vec![DataClass::Public]),
        )
        .unwrap();
        let result = state.topology_snapshot(envelope(
            "snapshot",
            Capability::WorkspaceRead,
            request_value(),
        ));
        assert!(matches!(
            result,
            IpcResult::Err { error, .. } if error.message == "policy denied topology source retention"
        ));
    }

    #[test]
    fn topology_source_retention_accepts_internal_policy() {
        let (_directory, mut state) = test_state();
        state.policy = PolicyRuntime::load(
            PolicyDocument::baseline(2).with_audit_log_data_classes(vec![DataClass::Internal]),
        )
        .unwrap();

        let result = state.topology_snapshot(envelope(
            "snapshot",
            Capability::WorkspaceRead,
            request_value(),
        ));

        assert!(matches!(result, IpcResult::Ok { .. }));
    }

    #[test]
    fn strict_payload_parser_rejects_unknown_fields() {
        let (_directory, state) = test_state();
        let mut payload = request_value();
        payload["unexpected"] = json!(true);
        let result =
            state.topology_snapshot(envelope("snapshot", Capability::WorkspaceRead, payload));
        assert!(matches!(
            result,
            IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
        ));
    }

    #[test]
    fn evidence_payload_rejects_empty_and_duplicate_ids() {
        let (_directory, state) = test_state();
        for payload in [
            json!({ "evidence_ids": [] }),
            json!({ "evidence_ids": ["evidence-topology-environment-aws", "evidence-topology-environment-aws"] }),
        ] {
            let result =
                state.topology_evidence(envelope("evidence", Capability::ResourceRead, payload));
            assert!(matches!(
                result,
                IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
            ));
        }
    }

    #[test]
    fn topology_payload_validation_preserves_typed_engine_error_mapping() {
        let invalid = TopologyRequest {
            filter: TopologyFilter {
                environment_ids: Vec::new(),
                team_ids: Vec::new(),
                incident_id: None,
            },
            focus_node_id: None,
            traversal: TopologyTraversal {
                direction: thalassa_domain::TopologyDirection::Both,
                max_depth: 9,
            },
        };
        let error = topology_ipc_error(invalid.validate().unwrap_err());
        assert_eq!(error.code, IpcErrorCode::InvalidRequest);
        assert_eq!(error.message, "invalid topology request");
    }
}
