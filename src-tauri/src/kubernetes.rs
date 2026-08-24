//! Read-only Kubernetes discovery and investigation adapter.
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, StatefulSet},
    core::v1::{Namespace, Node, Pod, Service},
    events::v1::Event,
};
use kube::{
    api::{Api, ListParams, LogParams},
    config::{KubeConfigOptions, Kubeconfig},
    Client, ResourceExt,
};
use serde::{Deserialize, Serialize};
use thalassa_domain::{EnvironmentId, Resource, ResourceScope};

pub const KUBERNETES_CONNECTOR_KIND: &str = "kubernetes";
pub const LOG_TAIL_LINES: i64 = 200;

/// Persisted connector metadata.  The kubeconfig itself remains outside the
/// database (and may in turn refer to credentials in the OS keychain).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesConnectorConfig {
    pub kubeconfig_path: String,
    pub context_name: String,
}

#[derive(Debug, thiserror::Error)]
pub enum KubernetesClientError {
    #[error("kubernetes connector metadata is invalid: {0}")]
    InvalidMetadata(String),
    #[error("unable to load kubeconfig: {0}")]
    Kubeconfig(String),
    #[error("unable to construct Kubernetes client: {0}")]
    Client(String),
}

/// Builds a client for precisely the named context; this never falls back to
/// the process default kubeconfig/context, which avoids surprising clusters.
pub async fn client_from_kubeconfig(
    config: &KubernetesConnectorConfig,
) -> Result<Client, KubernetesClientError> {
    if config.kubeconfig_path.trim().is_empty() {
        return Err(KubernetesClientError::InvalidMetadata(
            "kubeconfig_path is required".into(),
        ));
    }
    if config.context_name.trim().is_empty() {
        return Err(KubernetesClientError::InvalidMetadata(
            "context_name is required".into(),
        ));
    }
    let kubeconfig = Kubeconfig::read_from(&config.kubeconfig_path)
        .map_err(|error| KubernetesClientError::Kubeconfig(error.to_string()))?;
    let options = KubeConfigOptions {
        context: Some(config.context_name.clone()),
        ..KubeConfigOptions::default()
    };
    let client_config = kube::Config::from_custom_kubeconfig(kubeconfig, &options)
        .await
        .map_err(|error| KubernetesClientError::Kubeconfig(error.to_string()))?;
    Client::try_from(client_config)
        .map_err(|error| KubernetesClientError::Client(error.to_string()))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesCondition {
    pub type_: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesOwner {
    pub kind: String,
    pub name: String,
    pub uid: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesResource {
    pub resource: Resource,
    pub status: Option<String>,
    pub conditions: Vec<KubernetesCondition>,
    pub owner: Option<KubernetesOwner>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesEvent {
    pub type_: Option<String>,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub involved_kind: Option<String>,
    pub involved_name: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CapabilityAvailability {
    pub resource_kind: String,
    pub available: bool,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesInventory {
    pub resources: Vec<KubernetesResource>,
    pub availability: Vec<CapabilityAvailability>,
}

fn base_resource<T: ResourceExt>(
    object: &T,
    environment_id: EnvironmentId,
    scope: ResourceScope,
    kind: &str,
) -> Resource {
    let namespace = object
        .namespace()
        .map(|value| format!("{value}/"))
        .unwrap_or_default();
    let mut resource = Resource::new(
        environment_id,
        scope,
        kind,
        format!("{namespace}{}", object.name_any()),
    );
    resource.provider = Some(KUBERNETES_CONNECTOR_KIND.into());
    resource.native_id = object.meta().uid.clone();
    resource.labels = object.labels().clone();
    resource
}
fn owner<T: ResourceExt>(object: &T) -> Option<KubernetesOwner> {
    object
        .meta()
        .owner_references
        .as_ref()?
        .iter()
        .find(|value| value.controller.unwrap_or(false))
        .map(|value| KubernetesOwner {
            kind: value.kind.clone(),
            name: value.name.clone(),
            uid: Some(value.uid.clone()),
        })
}
pub fn map_pod(
    pod: &Pod,
    environment_id: EnvironmentId,
    scope: ResourceScope,
) -> KubernetesResource {
    let status = pod.status.as_ref();
    let conditions = status
        .and_then(|value| value.conditions.as_ref())
        .map(|items| {
            items
                .iter()
                .map(|item| KubernetesCondition {
                    type_: item.type_.clone(),
                    status: item.status.clone(),
                    reason: item.reason.clone(),
                    message: item.message.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    KubernetesResource {
        resource: base_resource(pod, environment_id, scope, "Pod"),
        status: status.and_then(|value| value.phase.clone()),
        conditions,
        owner: owner(pod),
    }
}
fn map_plain<T: ResourceExt>(
    object: &T,
    environment_id: EnvironmentId,
    scope: ResourceScope,
    kind: &str,
) -> KubernetesResource {
    KubernetesResource {
        resource: base_resource(object, environment_id, scope, kind),
        status: None,
        conditions: vec![],
        owner: owner(object),
    }
}
fn availability(kind: &str, error: Option<&kube::Error>) -> CapabilityAvailability {
    CapabilityAvailability {
        resource_kind: kind.into(),
        available: error.is_none(),
        reason: error.map(ToString::to_string),
    }
}

/// Uses only GET requests. RBAC availability is inferred from the attempted read, avoiding
/// SelfSubjectAccessReview because that Kubernetes API endpoint is a POST.
pub async fn discover(
    client: Client,
    environment_id: EnvironmentId,
    scope: ResourceScope,
) -> KubernetesInventory {
    let mut resources = Vec::new();
    let mut capabilities = Vec::new();
    macro_rules! list {
        ($type:ty, $kind:literal) => {{
            let response = Api::<$type>::all(client.clone())
                .list(&ListParams::default())
                .await;
            capabilities.push(availability($kind, response.as_ref().err()));
            if let Ok(items) = response {
                resources.extend(
                    items
                        .items
                        .iter()
                        .map(|item| map_plain(item, environment_id, scope.clone(), $kind)),
                );
            }
        }};
    }
    list!(Node, "Node");
    list!(Namespace, "Namespace");
    list!(Deployment, "Deployment");
    list!(StatefulSet, "StatefulSet");
    list!(DaemonSet, "DaemonSet");
    list!(Service, "Service");
    let pods = Api::<Pod>::all(client).list(&ListParams::default()).await;
    capabilities.push(availability("Pod", pods.as_ref().err()));
    if let Ok(items) = pods {
        resources.extend(
            items
                .items
                .iter()
                .map(|item| map_pod(item, environment_id, scope.clone())),
        );
    }
    KubernetesInventory {
        resources,
        availability: capabilities,
    }
}
pub async fn pod_logs(client: Client, namespace: &str, pod: &str) -> Result<String, kube::Error> {
    Api::<Pod>::namespaced(client, namespace)
        .logs(
            pod,
            &LogParams {
                tail_lines: Some(LOG_TAIL_LINES),
                ..LogParams::default()
            },
        )
        .await
}
pub async fn pod_events(
    client: Client,
    namespace: &str,
    pod: &str,
) -> Result<Vec<KubernetesEvent>, kube::Error> {
    let events = Api::<Event>::namespaced(client, namespace)
        .list(&ListParams::default().fields(&format!("regarding.name={pod}")))
        .await?;
    Ok(events
        .items
        .into_iter()
        .map(|event| {
            let regarding = event.regarding.unwrap_or_default();
            KubernetesEvent {
                type_: event.type_,
                reason: event.reason,
                message: event.note,
                involved_kind: regarding.kind,
                involved_name: regarding.name,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{Method::GET, MockServer};

    fn mock_client(server: &MockServer) -> Client {
        let config = kube::Config::new(server.base_url().parse().unwrap());
        Client::try_from(config).unwrap()
    }
    #[test]
    fn pod_mapping_preserves_conditions_and_owner_workload() {
        let pod: Pod = serde_json::from_value(serde_json::json!({"metadata":{"name":"api-7c9b","namespace":"production","uid":"pod-uid","labels":{"app":"api"},"ownerReferences":[{"apiVersion":"apps/v1","kind":"ReplicaSet","name":"api-7c9b4","uid":"rs-uid","controller":true}]},"status":{"phase":"Failed","conditions":[{"type":"Ready","status":"False","reason":"ContainersNotReady","message":"CrashLoopBackOff"}]}})).unwrap();
        let mapped = map_pod(&pod, uuid::Uuid::nil(), ResourceScope::default());
        assert_eq!(mapped.resource.name, "production/api-7c9b");
        assert_eq!(
            mapped.conditions[0].reason.as_deref(),
            Some("ContainersNotReady")
        );
        assert_eq!(mapped.owner.as_ref().unwrap().kind, "ReplicaSet");
    }

    #[tokio::test]
    async fn discovery_and_pod_investigation_use_only_get_endpoints() {
        let server = MockServer::start();
        let empty = r#"{"apiVersion":"v1","items":[]}"#;
        let mut discovery_mocks = Vec::new();
        for path in [
            "/api/v1/nodes",
            "/api/v1/namespaces",
            "/apis/apps/v1/deployments",
            "/apis/apps/v1/statefulsets",
            "/apis/apps/v1/daemonsets",
            "/api/v1/services",
        ] {
            discovery_mocks.push(server.mock(|when, then| {
                when.method(GET).path(path);
                then.status(200)
                    .header("content-type", "application/json")
                    .body(empty);
            }));
        }
        let pods = server.mock(|when, then| {
            when.method(GET).path("/api/v1/pods");
            then.status(200).header("content-type", "application/json").body(r#"{"apiVersion":"v1","items":[{"metadata":{"name":"api","namespace":"production","uid":"pod-1","ownerReferences":[{"apiVersion":"apps/v1","kind":"ReplicaSet","name":"api-7c9b4","uid":"rs-1","controller":true}]},"status":{"phase":"Failed","conditions":[{"type":"Ready","status":"False","reason":"ContainersNotReady"}]}}]}"#);
        });
        let logs = server.mock(|when, then| {
            when.method(GET)
                .path("/api/v1/namespaces/production/pods/api/log");
            then.status(200).body("container failed\n");
        });
        let events = server.mock(|when, then| {
            when.method(GET).path("/apis/events.k8s.io/v1/namespaces/production/events");
            then.status(200).header("content-type", "application/json").body(r#"{"apiVersion":"events.k8s.io/v1","items":[{"type":"Warning","reason":"BackOff","note":"Back-off restarting failed container","regarding":{"kind":"Pod","name":"api"}}]}"#);
        });

        let client = mock_client(&server);
        let inventory = discover(client.clone(), uuid::Uuid::nil(), ResourceScope::default()).await;
        assert_eq!(inventory.resources.len(), 1);
        assert_eq!(inventory.resources[0].status.as_deref(), Some("Failed"));
        assert_eq!(
            inventory.resources[0].owner.as_ref().unwrap().name,
            "api-7c9b4"
        );
        assert_eq!(
            pod_logs(client.clone(), "production", "api").await.unwrap(),
            "container failed\n"
        );
        assert_eq!(
            pod_events(client, "production", "api").await.unwrap()[0]
                .reason
                .as_deref(),
            Some("BackOff")
        );

        for mock in discovery_mocks {
            mock.assert_hits(1);
        }
        pods.assert_hits(1);
        logs.assert_hits(1);
        events.assert_hits(1);
    }
}
