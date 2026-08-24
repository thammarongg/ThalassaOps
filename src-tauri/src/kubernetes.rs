//! Read-only Kubernetes discovery and investigation adapter.
use k8s_openapi::api::{
    apps::v1::{DaemonSet, Deployment, StatefulSet},
    core::v1::{Namespace, Node, Pod, Service},
    events::v1::Event,
};
use kube::{
    api::{Api, ListParams, LogParams},
    Client, ResourceExt,
};
use serde::{Deserialize, Serialize};
use thalassa_domain::{EnvironmentId, Resource, ResourceScope};

pub const KUBERNETES_CONNECTOR_KIND: &str = "kubernetes";
pub const LOG_TAIL_LINES: i64 = 200;

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
}
