use super::*;
use crate::cloud::{
    self, AwsConnectorConfig, AwsCredentialProvider, AzureConnectorConfig, AzureCredentialProvider,
    CloudClient, CloudClientError, CloudEnvironment, CloudResource, GcpConnectorConfig,
    GcpCredentialProvider, AWS_CONNECTOR_KIND, AZURE_CONNECTOR_KIND, GCP_CONNECTOR_KIND,
};
use crate::connectors as connector_store;
use std::future::Future;
use std::sync::Arc;

#[derive(Clone, Debug, Deserialize)]
struct CloudConnectorRequest {
    connector_id: String,
}

enum CloudConnector {
    Aws {
        client: CloudClient,
        config: AwsConnectorConfig,
    },
    Azure {
        client: CloudClient,
        config: AzureConnectorConfig,
    },
    Gcp {
        client: CloudClient,
        config: GcpConnectorConfig,
    },
}

impl CloudConnector {
    async fn access_check(&self, connector_id: &str) -> CloudEnvironment {
        match self {
            Self::Aws { client, config } => {
                cloud::aws::access_check(client, config, connector_id).await
            }
            Self::Azure { client, config } => {
                cloud::azure::access_check(client, config, connector_id).await
            }
            Self::Gcp { client, config } => {
                cloud::gcp::access_check(client, config, connector_id).await
            }
        }
    }

    async fn inventory(&self, connector_id: &str) -> Result<Vec<CloudResource>, CloudClientError> {
        match self {
            Self::Aws { client, config } => {
                cloud::aws::inventory(client, config, connector_id).await
            }
            Self::Azure { client, config } => {
                cloud::azure::inventory(client, config, connector_id).await
            }
            Self::Gcp { client, config } => {
                cloud::gcp::inventory(client, config, connector_id).await
            }
        }
    }
}

impl AppState {
    pub async fn cloud_access_check(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<CloudEnvironment> {
        self.cloud_command(envelope, "access_check", |connector, connector_id| async move {
            Ok(connector.access_check(&connector_id).await)
        })
        .await
    }

    pub async fn cloud_inventory(
        &self,
        envelope: CommandEnvelope<Value>,
    ) -> IpcResult<Vec<CloudResource>> {
        self.cloud_command(
            envelope,
            "inventory",
            |connector, connector_id| async move { connector.inventory(&connector_id).await },
        )
        .await
    }

    async fn cloud_command<T, F, Fut>(
        &self,
        envelope: CommandEnvelope<Value>,
        verb: &str,
        operation: F,
    ) -> IpcResult<T>
    where
        F: FnOnce(CloudConnector, String) -> Fut,
        Fut: Future<Output = Result<T, CloudClientError>>,
    {
        let descriptor = CommandDescriptor::new(
            "cloud",
            verb,
            Capability::EnvironmentRead,
            thalassa_domain::Permission::Read,
        );
        let workspace_scope = ResourceScope::workspace(
            self.bootstrap.workspace.id,
            self.bootstrap.team.id,
            self.bootstrap.organization.id,
        );
        if envelope.command != descriptor.name
            || envelope.capability != descriptor.required_capability
            || envelope.scope.is_bounded()
            || !descriptor.scope.contains(&envelope.scope)
            || self.bootstrap.membership.status != thalassa_domain::MembershipStatus::Active
            || self.bootstrap.membership.principal_id != self.bootstrap.principal.id
            || !self.bootstrap.membership.grants(&workspace_scope)
            || !membership_role_grants_permission(
                &self.bootstrap.membership.role,
                &descriptor.required_permission,
            )
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
                EgressDestination::ExternalIntegration,
            ))
            .is_allowed()
        {
            return IpcResult::Err {
                ok: false,
                error: IpcError::new(
                    IpcErrorCode::PolicyDenied,
                    "policy denied cloud provider request",
                    json!({}),
                ),
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
                    "policy denied cloud response",
                    json!({}),
                ),
            };
        }

        let request = match serde_json::from_value::<CloudConnectorRequest>(envelope.payload) {
            Ok(request) => request,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: ipc_error_for(AppStateError::Serialization(error)),
                }
            }
        };

        let connector = match Connection::open(&self.database_path) {
            Ok(connection) => match connector_store::get(
                &connection,
                self.credential_store.as_ref(),
                &request.connector_id,
            ) {
                Ok(Some(connector)) => connector,
                Ok(None) => {
                    return IpcResult::Err {
                        ok: false,
                        error: IpcError::new(
                            IpcErrorCode::NotFound,
                            "connector not found",
                            json!({}),
                        ),
                    }
                }
                Err(error) => {
                    return IpcResult::Err {
                        ok: false,
                        error: ipc_error_for(AppStateError::Connector(error)),
                    }
                }
            },
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: ipc_error_for(AppStateError::Database(error)),
                }
            }
        };

        if !connector.enabled {
            return IpcResult::Err {
                ok: false,
                error: IpcError::new(
                    IpcErrorCode::ConnectorUnavailable,
                    "connector is disabled",
                    json!({}),
                ),
            };
        }

        let cloud_connector = match cloud_connector_for(&connector) {
            Ok(cloud_connector) => cloud_connector,
            Err(error) => {
                return IpcResult::Err {
                    ok: false,
                    error: cloud_client_ipc_error(error),
                }
            }
        };
        match operation(cloud_connector, request.connector_id).await {
            Ok(value) => IpcResult::Ok { ok: true, value },
            Err(error) => IpcResult::Err {
                ok: false,
                error: cloud_client_ipc_error(error),
            },
        }
    }
}

pub(crate) async fn cloud_access_check_for_connector(
    connector: &ConnectorSummary,
) -> Result<CloudEnvironment, CloudClientError> {
    let cloud_connector = cloud_connector_for(connector)?;
    Ok(cloud_connector.access_check(&connector.id).await)
}

fn cloud_connector_for(connector: &ConnectorSummary) -> Result<CloudConnector, CloudClientError> {
    match connector.kind.as_str() {
        AWS_CONNECTOR_KIND => {
            let config =
                serde_json::from_value::<AwsConnectorConfig>(connector.config_metadata.clone())
                    .map_err(|error| CloudClientError::Configuration(error.to_string()))?;
            let client = CloudClient::new(Arc::new(AwsCredentialProvider::new(
                config.profile.clone(),
                config.region.clone(),
            )))?;
            Ok(CloudConnector::Aws { client, config })
        }
        AZURE_CONNECTOR_KIND => {
            let config =
                serde_json::from_value::<AzureConnectorConfig>(connector.config_metadata.clone())
                    .map_err(|error| CloudClientError::Configuration(error.to_string()))?;
            let provider = AzureCredentialProvider::new(config.tenant_id.clone())?;
            let client = CloudClient::new(Arc::new(provider))?;
            Ok(CloudConnector::Azure { client, config })
        }
        GCP_CONNECTOR_KIND => {
            let config =
                serde_json::from_value::<GcpConnectorConfig>(connector.config_metadata.clone())
                    .map_err(|error| CloudClientError::Configuration(error.to_string()))?;
            let client = CloudClient::new(Arc::new(GcpCredentialProvider::new()))?;
            Ok(CloudConnector::Gcp { client, config })
        }
        _ => Err(CloudClientError::Configuration(
            "unsupported cloud connector kind".into(),
        )),
    }
}

fn invalid_cloud_configuration() -> IpcError {
    IpcError::new(
        IpcErrorCode::InvalidRequest,
        "invalid cloud connector configuration",
        json!({}),
    )
}

fn cloud_client_ipc_error(error: CloudClientError) -> IpcError {
    match error {
        CloudClientError::Configuration(_) => invalid_cloud_configuration(),
        CloudClientError::MalformedResponse => IpcError::new(
            IpcErrorCode::MalformedResponse,
            "malformed response from provider",
            json!({}),
        ),
        CloudClientError::Auth(_)
        | CloudClientError::RequestFailed
        | CloudClientError::ProviderError(_) => IpcError::new(
            IpcErrorCode::ConnectorUnavailable,
            "cloud provider request failed",
            json!({}),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectors::InMemoryCredentialStore;
    use std::sync::Arc;
    use tempfile::tempdir;
    use thalassa_domain::MembershipStatus;
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

    fn cloud_envelope(
        state: &AppState,
        verb: &str,
        capability: Capability,
        connector_id: &str,
    ) -> CommandEnvelope<Value> {
        CommandEnvelope {
            request_id: Uuid::new_v4(),
            command: thalassa_ipc::CommandName::new("cloud", verb).unwrap(),
            capability,
            scope: state.bootstrap.scope.clone(),
            payload: json!({ "connector_id": connector_id }),
        }
    }

    #[tokio::test]
    async fn cloud_commands_require_environment_read() {
        let (_directory, state) = test_state();
        let envelope = cloud_envelope(&state, "inventory", Capability::ResourceRead, "aws-1");
        assert!(matches!(
            state.cloud_inventory(envelope).await,
            IpcResult::Err { .. }
        ));
    }

    #[tokio::test]
    async fn cloud_inventory_rejects_an_inactive_membership() {
        let (_directory, mut state) = test_state();
        state.bootstrap.membership.status = MembershipStatus::Suspended;
        let envelope = cloud_envelope(&state, "inventory", Capability::EnvironmentRead, "aws-1");
        assert!(matches!(
            state.cloud_inventory(envelope).await,
            IpcResult::Err { .. }
        ));
    }

    #[tokio::test]
    async fn cloud_inventory_rejects_an_unknown_connector() {
        let (_directory, state) = test_state();
        let envelope = cloud_envelope(
            &state,
            "inventory",
            Capability::EnvironmentRead,
            "does-not-exist",
        );
        assert!(matches!(
            state.cloud_inventory(envelope).await,
            IpcResult::Err { .. }
        ));
    }
}
