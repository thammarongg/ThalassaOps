// SPDX-License-Identifier: Apache-2.0

use thalassa_connectors::*;
use thalassa_domain::{ActionRiskClass, ResourceScope};

#[test]
fn connector_manifest_declares_read_and_action_capabilities() {
    let manifest = ConnectorManifest::new("fixture-kubernetes", "Kubernetes", "0.1.0")
        .with_capability(ConnectorCapability::read(
            "resources.list",
            ["pod", "deployment"],
        ))
        .with_capability(ConnectorCapability::act(
            "workload.restart",
            ["deployment"],
            ActionRiskClass::Mutating,
        ));

    assert!(manifest.can_read("resources.list", "pod"));
    assert!(manifest.can_act("workload.restart", "deployment"));
    assert_eq!(
        manifest.capabilities[1].risk_class,
        Some(ActionRiskClass::Mutating)
    );
}

#[test]
fn connector_capabilities_remain_scope_descriptors_not_authorization() {
    let capability =
        ConnectorCapability::read("resources.list", ["pod"]).in_scope(ResourceScope::default());
    assert_eq!(capability.scope, ResourceScope::default());
    assert_eq!(capability.operation, ConnectorOperation::Read);
}
