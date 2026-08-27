use super::{
    classify_access, AzureConnectorConfig, CloudAccessState, CloudClient, CloudClientError,
    CloudEnvironment, CloudHealthState, CloudProvider, CloudResource, CloudResourceType,
};
use reqwest::Url;
use serde::Deserialize;

const ARM_ENDPOINT: &str = "https://management.azure.com";
const AKS_API_VERSION: &str = "2026-05-01";
const VM_API_VERSION: &str = "2026-03-01";
const AKS_PERMISSION: &str = "Microsoft.ContainerService/managedClusters/read";

#[derive(Debug, Deserialize)]
struct AzureListPage<T> {
    value: Vec<T>,
    #[serde(rename = "nextLink", default)]
    next_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AksCluster {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    properties: AksProperties,
}

#[derive(Debug, Default, Deserialize)]
struct AksProperties {
    #[serde(rename = "provisioningState", default)]
    provisioning_state: Option<String>,
    #[serde(rename = "powerState", default)]
    power_state: Option<AzurePowerState>,
}

#[derive(Debug, Deserialize)]
struct AzurePowerState {
    #[serde(default)]
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VirtualMachine {
    #[serde(default)]
    id: Option<String>,
    name: String,
    #[serde(default)]
    location: String,
    #[serde(default)]
    properties: VmProperties,
}

#[derive(Debug, Default, Deserialize)]
struct VmProperties {
    #[serde(rename = "provisioningState", default)]
    provisioning_state: Option<String>,
    #[serde(rename = "instanceView", default)]
    instance_view: Option<InstanceView>,
}

#[derive(Debug, Default, Deserialize)]
struct InstanceView {
    #[serde(default)]
    statuses: Vec<InstanceStatus>,
}

#[derive(Debug, Deserialize)]
struct InstanceStatus {
    #[serde(default)]
    code: Option<String>,
}

pub async fn inventory(
    client: &CloudClient,
    config: &AzureConnectorConfig,
    connector_id: &str,
) -> Result<Vec<CloudResource>, CloudClientError> {
    inventory_with_endpoint(client, config, connector_id, ARM_ENDPOINT).await
}

async fn inventory_with_endpoint(
    client: &CloudClient,
    config: &AzureConnectorConfig,
    connector_id: &str,
    endpoint: &str,
) -> Result<Vec<CloudResource>, CloudClientError> {
    let clusters = list_aks(client, config, endpoint).await?;
    let mut resources = clusters
        .into_iter()
        .map(|cluster| aks_resource(cluster, config, connector_id))
        .collect::<Vec<_>>();
    resources.extend(inventory_vms_with_endpoint(client, config, connector_id, endpoint).await?);
    Ok(resources)
}

async fn inventory_vms_with_endpoint(
    client: &CloudClient,
    config: &AzureConnectorConfig,
    connector_id: &str,
    endpoint: &str,
) -> Result<Vec<CloudResource>, CloudClientError> {
    let vms = list_vms(client, config, endpoint).await?;
    Ok(vms
        .into_iter()
        .map(|vm| vm_resource(vm, config, connector_id))
        .collect())
}

pub async fn access_check(
    client: &CloudClient,
    config: &AzureConnectorConfig,
    connector_id: &str,
) -> CloudEnvironment {
    access_check_with_endpoint(client, config, connector_id, ARM_ENDPOINT).await
}

async fn access_check_with_endpoint(
    client: &CloudClient,
    config: &AzureConnectorConfig,
    connector_id: &str,
    endpoint: &str,
) -> CloudEnvironment {
    let result = list_aks(client, config, endpoint).await.map(|_| ());
    let (access, mut remedy) = classify_access(&result);
    if access == CloudAccessState::NoCredential
        || (access == CloudAccessState::SessionExpired && remedy.is_empty())
    {
        remedy = azure_login_command(&config.tenant_id);
    } else if access == CloudAccessState::PermissionDenied && remedy.is_empty() {
        remedy = AKS_PERMISSION.into();
    }

    CloudEnvironment {
        connector_id: connector_id.to_owned(),
        provider: CloudProvider::Azure,
        account_label: config.subscription_id.clone(),
        location: String::new(),
        access,
        remedy,
    }
}

async fn list_aks(
    client: &CloudClient,
    config: &AzureConnectorConfig,
    endpoint: &str,
) -> Result<Vec<AksCluster>, CloudClientError> {
    let first = collection_url(
        endpoint,
        &config.subscription_id,
        "Microsoft.ContainerService",
        "managedClusters",
        &[("api-version", AKS_API_VERSION)],
    )?;
    client
        .get_paginated(first, |body| {
            let page: AzureListPage<AksCluster> = serde_json::from_value(body.clone()).ok()?;
            let next = next_link(page.next_link)?;
            Some((page.value, next))
        })
        .await
}

async fn list_vms(
    client: &CloudClient,
    config: &AzureConnectorConfig,
    endpoint: &str,
) -> Result<Vec<VirtualMachine>, CloudClientError> {
    let first = collection_url(
        endpoint,
        &config.subscription_id,
        "Microsoft.Compute",
        "virtualMachines",
        &[("api-version", VM_API_VERSION), ("statusOnly", "true")],
    )?;
    client
        .get_paginated(first, |body| {
            let page: AzureListPage<VirtualMachine> = serde_json::from_value(body.clone()).ok()?;
            let next = next_link(page.next_link)?;
            Some((page.value, next))
        })
        .await
}

fn next_link(link: Option<String>) -> Option<Option<Url>> {
    match link.filter(|link| !link.is_empty()) {
        Some(link) => match Url::parse(&link) {
            Ok(url) => Some(Some(url)),
            // Captured fixtures redact the continuation host as `<DNS_NAME>`;
            // that identity placeholder is intentionally not a live URL.
            Err(_) if link.contains('<') && link.contains('>') => Some(None),
            Err(_) => None,
        },
        None => Some(None),
    }
}

fn collection_url(
    endpoint: &str,
    subscription_id: &str,
    resource_provider: &str,
    resource_type: &str,
    query: &[(&str, &str)],
) -> Result<Url, CloudClientError> {
    let mut url = Url::parse(endpoint)
        .map_err(|_| CloudClientError::Configuration("invalid Azure endpoint".into()))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| CloudClientError::Configuration("invalid Azure endpoint".into()))?;
        segments
            .push("subscriptions")
            .push(subscription_id)
            .push("providers")
            .push(resource_provider)
            .push(resource_type);
    }
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(key, value);
        }
    }
    Ok(url)
}

fn aks_resource(
    cluster: AksCluster,
    config: &AzureConnectorConfig,
    connector_id: &str,
) -> CloudResource {
    let id = cluster.id.unwrap_or_else(|| cluster.name.clone());
    let status_detail = cluster
        .properties
        .power_state
        .as_ref()
        .and_then(|power| power.code.clone())
        .or(cluster.properties.provisioning_state.clone())
        .unwrap_or_default();
    let health = combine_health(
        cluster.properties.provisioning_state.as_deref(),
        cluster
            .properties
            .power_state
            .as_ref()
            .and_then(|power| power.code.as_deref()),
    );
    CloudResource {
        provider: CloudProvider::Azure,
        environment_id: connector_id.to_owned(),
        resource_type: CloudResourceType::KubernetesCluster,
        id: id.clone(),
        name: cluster.name.clone(),
        location: cluster.location,
        health,
        status_detail,
        console_url: azure_console_url(&config.tenant_id, &id),
        cli_command: format!(
            "az aks show --name {} --subscription {}",
            cluster.name, config.subscription_id
        ),
    }
}

fn vm_resource(
    vm: VirtualMachine,
    config: &AzureConnectorConfig,
    connector_id: &str,
) -> CloudResource {
    let id = vm.id.unwrap_or_else(|| vm.name.clone());
    let status_detail = vm_power_status(&vm.properties).unwrap_or_default();
    CloudResource {
        provider: CloudProvider::Azure,
        environment_id: connector_id.to_owned(),
        resource_type: CloudResourceType::ComputeInstance,
        id: id.clone(),
        name: vm.name.clone(),
        location: vm.location,
        health: vm_health(&vm.properties),
        status_detail,
        console_url: azure_console_url(&config.tenant_id, &id),
        cli_command: format!(
            "az vm show --name {} --subscription {}",
            vm.name, config.subscription_id
        ),
    }
}

fn vm_health(properties: &VmProperties) -> CloudHealthState {
    let statuses = properties
        .instance_view
        .as_ref()
        .map(|view| view.statuses.as_slice())
        .unwrap_or_default();
    let provisioning = properties.provisioning_state.as_deref().or_else(|| {
        statuses.iter().find_map(|status| {
            let code = status.code.as_deref()?;
            code.strip_prefix("ProvisioningState/")
                .or_else(|| code.strip_prefix("provisioningState/"))
        })
    });
    let power = statuses.iter().find_map(|status| {
        let code = status.code.as_deref()?;
        if code
            .split_once('/')
            .map(|(kind, _)| kind.eq_ignore_ascii_case("PowerState"))
            .unwrap_or(false)
        {
            Some(code)
        } else {
            None
        }
    });
    combine_health(provisioning, power)
}

fn vm_power_status(properties: &VmProperties) -> Option<String> {
    properties
        .instance_view
        .as_ref()?
        .statuses
        .iter()
        .find_map(|status| {
            let code = status.code.as_deref()?;
            if code
                .split_once('/')
                .map(|(kind, _)| kind.eq_ignore_ascii_case("PowerState"))
                .unwrap_or(false)
            {
                Some(code.to_owned())
            } else {
                None
            }
        })
}

fn combine_health(provisioning: Option<&str>, power: Option<&str>) -> CloudHealthState {
    let states = [provisioning, power]
        .into_iter()
        .flatten()
        .map(health_from_status)
        .collect::<Vec<_>>();
    if states.contains(&CloudHealthState::Unavailable) {
        return CloudHealthState::Unavailable;
    }
    if states.contains(&CloudHealthState::Degraded) {
        return CloudHealthState::Degraded;
    }
    if provisioning.is_some()
        && power.is_some()
        && states
            .iter()
            .all(|state| *state == CloudHealthState::Healthy)
    {
        CloudHealthState::Healthy
    } else {
        CloudHealthState::Unknown
    }
}

fn health_from_status(status: &str) -> CloudHealthState {
    let status = status.rsplit('/').next().unwrap_or(status);
    match status.to_ascii_uppercase().as_str() {
        "ACTIVE" | "AVAILABLE" | "OK" | "RUNNING" | "SUCCEEDED" => CloudHealthState::Healthy,
        "CREATING" | "DELETING" | "PENDING" | "PROVISIONING" | "REBOOTING" | "STARTING"
        | "STOPPING" | "UPDATING" => CloudHealthState::Degraded,
        "DEALLOCATED" | "DEALLOCATING" | "FAILED" | "STOPPED" | "TERMINATED" | "UNAVAILABLE" => {
            CloudHealthState::Unavailable
        }
        _ => CloudHealthState::Unknown,
    }
}

fn azure_console_url(tenant_id: &str, resource_id: &str) -> String {
    format!("https://portal.azure.com/#@{tenant_id}/resource{resource_id}/overview")
}

fn azure_login_command(tenant_id: &str) -> String {
    format!("az login --tenant {tenant_id}")
}

#[cfg(test)]
mod tests {
    use super::super::{
        AzureConnectorConfig, CloudAccessState, CloudClient, CloudHealthState, CloudProvider,
        CloudResourceType, FakeCredentialProvider,
    };
    use httpmock::MockServer;
    use std::sync::Arc;

    const AKS_FIXTURE: &str = include_str!(
        "../../../docs/superpowers/fixtures/2026-08-27-capture/azure/azure_aks_managed_clusters.json"
    );
    const VM_FIXTURE: &str = include_str!(
        "../../../docs/superpowers/fixtures/2026-08-27-capture/azure/azure_compute_virtual_machines_status_only.json"
    );

    #[tokio::test]
    async fn inventory_maps_aks_clusters_and_virtual_machines_into_the_shared_model() {
        let server = MockServer::start();
        let aks = server.mock(|when, then| {
            when.method("GET")
                .path("/subscriptions/sub-1/providers/Microsoft.ContainerService/managedClusters")
                .query_param("api-version", "2026-05-01");
            then.status(200)
                .header("content-type", "application/json")
                .body(AKS_FIXTURE);
        });
        let vms = server.mock(|when, then| {
            when.method("GET")
                .path("/subscriptions/sub-1/providers/Microsoft.Compute/virtualMachines")
                .query_param("api-version", "2026-03-01")
                .query_param("statusOnly", "true");
            then.status(200)
                .header("content-type", "application/json")
                .body(VM_FIXTURE);
        });
        let unexpected_instance_view = server.mock(|when, then| {
            when.method("GET").path_contains("/instanceView");
            then.status(500);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = AzureConnectorConfig {
            subscription_id: "sub-1".into(),
            tenant_id: "tenant-1".into(),
        };

        let resources =
            super::inventory_with_endpoint(&client, &config, "azure-1", &server.url(""))
                .await
                .unwrap();

        aks.assert_hits(1);
        vms.assert_hits(1);
        unexpected_instance_view.assert_hits(0);

        let cluster = resources
            .iter()
            .find(|resource| resource.resource_type == CloudResourceType::KubernetesCluster)
            .expect("a cluster");
        assert_eq!(cluster.provider, CloudProvider::Azure);
        assert_eq!(cluster.environment_id, "azure-1");
        assert!(!cluster.location.is_empty());
        assert_eq!(cluster.health, CloudHealthState::Healthy);
        assert_eq!(cluster.status_detail, "Running");
        assert!(cluster.console_url.starts_with("https://"));
        assert!(cluster.cli_command.starts_with("az aks"));

        let vm = resources
            .iter()
            .find(|resource| resource.resource_type == CloudResourceType::ComputeInstance)
            .expect("a vm");
        assert_eq!(vm.provider, CloudProvider::Azure);
        assert_eq!(vm.health, CloudHealthState::Healthy);
        assert_eq!(vm.status_detail, "PowerState/running");
        assert!(vm.cli_command.starts_with("az vm"));
        assert!(
            !vm.status_detail.is_empty(),
            "the provider's own status is preserved"
        );
    }

    #[tokio::test]
    async fn access_check_names_the_missing_permission_on_403() {
        let server = MockServer::start();
        let denied = server.mock(|when, then| {
            when.method("GET")
                .path("/subscriptions/sub-1/providers/Microsoft.ContainerService/managedClusters")
                .query_param("api-version", "2026-05-01");
            then.status(403).body("AuthorizationFailed");
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = AzureConnectorConfig {
            subscription_id: "sub-1".into(),
            tenant_id: "tenant-1".into(),
        };

        let environment =
            super::access_check_with_endpoint(&client, &config, "azure-1", &server.url("")).await;

        denied.assert_hits(1);
        assert_eq!(environment.access, CloudAccessState::PermissionDenied);
        assert_eq!(
            environment.remedy,
            "Microsoft.ContainerService/managedClusters/read"
        );
        assert_eq!(environment.account_label, "sub-1");
    }

    #[tokio::test]
    async fn access_check_names_the_login_command_on_401() {
        let server = MockServer::start();
        let expired = server.mock(|when, then| {
            when.method("GET")
                .path("/subscriptions/sub-1/providers/Microsoft.ContainerService/managedClusters")
                .query_param("api-version", "2026-05-01");
            then.status(401);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = AzureConnectorConfig {
            subscription_id: "sub-1".into(),
            tenant_id: "tenant-1".into(),
        };

        let environment =
            super::access_check_with_endpoint(&client, &config, "azure-1", &server.url("")).await;

        expired.assert_hits(1);
        assert_eq!(environment.access, CloudAccessState::SessionExpired);
        assert_eq!(environment.remedy, "az login --tenant tenant-1");
    }

    #[tokio::test]
    async fn access_check_offers_the_login_command_when_no_credential_resolves() {
        let client = CloudClient::new(Arc::new(FakeCredentialProvider::no_credential())).unwrap();
        let config = AzureConnectorConfig {
            subscription_id: "sub-1".into(),
            tenant_id: "tenant-1".into(),
        };

        let environment = super::access_check(&client, &config, "azure-1").await;

        assert_eq!(environment.access, CloudAccessState::NoCredential);
        assert_eq!(environment.remedy, "az login --tenant tenant-1");
    }

    #[tokio::test]
    async fn virtual_machine_health_comes_from_the_status_carrying_call() {
        let server = MockServer::start();
        let vms = server.mock(|when, then| {
            when.method("GET")
                .path("/subscriptions/sub-1/providers/Microsoft.Compute/virtualMachines")
                .query_param("api-version", "2026-03-01")
                .query_param("statusOnly", "true");
            then.status(200)
                .header("content-type", "application/json")
                .body(VM_FIXTURE);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = AzureConnectorConfig {
            subscription_id: "sub-1".into(),
            tenant_id: "tenant-1".into(),
        };

        let resources =
            super::inventory_vms_with_endpoint(&client, &config, "azure-1", &server.url(""))
                .await
                .unwrap();

        vms.assert_hits(1);
        let vm = resources
            .iter()
            .find(|resource| resource.resource_type == CloudResourceType::ComputeInstance)
            .expect("a vm");
        assert_eq!(vm.health, CloudHealthState::Healthy);
        assert_eq!(vm.status_detail, "PowerState/running");
    }

    #[test]
    fn absent_azure_health_status_maps_to_unknown() {
        assert_eq!(
            super::combine_health(None, Some("Running")),
            CloudHealthState::Unknown
        );
        assert_eq!(
            super::combine_health(Some("Succeeded"), None),
            CloudHealthState::Unknown
        );
    }

    #[test]
    fn redacted_azure_next_link_ends_the_captured_page() {
        assert_eq!(
            super::next_link(Some(
                "https://<DNS_NAME>/subscriptions/<AZURE_SUBSCRIPTION_ID>".into(),
            )),
            Some(None)
        );
        assert!(super::next_link(Some("not a URL".into())).is_none());
    }
}
