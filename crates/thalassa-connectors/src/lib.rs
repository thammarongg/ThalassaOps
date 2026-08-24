// SPDX-License-Identifier: Apache-2.0

//! Connector declarations. A capability describes what an adapter can do;
//! policy and membership remain the authorization boundary.

use serde::{Deserialize, Serialize};
use thalassa_domain::{ActionRiskClass, ResourceScope};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum ConnectorOperation {
    Read,
    Act,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConnectorCapability {
    pub key: String,
    pub operation: ConnectorOperation,
    pub resource_kinds: Vec<String>,
    pub risk_class: Option<ActionRiskClass>,
    pub scope: ResourceScope,
    pub supports_dry_run: bool,
}
impl ConnectorCapability {
    pub fn read<const N: usize>(key: impl Into<String>, resource_kinds: [&str; N]) -> Self {
        Self {
            key: key.into(),
            operation: ConnectorOperation::Read,
            resource_kinds: resource_kinds.into_iter().map(str::to_string).collect(),
            risk_class: None,
            scope: ResourceScope::default(),
            supports_dry_run: false,
        }
    }
    pub fn act<const N: usize>(
        key: impl Into<String>,
        resource_kinds: [&str; N],
        risk_class: ActionRiskClass,
    ) -> Self {
        Self {
            key: key.into(),
            operation: ConnectorOperation::Act,
            resource_kinds: resource_kinds.into_iter().map(str::to_string).collect(),
            risk_class: Some(risk_class),
            scope: ResourceScope::default(),
            supports_dry_run: true,
        }
    }
    pub fn in_scope(mut self, scope: ResourceScope) -> Self {
        self.scope = scope;
        self
    }
    pub fn with_dry_run(mut self, supports: bool) -> Self {
        self.supports_dry_run = supports;
        self
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ConnectorManifest {
    pub id: String,
    pub display_name: String,
    pub version: String,
    pub capabilities: Vec<ConnectorCapability>,
}
impl ConnectorManifest {
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            version: version.into(),
            capabilities: vec![],
        }
    }
    pub fn with_capability(mut self, capability: ConnectorCapability) -> Self {
        self.capabilities.push(capability);
        self
    }
    pub fn can_read(&self, key: &str, resource_kind: &str) -> bool {
        self.capabilities.iter().any(|c| {
            c.key == key
                && c.operation == ConnectorOperation::Read
                && c.resource_kinds.iter().any(|kind| kind == resource_kind)
        })
    }
    pub fn can_act(&self, key: &str, resource_kind: &str) -> bool {
        self.capabilities.iter().any(|c| {
            c.key == key
                && c.operation == ConnectorOperation::Act
                && c.resource_kinds.iter().any(|kind| kind == resource_kind)
        })
    }
}
