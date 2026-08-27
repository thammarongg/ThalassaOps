use super::*;
use crate::connectors as connector_store;
use crate::connectors::{AddConnectorRequest, ConnectorDiagnostics, ConnectorIdRequest};

impl AppState {
    pub fn connector_list(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<ConnectorSummary>> {
        self.connector_read(envelope, "list", |connection, store, _| {
            Ok(connector_store::list(connection, store)?)
        })
    }

    pub fn connector_add(&self, envelope: CommandEnvelope<Value>) -> IpcResult<ConnectorSummary> {
        self.connector_act(envelope, "add", |connection, store, payload| {
            let request: AddConnectorRequest =
                serde_json::from_value(payload.clone()).map_err(AppStateError::Serialization)?;
            connector_store::add(connection, store, request).map_err(AppStateError::Connector)
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
            connector_store::remove(connection, store, &request.id)
                .map_err(AppStateError::Connector)?;
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
                Ok(connection) => connector_store::test_connection(
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
            connector_store::diagnose(connection, store, &request.id)
                .map_err(AppStateError::Connector)
        })
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
            connector_store::set_enabled(connection, store, &request.id, enabled)
                .map_err(AppStateError::Connector)
        })
    }

    fn connector_read<T>(
        &self,
        envelope: CommandEnvelope<Value>,
        verb: &str,
        operation: impl FnOnce(
            &Connection,
            &dyn connector_store::CredentialStore,
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
            &dyn connector_store::CredentialStore,
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
            &dyn connector_store::CredentialStore,
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
