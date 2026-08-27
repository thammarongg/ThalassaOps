use super::{
    classify_access, CloudAccessState, CloudClient, CloudClientError, CloudEnvironment,
    CloudHealthState, CloudProvider, CloudResource, CloudResourceType, GcpConnectorConfig,
};
use reqwest::Url;
use serde::Deserialize;
use std::collections::BTreeMap;

const CONTAINER_ENDPOINT: &str = "https://container.googleapis.com";
const COMPUTE_ENDPOINT: &str = "https://compute.googleapis.com";
const COMPUTE_MAX_RESULTS: &str = "100";
const GCP_LOGIN_COMMAND: &str = "gcloud auth application-default login";
const GKE_PERMISSION: &str = "container.clusters.list";

#[derive(Debug, Default, Deserialize)]
struct GkeListResponse {
    #[serde(default)]
    clusters: Vec<GkeCluster>,
}

#[derive(Debug, Deserialize)]
struct GkeCluster {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    zone: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(rename = "statusMessage", default)]
    status_message: Option<String>,
    #[serde(rename = "selfLink", default)]
    self_link: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct AggregatedInstancesResponse {
    #[serde(default)]
    items: BTreeMap<String, AggregatedInstancesScope>,
    #[serde(rename = "nextPageToken", default)]
    next_page_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct AggregatedInstancesScope {
    #[serde(default)]
    instances: Option<Vec<GcpInstance>>,
}

#[derive(Debug, Deserialize)]
struct GcpInstance {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    zone: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(rename = "statusMessage", default)]
    status_message: Option<String>,
    #[serde(rename = "selfLink", default)]
    self_link: Option<String>,
}

#[derive(Debug)]
struct ScopedInstance {
    location: String,
    instance: GcpInstance,
}

pub async fn inventory(
    client: &CloudClient,
    config: &GcpConnectorConfig,
    connector_id: &str,
) -> Result<Vec<CloudResource>, CloudClientError> {
    inventory_with_endpoints(
        client,
        config,
        connector_id,
        CONTAINER_ENDPOINT,
        COMPUTE_ENDPOINT,
    )
    .await
}

async fn inventory_with_endpoints(
    client: &CloudClient,
    config: &GcpConnectorConfig,
    connector_id: &str,
    container_endpoint: &str,
    compute_endpoint: &str,
) -> Result<Vec<CloudResource>, CloudClientError> {
    let clusters = list_clusters(client, container_endpoint, &config.project_id).await?;
    let mut resources = clusters
        .into_iter()
        .map(|cluster| cluster_resource(cluster, config, connector_id))
        .collect::<Vec<_>>();

    let instances = list_instances(client, compute_endpoint, &config.project_id).await?;
    resources.extend(
        instances
            .into_iter()
            .map(|instance| instance_resource(instance, config, connector_id)),
    );
    Ok(resources)
}

pub async fn access_check(
    client: &CloudClient,
    config: &GcpConnectorConfig,
    connector_id: &str,
) -> CloudEnvironment {
    access_check_with_endpoint(client, config, connector_id, CONTAINER_ENDPOINT).await
}

async fn access_check_with_endpoint(
    client: &CloudClient,
    config: &GcpConnectorConfig,
    connector_id: &str,
    container_endpoint: &str,
) -> CloudEnvironment {
    let result = list_clusters(client, container_endpoint, &config.project_id)
        .await
        .map(|_| ());
    let (access, mut remedy) = classify_access(&result);
    if access == CloudAccessState::NoCredential
        || (access == CloudAccessState::SessionExpired && remedy.is_empty())
    {
        remedy = GCP_LOGIN_COMMAND.into();
    } else if access == CloudAccessState::PermissionDenied && remedy.is_empty() {
        remedy = GKE_PERMISSION.into();
    }

    CloudEnvironment {
        connector_id: connector_id.to_owned(),
        provider: CloudProvider::Gcp,
        account_label: config.project_id.clone(),
        location: String::new(),
        access,
        remedy,
    }
}

async fn list_clusters(
    client: &CloudClient,
    endpoint: &str,
    project_id: &str,
) -> Result<Vec<GkeCluster>, CloudClientError> {
    let url = gcp_url(
        endpoint,
        &["v1", "projects", project_id, "locations", "-", "clusters"],
        &[],
    )?;
    let response: GkeListResponse = client.get_json(url).await?;
    Ok(response.clusters)
}

async fn list_instances(
    client: &CloudClient,
    endpoint: &str,
    project_id: &str,
) -> Result<Vec<ScopedInstance>, CloudClientError> {
    let first = gcp_url(
        endpoint,
        &[
            "compute",
            "v1",
            "projects",
            project_id,
            "aggregated",
            "instances",
        ],
        &[
            ("maxResults", COMPUTE_MAX_RESULTS),
            ("returnPartialSuccess", "true"),
        ],
    )?;
    let base = first.clone();
    client
        .get_paginated(first, move |body| {
            let response: AggregatedInstancesResponse =
                serde_json::from_value(body.clone()).ok()?;
            let AggregatedInstancesResponse {
                items,
                next_page_token,
            } = response;
            let instances = items
                .into_iter()
                .flat_map(|(scope, scoped)| {
                    let location = location_from_scope(&scope);
                    scoped
                        .instances
                        .unwrap_or_default()
                        .into_iter()
                        .map(move |instance| ScopedInstance {
                            location: location.clone(),
                            instance,
                        })
                })
                .collect();
            let next = next_page_token
                .filter(|token| !token.is_empty())
                .map(|token| {
                    let mut url = base.clone();
                    url.set_query(None);
                    url.query_pairs_mut()
                        .append_pair("maxResults", COMPUTE_MAX_RESULTS)
                        .append_pair("returnPartialSuccess", "true")
                        .append_pair("pageToken", &token);
                    url
                });
            Some((instances, next))
        })
        .await
}

fn cluster_resource(
    cluster: GkeCluster,
    config: &GcpConnectorConfig,
    connector_id: &str,
) -> CloudResource {
    let GkeCluster {
        id,
        name,
        location,
        zone,
        status,
        status_message,
        self_link,
    } = cluster;
    let location = location.or(zone).unwrap_or_default();
    let status_detail = status.or(status_message).unwrap_or_default();
    let id = id.or(self_link).unwrap_or_else(|| name.clone());
    CloudResource {
        provider: CloudProvider::Gcp,
        environment_id: connector_id.to_owned(),
        resource_type: CloudResourceType::KubernetesCluster,
        id,
        name: name.clone(),
        location: location.clone(),
        health: health_from_status(&status_detail),
        status_detail,
        console_url: format!(
            "https://console.cloud.google.com/kubernetes/clusters/details/{location}/{name}?project={}",
            config.project_id
        ),
        cli_command: format!(
            "gcloud container clusters describe {name} --location {location} --project {}",
            config.project_id
        ),
    }
}

fn instance_resource(
    scoped: ScopedInstance,
    config: &GcpConnectorConfig,
    connector_id: &str,
) -> CloudResource {
    let ScopedInstance { location, instance } = scoped;
    let GcpInstance {
        id,
        name,
        zone,
        status,
        status_message,
        self_link,
    } = instance;
    let location = if location.is_empty() {
        zone.as_deref().map(location_from_scope).unwrap_or_default()
    } else {
        location
    };
    let status_detail = status.or(status_message).unwrap_or_default();
    let id = id.or(self_link).unwrap_or_else(|| name.clone());
    CloudResource {
        provider: CloudProvider::Gcp,
        environment_id: connector_id.to_owned(),
        resource_type: CloudResourceType::ComputeInstance,
        id,
        name: name.clone(),
        location: location.clone(),
        health: health_from_status(&status_detail),
        status_detail,
        console_url: format!(
            "https://console.cloud.google.com/compute/instancesDetail/zones/{location}/{name}?project={}",
            config.project_id
        ),
        cli_command: format!(
            "gcloud compute instances describe {name} --zone {location} --project {}",
            config.project_id
        ),
    }
}

fn gcp_url(endpoint: &str, path: &[&str], query: &[(&str, &str)]) -> Result<Url, CloudClientError> {
    let mut url = Url::parse(endpoint)
        .map_err(|_| CloudClientError::Configuration("invalid GCP endpoint".into()))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| CloudClientError::Configuration("invalid GCP endpoint".into()))?;
        for segment in path {
            segments.push(segment);
        }
    }
    if !query.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(key, value);
        }
    }
    Ok(url)
}

fn location_from_scope(scope: &str) -> String {
    scope
        .rsplit('/')
        .next()
        .filter(|location| !location.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn health_from_status(status: &str) -> CloudHealthState {
    match status.to_ascii_uppercase().as_str() {
        "ACTIVE" | "AVAILABLE" | "OK" | "READY" | "RUNNING" | "SUCCEEDED" => {
            CloudHealthState::Healthy
        }
        "DEGRADED" | "PROVISIONING" | "RECONCILING" | "REPAIRING" | "STAGING" | "STARTING"
        | "STOPPING" | "SUSPENDING" | "UPDATING" => CloudHealthState::Degraded,
        "ERROR" | "FAILED" | "FINISHED" | "STOPPED" | "SUSPENDED" | "TERMINATED"
        | "UNAVAILABLE" => CloudHealthState::Unavailable,
        _ => CloudHealthState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        CloudAccessState, CloudClient, CloudHealthState, CloudProvider, CloudResourceType,
        FakeCredentialProvider, GcpConnectorConfig,
    };
    use httpmock::MockServer;
    use std::sync::Arc;

    const GKE_FIXTURE: &str = include_str!(
        "../../../docs/superpowers/fixtures/2026-08-27-capture/gcp/gcp_gke_list_clusters.json"
    );
    const COMPUTE_FIXTURE: &str = include_str!(
        "../../../docs/superpowers/fixtures/2026-08-27-capture/gcp/gcp_compute_aggregated_instances.json"
    );

    #[tokio::test]
    async fn inventory_maps_gke_clusters_and_compute_instances_into_the_shared_model() {
        let server = MockServer::start();
        let gke = server.mock(|when, then| {
            when.method("GET")
                .path("/container/v1/projects/my-project/locations/-/clusters");
            then.status(200)
                .header("content-type", "application/json")
                .body(GKE_FIXTURE);
        });
        let compute = server.mock(|when, then| {
            when.method("GET")
                .path("/compute/compute/v1/projects/my-project/aggregated/instances")
                .query_param("maxResults", "100")
                .query_param("returnPartialSuccess", "true");
            then.status(200)
                .header("content-type", "application/json")
                .body(COMPUTE_FIXTURE);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = GcpConnectorConfig {
            project_id: "my-project".into(),
        };

        let resources = super::inventory_with_endpoints(
            &client,
            &config,
            "gcp-1",
            &server.url("/container"),
            &server.url("/compute"),
        )
        .await
        .unwrap();

        gke.assert_hits(1);
        compute.assert_hits(1);

        let cluster = resources
            .iter()
            .find(|resource| resource.resource_type == CloudResourceType::KubernetesCluster)
            .expect("a cluster");
        assert_eq!(cluster.provider, CloudProvider::Gcp);
        assert_eq!(cluster.environment_id, "gcp-1");
        assert!(!cluster.location.is_empty());
        assert_eq!(cluster.health, CloudHealthState::Healthy);
        assert_eq!(cluster.status_detail, "RUNNING");
        assert!(cluster.console_url.starts_with("https://"));
        assert!(cluster.cli_command.starts_with("gcloud container clusters"));

        let instance = resources
            .iter()
            .find(|resource| resource.resource_type == CloudResourceType::ComputeInstance)
            .expect("an instance");
        assert_eq!(instance.provider, CloudProvider::Gcp);
        assert_eq!(instance.environment_id, "gcp-1");
        assert!(!instance.location.is_empty());
        assert_eq!(instance.health, CloudHealthState::Healthy);
        assert_eq!(instance.status_detail, "RUNNING");
        assert!(instance.console_url.starts_with("https://"));
        assert!(instance.cli_command.starts_with("gcloud compute instances"));
    }

    #[tokio::test]
    async fn access_check_names_the_missing_permission_on_403() {
        let server = MockServer::start();
        let denied = server.mock(|when, then| {
            when.method("GET")
                .path("/container/v1/projects/my-project/locations/-/clusters");
            then.status(403).body("PERMISSION_DENIED");
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = GcpConnectorConfig {
            project_id: "my-project".into(),
        };

        let environment =
            super::access_check_with_endpoint(&client, &config, "gcp-1", &server.url("/container"))
                .await;

        denied.assert_hits(1);
        assert_eq!(environment.access, CloudAccessState::PermissionDenied);
        assert_eq!(environment.remedy, "container.clusters.list");
        assert_eq!(environment.account_label, "my-project");
    }

    #[tokio::test]
    async fn access_check_names_the_login_command_on_401() {
        let server = MockServer::start();
        let expired = server.mock(|when, then| {
            when.method("GET")
                .path("/container/v1/projects/my-project/locations/-/clusters");
            then.status(401);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = GcpConnectorConfig {
            project_id: "my-project".into(),
        };

        let environment =
            super::access_check_with_endpoint(&client, &config, "gcp-1", &server.url("/container"))
                .await;

        expired.assert_hits(1);
        assert_eq!(environment.access, CloudAccessState::SessionExpired);
        assert_eq!(environment.remedy, "gcloud auth application-default login");
    }

    #[tokio::test]
    async fn access_check_offers_the_login_command_when_no_credential_resolves() {
        let client = CloudClient::new(Arc::new(FakeCredentialProvider::no_credential())).unwrap();
        let config = GcpConnectorConfig {
            project_id: "my-project".into(),
        };

        let environment = super::access_check(&client, &config, "gcp-1").await;

        assert_eq!(environment.access, CloudAccessState::NoCredential);
        assert_eq!(environment.remedy, "gcloud auth application-default login");
    }

    #[tokio::test]
    async fn aggregated_instance_list_flattens_zones_and_skips_empty_scopes() {
        let server = MockServer::start();
        let gke = server.mock(|when, then| {
            when.method("GET")
                .path("/container/v1/projects/my-project/locations/-/clusters");
            then.status(200)
                .header("content-type", "application/json")
                .body(GKE_FIXTURE);
        });
        let compute = server.mock(|when, then| {
            when.method("GET")
                .path("/compute/compute/v1/projects/my-project/aggregated/instances")
                .query_param("maxResults", "100")
                .query_param("returnPartialSuccess", "true");
            then.status(200)
                .header("content-type", "application/json")
                .body(COMPUTE_FIXTURE);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = GcpConnectorConfig {
            project_id: "my-project".into(),
        };

        let resources = super::inventory_with_endpoints(
            &client,
            &config,
            "gcp-1",
            &server.url("/container"),
            &server.url("/compute"),
        )
        .await
        .unwrap();
        let instances: Vec<_> = resources
            .iter()
            .filter(|resource| resource.resource_type == CloudResourceType::ComputeInstance)
            .collect();

        gke.assert_hits(1);
        compute.assert_hits(1);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].location, "asia-southeast1-a");
        assert!(!instances[0].status_detail.is_empty());
    }
}
