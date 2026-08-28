// SPDX-License-Identifier: Apache-2.0

use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::tempdir;
use thalassa_domain::{
    MembershipStatus, ResourceScope, TopologyDirection, TopologyEvidenceRequest, TopologyFilter,
    TopologyRequest, TopologyTraversal,
};
use thalassa_ipc::{Capability, CommandEnvelope, CommandName, IpcErrorCode};
use thalassa_policy::{DataClass, PolicyDocument, PolicyRuntime};
use thalassaops::app::{AppState, IpcResult};
use thalassaops::connectors::InMemoryCredentialStore;
use thalassaops::topology::{fixture_scope, topology_fixture_input};
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

fn topology_request() -> TopologyRequest {
    TopologyRequest {
        filter: TopologyFilter {
            environment_ids: Vec::new(),
            team_ids: Vec::new(),
            incident_id: None,
        },
        focus_node_id: None,
        traversal: TopologyTraversal {
            direction: TopologyDirection::Both,
            max_depth: 3,
        },
    }
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
fn snapshot_command_returns_a_valid_workspace_scoped_projection() {
    let (_directory, state) = test_state();
    let result = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(topology_request()).unwrap(),
    ));

    let IpcResult::Ok { value, .. } = result else {
        panic!("topology.snapshot should succeed")
    };
    assert_eq!(value.scope.workspace_id, Some(state.bootstrap.workspace.id));
    assert!(!value.nodes.is_empty());
    assert!(value.validate().is_ok());
}

#[test]
fn snapshot_command_rejects_a_capability_that_is_not_workspace_read() {
    let (_directory, state) = test_state();
    let result = state.topology_snapshot(envelope(
        "snapshot",
        Capability::ResourceRead,
        serde_json::to_value(topology_request()).unwrap(),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));
}

#[test]
fn evidence_command_rejects_a_capability_that_is_not_resource_read() {
    let (_directory, state) = test_state();
    let result = state.topology_evidence(envelope(
        "evidence",
        Capability::WorkspaceRead,
        json!({ "evidence_ids": ["evidence-topology-environment-aws"] }),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));
}

#[test]
fn snapshot_command_rejects_a_malformed_payload_before_graph_work() {
    let (_directory, state) = test_state();
    let result = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        json!({ "filter": { "environment_ids": [] } }),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
    ));
}

#[test]
fn evidence_command_rejects_a_malformed_payload_before_lookup() {
    let (_directory, state) = test_state();
    let result = state.topology_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": "not-an-array" }),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::InvalidRequest
    ));
}

#[test]
fn evidence_command_accepts_only_ids_emitted_by_a_snapshot() {
    let (_directory, state) = test_state();
    let snapshot = match state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(topology_request()).unwrap(),
    )) {
        IpcResult::Ok { value, .. } => value,
        IpcResult::Err { error, .. } => panic!("topology snapshot failed: {error:?}"),
    };
    let evidence_id = snapshot.evidence[0].id.clone();
    let result = state.topology_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        serde_json::to_value(TopologyEvidenceRequest {
            evidence_ids: vec![evidence_id.clone()],
        })
        .unwrap(),
    ));

    let IpcResult::Ok { value, .. } = result else {
        panic!("topology.evidence should succeed")
    };
    assert_eq!(
        value.iter().map(|item| &item.id).collect::<Vec<_>>(),
        vec![&evidence_id]
    );
}

#[test]
fn evidence_command_rejects_an_id_not_emitted_by_a_snapshot() {
    let (_directory, state) = test_state();
    let result = state.topology_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": ["evidence-not-emitted"] }),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. }
            if error.code == IpcErrorCode::NotFound
                && error.message == "topology evidence not found"
    ));
}

#[test]
fn snapshot_command_preserves_typed_not_found_failures() {
    let (_directory, state) = test_state();
    let mut unknown_focus = topology_request();
    unknown_focus.focus_node_id = Some("node-not-emitted".into());
    let focus_error = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(unknown_focus).unwrap(),
    ));
    assert!(matches!(
        focus_error,
        IpcResult::Err { error, .. }
            if error.code == IpcErrorCode::NotFound
                && error.message == "topology node not found"
    ));

    let mut unknown_incident = topology_request();
    unknown_incident.filter.incident_id = Some("incident-not-emitted".into());
    let incident_error = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(unknown_incident).unwrap(),
    ));
    assert!(matches!(
        incident_error,
        IpcResult::Err { error, .. }
            if error.code == IpcErrorCode::NotFound
                && error.message == "topology incident queue item not found"
    ));
}

#[test]
fn topology_descriptor_contracts_are_read_only_and_capability_scoped() {
    let snapshot = thalassa_ipc::topology_snapshot_descriptor();
    assert_eq!(snapshot.name.to_string(), "topology.snapshot");
    assert_eq!(snapshot.required_capability, Capability::WorkspaceRead);
    assert_eq!(
        snapshot.required_permission,
        thalassa_domain::Permission::Read
    );
    assert!(!snapshot.scope.is_bounded());

    let evidence = thalassa_ipc::topology_evidence_descriptor();
    assert_eq!(evidence.name.to_string(), "topology.evidence");
    assert_eq!(evidence.required_capability, Capability::ResourceRead);
    assert_eq!(
        evidence.required_permission,
        thalassa_domain::Permission::Read
    );
    assert!(!evidence.scope.is_bounded());
}

#[test]
fn topology_commands_reject_inactive_memberships_and_ui_policy_denials() {
    let (_directory, mut state) = test_state();
    state.bootstrap.membership.status = MembershipStatus::Suspended;
    let suspended = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(topology_request()).unwrap(),
    ));
    assert!(
        matches!(suspended, IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied)
    );

    state.bootstrap.membership.status = MembershipStatus::Active;
    state.policy = PolicyRuntime::load(
        PolicyDocument::baseline(2).with_audit_log_data_classes(vec![DataClass::Public]),
    )
    .unwrap();
    let denied = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(topology_request()).unwrap(),
    ));
    assert!(
        matches!(denied, IpcResult::Err { error, .. } if error.code == IpcErrorCode::PolicyDenied)
    );
}

#[test]
fn topology_commands_reject_mismatched_principals_and_accept_read_roles() {
    let (_directory, mut state) = test_state();
    state.bootstrap.membership.principal_id = Uuid::new_v4();
    let principal_mismatch = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(topology_request()).unwrap(),
    ));
    assert!(matches!(
        principal_mismatch,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));

    state.bootstrap.membership.principal_id = state.bootstrap.principal.id;
    state.bootstrap.membership.role = thalassa_domain::MembershipRole::Viewer;
    let viewer_read = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(topology_request()).unwrap(),
    ));
    assert!(matches!(viewer_read, IpcResult::Ok { .. }));

    state.bootstrap.membership.role = thalassa_domain::MembershipRole::Auditor;
    let auditor_read = state.topology_evidence(envelope(
        "evidence",
        Capability::ResourceRead,
        json!({ "evidence_ids": ["evidence-topology-environment-aws"] }),
    ));
    assert!(matches!(auditor_read, IpcResult::Ok { .. }));
}

#[test]
fn topology_commands_reject_a_membership_scope_that_does_not_grant_the_workspace() {
    let (_directory, mut state) = test_state();
    state.bootstrap.membership.scope = ResourceScope::workspace(
        Uuid::new_v4(),
        state.bootstrap.team.id,
        state.bootstrap.organization.id,
    );
    let result = state.topology_snapshot(envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(topology_request()).unwrap(),
    ));

    assert!(matches!(
        result,
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));
}

#[test]
fn fixture_scope_is_not_accepted_as_the_callers_envelope_scope() {
    let (_directory, state) = test_state();
    let mut request = envelope(
        "snapshot",
        Capability::WorkspaceRead,
        serde_json::to_value(topology_request()).unwrap(),
    );
    request.scope = fixture_scope();
    assert!(matches!(
        state.topology_snapshot(request),
        IpcResult::Err { error, .. } if error.code == IpcErrorCode::PermissionDenied
    ));
}

#[test]
fn topology_fixture_input_stays_workspace_scoped_when_built_for_a_scope() {
    let input = topology_fixture_input(fixture_scope());
    assert!(input
        .evidence
        .iter()
        .all(|evidence| fixture_scope().contains(&evidence.scope)));
}
