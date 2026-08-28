// SPDX-License-Identifier: Apache-2.0

use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::tempdir;
use thalassa_domain::{MembershipStatus, Permission, ResourceScope};
use thalassa_ipc::{Capability, CommandEnvelope, CommandName};
use thalassa_policy::{DataClass, PolicyDocument, PolicyRuntime};
use thalassaops::app::{AppState, IpcResult};
use thalassaops::connectors::InMemoryCredentialStore;
use thalassaops::operations::OperationsEvidenceRequest;
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
fn snapshot_command_returns_a_deterministic_workspace_scoped_projection() {
    let (_directory, state) = test_state();
    let result = state.operations_snapshot(envelope(
        &state,
        "snapshot",
        Capability::WorkspaceRead,
        Value::Null,
    ));

    let IpcResult::Ok { value, .. } = result else {
        panic!("operations.snapshot should succeed")
    };
    assert_eq!(value.generated_at, "2026-08-28T09:00:00Z");
    assert_eq!(
        value.scope,
        ResourceScope::workspace(
            state.bootstrap.workspace.id,
            state.bootstrap.team.id,
            state.bootstrap.organization.id,
        )
    );
    assert!(!value.evidence.is_empty());
    assert!(value.validate().is_ok());
}

#[test]
fn snapshot_command_rejects_a_capability_that_is_not_workspace_read() {
    let (_directory, state) = test_state();
    let result = state.operations_snapshot(envelope(
        &state,
        "snapshot",
        Capability::ResourceRead,
        Value::Null,
    ));

    assert!(
        matches!(result, IpcResult::Err { error, .. } if error.code == thalassa_ipc::IpcErrorCode::PermissionDenied)
    );
}

#[test]
fn snapshot_command_rejects_a_malformed_payload() {
    let (_directory, state) = test_state();
    let result = state.operations_snapshot(envelope(
        &state,
        "snapshot",
        Capability::WorkspaceRead,
        json!({ "unexpected": true }),
    ));

    assert!(
        matches!(result, IpcResult::Err { error, .. } if error.code == thalassa_ipc::IpcErrorCode::InvalidRequest)
    );
}

#[test]
fn snapshot_command_rejects_inactive_memberships_and_mismatched_principals() {
    let (_directory, mut state) = test_state();
    state.bootstrap.membership.status = MembershipStatus::Suspended;
    let suspended = state.operations_snapshot(envelope(
        &state,
        "snapshot",
        Capability::WorkspaceRead,
        Value::Null,
    ));
    assert!(matches!(suspended, IpcResult::Err { .. }));

    state.bootstrap.membership.status = MembershipStatus::Active;
    state.bootstrap.membership.principal_id = Uuid::new_v4();
    let mismatched = state.operations_snapshot(envelope(
        &state,
        "snapshot",
        Capability::WorkspaceRead,
        Value::Null,
    ));
    assert!(matches!(mismatched, IpcResult::Err { .. }));
}

#[test]
fn snapshot_command_rejects_bounded_scope_and_role_without_read_permission() {
    let (_directory, mut state) = test_state();
    let mut bounded = envelope(&state, "snapshot", Capability::WorkspaceRead, Value::Null);
    bounded.scope.workspace_id = Some(state.bootstrap.workspace.id);
    assert!(matches!(
        state.operations_snapshot(bounded),
        IpcResult::Err { .. }
    ));

    state.bootstrap.membership.scope = ResourceScope::workspace(
        Uuid::new_v4(),
        state.bootstrap.team.id,
        state.bootstrap.organization.id,
    );
    let denied_scope = state.operations_snapshot(envelope(
        &state,
        "snapshot",
        Capability::WorkspaceRead,
        Value::Null,
    ));
    assert!(matches!(denied_scope, IpcResult::Err { .. }));
}

#[test]
fn snapshot_command_checks_audit_policy_before_retaining_health_check_metadata() {
    let (_directory, mut state) = test_state();
    state.policy = PolicyRuntime::load(
        PolicyDocument::baseline(2).with_audit_log_data_classes(vec![DataClass::Public]),
    )
    .unwrap();

    let result = state.operations_snapshot(envelope(
        &state,
        "snapshot",
        Capability::WorkspaceRead,
        Value::Null,
    ));

    assert!(
        matches!(result, IpcResult::Err { error, .. } if error.code == thalassa_ipc::IpcErrorCode::PolicyDenied)
    );
}

#[test]
fn evidence_command_returns_only_ids_emitted_by_a_snapshot() {
    let (_directory, state) = test_state();
    let snapshot = match state.operations_snapshot(envelope(
        &state,
        "snapshot",
        Capability::WorkspaceRead,
        Value::Null,
    )) {
        IpcResult::Ok { value, .. } => value,
        IpcResult::Err { error, .. } => panic!("snapshot failed: {error:?}"),
    };
    let evidence_id = snapshot.evidence[0].id.clone();
    let result = state.operations_evidence(envelope(
        &state,
        "evidence",
        Capability::ResourceRead,
        serde_json::to_value(OperationsEvidenceRequest {
            evidence_ids: vec![evidence_id.clone()],
        })
        .unwrap(),
    ));

    let IpcResult::Ok { value, .. } = result else {
        panic!("operations.evidence should succeed")
    };
    assert_eq!(value.len(), 1);
    assert_eq!(value[0].id, evidence_id);
}

#[test]
fn evidence_command_rejects_a_capability_that_is_not_resource_read() {
    let (_directory, state) = test_state();
    let result = state.operations_evidence(envelope(
        &state,
        "evidence",
        Capability::WorkspaceRead,
        json!({ "evidence_ids": ["evidence-alert-checkout-s1"] }),
    ));

    assert!(
        matches!(result, IpcResult::Err { error, .. } if error.code == thalassa_ipc::IpcErrorCode::PermissionDenied)
    );
}

#[test]
fn evidence_command_rejects_malformed_duplicate_and_unknown_requests() {
    let (_directory, state) = test_state();
    let malformed = state.operations_evidence(envelope(
        &state,
        "evidence",
        Capability::ResourceRead,
        json!({}),
    ));
    assert!(
        matches!(malformed, IpcResult::Err { error, .. } if error.code == thalassa_ipc::IpcErrorCode::InvalidRequest)
    );

    let duplicate = state.operations_evidence(envelope(
        &state,
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": ["evidence-alert-checkout-s1", "evidence-alert-checkout-s1"] }),
    ));
    assert!(matches!(duplicate, IpcResult::Err { .. }));

    let unknown = state.operations_evidence(envelope(
        &state,
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": ["unknown-evidence"] }),
    ));
    assert!(matches!(unknown, IpcResult::Err { .. }));
}

#[test]
fn command_envelopes_keep_the_shared_wire_shape_for_operations() {
    let (_directory, state) = test_state();
    let request = envelope(&state, "snapshot", Capability::WorkspaceRead, Value::Null);
    let serialized = serde_json::to_value(request).unwrap();
    assert!(serialized["request_id"].is_string());
    assert_eq!(serialized["command"], "operations.snapshot");
    assert_eq!(serialized["capability"], "WorkspaceRead");
    assert!(serialized["scope"].is_object());
    assert!(serialized["payload"].is_null());
}

#[test]
fn policy_permission_is_read_only_for_both_console_descriptors() {
    assert_eq!(
        thalassa_ipc::operations_snapshot_descriptor().required_permission,
        Permission::Read
    );
    assert_eq!(
        thalassa_ipc::operations_evidence_descriptor().required_permission,
        Permission::Read
    );
}
