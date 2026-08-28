use super::*;
use crate::connectors as connector_store;
use crate::connectors::ConnectorIdRequest;
use crate::observability::client::ObservabilityClient;

impl AppState {
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
                match connector_store::get(&conn, self.credential_store.as_ref(), &req.connector_id)
                {
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
                match connector_store::get(&conn, self.credential_store.as_ref(), &req.connector_id)
                {
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
                match connector_store::get(
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

    pub async fn tempo_trace(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<crate::observability::tempo::TraceResult> {
        use crate::observability::{
            client::ObservabilityClient,
            tempo::{self, TempoTraceRequest},
            TEMPO_CONNECTOR_KIND,
        };

        let descriptor = CommandDescriptor::new(
            "tempo",
            "trace",
            Capability::ResourceRead,
            thalassa_domain::Permission::Read,
        );
        if let Err(error) = self.authorize_observability(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let req = match serde_json::from_value::<TempoTraceRequest>(envelope.payload.clone()) {
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
                match connector_store::get(
                    &connection,
                    self.credential_store.as_ref(),
                    &req.connector_id,
                ) {
                    Ok(Some(connector)) if connector.kind == TEMPO_CONNECTOR_KIND => {
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
                                Ok(client) => tempo::trace(&client, req)
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
                    "policy denied Tempo response",
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

    pub async fn tempo_health(&self, envelope: CommandEnvelope<Value>) -> IpcResult<()> {
        use crate::observability::{client::ObservabilityClient, tempo, TEMPO_CONNECTOR_KIND};

        let descriptor = CommandDescriptor::new(
            "tempo",
            "health",
            Capability::ResourceRead,
            thalassa_domain::Permission::Read,
        );
        if let Err(error) = self.authorize_observability(&envelope, &descriptor) {
            return IpcResult::Err { ok: false, error };
        }

        let req = match serde_json::from_value::<ConnectorIdRequest>(envelope.payload.clone()) {
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
                match connector_store::get(&connection, self.credential_store.as_ref(), &req.id) {
                    Ok(Some(connector)) if connector.kind == TEMPO_CONNECTOR_KIND => {
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
                                    tempo::health(&client).await.map_err(AppStateError::from)
                                }
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
                    "policy denied Tempo response",
                    json!({}),
                ),
            };
        }

        match result {
            Ok(()) => IpcResult::Ok {
                ok: true,
                value: (),
            },
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
                match connector_store::get(&conn, self.credential_store.as_ref(), &req.connector_id)
                {
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
            Ok(conn) => {
                match connector_store::get(&conn, self.credential_store.as_ref(), &req.id) {
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
                                    grafana::health(&client).await.map_err(AppStateError::from)
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
                match connector_store::get(&conn, self.credential_store.as_ref(), &req.connector_id)
                {
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
}
