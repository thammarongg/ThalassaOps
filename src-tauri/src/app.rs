use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use thalassa_domain::{Membership, Organization, Principal, ResourceScope, Team, Workspace};
use thalassa_ipc::{Capability, CommandDescriptor, CommandEnvelope, IpcError, IpcErrorCode};
use thalassa_policy::{DataClass, EgressDestination, EgressRequest, PolicyDocument, PolicyRuntime};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_local_workspace.sql");

#[derive(Clone, Debug)]
pub struct BootstrapState {
    pub principal: Principal,
    pub organization: Organization,
    pub team: Team,
    pub workspace: Workspace,
    pub membership: Membership,
    pub scope: ResourceScope,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub bootstrap: BootstrapState,
    pub policy: PolicyRuntime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum IpcResult<T> {
    Ok { ok: bool, value: T },
    Err { ok: bool, error: IpcError },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub policy_version: u64,
}

impl AppState {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppStateError> {
        let connection = Connection::open(path)?;
        apply_migrations(&connection)?;
        let bootstrap = load_or_bootstrap(&connection)?;
        let policy = load_or_seed_policy(&connection)?;
        Ok(Self { bootstrap, policy })
    }

    pub fn health(&self, envelope: CommandEnvelope<Value>) -> IpcResult<HealthResponse> {
        let descriptor = CommandDescriptor::new(
            "system",
            "health",
            Capability::WorkspaceRead,
            thalassa_domain::Permission::Read,
        );
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
        {
            return IpcResult::Err {
                ok: false,
                error: IpcError::permission_denied(descriptor.name.to_string(), envelope.scope),
            };
        }
        // `system.health` has no resource target, so it rejects any claimed resource scope.
        // Resource commands will require their descriptor scope to contain the envelope scope.
        if envelope.scope.is_bounded()
            || !descriptor.scope.contains(&envelope.scope)
            || self.bootstrap.membership.status != thalassa_domain::MembershipStatus::Active
        {
            return IpcResult::Err {
                ok: false,
                error: IpcError::permission_denied(descriptor.name.to_string(), envelope.scope),
            };
        }
        if !self
            .policy
            .evaluate_egress(EgressRequest::verified(
                DataClass::Internal,
                EgressDestination::Ui,
            ))
            .is_allowed()
        {
            return IpcResult::Err {
                ok: false,
                error: IpcError::new(
                    IpcErrorCode::PolicyDenied,
                    "policy denied health response",
                    json!({}),
                ),
            };
        }
        IpcResult::Ok {
            ok: true,
            value: HealthResponse {
                status: "healthy".into(),
                policy_version: self.policy.version(),
            },
        }
    }
}

fn apply_migrations(connection: &Connection) -> Result<(), AppStateError> {
    connection.execute_batch(INITIAL_MIGRATION)?;
    let exists: Option<i64> = connection
        .query_row(
            "SELECT version FROM schema_migrations WHERE version = 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if exists.is_none() {
        connection.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
            [Utc::now().to_rfc3339()],
        )?;
    }
    Ok(())
}

fn load_or_bootstrap(connection: &Connection) -> Result<BootstrapState, AppStateError> {
    let existing: Option<String> = connection
        .query_row("SELECT document_json FROM principals LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()?;
    if let Some(principal) = existing {
        return load_existing_bootstrap(connection, serde_json::from_str(&principal)?);
    }
    let principal = Principal::local("local-administrator", "Local Administrator");
    let organization = Organization::new("Local Organization");
    let team = Team::new(organization.id, "Local Team");
    let workspace = Workspace::new(team.id, "Local Workspace");
    let membership = Membership::workspace_owner(principal.id, workspace.id);
    persist(
        connection,
        "principals",
        principal.id.to_string(),
        &principal,
    )?;
    persist(
        connection,
        "organizations",
        organization.id.to_string(),
        &organization,
    )?;
    persist(connection, "teams", team.id.to_string(), &team)?;
    persist(
        connection,
        "workspaces",
        workspace.id.to_string(),
        &workspace,
    )?;
    persist(
        connection,
        "memberships",
        principal.id.to_string(),
        &membership,
    )?;
    Ok(BootstrapState {
        principal,
        organization,
        team,
        workspace,
        membership,
        scope: ResourceScope::default(),
    })
}

fn load_existing_bootstrap(
    connection: &Connection,
    principal: Principal,
) -> Result<BootstrapState, AppStateError> {
    Ok(BootstrapState {
        principal,
        organization: load_one(connection, "organizations")?,
        team: load_one(connection, "teams")?,
        workspace: load_one(connection, "workspaces")?,
        membership: load_one(connection, "memberships")?,
        scope: ResourceScope::default(),
    })
}

fn load_one<T: for<'de> Deserialize<'de>>(
    connection: &Connection,
    table: &str,
) -> Result<T, AppStateError> {
    let statement = format!("SELECT document_json FROM {table} LIMIT 1");
    let value: String = connection.query_row(&statement, [], |row| row.get(0))?;
    Ok(serde_json::from_str(&value)?)
}

fn persist<T: Serialize>(
    connection: &Connection,
    table: &str,
    id: String,
    document: &T,
) -> Result<(), AppStateError> {
    let statement = format!("INSERT INTO {table} (id, document_json) VALUES (?1, ?2)");
    connection.execute(&statement, params![id, serde_json::to_string(document)?])?;
    Ok(())
}

fn load_or_seed_policy(connection: &Connection) -> Result<PolicyRuntime, AppStateError> {
    let existing: Option<String> = connection
        .query_row(
            "SELECT document_json FROM policy_store WHERE id = 'system-baseline'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    let document = match existing {
        Some(document) => serde_json::from_str(&document)?,
        None => {
            let document = PolicyDocument::baseline(1);
            connection.execute("INSERT INTO policy_store (id, version, document_json, migrated_at) VALUES (?1, ?2, ?3, ?4)", params![document.id, document.version, serde_json::to_string(&document)?, Utc::now().to_rfc3339()])?;
            document
        }
    };
    Ok(PolicyRuntime::load(document)?)
}

#[derive(Debug, thiserror::Error)]
pub enum AppStateError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("policy error: {0}")]
    Policy(#[from] thalassa_policy::PolicyLoadError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn health_envelope(state: &AppState) -> CommandEnvelope<Value> {
        CommandEnvelope {
            request_id: Uuid::new_v4(),
            command: thalassa_ipc::CommandName::new("system", "health").unwrap(),
            capability: Capability::WorkspaceRead,
            scope: state.bootstrap.scope.clone(),
            payload: Value::Null,
        }
    }

    #[test]
    fn bootstrap_persists_local_administrator_and_workspace_hierarchy() {
        let directory = tempdir().unwrap();
        let state = AppState::open(directory.path().join("thalassaops.sqlite")).unwrap();
        assert_eq!(
            state.bootstrap.principal.kind,
            thalassa_domain::PrincipalKind::Local
        );
        assert_eq!(
            state.bootstrap.membership.status,
            thalassa_domain::MembershipStatus::Active
        );
        assert_eq!(state.policy.document().id, "system-baseline");
    }

    #[test]
    fn health_command_accepts_matching_capability_and_scope() {
        let directory = tempdir().unwrap();
        let state = AppState::open(directory.path().join("thalassaops.sqlite")).unwrap();
        assert!(matches!(
            state.health(health_envelope(&state)),
            IpcResult::Ok { .. }
        ));
    }

    #[test]
    fn health_command_rejects_wrong_capability_or_scope() {
        let directory = tempdir().unwrap();
        let state = AppState::open(directory.path().join("thalassaops.sqlite")).unwrap();
        let mut envelope = health_envelope(&state);
        envelope.scope.workspace_id = Some(state.bootstrap.workspace.id);
        assert!(matches!(state.health(envelope), IpcResult::Err { .. }));
    }

    #[test]
    fn health_result_uses_the_frontend_ipc_result_shape() {
        let directory = tempdir().unwrap();
        let state = AppState::open(directory.path().join("thalassaops.sqlite")).unwrap();
        let json = serde_json::to_value(state.health(health_envelope(&state))).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["value"]["status"], "healthy");
    }
}
