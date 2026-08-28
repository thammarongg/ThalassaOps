// SPDX-License-Identifier: Apache-2.0

use thalassa_domain::ResourceScope;
use thalassa_ipc::*;

#[test]
fn command_names_are_capability_scoped_and_predictable() {
    let name = CommandName::new("incident", "list").unwrap();
    assert_eq!(name.to_string(), "incident.list");
    assert!(CommandName::new("Incident", "list").is_err());
}

#[test]
fn errors_have_one_serializable_shape_for_rust_and_react() {
    let error = IpcError::permission_denied("incident.read", ResourceScope::default());
    let value = serde_json::to_value(&error).unwrap();
    assert_eq!(value["code"], "PERMISSION_DENIED");
    assert_eq!(value["message"], "permission denied");
    assert!(value["details"]["required_command"].is_string());
}

#[test]
fn command_descriptors_declare_required_capability_and_scope() {
    let descriptor = CommandDescriptor::new(
        "incident",
        "list",
        Capability::IncidentRead,
        Permission::Read,
    );
    assert_eq!(descriptor.name.to_string(), "incident.list");
    assert_eq!(descriptor.required_capability, Capability::IncidentRead);
}

#[test]
fn operations_commands_declare_their_read_only_capabilities() {
    let snapshot = operations_snapshot_descriptor();
    assert_eq!(snapshot.name.to_string(), "operations.snapshot");
    assert_eq!(snapshot.required_capability, Capability::WorkspaceRead);
    assert_eq!(snapshot.required_permission, Permission::Read);
    assert!(!snapshot.scope.is_bounded());

    let evidence = operations_evidence_descriptor();
    assert_eq!(evidence.name.to_string(), "operations.evidence");
    assert_eq!(evidence.required_capability, Capability::ResourceRead);
    assert_eq!(evidence.required_permission, Permission::Read);
    assert!(!evidence.scope.is_bounded());
}

#[test]
fn topology_commands_declare_their_read_only_capabilities() {
    let snapshot = topology_snapshot_descriptor();
    assert_eq!(snapshot.name.to_string(), "topology.snapshot");
    assert_eq!(snapshot.required_capability, Capability::WorkspaceRead);
    assert_eq!(snapshot.required_permission, Permission::Read);
    assert!(!snapshot.scope.is_bounded());

    let evidence = topology_evidence_descriptor();
    assert_eq!(evidence.name.to_string(), "topology.evidence");
    assert_eq!(evidence.required_capability, Capability::ResourceRead);
    assert_eq!(evidence.required_permission, Permission::Read);
    assert!(!evidence.scope.is_bounded());
}
