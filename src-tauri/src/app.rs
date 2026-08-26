use crate::connectors::{
    self, AddConnectorRequest, ConnectorDiagnostics, ConnectorError, ConnectorIdRequest,
    ConnectorSummary, OsKeychainCredentialStore, SharedCredentialStore,
};
use crate::kubernetes::{
    client_from_kubeconfig, discover, pod_events, pod_logs, resource_manifest,
    KubernetesConnectorConfig, KubernetesEvent, KubernetesInventory, KubernetesManifest,
};
use crate::observability::client::ObservabilityClient;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thalassa_domain::{Membership, Organization, Principal, ResourceScope, Team, Workspace};
use thalassa_ipc::{Capability, CommandDescriptor, CommandEnvelope, IpcError, IpcErrorCode};
use thalassa_policy::{DataClass, EgressDestination, EgressRequest, PolicyDocument, PolicyRuntime};

const INITIAL_MIGRATION: &str = include_str!("../migrations/0001_local_workspace.sql");
const CONNECTOR_MIGRATION: &str = include_str!("../migrations/0002_connector_registry.sql");

#[derive(Clone, Debug)]
pub struct BootstrapState {
    pub principal: Principal,
    pub organization: Organization,
    pub team: Team,
    pub workspace: Workspace,
    pub membership: Membership,
    pub scope: ResourceScope,
}

#[derive(Clone)]
pub struct AppState {
    pub bootstrap: BootstrapState,
    pub policy: PolicyRuntime,
    database_path: PathBuf,
    credential_store: SharedCredentialStore,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ContextResponse {
    pub organization_name: String,
    pub team_name: String,
    pub workspace_name: String,
    pub policy_version: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KubernetesConnectorRequest {
    pub connector_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct KubernetesPodRequest {
    pub connector_id: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub pod: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub name: String,
}

impl AppState {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppStateError> {
        Self::open_with_credential_store(path, Arc::new(OsKeychainCredentialStore))
    }

    pub fn open_with_credential_store(
        path: impl AsRef<Path>,
        credential_store: SharedCredentialStore,
    ) -> Result<Self, AppStateError> {
        let database_path = path.as_ref().to_path_buf();
        let mut connection = Connection::open(&database_path)?;
        apply_migrations(&connection)?;
        let bootstrap = load_or_bootstrap(&mut connection)?;
        let policy = load_or_seed_policy(&connection)?;
        Ok(Self {
            bootstrap,
            policy,
            database_path,
            credential_store,
        })
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

    pub fn context(&self, envelope: CommandEnvelope<Value>) -> IpcResult<ContextResponse> {
        let descriptor = CommandDescriptor::new(
            "system",
            "context",
            Capability::WorkspaceRead,
            thalassa_domain::Permission::Read,
        );
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
            || envelope.scope.is_bounded()
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
                    "policy denied context response",
                    json!({}),
                ),
            };
        }
        IpcResult::Ok {
            ok: true,
            value: ContextResponse {
                organization_name: self.bootstrap.organization.name.clone(),
                team_name: self.bootstrap.team.name.clone(),
                workspace_name: self.bootstrap.workspace.name.clone(),
                policy_version: self.policy.version(),
            },
        }
    }

    pub fn connector_list(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<ConnectorSummary>> {
        self.connector_read(envelope, "list", |connection, store, _| {
            Ok(connectors::list(connection, store)?)
        })
    }

    pub fn connector_add(&self, envelope: CommandEnvelope<Value>) -> IpcResult<ConnectorSummary> {
        self.connector_act(envelope, "add", |connection, store, payload| {
            let request: AddConnectorRequest =
                serde_json::from_value(payload.clone()).map_err(AppStateError::Serialization)?;
            connectors::add(connection, store, request).map_err(AppStateError::Connector)
        })
    }

    pub fn connector_enable(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<ConnectorSummary> {
        self.connector_set_enabled(envelope, "enable", true)
    }
    pub fn connector_disable(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<ConnectorSummary> {
        self.connector_set_enabled(envelope, "disable", false)
    }

    pub fn connector_remove(&self, envelope: CommandEnvelope<Value>) -> IpcResult<Value> {
        self.connector_act(envelope, "remove", |connection, store, payload| {
            let request: ConnectorIdRequest =
                serde_json::from_value(payload.clone()).map_err(AppStateError::Serialization)?;
            connectors::remove(connection, store, &request.id).map_err(AppStateError::Connector)?;
            Ok(json!({ "id": request.id }))
        })
    }

    pub async fn connector_test(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<ConnectorSummary> {
        let descriptor = CommandDescriptor::new(
            "connector",
            "test",
            Capability::ConnectorAct,
            thalassa_domain::Permission::Read,
        );
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
            || envelope.scope.is_bounded()
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
                    "policy denied connector response",
                    json!({}),
                ),
            };
        }
        if !self
            .policy
            .evaluate_egress(EgressRequest::verified(
                DataClass::Internal,
                EgressDestination::ExternalIntegration,
            ))
            .is_allowed()
        {
            return IpcResult::Err {
                ok: false,
                error: IpcError::new(
                    IpcErrorCode::PolicyDenied,
                    "policy denied external connector probe",
                    json!({}),
                ),
            };
        }
        let result = match serde_json::from_value::<ConnectorIdRequest>(envelope.payload) {
            Ok(request) => match Connection::open(&self.database_path) {
                Ok(connection) => connectors::test_connection(
                    connection,
                    self.credential_store.as_ref(),
                    &request.id,
                )
                .await
                .map_err(AppStateError::Connector),
                Err(error) => Err(AppStateError::Database(error)),
            },
            Err(error) => Err(AppStateError::Serialization(error)),
        };
        match result {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }

    pub fn connector_diagnose(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<ConnectorDiagnostics> {
        self.connector_read(envelope, "diagnose", |connection, store, payload| {
            let request: ConnectorIdRequest =
                serde_json::from_value(payload.clone()).map_err(AppStateError::Serialization)?;
            connectors::diagnose(connection, store, &request.id).map_err(AppStateError::Connector)
        })
    }

    pub async fn prometheus_query(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<crate::observability::prometheus::PrometheusQueryResult> {
        use crate::observability::{
            client::ObservabilityClient,
            prometheus::{self, PrometheusQueryRequest},
            PROMETHEUS_CONNECTOR_KIND,
        };

        let descriptor = CommandDescriptor::new(
            "prometheus",
            "query",
            Capability::ResourceRead,
            thalassa_domain::Permission::Read,
        );
        if let Err(error) = self.authorize_observability(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let req = match serde_json::from_value::<PrometheusQueryRequest>(envelope.payload.clone()) {
            Ok(r) => r,
            Err(e) => {
                return IpcResult::Err {
                    ok: false,
                    error: ipc_error_for(AppStateError::Serialization(e)),
                }
            }
        };

        let result = match Connection::open(&self.database_path) {
            Ok(conn) => {
                match connectors::get(&conn, self.credential_store.as_ref(), &req.connector_id) {
                    Ok(Some(connector)) if connector.kind == PROMETHEUS_CONNECTOR_KIND => {
                        if !connector.enabled {
                            Err(AppStateError::Connector(ConnectorError::Disabled))
                        } else if !self
                            .policy
                            .evaluate_egress(EgressRequest::verified(
                                DataClass::Internal,
                                EgressDestination::ExternalIntegration,
                            ))
                            .is_allowed()
                        {
                            Err(AppStateError::PolicyDenied)
                        } else {
                            match ObservabilityClient::new(
                                &connector,
                                self.credential_store.as_ref(),
                            ) {
                                Ok(client) => prometheus::query(&client, req)
                                    .await
                                    .map_err(AppStateError::from),
                                Err(e) => Err(AppStateError::ObservabilityClient(e)),
                            }
                        }
                    }
                    Ok(_) => Err(AppStateError::Connector(ConnectorError::NotFound)),
                    Err(e) => Err(AppStateError::Connector(e)),
                }
            }
            Err(e) => Err(AppStateError::Database(e)),
        };

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
                    "policy denied prometheus response",
                    json!({}),
                ),
            };
        }

        match result {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }

    pub async fn prometheus_query_range(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<crate::observability::prometheus::PrometheusQueryResult> {
        use crate::observability::{
            client::ObservabilityClient,
            prometheus::{self, PrometheusQueryRangeRequest},
            PROMETHEUS_CONNECTOR_KIND,
        };

        let descriptor = CommandDescriptor::new(
            "prometheus",
            "query_range",
            Capability::ResourceRead,
            thalassa_domain::Permission::Read,
        );
        if let Err(error) = self.authorize_observability(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let req =
            match serde_json::from_value::<PrometheusQueryRangeRequest>(envelope.payload.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return IpcResult::Err {
                        ok: false,
                        error: ipc_error_for(AppStateError::Serialization(e)),
                    }
                }
            };

        let result = match Connection::open(&self.database_path) {
            Ok(conn) => {
                match connectors::get(&conn, self.credential_store.as_ref(), &req.connector_id) {
                    Ok(Some(connector)) if connector.kind == PROMETHEUS_CONNECTOR_KIND => {
                        if !connector.enabled {
                            Err(AppStateError::Connector(ConnectorError::Disabled))
                        } else if !self
                            .policy
                            .evaluate_egress(EgressRequest::verified(
                                DataClass::Internal,
                                EgressDestination::ExternalIntegration,
                            ))
                            .is_allowed()
                        {
                            Err(AppStateError::PolicyDenied)
                        } else {
                            match ObservabilityClient::new(
                                &connector,
                                self.credential_store.as_ref(),
                            ) {
                                Ok(client) => prometheus::query_range(&client, req)
                                    .await
                                    .map_err(AppStateError::from),
                                Err(e) => Err(AppStateError::ObservabilityClient(e)),
                            }
                        }
                    }
                    Ok(_) => Err(AppStateError::Connector(ConnectorError::NotFound)),
                    Err(e) => Err(AppStateError::Connector(e)),
                }
            }
            Err(e) => Err(AppStateError::Database(e)),
        };

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
                    "policy denied prometheus response",
                    json!({}),
                ),
            };
        }

        match result {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }

    pub async fn loki_query_range(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<crate::observability::loki::LokiQueryResult> {
        use crate::observability::{
            client::ObservabilityClient,
            loki::{self, LokiQueryRangeRequest},
            LOKI_CONNECTOR_KIND,
        };

        let descriptor = CommandDescriptor::new(
            "loki",
            "query_range",
            Capability::ResourceRead,
            thalassa_domain::Permission::Read,
        );
        if let Err(error) = self.authorize_observability(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let req = match serde_json::from_value::<LokiQueryRangeRequest>(envelope.payload.clone()) {
            Ok(request) => request,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: ipc_error_for(AppStateError::Serialization(error)),
                }
            }
        };

        let result = match Connection::open(&self.database_path) {
            Ok(connection) => {
                match connectors::get(
                    &connection,
                    self.credential_store.as_ref(),
                    &req.connector_id,
                ) {
                    Ok(Some(connector)) if connector.kind == LOKI_CONNECTOR_KIND => {
                        if !connector.enabled {
                            Err(AppStateError::Connector(ConnectorError::Disabled))
                        } else if !self
                            .policy
                            .evaluate_egress(EgressRequest::verified(
                                DataClass::Internal,
                                EgressDestination::ExternalIntegration,
                            ))
                            .is_allowed()
                        {
                            Err(AppStateError::PolicyDenied)
                        } else {
                            match ObservabilityClient::new(
                                &connector,
                                self.credential_store.as_ref(),
                            ) {
                                Ok(client) => loki::query_range(&client, req)
                                    .await
                                    .map_err(AppStateError::from),
                                Err(error) => Err(AppStateError::ObservabilityClient(error)),
                            }
                        }
                    }
                    Ok(_) => Err(AppStateError::Connector(ConnectorError::NotFound)),
                    Err(error) => Err(AppStateError::Connector(error)),
                }
            }
            Err(error) => Err(AppStateError::Database(error)),
        };

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
                    "policy denied Loki response",
                    json!({}),
                ),
            };
        }

        match result {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }

    pub async fn alertmanager_alerts(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<crate::observability::alertmanager::NormalizedAlert>> {
        use crate::observability::alertmanager::{self, AlertmanagerAlertsRequest};
        use crate::observability::client::ObservabilityClient;
        use crate::observability::ALERTMANAGER_CONNECTOR_KIND;

        let descriptor = CommandDescriptor::new(
            "alertmanager",
            "alerts",
            Capability::ResourceRead,
            thalassa_domain::Permission::Read,
        );
        if let Err(error) = self.authorize_observability(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let req =
            match serde_json::from_value::<AlertmanagerAlertsRequest>(envelope.payload.clone()) {
                Ok(r) => r,
                Err(e) => {
                    return IpcResult::Err {
                        ok: false,
                        error: ipc_error_for(AppStateError::Serialization(e)),
                    }
                }
            };

        let result = match Connection::open(&self.database_path) {
            Ok(conn) => {
                match connectors::get(&conn, self.credential_store.as_ref(), &req.connector_id) {
                    Ok(Some(connector)) if connector.kind == ALERTMANAGER_CONNECTOR_KIND => {
                        if !connector.enabled {
                            Err(AppStateError::Connector(ConnectorError::Disabled))
                        } else if !self
                            .policy
                            .evaluate_egress(EgressRequest::verified(
                                DataClass::Internal,
                                EgressDestination::ExternalIntegration,
                            ))
                            .is_allowed()
                        {
                            Err(AppStateError::PolicyDenied)
                        } else {
                            match ObservabilityClient::new(
                                &connector,
                                self.credential_store.as_ref(),
                            ) {
                                Ok(client) => alertmanager::alerts(&client, req)
                                    .await
                                    .map_err(AppStateError::from),
                                Err(e) => Err(AppStateError::ObservabilityClient(e)),
                            }
                        }
                    }
                    Ok(_) => Err(AppStateError::Connector(ConnectorError::NotFound)),
                    Err(e) => Err(AppStateError::Connector(e)),
                }
            }
            Err(e) => Err(AppStateError::Database(e)),
        };

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
                    "policy denied alertmanager response",
                    json!({}),
                ),
            };
        }

        match result {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }

    pub async fn grafana_health(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<crate::observability::grafana::GrafanaHealth> {
        use crate::observability::{client::ObservabilityClient, grafana, GRAFANA_CONNECTOR_KIND};

        let descriptor = CommandDescriptor::new(
            "grafana",
            "health",
            Capability::ResourceRead,
            thalassa_domain::Permission::Read,
        );
        if let Err(error) = self.authorize_observability(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let req = match serde_json::from_value::<ConnectorIdRequest>(envelope.payload.clone()) {
            Ok(r) => r,
            Err(e) => {
                return IpcResult::Err {
                    ok: false,
                    error: ipc_error_for(AppStateError::Serialization(e)),
                }
            }
        };

        let result = match Connection::open(&self.database_path) {
            Ok(conn) => match connectors::get(&conn, self.credential_store.as_ref(), &req.id) {
                Ok(Some(connector)) if connector.kind == GRAFANA_CONNECTOR_KIND => {
                    if !connector.enabled {
                        Err(AppStateError::Connector(ConnectorError::Disabled))
                    } else if !self
                        .policy
                        .evaluate_egress(EgressRequest::verified(
                            DataClass::Internal,
                            EgressDestination::ExternalIntegration,
                        ))
                        .is_allowed()
                    {
                        Err(AppStateError::PolicyDenied)
                    } else {
                        match ObservabilityClient::new(&connector, self.credential_store.as_ref()) {
                            Ok(client) => {
                                grafana::health(&client).await.map_err(AppStateError::from)
                            }
                            Err(e) => Err(AppStateError::ObservabilityClient(e)),
                        }
                    }
                }
                Ok(_) => Err(AppStateError::Connector(ConnectorError::NotFound)),
                Err(e) => Err(AppStateError::Connector(e)),
            },
            Err(e) => Err(AppStateError::Database(e)),
        };

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
                    "policy denied grafana response",
                    json!({}),
                ),
            };
        }

        match result {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }

    pub async fn grafana_link(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<crate::observability::grafana::GrafanaLinkResult> {
        use crate::observability::{
            grafana::{self, GrafanaLinkRequest},
            ObservabilityConnectorConfig, GRAFANA_CONNECTOR_KIND,
        };

        let descriptor = CommandDescriptor::new(
            "grafana",
            "link",
            Capability::ResourceRead,
            thalassa_domain::Permission::Read,
        );
        if let Err(error) = self.authorize_observability(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let req = match serde_json::from_value::<GrafanaLinkRequest>(envelope.payload.clone()) {
            Ok(r) => r,
            Err(e) => {
                return IpcResult::Err {
                    ok: false,
                    error: ipc_error_for(AppStateError::Serialization(e)),
                }
            }
        };

        let result = match Connection::open(&self.database_path) {
            Ok(conn) => {
                match connectors::get(&conn, self.credential_store.as_ref(), &req.connector_id) {
                    Ok(Some(connector)) if connector.kind == GRAFANA_CONNECTOR_KIND => {
                        if !connector.enabled {
                            Err(AppStateError::Connector(ConnectorError::Disabled))
                        } else if !self
                            .policy
                            .evaluate_egress(EgressRequest::verified(
                                DataClass::Internal,
                                EgressDestination::ExternalIntegration,
                            ))
                            .is_allowed()
                        {
                            Err(AppStateError::PolicyDenied)
                        } else {
                            match ObservabilityClient::new(
                                &connector,
                                self.credential_store.as_ref(),
                            ) {
                                Ok(client) => {
                                    let config: ObservabilityConnectorConfig =
                                        match serde_json::from_value(connector.config_metadata) {
                                            Ok(c) => c,
                                            Err(e) => {
                                                return IpcResult::Err {
                                                    ok: false,
                                                    error: ipc_error_for(
                                                        AppStateError::Serialization(e),
                                                    ),
                                                }
                                            }
                                        };
                                    grafana::link(
                                        req,
                                        &client,
                                        config.datasource_uid.as_deref(),
                                        config.default_dashboard_uid.as_deref(),
                                    )
                                    .map_err(AppStateError::from)
                                }
                                Err(e) => Err(AppStateError::ObservabilityClient(e)),
                            }
                        }
                    }
                    Ok(_) => Err(AppStateError::Connector(ConnectorError::NotFound)),
                    Err(e) => Err(AppStateError::Connector(e)),
                }
            }
            Err(e) => Err(AppStateError::Database(e)),
        };

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
                    "policy denied grafana response",
                    json!({}),
                ),
            };
        }

        match result {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }

    fn authorize_observability(
        &self,
        envelope: &CommandEnvelope<Value>,
        descriptor: &CommandDescriptor,
    ) -> Result<(), IpcError> {
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
            || envelope.scope.is_bounded()
            || !descriptor.scope.contains(&envelope.scope)
            || self.bootstrap.membership.status != thalassa_domain::MembershipStatus::Active
        {
            Err(IpcError::permission_denied(
                descriptor.name.to_string(),
                envelope.scope.clone(),
            ))
        } else {
            Ok(())
        }
    }

    pub async fn kubernetes_inventory(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<KubernetesInventory> {
        self.kubernetes_command(
            envelope,
            "inventory",
            Capability::EnvironmentRead,
            |client, _| async move {
                Ok(discover(
                    client,
                    self.bootstrap.workspace.id,
                    self.bootstrap.scope.clone(),
                )
                .await)
            },
        )
        .await
    }

    pub async fn kubernetes_pod_logs(&self, envelope: CommandEnvelope<Value>) -> IpcResult<String> {
        self.kubernetes_command(
            envelope,
            "pod_logs",
            Capability::ResourceRead,
            |client, request| async move {
                pod_logs(client, &request.namespace, &request.pod)
                    .await
                    .map_err(|error| AppStateError::Kubernetes(error.to_string()))
            },
        )
        .await
    }

    pub async fn kubernetes_pod_events(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<KubernetesEvent>> {
        self.kubernetes_command(
            envelope,
            "pod_events",
            Capability::ResourceRead,
            |client, request| async move {
                pod_events(client, &request.namespace, &request.pod)
                    .await
                    .map_err(|error| AppStateError::Kubernetes(error.to_string()))
            },
        )
        .await
    }

    pub async fn kubernetes_resource_manifest(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<KubernetesManifest> {
        self.kubernetes_command(
            envelope,
            "resource_manifest",
            Capability::ResourceRead,
            |client, request| async move {
                resource_manifest(client, &request.kind, &request.namespace, &request.name)
                    .await
                    .map_err(|error| AppStateError::Kubernetes(error.to_string()))
            },
        )
        .await
    }

    async fn kubernetes_command<T, F, Fut>(
        &self,
        envelope: CommandEnvelope<Value>,
        verb: &str,
        capability: Capability,
        operation: F,
    ) -> IpcResult<T>
    where
        F: FnOnce(kube::Client, KubernetesPodRequest) -> Fut,
        Fut: std::future::Future<Output = Result<T, AppStateError>>,
    {
        let descriptor = CommandDescriptor::new(
            "kubernetes",
            verb,
            capability,
            thalassa_domain::Permission::Read,
        );
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
            || envelope.scope.is_bounded()
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
                    "policy denied Kubernetes response",
                    json!({}),
                ),
            };
        }
        let request = match serde_json::from_value::<KubernetesPodRequest>(envelope.payload) {
            Ok(request) => request,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: ipc_error_for(AppStateError::Serialization(error)),
                }
            }
        };
        let result = (|| {
            let connection =
                Connection::open(&self.database_path).map_err(AppStateError::Database)?;
            let connector = connectors::get(
                &connection,
                self.credential_store.as_ref(),
                &request.connector_id,
            )?
            .ok_or(ConnectorError::NotFound)?;
            if !connector.enabled {
                return Err(AppStateError::Connector(ConnectorError::Disabled));
            }
            if connector.kind != crate::kubernetes::KUBERNETES_CONNECTOR_KIND {
                return Err(AppStateError::Connector(
                    ConnectorError::InvalidConfiguration("connector is not Kubernetes".into()),
                ));
            }
            let config: KubernetesConnectorConfig =
                serde_json::from_value(connector.config_metadata)
                    .map_err(AppStateError::Serialization)?;
            Ok((config, request))
        })();
        let result = match result {
            Ok((config, request)) => match client_from_kubeconfig(&config).await {
                Ok(client) => operation(client, request).await,
                Err(error) => Err(AppStateError::Kubernetes(error.to_string())),
            },
            Err(error) => Err(error),
        };
        match result {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }

    fn connector_set_enabled(
        &self,
        envelope: CommandEnvelope<Value>,
        verb: &str,
        enabled: bool,
    ) -> IpcResult<ConnectorSummary> {
        self.connector_act(envelope, verb, move |connection, store, payload| {
            let request: ConnectorIdRequest =
                serde_json::from_value(payload.clone()).map_err(AppStateError::Serialization)?;
            connectors::set_enabled(connection, store, &request.id, enabled)
                .map_err(AppStateError::Connector)
        })
    }

    fn connector_read<T>(
        &self,
        envelope: CommandEnvelope<Value>,
        verb: &str,
        operation: impl FnOnce(
            &Connection,
            &dyn connectors::CredentialStore,
            &Value,
        ) -> Result<T, AppStateError>,
    ) -> IpcResult<T> {
        self.connector_command(envelope, verb, Capability::ConnectorRead, operation)
    }
    fn connector_act<T>(
        &self,
        envelope: CommandEnvelope<Value>,
        verb: &str,
        operation: impl FnOnce(
            &Connection,
            &dyn connectors::CredentialStore,
            &Value,
        ) -> Result<T, AppStateError>,
    ) -> IpcResult<T> {
        self.connector_command(envelope, verb, Capability::ConnectorAct, operation)
    }
    fn connector_command<T>(
        &self,
        envelope: CommandEnvelope<Value>,
        verb: &str,
        capability: Capability,
        operation: impl FnOnce(
            &Connection,
            &dyn connectors::CredentialStore,
            &Value,
        ) -> Result<T, AppStateError>,
    ) -> IpcResult<T> {
        let descriptor = CommandDescriptor::new(
            "connector",
            verb,
            capability,
            thalassa_domain::Permission::Read,
        );
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
            || envelope.scope.is_bounded()
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
                    "policy denied connector response",
                    json!({}),
                ),
            };
        }
        match Connection::open(&self.database_path)
            .map_err(AppStateError::Database)
            .and_then(|connection| {
                operation(
                    &connection,
                    self.credential_store.as_ref(),
                    &envelope.payload,
                )
            }) {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: ipc_error_for(error),
            },
        }
    }
}

pub(crate) fn apply_migrations(connection: &Connection) -> Result<(), AppStateError> {
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
    connection.execute_batch(CONNECTOR_MIGRATION)?;
    let connector_migration: Option<i64> = connection
        .query_row(
            "SELECT version FROM schema_migrations WHERE version = 2",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if connector_migration.is_none() {
        connection.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (2, ?1)",
            [Utc::now().to_rfc3339()],
        )?;
    }
    Ok(())
}

fn load_or_bootstrap(connection: &mut Connection) -> Result<BootstrapState, AppStateError> {
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
    let transaction = connection.transaction()?;
    persist(
        &transaction,
        "principals",
        principal.id.to_string(),
        &principal,
    )?;
    persist(
        &transaction,
        "organizations",
        organization.id.to_string(),
        &organization,
    )?;
    persist(&transaction, "teams", team.id.to_string(), &team)?;
    persist(
        &transaction,
        "workspaces",
        workspace.id.to_string(),
        &workspace,
    )?;
    persist(
        &transaction,
        "memberships",
        principal.id.to_string(),
        &membership,
    )?;
    transaction.commit()?;
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
    #[error("connector error: {0}")]
    Connector(#[from] ConnectorError),
    #[error("observability client error: {0}")]
    ObservabilityClient(#[from] crate::observability::client::ObservabilityClientError),
    #[error("prometheus error: {0}")]
    Prometheus(#[from] crate::observability::prometheus::PrometheusError),
    #[error("alertmanager error: {0}")]
    Alertmanager(#[from] crate::observability::alertmanager::AlertmanagerError),
    #[error("grafana error: {0}")]
    Grafana(#[from] crate::observability::grafana::GrafanaError),
    #[error("loki error: {0}")]
    Loki(#[from] crate::observability::loki::LokiError),
    #[error("policy denied")]
    PolicyDenied,
    #[error("kubernetes error: {0}")]
    Kubernetes(String),
}

fn ipc_error_for(error: AppStateError) -> IpcError {
    match error {
        AppStateError::Connector(ConnectorError::NotFound) => {
            IpcError::new(IpcErrorCode::NotFound, "connector not found", json!({}))
        }
        AppStateError::Connector(ConnectorError::Disabled) => IpcError::new(
            IpcErrorCode::ConnectorUnavailable,
            "connector is disabled",
            json!({}),
        ),
        AppStateError::Connector(ConnectorError::InvalidConfiguration(_)) => IpcError::new(
            IpcErrorCode::InvalidRequest,
            "invalid connector configuration",
            json!({}),
        ),
        AppStateError::Serialization(_) => IpcError::new(
            IpcErrorCode::InvalidRequest,
            "invalid request payload",
            json!({}),
        ),
        AppStateError::PolicyDenied => IpcError::new(
            IpcErrorCode::PolicyDenied,
            "policy denied connector response",
            json!({}),
        ),
        AppStateError::Prometheus(crate::observability::prometheus::PrometheusError::Client(
            err,
        ))
        | AppStateError::Alertmanager(
            crate::observability::alertmanager::AlertmanagerError::Client(err),
        )
        | AppStateError::Grafana(crate::observability::grafana::GrafanaError::Client(err))
        | AppStateError::Loki(crate::observability::loki::LokiError::Client(err))
        | AppStateError::ObservabilityClient(err) => match err {
            crate::observability::client::ObservabilityClientError::MalformedResponse => {
                IpcError::new(
                    IpcErrorCode::MalformedResponse,
                    "malformed response from provider",
                    json!({}),
                )
            }
            crate::observability::client::ObservabilityClientError::Configuration(_)
            | crate::observability::client::ObservabilityClientError::InvalidUrl(_) => {
                IpcError::new(
                    IpcErrorCode::InvalidRequest,
                    "invalid configuration",
                    json!({}),
                )
            }
            _ => IpcError::new(
                IpcErrorCode::InternalError,
                "connector operation failed",
                json!({}),
            ),
        },
        AppStateError::Prometheus(
            crate::observability::prometheus::PrometheusError::Validation(_),
        )
        | AppStateError::Grafana(crate::observability::grafana::GrafanaError::Validation(_))
        | AppStateError::Loki(crate::observability::loki::LokiError::Validation(_)) => {
            IpcError::new(IpcErrorCode::InvalidRequest, "invalid request", json!({}))
        }
        AppStateError::Loki(crate::observability::loki::LokiError::Provider(_)) => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "malformed response from provider",
            json!({}),
        ),
        _ => IpcError::new(
            IpcErrorCode::InternalError,
            "connector operation failed",
            json!({}),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::InMemoryCredentialStore;
    use httpmock::MockServer;
    use std::sync::Arc;
    use tempfile::tempdir;
    use thalassa_domain::MembershipStatus;
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

    fn context_envelope(state: &AppState) -> CommandEnvelope<Value> {
        CommandEnvelope {
            request_id: Uuid::new_v4(),
            command: thalassa_ipc::CommandName::new("system", "context").unwrap(),
            capability: Capability::WorkspaceRead,
            scope: state.bootstrap.scope.clone(),
            payload: Value::Null,
        }
    }

    fn connector_envelope(
        state: &AppState,
        verb: &str,
        capability: Capability,
        payload: Value,
    ) -> CommandEnvelope<Value> {
        CommandEnvelope {
            request_id: Uuid::new_v4(),
            command: thalassa_ipc::CommandName::new("connector", verb).unwrap(),
            capability,
            scope: state.bootstrap.scope.clone(),
            payload,
        }
    }

    fn kubernetes_envelope(
        state: &AppState,
        verb: &str,
        capability: Capability,
        payload: Value,
    ) -> CommandEnvelope<Value> {
        CommandEnvelope {
            request_id: Uuid::new_v4(),
            command: thalassa_ipc::CommandName::new("kubernetes", verb).unwrap(),
            capability,
            scope: state.bootstrap.scope.clone(),
            payload,
        }
    }

    fn test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempdir().unwrap();
        let state = AppState::open_with_credential_store(
            directory.path().join("thalassaops.sqlite"),
            Arc::new(InMemoryCredentialStore::default()),
        )
        .unwrap();
        (directory, state)
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
    fn bootstrap_rolls_back_every_record_when_a_write_fails() {
        let mut connection = Connection::open_in_memory().unwrap();
        apply_migrations(&connection).unwrap();
        connection
            .execute_batch(
                r#"
                    CREATE TRIGGER interrupt_bootstrap BEFORE INSERT ON teams
                    BEGIN
                        SELECT RAISE(ABORT, 'interrupted bootstrap');
                    END;
                "#,
            )
            .unwrap();

        assert!(load_or_bootstrap(&mut connection).is_err());

        for table in [
            "principals",
            "organizations",
            "teams",
            "workspaces",
            "memberships",
        ] {
            let statement = format!("SELECT COUNT(*) FROM {table}");
            let count: i64 = connection
                .query_row(&statement, [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0, "{table} should be empty after a failed bootstrap");
        }
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

    #[test]
    fn context_command_returns_bootstrapped_hierarchy_and_policy_version() {
        let directory = tempdir().unwrap();
        let state = AppState::open(directory.path().join("thalassaops.sqlite")).unwrap();
        let IpcResult::Ok { value, .. } = state.context(context_envelope(&state)) else {
            panic!("context should succeed")
        };
        assert_eq!(value.organization_name, "Local Organization");
        assert_eq!(value.team_name, "Local Team");
        assert_eq!(value.workspace_name, "Local Workspace");
        assert_eq!(value.policy_version, state.policy.version());
    }

    #[test]
    fn context_command_rejects_wrong_scope() {
        let directory = tempdir().unwrap();
        let state = AppState::open(directory.path().join("thalassaops.sqlite")).unwrap();
        let mut envelope = context_envelope(&state);
        envelope.scope.workspace_id = Some(state.bootstrap.workspace.id);
        assert!(matches!(state.context(envelope), IpcResult::Err { .. }));
    }

    #[tokio::test]
    async fn connector_registry_persists_metadata_but_never_a_credential_value() {
        let (directory, state) = test_state();
        let secret = "fixture-secret-must-not-leak";
        let added = state.connector_add(connector_envelope(&state, "add", Capability::ConnectorAct, json!({ "kind": "fixture", "display_name": "Fixture", "config_metadata": { "fixture_health": "healthy" }, "credential_value": secret })));
        let IpcResult::Ok {
            value: connector, ..
        } = added
        else {
            panic!("add should succeed")
        };
        assert!(connector.credential_configured);
        let connection = Connection::open(directory.path().join("thalassaops.sqlite")).unwrap();
        let serialized_row: String = connection.query_row("SELECT kind || display_name || config_metadata_json || COALESCE(credential_reference, '') FROM connector_instances WHERE id = ?1", [&connector.id], |row| row.get(0)).unwrap();
        assert!(!serialized_row.contains(secret));
        let list_json = serde_json::to_string(&state.connector_list(connector_envelope(
            &state,
            "list",
            Capability::ConnectorRead,
            Value::Null,
        )))
        .unwrap();
        let _ = state
            .connector_test(connector_envelope(
                &state,
                "test",
                Capability::ConnectorAct,
                json!({ "id": connector.id }),
            ))
            .await;
        let diagnose_json = serde_json::to_string(&state.connector_diagnose(connector_envelope(
            &state,
            "diagnose",
            Capability::ConnectorRead,
            json!({ "id": connector.id }),
        )))
        .unwrap();
        assert!(!list_json.contains(secret));
        assert!(!diagnose_json.contains(secret));
    }

    #[tokio::test]
    async fn fixture_connector_can_be_tested_disabled_and_diagnosed() {
        let (_directory, state) = test_state();
        let IpcResult::Ok { value: connector, .. } = state.connector_add(connector_envelope(&state, "add", Capability::ConnectorAct, json!({ "kind": "fixture", "display_name": "Test fixture", "config_metadata": { "fixture_health": "timeout", "fixture_timeout_ms": 1, "fixture_retry_backoff_ms": 1 } }))) else { panic!("add should succeed") };
        let IpcResult::Ok { value: checked, .. } = state
            .connector_test(connector_envelope(
                &state,
                "test",
                Capability::ConnectorAct,
                json!({ "id": connector.id }),
            ))
            .await
        else {
            panic!("test should succeed")
        };
        assert_eq!(checked.health_state, "unavailable");
        assert!(checked.last_checked_at.is_some());
        let IpcResult::Ok {
            value: diagnostics, ..
        } = state.connector_diagnose(connector_envelope(
            &state,
            "diagnose",
            Capability::ConnectorRead,
            json!({ "id": connector.id }),
        ))
        else {
            panic!("diagnose should succeed")
        };
        assert_eq!(diagnostics.manifest.id, "fixture");
        assert_eq!(diagnostics.logs[0].outcome, "unavailable");
        assert!(matches!(
            state.connector_disable(connector_envelope(
                &state,
                "disable",
                Capability::ConnectorAct,
                json!({ "id": connector.id })
            )),
            IpcResult::Ok { .. }
        ));
        assert!(matches!(
            state
                .connector_test(connector_envelope(
                    &state,
                    "test",
                    Capability::ConnectorAct,
                    json!({ "id": connector.id })
                ))
                .await,
            IpcResult::Err { .. }
        ));
    }

    #[tokio::test]
    async fn kubernetes_inventory_returns_an_ipc_error_inside_a_tokio_runtime() {
        let (_directory, state) = test_state();
        let IpcResult::Ok {
            value: connector, ..
        } = state.connector_add(connector_envelope(
            &state,
            "add",
            Capability::ConnectorAct,
            json!({
                "kind": "kubernetes",
                "display_name": "Unavailable Kubernetes",
                "config_metadata": {
                    "kubeconfig_path": "/definitely/not/a/kubeconfig",
                    "context_name": "missing"
                }
            }),
        ))
        else {
            panic!("add should succeed")
        };

        assert!(matches!(
            state
                .kubernetes_inventory(kubernetes_envelope(
                    &state,
                    "inventory",
                    Capability::EnvironmentRead,
                    json!({ "connector_id": connector.id }),
                ))
                .await,
            IpcResult::Err { .. }
        ));
    }
    fn observability_envelope_with_scope(
        scope: ResourceScope,
        resource: &str,
        verb: &str,
        capability: Capability,
        payload: Value,
    ) -> CommandEnvelope<Value> {
        CommandEnvelope {
            request_id: Uuid::new_v4(),
            command: thalassa_ipc::CommandName::new(resource, verb).unwrap(),
            capability,
            scope,
            payload,
        }
    }

    fn observability_envelope(
        state: &AppState,
        resource: &str,
        verb: &str,
        capability: Capability,
        payload: Value,
    ) -> CommandEnvelope<Value> {
        observability_envelope_with_scope(
            state.bootstrap.scope.clone(),
            resource,
            verb,
            capability,
            payload,
        )
    }

    fn add_observability_connector(
        state: &AppState,
        kind: &str,
        base_url: &str,
        auth_mode: &str,
        credential: Option<&str>,
    ) -> ConnectorSummary {
        let mut config = json!({
            "base_url": base_url,
            "auth_mode": auth_mode,
        });
        if auth_mode == "basic" {
            config["username"] = json!("test-user");
        }
        if kind == "grafana" {
            config["datasource_uid"] = json!("datasource-main");
            config["default_dashboard_uid"] = json!("dashboard-main");
        }

        let mut payload = json!({
            "kind": kind,
            "display_name": format!("{kind} test connector"),
            "config_metadata": config,
        });
        if let Some(credential) = credential {
            payload["credential_value"] = json!(credential);
        }

        let IpcResult::Ok { value, .. } = state.connector_add(connector_envelope(
            state,
            "add",
            Capability::ConnectorAct,
            payload,
        )) else {
            panic!("observability connector should be added");
        };
        value
    }

    fn assert_error_code<T>(result: IpcResult<T>, expected: IpcErrorCode) {
        match result {
            IpcResult::Err { error, .. } => assert_eq!(error.code, expected),
            IpcResult::Ok { .. } => panic!("expected IPC error"),
        }
    }

    fn assert_result_has_no_secret_or_credential_reference<T: Serialize>(
        result: &IpcResult<T>,
        secret: &str,
    ) {
        let serialized = serde_json::to_string(result).unwrap();
        assert!(
            !serialized.contains(secret),
            "IPC result contains a fixture secret"
        );
        assert!(
            !serialized.contains("credential_reference"),
            "IPC result contains a credential reference"
        );
    }

    #[tokio::test]
    async fn observability_commands_enforce_authorization_and_policy() {
        let (_directory, mut state) = test_state();
        let server = MockServer::start();
        let prom = add_observability_connector(&state, "prometheus", &server.url(""), "none", None);
        let alertmanager =
            add_observability_connector(&state, "alertmanager", &server.url(""), "none", None);
        let grafana = add_observability_connector(&state, "grafana", &server.url(""), "none", None);
        let query = |connector_id: &str| json!({ "connector_id": connector_id, "query": "up" });
        let query_range = |connector_id: &str| {
            json!({
                "connector_id": connector_id,
                "query": "up",
                "start": "2024-01-01T00:00:00Z",
                "end": "2024-01-01T01:00:00Z",
                "step_seconds": 60,
            })
        };
        let alerts = |connector_id: &str| json!({ "connector_id": connector_id });
        let health = |connector_id: &str| json!({ "id": connector_id });
        let link = |connector_id: &str| {
            json!({
                "connector_id": connector_id,
                "target": "dashboard",
                "query": "up",
                "start": "2024-01-01T00:00:00Z",
                "end": "2024-01-01T01:00:00Z",
            })
        };

        let mut wrong_command = observability_envelope(
            &state,
            "prometheus",
            "query_range",
            Capability::ResourceRead,
            query(&prom.id),
        );
        assert_error_code(
            state.prometheus_query(wrong_command).await,
            IpcErrorCode::PermissionDenied,
        );
        wrong_command = observability_envelope(
            &state,
            "prometheus",
            "query",
            Capability::ResourceRead,
            query_range(&prom.id),
        );
        assert_error_code(
            state.prometheus_query_range(wrong_command).await,
            IpcErrorCode::PermissionDenied,
        );
        wrong_command = observability_envelope(
            &state,
            "alertmanager",
            "query",
            Capability::ResourceRead,
            alerts(&alertmanager.id),
        );
        assert_error_code(
            state.alertmanager_alerts(wrong_command).await,
            IpcErrorCode::PermissionDenied,
        );
        wrong_command = observability_envelope(
            &state,
            "grafana",
            "link",
            Capability::ResourceRead,
            health(&grafana.id),
        );
        assert_error_code(
            state.grafana_health(wrong_command).await,
            IpcErrorCode::PermissionDenied,
        );
        wrong_command = observability_envelope(
            &state,
            "grafana",
            "health",
            Capability::ResourceRead,
            link(&grafana.id),
        );
        assert_error_code(
            state.grafana_link(wrong_command).await,
            IpcErrorCode::PermissionDenied,
        );

        let mut wrong_capability = observability_envelope(
            &state,
            "prometheus",
            "query",
            Capability::ResourceRead,
            query(&prom.id),
        );
        wrong_capability.capability = Capability::WorkspaceRead;
        assert_error_code(
            state.prometheus_query(wrong_capability).await,
            IpcErrorCode::PermissionDenied,
        );
        let mut wrong_capability = observability_envelope(
            &state,
            "prometheus",
            "query_range",
            Capability::ResourceRead,
            query_range(&prom.id),
        );
        wrong_capability.capability = Capability::WorkspaceRead;
        assert_error_code(
            state.prometheus_query_range(wrong_capability).await,
            IpcErrorCode::PermissionDenied,
        );
        let mut wrong_capability = observability_envelope(
            &state,
            "alertmanager",
            "alerts",
            Capability::ResourceRead,
            alerts(&alertmanager.id),
        );
        wrong_capability.capability = Capability::WorkspaceRead;
        assert_error_code(
            state.alertmanager_alerts(wrong_capability).await,
            IpcErrorCode::PermissionDenied,
        );
        let mut wrong_capability = observability_envelope(
            &state,
            "grafana",
            "health",
            Capability::ResourceRead,
            health(&grafana.id),
        );
        wrong_capability.capability = Capability::WorkspaceRead;
        assert_error_code(
            state.grafana_health(wrong_capability).await,
            IpcErrorCode::PermissionDenied,
        );
        let mut wrong_capability = observability_envelope(
            &state,
            "grafana",
            "link",
            Capability::ResourceRead,
            link(&grafana.id),
        );
        wrong_capability.capability = Capability::WorkspaceRead;
        assert_error_code(
            state.grafana_link(wrong_capability).await,
            IpcErrorCode::PermissionDenied,
        );

        let bounded_scope = ResourceScope::workspace(
            state.bootstrap.workspace.id,
            state.bootstrap.team.id,
            state.bootstrap.organization.id,
        );
        assert_error_code(
            state
                .prometheus_query(observability_envelope_with_scope(
                    bounded_scope.clone(),
                    "prometheus",
                    "query",
                    Capability::ResourceRead,
                    query(&prom.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        assert_error_code(
            state
                .prometheus_query_range(observability_envelope_with_scope(
                    bounded_scope.clone(),
                    "prometheus",
                    "query_range",
                    Capability::ResourceRead,
                    query_range(&prom.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        assert_error_code(
            state
                .alertmanager_alerts(observability_envelope_with_scope(
                    bounded_scope.clone(),
                    "alertmanager",
                    "alerts",
                    Capability::ResourceRead,
                    alerts(&alertmanager.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        assert_error_code(
            state
                .grafana_health(observability_envelope_with_scope(
                    bounded_scope.clone(),
                    "grafana",
                    "health",
                    Capability::ResourceRead,
                    health(&grafana.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        assert_error_code(
            state
                .grafana_link(observability_envelope_with_scope(
                    bounded_scope,
                    "grafana",
                    "link",
                    Capability::ResourceRead,
                    link(&grafana.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );

        state.bootstrap.membership.status = MembershipStatus::Suspended;
        assert_error_code(
            state
                .prometheus_query(observability_envelope(
                    &state,
                    "prometheus",
                    "query",
                    Capability::ResourceRead,
                    query(&prom.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        assert_error_code(
            state
                .prometheus_query_range(observability_envelope(
                    &state,
                    "prometheus",
                    "query_range",
                    Capability::ResourceRead,
                    query_range(&prom.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        assert_error_code(
            state
                .alertmanager_alerts(observability_envelope(
                    &state,
                    "alertmanager",
                    "alerts",
                    Capability::ResourceRead,
                    alerts(&alertmanager.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        assert_error_code(
            state
                .grafana_health(observability_envelope(
                    &state,
                    "grafana",
                    "health",
                    Capability::ResourceRead,
                    health(&grafana.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        assert_error_code(
            state
                .grafana_link(observability_envelope(
                    &state,
                    "grafana",
                    "link",
                    Capability::ResourceRead,
                    link(&grafana.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        state.bootstrap.membership.status = MembershipStatus::Active;

        state.policy = PolicyRuntime::load(
            PolicyDocument::baseline(2).with_external_integration_data_classes(vec![]),
        )
        .unwrap();
        assert_error_code(
            state
                .prometheus_query(observability_envelope(
                    &state,
                    "prometheus",
                    "query",
                    Capability::ResourceRead,
                    query(&prom.id),
                ))
                .await,
            IpcErrorCode::PolicyDenied,
        );
        assert_error_code(
            state
                .prometheus_query_range(observability_envelope(
                    &state,
                    "prometheus",
                    "query_range",
                    Capability::ResourceRead,
                    query_range(&prom.id),
                ))
                .await,
            IpcErrorCode::PolicyDenied,
        );
        assert_error_code(
            state
                .alertmanager_alerts(observability_envelope(
                    &state,
                    "alertmanager",
                    "alerts",
                    Capability::ResourceRead,
                    alerts(&alertmanager.id),
                ))
                .await,
            IpcErrorCode::PolicyDenied,
        );
        assert_error_code(
            state
                .grafana_health(observability_envelope(
                    &state,
                    "grafana",
                    "health",
                    Capability::ResourceRead,
                    health(&grafana.id),
                ))
                .await,
            IpcErrorCode::PolicyDenied,
        );
        assert_error_code(
            state
                .grafana_link(observability_envelope(
                    &state,
                    "grafana",
                    "link",
                    Capability::ResourceRead,
                    link(&grafana.id),
                ))
                .await,
            IpcErrorCode::PolicyDenied,
        );
        state.policy = PolicyRuntime::baseline();

        assert_error_code(
            state
                .prometheus_query(observability_envelope(
                    &state,
                    "prometheus",
                    "query",
                    Capability::ResourceRead,
                    query(&alertmanager.id),
                ))
                .await,
            IpcErrorCode::NotFound,
        );
        assert_error_code(
            state
                .prometheus_query_range(observability_envelope(
                    &state,
                    "prometheus",
                    "query_range",
                    Capability::ResourceRead,
                    query_range(&grafana.id),
                ))
                .await,
            IpcErrorCode::NotFound,
        );
        assert_error_code(
            state
                .alertmanager_alerts(observability_envelope(
                    &state,
                    "alertmanager",
                    "alerts",
                    Capability::ResourceRead,
                    alerts(&prom.id),
                ))
                .await,
            IpcErrorCode::NotFound,
        );
        assert_error_code(
            state
                .grafana_health(observability_envelope(
                    &state,
                    "grafana",
                    "health",
                    Capability::ResourceRead,
                    health(&alertmanager.id),
                ))
                .await,
            IpcErrorCode::NotFound,
        );
        assert_error_code(
            state
                .grafana_link(observability_envelope(
                    &state,
                    "grafana",
                    "link",
                    Capability::ResourceRead,
                    link(&prom.id),
                ))
                .await,
            IpcErrorCode::NotFound,
        );

        for connector in [&prom, &alertmanager, &grafana] {
            assert!(matches!(
                state.connector_disable(connector_envelope(
                    &state,
                    "disable",
                    Capability::ConnectorAct,
                    json!({ "id": connector.id }),
                )),
                IpcResult::Ok { .. }
            ));
        }
        assert_error_code(
            state
                .prometheus_query(observability_envelope(
                    &state,
                    "prometheus",
                    "query",
                    Capability::ResourceRead,
                    query(&prom.id),
                ))
                .await,
            IpcErrorCode::ConnectorUnavailable,
        );
        assert_error_code(
            state
                .prometheus_query_range(observability_envelope(
                    &state,
                    "prometheus",
                    "query_range",
                    Capability::ResourceRead,
                    query_range(&prom.id),
                ))
                .await,
            IpcErrorCode::ConnectorUnavailable,
        );
        assert_error_code(
            state
                .alertmanager_alerts(observability_envelope(
                    &state,
                    "alertmanager",
                    "alerts",
                    Capability::ResourceRead,
                    alerts(&alertmanager.id),
                ))
                .await,
            IpcErrorCode::ConnectorUnavailable,
        );
        assert_error_code(
            state
                .grafana_health(observability_envelope(
                    &state,
                    "grafana",
                    "health",
                    Capability::ResourceRead,
                    health(&grafana.id),
                ))
                .await,
            IpcErrorCode::ConnectorUnavailable,
        );
        assert_error_code(
            state
                .grafana_link(observability_envelope(
                    &state,
                    "grafana",
                    "link",
                    Capability::ResourceRead,
                    link(&grafana.id),
                ))
                .await,
            IpcErrorCode::ConnectorUnavailable,
        );
    }

    #[tokio::test]
    async fn observability_commands_return_safe_success_results() {
        let (_directory, state) = test_state();
        let server = MockServer::start();
        let query_mock = server.mock(|when, then| {
            when.method("GET")
                .path("/api/v1/query")
                .query_param("query", "up")
                .header("Authorization", "Bearer prometheus-secret");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json!({
                        "status": "success",
                        "data": {
                            "resultType": "vector",
                            "result": [{
                                "metric": { "__name__": "up" },
                                "value": [1704067200.0, "1"]
                            }]
                        }
                    })
                    .to_string(),
                );
        });
        let range_mock = server.mock(|when, then| {
            when.method("GET")
                .path("/api/v1/query_range")
                .query_param("query", "up")
                .query_param("step", "60")
                .query_param_exists("start")
                .query_param_exists("end")
                .header("Authorization", "Bearer prometheus-secret");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json!({
                        "status": "success",
                        "data": {
                            "resultType": "matrix",
                            "result": [{
                                "metric": { "__name__": "up" },
                                "values": [[1704067200.0, "1"], [1704067260.0, "1"]]
                            }]
                        }
                    })
                    .to_string(),
                );
        });
        let alert_mock = server.mock(|when, then| {
            when.method("GET")
                .path("/api/v2/alerts")
                .header("Authorization", "Bearer alertmanager-secret");
            then.status(200)
                .header("content-type", "application/json")
                .body(
                    json!([{
                        "fingerprint": "alert-1",
                        "status": { "state": "firing" },
                        "startsAt": "2024-01-01T00:00:00Z",
                        "endsAt": "2024-01-01T01:00:00Z",
                        "labels": { "alertname": "HighCPU", "namespace": "prod", "pod": "api" },
                        "annotations": { "summary": "CPU is high" },
                        "generatorURL": "http://prometheus.example.test/graph"
                    }])
                    .to_string(),
                );
        });
        let health_mock = server.mock(|when, then| {
            when.method("GET")
                .path("/api/health")
                .header("Authorization", "Bearer grafana-secret");
            then.status(200)
                .header("content-type", "application/json")
                .body(json!({ "database": "ok", "version": "10.0.0" }).to_string());
        });

        let prom = add_observability_connector(
            &state,
            "prometheus",
            &server.url(""),
            "bearer",
            Some("prometheus-secret"),
        );
        let alertmanager = add_observability_connector(
            &state,
            "alertmanager",
            &server.url(""),
            "bearer",
            Some("alertmanager-secret"),
        );
        let grafana = add_observability_connector(
            &state,
            "grafana",
            &server.url(""),
            "bearer",
            Some("grafana-secret"),
        );

        let query_result = state
            .prometheus_query(observability_envelope(
                &state,
                "prometheus",
                "query",
                Capability::ResourceRead,
                json!({ "connector_id": prom.id, "query": "up" }),
            ))
            .await;
        assert_result_has_no_secret_or_credential_reference(&query_result, "prometheus-secret");
        let IpcResult::Ok { value, .. } = &query_result else {
            panic!("instant query should succeed");
        };
        assert_eq!(value.source.connector_id, prom.id);
        assert_eq!(value.source.endpoint, "/api/v1/query");

        let range_result = state
            .prometheus_query_range(observability_envelope(
                &state,
                "prometheus",
                "query_range",
                Capability::ResourceRead,
                json!({
                    "connector_id": prom.id,
                    "query": "up",
                    "start": "2024-01-01T00:00:00Z",
                    "end": "2024-01-01T01:00:00Z",
                    "step_seconds": 60,
                }),
            ))
            .await;
        assert_result_has_no_secret_or_credential_reference(&range_result, "prometheus-secret");
        let IpcResult::Ok { value, .. } = &range_result else {
            panic!("range query should succeed");
        };
        assert_eq!(value.source.connector_id, prom.id);
        assert_eq!(value.source.endpoint, "/api/v1/query_range");
        assert_eq!(value.series[0].samples.len(), 2);

        let alerts_result = state
            .alertmanager_alerts(observability_envelope(
                &state,
                "alertmanager",
                "alerts",
                Capability::ResourceRead,
                json!({ "connector_id": alertmanager.id }),
            ))
            .await;
        assert_result_has_no_secret_or_credential_reference(&alerts_result, "alertmanager-secret");
        let IpcResult::Ok { value, .. } = &alerts_result else {
            panic!("alert ingestion should succeed");
        };
        assert_eq!(value[0].source.connector_id, alertmanager.id);
        assert_eq!(
            value[0].resource_reference,
            crate::observability::alertmanager::ResourceReference::Resolved {
                namespace: "prod".into(),
                kind: "Pod".into(),
                name: "api".into(),
            }
        );

        let health_result = state
            .grafana_health(observability_envelope(
                &state,
                "grafana",
                "health",
                Capability::ResourceRead,
                json!({ "id": grafana.id }),
            ))
            .await;
        assert_result_has_no_secret_or_credential_reference(&health_result, "grafana-secret");
        let IpcResult::Ok { value, .. } = &health_result else {
            panic!("Grafana health should succeed");
        };
        assert_eq!(value.version, "10.0.0");

        let link_result = state
            .grafana_link(observability_envelope(
                &state,
                "grafana",
                "link",
                Capability::ResourceRead,
                json!({
                    "connector_id": grafana.id,
                    "target": "dashboard",
                    "query": "up",
                    "start": "2024-01-01T00:00:00Z",
                    "end": "2024-01-01T01:00:00Z",
                }),
            ))
            .await;
        assert_result_has_no_secret_or_credential_reference(&link_result, "grafana-secret");
        let IpcResult::Ok { value, .. } = &link_result else {
            panic!("Grafana link should succeed");
        };
        let parsed = reqwest::Url::parse(&value.url).unwrap();
        assert_eq!(parsed.path(), "/d/dashboard-main");
        let query: std::collections::BTreeMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(query.len(), 2);
        assert_eq!(query.get("from"), Some(&"1704067200000".to_string()));
        assert_eq!(query.get("to"), Some(&"1704070800000".to_string()));
        assert!(!value.url.contains("var-query"));

        query_mock.assert();
        range_mock.assert();
        alert_mock.assert();
        health_mock.assert();
    }

    #[tokio::test]
    async fn loki_query_range_enforces_authorization_and_returns_masked_data() {
        let (_directory, mut state) = test_state();
        let server = MockServer::start();
        let loki = add_observability_connector(&state, "loki", &server.url(""), "none", None);
        let prometheus =
            add_observability_connector(&state, "prometheus", &server.url(""), "none", None);
        let secret = "loki-secret";
        let secure_loki =
            add_observability_connector(&state, "loki", &server.url(""), "bearer", Some(secret));
        let query = |connector_id: &str| {
            json!({
                "connector_id": connector_id,
                "query": "{namespace=\"prod\"}",
                "start": "2024-01-01T00:00:00Z",
                "end": "2024-01-01T01:00:00Z",
                "limit": 20,
            })
        };

        let response = json!({
            "status": "success",
            "data": {
                "resultType": "streams",
                "result": [{
                    "stream": {"namespace": "prod"},
                    "values": [["1735689600000000001", "{\"api_key\":\"sk-live-1\"}"]]
                }]
            }
        });
        let query_mock = server.mock(|when, then| {
            when.method("GET")
                .path("/loki/api/v1/query_range")
                .query_param("query", "{namespace=\"prod\"}")
                .query_param("limit", "20")
                .query_param("direction", "backward")
                .query_param_exists("start")
                .query_param_exists("end")
                .header("Authorization", "Bearer loki-secret");
            then.status(200)
                .header("content-type", "application/json")
                .body(response.to_string());
        });

        let wrong_command = observability_envelope(
            &state,
            "loki",
            "health",
            Capability::ResourceRead,
            query(&secure_loki.id),
        );
        assert_error_code(
            state.loki_query_range(wrong_command).await,
            IpcErrorCode::PermissionDenied,
        );

        let mut wrong_capability = observability_envelope(
            &state,
            "loki",
            "query_range",
            Capability::ResourceRead,
            query(&secure_loki.id),
        );
        wrong_capability.capability = Capability::WorkspaceRead;
        assert_error_code(
            state.loki_query_range(wrong_capability).await,
            IpcErrorCode::PermissionDenied,
        );

        let bounded_scope = ResourceScope::workspace(
            state.bootstrap.workspace.id,
            state.bootstrap.team.id,
            state.bootstrap.organization.id,
        );
        assert_error_code(
            state
                .loki_query_range(observability_envelope_with_scope(
                    bounded_scope,
                    "loki",
                    "query_range",
                    Capability::ResourceRead,
                    query(&secure_loki.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );

        state.bootstrap.membership.status = MembershipStatus::Suspended;
        assert_error_code(
            state
                .loki_query_range(observability_envelope(
                    &state,
                    "loki",
                    "query_range",
                    Capability::ResourceRead,
                    query(&secure_loki.id),
                ))
                .await,
            IpcErrorCode::PermissionDenied,
        );
        state.bootstrap.membership.status = MembershipStatus::Active;

        assert_error_code(
            state
                .loki_query_range(observability_envelope(
                    &state,
                    "loki",
                    "query_range",
                    Capability::ResourceRead,
                    query(&prometheus.id),
                ))
                .await,
            IpcErrorCode::NotFound,
        );

        state.policy = PolicyRuntime::load(
            PolicyDocument::baseline(2).with_external_integration_data_classes(vec![]),
        )
        .unwrap();
        assert_error_code(
            state
                .loki_query_range(observability_envelope(
                    &state,
                    "loki",
                    "query_range",
                    Capability::ResourceRead,
                    query(&secure_loki.id),
                ))
                .await,
            IpcErrorCode::PolicyDenied,
        );
        state.policy = PolicyRuntime::baseline();

        assert!(matches!(
            state.connector_disable(connector_envelope(
                &state,
                "disable",
                Capability::ConnectorAct,
                json!({ "id": loki.id }),
            )),
            IpcResult::Ok { .. }
        ));
        assert_error_code(
            state
                .loki_query_range(observability_envelope(
                    &state,
                    "loki",
                    "query_range",
                    Capability::ResourceRead,
                    query(&loki.id),
                ))
                .await,
            IpcErrorCode::ConnectorUnavailable,
        );

        let result = state
            .loki_query_range(observability_envelope(
                &state,
                "loki",
                "query_range",
                Capability::ResourceRead,
                query(&secure_loki.id),
            ))
            .await;
        assert_result_has_no_secret_or_credential_reference(&result, secret);
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(!serialized.contains("sk-live-"));
        let IpcResult::Ok { value, .. } = result else {
            panic!("Loki query should succeed")
        };
        assert_eq!(
            value.streams[0].entries[0].fields.as_ref().unwrap()["api_key"],
            "<REDACTED>"
        );
        assert_eq!(value.unparsed_count, 0);
        query_mock.assert();
    }
}
