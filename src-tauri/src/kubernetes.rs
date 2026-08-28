//! Read-only Kubernetes discovery and investigation adapter.
use crate::observability::masking::{sensitive_key, REDACTED};
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, StatefulSet},
    core::v1::{Namespace, Node, Pod, Service},
    events::v1::Event,
};
use kube::{
    api::{Api, ApiResource, DynamicObject, GroupVersionKind, ListParams, LogParams},
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
    #[serde(default)]
    pub console_url_template: Option<String>,
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
    #[serde(default)]
    pub service_selector: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub replicas: Option<KubernetesReplicaSummary>,
    #[serde(default)]
    pub containers: Vec<KubernetesContainerStatus>,
    pub health: KubernetesHealth,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesReplicaSummary {
    pub desired: i32,
    pub ready: i32,
    pub available: Option<i32>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesContainerStatus {
    pub name: String,
    pub restart_count: i32,
    pub waiting_reason: Option<String>,
    pub terminated_reason: Option<String>,
    pub last_terminated_reason: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KubernetesHealth {
    Healthy,
    Degraded,
    CrashLoopBackOff,
    OomKilled,
    Pending,
    Unknown,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesTopologyEdge {
    pub from_kind: String,
    pub from_name: String,
    pub to_kind: String,
    pub to_name: String,
    pub relationship: String,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KubernetesManifest {
    pub yaml: String,
    pub masked: bool,
    pub risk_class: String,
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
    pub topology: Vec<KubernetesTopologyEdge>,
}

pub fn pod_health(
    phase: Option<&str>,
    containers: &[KubernetesContainerStatus],
) -> KubernetesHealth {
    if containers
        .iter()
        .any(|item| item.waiting_reason.as_deref() == Some("CrashLoopBackOff"))
    {
        return KubernetesHealth::CrashLoopBackOff;
    }
    if containers.iter().any(|item| {
        item.terminated_reason.as_deref() == Some("OOMKilled")
            || item.last_terminated_reason.as_deref() == Some("OOMKilled")
    }) {
        return KubernetesHealth::OomKilled;
    }
    if phase == Some("Pending") {
        return KubernetesHealth::Pending;
    }
    if phase == Some("Running") || phase == Some("Succeeded") {
        KubernetesHealth::Healthy
    } else {
        KubernetesHealth::Degraded
    }
}
pub fn workload_health(replicas: &KubernetesReplicaSummary) -> KubernetesHealth {
    if replicas.ready >= replicas.desired {
        KubernetesHealth::Healthy
    } else {
        KubernetesHealth::Degraded
    }
}
pub fn topology_edges(inventory: &KubernetesInventory) -> Vec<KubernetesTopologyEdge> {
    let mut edges = Vec::new();
    for item in &inventory.resources {
        if item.resource.kind == "Pod" {
            if let Some(owner) = &item.owner {
                let owner_name = if owner.name.contains('/') {
                    owner.name.clone()
                } else if let Some((namespace, _)) = item.resource.name.split_once('/') {
                    format!("{namespace}/{}", owner.name)
                } else {
                    owner.name.clone()
                };
                edges.push(KubernetesTopologyEdge {
                    from_kind: owner.kind.clone(),
                    from_name: owner_name,
                    to_kind: "Pod".into(),
                    to_name: item.resource.name.clone(),
                    relationship: "owns".into(),
                });
            }
        }
        if item.resource.kind == "Service" {
            if let Some(selector) = &item.service_selector {
                let Some((service_namespace, _)) = item.resource.name.split_once('/') else {
                    continue;
                };
                for pod in inventory.resources.iter().filter(|candidate| {
                    let Some((pod_namespace, _)) = candidate.resource.name.split_once('/') else {
                        return false;
                    };
                    candidate.resource.kind == "Pod"
                        && pod_namespace == service_namespace
                        && selector
                            .iter()
                            .all(|(key, value)| candidate.resource.labels.get(key) == Some(value))
                }) {
                    edges.push(KubernetesTopologyEdge {
                        from_kind: "Service".into(),
                        from_name: item.resource.name.clone(),
                        to_kind: "Pod".into(),
                        to_name: pod.resource.name.clone(),
                        relationship: "selects".into(),
                    });
                }
            }
        }
    }
    edges
}
pub fn kubectl_command(kind: &str, namespace: Option<&str>, name: &str, context: &str) -> String {
    let kind_lower = kind.to_ascii_lowercase();
    let scope = namespace
        .filter(|value| !value.is_empty())
        .map(|value| format!(" -n {value}"))
        .unwrap_or_default();
    if kind == "Pod" {
        format!("kubectl --context {context}{scope} logs {name} --tail=200")
    } else {
        format!("kubectl --context {context}{scope} get {kind_lower} {name} -o yaml")
    }
}
fn mask_containers(value: &mut serde_json::Value, pointer: &str) -> bool {
    let mut masked = false;
    if let Some(containers) = value
        .pointer_mut(pointer)
        .and_then(serde_json::Value::as_array_mut)
    {
        for container in containers {
            if let Some(env) = container
                .get_mut("env")
                .and_then(serde_json::Value::as_array_mut)
            {
                for entry in env {
                    if entry
                        .get("name")
                        .and_then(|item| item.as_str())
                        .is_some_and(sensitive_key)
                    {
                        if let Some(item) = entry.get_mut("value") {
                            *item = serde_json::Value::String(REDACTED.into());
                            masked = true;
                        }
                    }
                }
            }
        }
    }
    masked
}
pub fn mask_sensitive_manifest(value: &mut serde_json::Value) -> bool {
    let mut masked = false;
    if value.get("kind").and_then(|item| item.as_str()) == Some("Secret") {
        for field in ["data", "stringData"] {
            if let Some(object) = value
                .get_mut(field)
                .and_then(serde_json::Value::as_object_mut)
            {
                for item in object.values_mut() {
                    *item = serde_json::Value::String(REDACTED.into());
                    masked = true;
                }
            }
        }
    }
    if let Some(metadata) = value
        .get_mut("metadata")
        .and_then(serde_json::Value::as_object_mut)
    {
        for field in ["annotations", "labels"] {
            if let Some(object) = metadata
                .get_mut(field)
                .and_then(serde_json::Value::as_object_mut)
            {
                for (key, item) in object {
                    if sensitive_key(key) {
                        *item = serde_json::Value::String(REDACTED.into());
                        masked = true;
                    }
                }
            }
        }
    }
    for pointer in [
        "/spec/containers",
        "/spec/initContainers",
        "/spec/ephemeralContainers",
        "/spec/template/spec/containers",
        "/spec/template/spec/initContainers",
        "/spec/template/spec/ephemeralContainers",
    ] {
        masked |= mask_containers(value, pointer);
    }
    masked
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
    let containers: Vec<KubernetesContainerStatus> = status
        .and_then(|value| value.container_statuses.as_ref())
        .map(|items| {
            items
                .iter()
                .map(|item| KubernetesContainerStatus {
                    name: item.name.clone(),
                    restart_count: item.restart_count,
                    waiting_reason: item
                        .state
                        .as_ref()
                        .and_then(|state| state.waiting.as_ref())
                        .and_then(|waiting| waiting.reason.clone()),
                    terminated_reason: item
                        .state
                        .as_ref()
                        .and_then(|state| state.terminated.as_ref())
                        .and_then(|terminated| terminated.reason.clone()),
                    last_terminated_reason: item
                        .last_state
                        .as_ref()
                        .and_then(|state| state.terminated.as_ref())
                        .and_then(|terminated| terminated.reason.clone()),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let phase = status.and_then(|value| value.phase.clone());
    KubernetesResource {
        resource: base_resource(pod, environment_id, scope, "Pod"),
        status: phase.clone(),
        conditions,
        owner: owner(pod),
        service_selector: None,
        replicas: None,
        health: pod_health(phase.as_deref(), &containers),
        containers,
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
        service_selector: None,
        replicas: None,
        containers: vec![],
        health: KubernetesHealth::Unknown,
    }
}
fn map_workload<T: ResourceExt>(
    object: &T,
    environment_id: EnvironmentId,
    scope: ResourceScope,
    kind: &str,
    desired: Option<i32>,
    ready: Option<i32>,
    available: Option<i32>,
) -> KubernetesResource {
    let replicas = KubernetesReplicaSummary {
        desired: desired.unwrap_or_default(),
        ready: ready.unwrap_or_default(),
        available,
    };
    KubernetesResource {
        resource: base_resource(object, environment_id, scope, kind),
        status: Some(format!("{}/{} ready", replicas.ready, replicas.desired)),
        conditions: vec![],
        owner: owner(object),
        service_selector: None,
        health: workload_health(&replicas),
        replicas: Some(replicas),
        containers: vec![],
    }
}
fn map_service(
    service: &Service,
    environment_id: EnvironmentId,
    scope: ResourceScope,
) -> KubernetesResource {
    KubernetesResource {
        resource: base_resource(service, environment_id, scope, "Service"),
        status: None,
        conditions: vec![],
        owner: owner(service),
        service_selector: service
            .spec
            .as_ref()
            .and_then(|spec| spec.selector.clone())
            .map(|items| items.into_iter().collect()),
        replicas: None,
        containers: vec![],
        health: KubernetesHealth::Unknown,
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
    let deployments = Api::<Deployment>::all(client.clone())
        .list(&ListParams::default())
        .await;
    capabilities.push(availability("Deployment", deployments.as_ref().err()));
    if let Ok(items) = deployments {
        resources.extend(items.items.iter().map(|item| {
            map_workload(
                item,
                environment_id,
                scope.clone(),
                "Deployment",
                item.spec.as_ref().and_then(|spec| spec.replicas),
                item.status
                    .as_ref()
                    .and_then(|status| status.ready_replicas),
                item.status
                    .as_ref()
                    .and_then(|status| status.available_replicas),
            )
        }));
    }
    let statefulsets = Api::<StatefulSet>::all(client.clone())
        .list(&ListParams::default())
        .await;
    capabilities.push(availability("StatefulSet", statefulsets.as_ref().err()));
    if let Ok(items) = statefulsets {
        resources.extend(items.items.iter().map(|item| {
            map_workload(
                item,
                environment_id,
                scope.clone(),
                "StatefulSet",
                item.spec.as_ref().and_then(|spec| spec.replicas),
                item.status
                    .as_ref()
                    .and_then(|status| status.ready_replicas),
                item.status
                    .as_ref()
                    .and_then(|status| status.available_replicas),
            )
        }));
    }
    let daemonsets = Api::<DaemonSet>::all(client.clone())
        .list(&ListParams::default())
        .await;
    capabilities.push(availability("DaemonSet", daemonsets.as_ref().err()));
    if let Ok(items) = daemonsets {
        resources.extend(items.items.iter().map(|item| {
            map_workload(
                item,
                environment_id,
                scope.clone(),
                "DaemonSet",
                item.status
                    .as_ref()
                    .map(|status| status.desired_number_scheduled),
                item.status.as_ref().map(|status| status.number_ready),
                item.status
                    .as_ref()
                    .and_then(|status| status.number_available),
            )
        }));
    }
    let services = Api::<Service>::all(client.clone())
        .list(&ListParams::default())
        .await;
    capabilities.push(availability("Service", services.as_ref().err()));
    if let Ok(items) = services {
        resources.extend(
            items
                .items
                .iter()
                .map(|item| map_service(item, environment_id, scope.clone())),
        );
    }
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
    let mut inventory = KubernetesInventory {
        resources,
        availability: capabilities,
        topology: vec![],
    };
    inventory.topology = topology_edges(&inventory);
    inventory
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

pub async fn resource_manifest(
    client: Client,
    kind: &str,
    namespace: &str,
    name: &str,
) -> Result<KubernetesManifest, kube::Error> {
    let (group, version, namespaced) = match kind {
        "Node" | "Namespace" => ("", "v1", false),
        "Pod" | "Service" => ("", "v1", true),
        "Deployment" | "StatefulSet" | "DaemonSet" => ("apps", "v1", true),
        _ => {
            return Err(kube::Error::SerdeError(serde_json::Error::io(
                std::io::Error::other("unsupported Kubernetes resource kind"),
            )))
        }
    };
    let resource = ApiResource::from_gvk(&GroupVersionKind::gvk(group, version, kind));
    let object: DynamicObject = if namespaced {
        Api::namespaced_with(client, namespace, &resource)
            .get(name)
            .await?
    } else {
        Api::all_with(client, &resource).get(name).await?
    };
    let mut value = serde_json::to_value(object).map_err(kube::Error::SerdeError)?;
    let masked = mask_sensitive_manifest(&mut value);
    let yaml = serde_yaml::to_string(&value).map_err(|error| {
        kube::Error::SerdeError(serde_json::Error::io(std::io::Error::other(error)))
    })?;
    Ok(KubernetesManifest {
        yaml,
        masked,
        risk_class: "READ-ONLY".into(),
    })
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

    #[test]
    fn health_classifies_crash_loop_oom_pending_and_healthy_pods() {
        let crash = KubernetesContainerStatus {
            name: "api".into(),
            restart_count: 3,
            waiting_reason: Some("CrashLoopBackOff".into()),
            terminated_reason: None,
            last_terminated_reason: None,
        };
        let oom = KubernetesContainerStatus {
            name: "api".into(),
            restart_count: 1,
            waiting_reason: None,
            terminated_reason: None,
            last_terminated_reason: Some("OOMKilled".into()),
        };
        assert_eq!(
            pod_health(Some("Running"), &[crash]),
            KubernetesHealth::CrashLoopBackOff
        );
        assert_eq!(
            pod_health(Some("Failed"), &[oom]),
            KubernetesHealth::OomKilled
        );
        assert_eq!(pod_health(Some("Pending"), &[]), KubernetesHealth::Pending);
        assert_eq!(pod_health(Some("Running"), &[]), KubernetesHealth::Healthy);
    }
    #[test]
    fn masking_redacts_secret_and_sensitive_metadata_but_preserves_name() {
        let mut value = serde_json::json!({"kind":"Secret","metadata":{"name":"safe","annotations":{"api-token":"x","note":"ok"}},"data":{"password":"abc"},"stringData":{"token":"def"}});
        assert!(mask_sensitive_manifest(&mut value));
        assert_eq!(value["data"]["password"], REDACTED);
        assert_eq!(value["stringData"]["token"], REDACTED);
        assert_eq!(value["metadata"]["annotations"]["api-token"], REDACTED);
        assert_eq!(value["metadata"]["name"], "safe");
    }
    #[test]
    fn masking_redacts_sensitive_deployment_container_environment_values() {
        let mut value = serde_json::json!({"kind":"Deployment","spec":{"template":{"spec":{"containers":[{"name":"api","env":[{"name":"PASSWORD","value":"raw-secret"}]}]}}}});
        assert!(mask_sensitive_manifest(&mut value));
        assert_eq!(
            value["spec"]["template"]["spec"]["containers"][0]["env"][0]["value"],
            REDACTED
        );
    }
    #[test]
    fn topology_and_kubectl_commands_are_read_only() {
        let pod = KubernetesResource {
            resource: Resource::new(
                uuid::Uuid::nil(),
                ResourceScope::default(),
                "Pod",
                "prod/api",
            ),
            status: Some("Running".into()),
            conditions: vec![],
            owner: Some(KubernetesOwner {
                kind: "ReplicaSet".into(),
                name: "api-rs".into(),
                uid: None,
            }),
            service_selector: None,
            replicas: None,
            containers: vec![],
            health: KubernetesHealth::Healthy,
        };
        let mut service_resource = Resource::new(
            uuid::Uuid::nil(),
            ResourceScope::default(),
            "Service",
            "prod/api",
        );
        service_resource.labels = Default::default();
        let service = KubernetesResource {
            resource: service_resource,
            status: None,
            conditions: vec![],
            owner: None,
            service_selector: Some([("app".into(), "api".into())].into_iter().collect()),
            replicas: None,
            containers: vec![],
            health: KubernetesHealth::Unknown,
        };
        let mut pod = pod;
        pod.resource.labels.insert("app".into(), "api".into());
        let mut staging_pod = pod.clone();
        staging_pod.resource.name = "staging/api".into();
        staging_pod.owner = None;
        let inventory = KubernetesInventory {
            resources: vec![pod, staging_pod, service],
            availability: vec![],
            topology: vec![],
        };
        let edges = topology_edges(&inventory);
        assert_eq!(edges.len(), 2);
        assert!(edges.iter().all(|edge| {
            edge.relationship != "selects" || edge.to_name == "prod/api"
        }));
        assert!(edges.iter().any(|edge| {
            edge.relationship == "owns" && edge.from_name == "prod/api-rs"
        }));
        assert_eq!(
            kubectl_command("Pod", Some("prod"), "api", "ctx"),
            "kubectl --context ctx -n prod logs api --tail=200"
        );
        assert_eq!(
            kubectl_command("Deployment", Some("prod"), "api", "ctx"),
            "kubectl --context ctx -n prod get deployment api -o yaml"
        );
    }

    #[tokio::test]
    async fn manifest_fetch_uses_get_and_masks_returned_resource() {
        let server = MockServer::start();
        let manifest = server.mock(|when, then| {
            when.method(GET).path("/api/v1/namespaces/production/pods/api");
            then.status(200).header("content-type", "application/json").body(r#"{"apiVersion":"v1","kind":"Secret","metadata":{"name":"api","namespace":"production"},"data":{"token":"raw-secret"}}"#);
        });
        let result = resource_manifest(mock_client(&server), "Pod", "production", "api")
            .await
            .unwrap();
        manifest.assert_hits(1);
        assert!(result.masked);
        assert!(result.yaml.contains(REDACTED));
        assert!(!result.yaml.contains("raw-secret"));
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
