use super::*;
use crate::connectors as connector_store;
use crate::kubernetes::{
    client_from_kubeconfig, discover, pod_events, pod_logs, resource_manifest,
    KubernetesConnectorConfig, KubernetesEvent, KubernetesInventory, KubernetesManifest,
};

impl AppState {
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
            let connector = connector_store::get(
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
}
