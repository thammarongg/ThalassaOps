// SPDX-License-Identifier: Apache-2.0

//! Stable IPC conventions for Tauri commands and their React consumers.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{json, Value};
use std::{fmt, str::FromStr};
pub use thalassa_domain::Permission;
use thalassa_domain::ResourceScope;
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CommandName {
    pub resource: String,
    pub verb: String,
}
impl CommandName {
    pub fn new(
        resource: impl Into<String>,
        verb: impl Into<String>,
    ) -> Result<Self, CommandNameError> {
        let resource = resource.into();
        let verb = verb.into();
        if valid_component(&resource) && valid_component(&verb) {
            Ok(Self { resource, verb })
        } else {
            Err(CommandNameError::InvalidComponent)
        }
    }
}
fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}
impl fmt::Display for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.resource, self.verb)
    }
}
impl Serialize for CommandName {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for CommandName {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}
impl FromStr for CommandName {
    type Err = CommandNameError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (resource, verb) = value
            .split_once('.')
            .ok_or(CommandNameError::InvalidComponent)?;
        Self::new(resource, verb)
    }
}
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum CommandNameError {
    #[error("command names must use lowercase resource.verb components")]
    InvalidComponent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Capability {
    WorkspaceRead,
    EnvironmentRead,
    ResourceRead,
    IncidentRead,
    IncidentWrite,
    PolicyEvaluate,
    PolicyManage,
    ConnectorRead,
    ConnectorAct,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandDescriptor {
    pub name: CommandName,
    pub required_capability: Capability,
    pub required_permission: Permission,
    pub scope: ResourceScope,
}
impl CommandDescriptor {
    pub fn new(
        resource: impl Into<String>,
        verb: impl Into<String>,
        required_capability: Capability,
        required_permission: Permission,
    ) -> Self {
        Self {
            name: CommandName::new(resource, verb)
                .expect("command descriptors must use valid command names"),
            required_capability,
            required_permission,
            scope: ResourceScope::default(),
        }
    }
    pub fn with_scope(mut self, scope: ResourceScope) -> Self {
        self.scope = scope;
        self
    }
}

/// Stable command descriptors for the read-only Operations Console surface.
///
/// Keeping the command name, capability and permission in the IPC crate gives
/// Tauri handlers and contract tests one source of truth.  The operation
/// payloads and responses remain provider-neutral domain values.
pub fn operations_snapshot_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "operations",
        "snapshot",
        Capability::WorkspaceRead,
        Permission::Read,
    )
}

pub fn operations_evidence_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "operations",
        "evidence",
        Capability::ResourceRead,
        Permission::Read,
    )
}

/// Stable command descriptor for the read-only topology snapshot projection.
pub fn topology_snapshot_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "topology",
        "snapshot",
        Capability::WorkspaceRead,
        Permission::Read,
    )
}

/// Stable command descriptor for workspace-scoped topology evidence lookup.
pub fn topology_evidence_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "topology",
        "evidence",
        Capability::ResourceRead,
        Permission::Read,
    )
}

/// Stable command descriptor for the read-only correlation snapshot projection.
pub fn correlation_snapshot_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "correlation",
        "snapshot",
        Capability::WorkspaceRead,
        Permission::Read,
    )
}

/// Stable command descriptor for workspace-scoped correlation evidence lookup.
pub fn correlation_evidence_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "correlation",
        "evidence",
        Capability::ResourceRead,
        Permission::Read,
    )
}

/// Stable command descriptor for the read-only change snapshot projection.
pub fn change_snapshot_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "change",
        "snapshot",
        Capability::WorkspaceRead,
        Permission::Read,
    )
}

/// Stable command descriptor for workspace-scoped change evidence lookup.
pub fn change_evidence_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "change",
        "evidence",
        Capability::ResourceRead,
        Permission::Read,
    )
}

/// Stable descriptors for the Sprint 15 incident command surface.  Reads use
/// `IncidentRead` plus `Permission::Read`; writes use `IncidentWrite` plus
/// `Permission::ManageIncident`.  Descriptor scopes stay unbounded so the
/// application layer resolves the active workspace per request.
pub fn incident_create_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "create",
        Capability::IncidentWrite,
        Permission::ManageIncident,
    )
}

pub fn incident_get_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "get",
        Capability::IncidentRead,
        Permission::Read,
    )
}

pub fn incident_list_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "list",
        Capability::IncidentRead,
        Permission::Read,
    )
}

pub fn incident_timeline_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "timeline",
        Capability::IncidentRead,
        Permission::Read,
    )
}

pub fn incident_transition_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "transition",
        Capability::IncidentWrite,
        Permission::ManageIncident,
    )
}

pub fn incident_set_severity_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "set_severity",
        Capability::IncidentWrite,
        Permission::ManageIncident,
    )
}

pub fn incident_set_disposition_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "set_disposition",
        Capability::IncidentWrite,
        Permission::ManageIncident,
    )
}

pub fn incident_assign_role_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "assign_role",
        Capability::IncidentWrite,
        Permission::ManageIncident,
    )
}

/// Stable command descriptor for appending one responder comment.  A comment
/// reuses the incident write capability and permission rather than adding a
/// narrower one; see the Sprint 16 design, section 14, debt 1.
pub fn incident_add_comment_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "incident",
        "add_comment",
        Capability::IncidentWrite,
        Permission::ManageIncident,
    )
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandEnvelope<T> {
    pub request_id: Uuid,
    pub command: CommandName,
    pub capability: Capability,
    pub scope: ResourceScope,
    pub payload: T,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum IpcErrorCode {
    #[serde(rename = "INVALID_REQUEST")]
    InvalidRequest,
    #[serde(rename = "NOT_FOUND")]
    NotFound,
    #[serde(rename = "PERMISSION_DENIED")]
    PermissionDenied,
    #[serde(rename = "POLICY_DENIED")]
    PolicyDenied,
    #[serde(rename = "CONNECTOR_UNAVAILABLE")]
    ConnectorUnavailable,
    #[serde(rename = "MALFORMED_RESPONSE")]
    MalformedResponse,
    #[serde(rename = "INVALID_EVENT_SEQUENCE")]
    InvalidEventSequence,
    #[serde(rename = "INVALID_SEVERITY_OVERRIDE")]
    InvalidSeverityOverride,
    #[serde(rename = "WRITE_CONTENTION")]
    WriteContention,
    #[serde(rename = "INTERNAL_ERROR")]
    InternalError,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct IpcError {
    pub code: IpcErrorCode,
    pub message: String,
    pub details: Value,
}
impl IpcError {
    pub fn new(code: IpcErrorCode, message: impl Into<String>, details: Value) -> Self {
        Self {
            code,
            message: message.into(),
            details,
        }
    }
    pub fn permission_denied(command: impl Into<String>, scope: ResourceScope) -> Self {
        Self::new(
            IpcErrorCode::PermissionDenied,
            "permission denied",
            json!({ "required_command": command.into(), "scope": scope }),
        )
    }
}
