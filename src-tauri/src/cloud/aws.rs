use super::{
    classify_access, AwsConnectorConfig, CloudAccessState, CloudClient, CloudClientError,
    CloudEnvironment, CloudHealthState, CloudProvider, CloudResource, CloudResourceType,
};
use reqwest::Url;
use serde::Deserialize;

const EKS_SERVICE: &str = "eks";
const EC2_SERVICE: &str = "ec2";
const EKS_MAX_RESULTS: &str = "100";
const ACCESS_CHECK_MAX_RESULTS: &str = "1";
const EC2_MAX_RESULTS: &str = "100";
const EC2_API_VERSION: &str = "2016-11-15";
const MAX_EC2_PAGES: usize = 50;

#[derive(Debug, Deserialize)]
struct EksListPage {
    #[serde(default)]
    clusters: Vec<String>,
    #[serde(rename = "nextToken", default)]
    next_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EksDescribeResponse {
    cluster: EksCluster,
}

#[derive(Debug, Deserialize)]
struct EksCluster {
    #[serde(default)]
    arn: Option<String>,
    name: String,
    status: String,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2DescribeResponse {
    #[serde(rename = "reservationSet", default)]
    reservations: Ec2ReservationSet,
    #[serde(rename = "nextToken", default)]
    next_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2ReservationSet {
    #[serde(rename = "item", default)]
    items: Vec<Ec2Reservation>,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2Reservation {
    #[serde(rename = "instancesSet", default)]
    instances: Ec2InstanceSet,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2InstanceSet {
    #[serde(rename = "item", default)]
    items: Vec<Ec2Instance>,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2Instance {
    #[serde(rename = "instanceId", default)]
    id: String,
    #[serde(rename = "instanceState", default)]
    state: Ec2InstanceState,
    #[serde(default)]
    placement: Ec2Placement,
    #[serde(rename = "tagSet", default)]
    tags: Ec2TagSet,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2InstanceState {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2Placement {
    #[serde(rename = "availabilityZone", default)]
    availability_zone: String,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2TagSet {
    #[serde(rename = "item", default)]
    items: Vec<Ec2Tag>,
}

#[derive(Debug, Default, Deserialize)]
struct Ec2Tag {
    #[serde(default)]
    key: String,
    #[serde(default)]
    value: String,
}

pub async fn inventory(
    client: &CloudClient,
    config: &AwsConnectorConfig,
    connector_id: &str,
) -> Result<Vec<CloudResource>, CloudClientError> {
    let eks_base = format!("https://{EKS_SERVICE}.{}.amazonaws.com", config.region);
    let ec2_base = format!("https://{EC2_SERVICE}.{}.amazonaws.com", config.region);
    inventory_with_endpoints(client, config, connector_id, &eks_base, &ec2_base).await
}

async fn inventory_with_endpoints(
    client: &CloudClient,
    config: &AwsConnectorConfig,
    connector_id: &str,
    eks_base: &str,
    ec2_base: &str,
) -> Result<Vec<CloudResource>, CloudClientError> {
    let cluster_names = list_cluster_names(client, eks_base, EKS_MAX_RESULTS).await?;
    let mut resources = Vec::with_capacity(cluster_names.len());

    for cluster_name in cluster_names {
        let url = url_with_query(eks_base, &["clusters", &cluster_name], &[])?;
        let response: EksDescribeResponse = client.get_json(url).await?;
        let cluster = response.cluster;
        let id = cluster.arn.clone().unwrap_or_else(|| cluster.name.clone());
        resources.push(CloudResource {
            provider: CloudProvider::Aws,
            environment_id: connector_id.to_owned(),
            resource_type: CloudResourceType::KubernetesCluster,
            id,
            name: cluster.name.clone(),
            location: config.region.clone(),
            health: health_from_status(&cluster.status),
            status_detail: cluster.status,
            console_url: format!(
                "https://{}.console.aws.amazon.com/eks/home?region={}#/clusters/{}",
                config.region, config.region, cluster.name
            ),
            cli_command: format!(
                "aws eks describe-cluster --name {} --profile {} --region {}",
                cluster.name, config.profile, config.region
            ),
        });
    }

    resources.extend(list_ec2_instances(client, config, connector_id, ec2_base).await?);
    Ok(resources)
}

pub async fn access_check(
    client: &CloudClient,
    config: &AwsConnectorConfig,
    connector_id: &str,
) -> CloudEnvironment {
    let eks_base = format!("https://{EKS_SERVICE}.{}.amazonaws.com", config.region);
    access_check_with_endpoint(client, config, connector_id, &eks_base).await
}

async fn access_check_with_endpoint(
    client: &CloudClient,
    config: &AwsConnectorConfig,
    connector_id: &str,
    eks_base: &str,
) -> CloudEnvironment {
    let result = list_cluster_names(client, eks_base, ACCESS_CHECK_MAX_RESULTS)
        .await
        .map(|_| ());
    let (access, mut remedy) = classify_access(&result);
    if access == CloudAccessState::SessionExpired && remedy.is_empty() {
        remedy = aws_login_command(&config.profile);
    } else if access == CloudAccessState::PermissionDenied && remedy.is_empty() {
        remedy = "eks:ListClusters".into();
    }

    CloudEnvironment {
        connector_id: connector_id.to_owned(),
        provider: CloudProvider::Aws,
        account_label: config.profile.clone(),
        location: config.region.clone(),
        access,
        remedy,
    }
}

async fn list_cluster_names(
    client: &CloudClient,
    eks_base: &str,
    max_results: &str,
) -> Result<Vec<String>, CloudClientError> {
    let first = url_with_query(eks_base, &["clusters"], &[("maxResults", max_results)])?;
    let base = first.clone();
    client
        .get_paginated(first, move |body| {
            let page: EksListPage = serde_json::from_value(body.clone()).ok()?;
            let next = page
                .next_token
                .filter(|token| !token.is_empty())
                .map(|token| {
                    let mut url = base.clone();
                    url.set_query(None);
                    url.query_pairs_mut()
                        .append_pair("maxResults", max_results)
                        .append_pair("nextToken", &token);
                    url
                });
            Some((page.clusters, next))
        })
        .await
}

async fn list_ec2_instances(
    client: &CloudClient,
    config: &AwsConnectorConfig,
    connector_id: &str,
    ec2_base: &str,
) -> Result<Vec<CloudResource>, CloudClientError> {
    let mut next_token = None;
    let mut resources = Vec::new();

    for _ in 0..MAX_EC2_PAGES {
        let mut query = vec![
            ("Action", "DescribeInstances"),
            ("Version", EC2_API_VERSION),
            ("MaxResults", EC2_MAX_RESULTS),
        ];
        if let Some(token) = next_token.as_deref() {
            query.push(("NextToken", token));
        }
        let url = url_with_query(ec2_base, &[], &query)?;
        let (body, _) = client.get_text(url).await?;
        let response: Ec2DescribeResponse =
            quick_xml::de::from_str(&body).map_err(|_| CloudClientError::MalformedResponse)?;

        for reservation in response.reservations.items {
            for instance in reservation.instances.items {
                let status_detail = if instance.state.name.is_empty() {
                    "unknown".to_owned()
                } else {
                    instance.state.name
                };
                let name = instance
                    .tags
                    .items
                    .iter()
                    .find(|tag| tag.key == "Name")
                    .map(|tag| tag.value.clone())
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| instance.id.clone());
                let location = if instance.placement.availability_zone.is_empty() {
                    config.region.clone()
                } else {
                    instance.placement.availability_zone
                };
                resources.push(CloudResource {
                    provider: CloudProvider::Aws,
                    environment_id: connector_id.to_owned(),
                    resource_type: CloudResourceType::ComputeInstance,
                    id: instance.id.clone(),
                    name,
                    location,
                    health: health_from_status(&status_detail),
                    status_detail,
                    console_url: format!(
                        "https://{}.console.aws.amazon.com/ec2/home?region={}#Instances:instanceId={}",
                        config.region, config.region, instance.id
                    ),
                    cli_command: format!(
                        "aws ec2 describe-instances --instance-ids {} --profile {} --region {}",
                        instance.id, config.profile, config.region
                    ),
                });
            }
        }

        next_token = response.next_token.filter(|token| !token.is_empty());
        if next_token.is_none() {
            break;
        }
    }

    Ok(resources)
}

fn url_with_query(
    base: &str,
    path: &[&str],
    query: &[(&str, &str)],
) -> Result<Url, CloudClientError> {
    let mut url = Url::parse(base)
        .map_err(|_| CloudClientError::Configuration("invalid AWS endpoint".into()))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| CloudClientError::Configuration("invalid AWS endpoint".into()))?;
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

fn aws_login_command(profile: &str) -> String {
    format!("aws sso login --profile {profile}")
}

fn health_from_status(status: &str) -> CloudHealthState {
    match status.to_ascii_uppercase().as_str() {
        "ACTIVE" | "RUNNING" | "AVAILABLE" | "OK" => CloudHealthState::Healthy,
        "CREATING" | "UPDATING" | "PENDING" | "STARTING" | "STOPPING" | "REBOOTING"
        | "DEGRADED" => CloudHealthState::Degraded,
        "FAILED" | "UNAVAILABLE" | "STOPPED" | "TERMINATED" | "SHUTTING-DOWN" | "SHUTDOWN" => {
            CloudHealthState::Unavailable
        }
        _ => CloudHealthState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        AwsConnectorConfig, CloudAccessState, CloudClient, CloudHealthState, CloudProvider,
        CloudResourceType, FakeCredentialProvider,
    };
    use httpmock::MockServer;
    use std::sync::Arc;

    const EKS_LIST_FIXTURE: &str = include_str!(
        "../../../docs/superpowers/fixtures/2026-08-27-capture/aws/aws_eks_list_clusters.json"
    );
    const EKS_DESCRIBE_FIXTURE: &str = include_str!(
        "../../../docs/superpowers/fixtures/2026-08-27-capture/aws/aws_eks_describe_cluster.json"
    );
    const EC2_FIXTURE: &str = include_str!(
        "../../../docs/superpowers/fixtures/2026-08-27-capture/aws/aws_ec2_describe_instances.xml"
    );

    #[tokio::test]
    async fn inventory_maps_captured_eks_and_ec2_fixtures_into_the_shared_model() {
        let server = MockServer::start();
        let eks_list = server.mock(|when, then| {
            when.method("GET")
                .path("/eks/clusters")
                .query_param("maxResults", "100");
            then.status(200)
                .header("content-type", "application/json")
                .body(EKS_LIST_FIXTURE);
        });
        let eks_describe = server.mock(|when, then| {
            when.method("GET").path_contains("/eks/clusters/");
            then.status(200)
                .header("content-type", "application/json")
                .body(EKS_DESCRIBE_FIXTURE);
        });
        let ec2 = server.mock(|when, then| {
            when.method("GET")
                .path("/ec2")
                .query_param("Action", "DescribeInstances")
                .query_param("Version", "2016-11-15")
                .query_param("MaxResults", "100");
            then.status(200)
                .header("content-type", "text/xml;charset=UTF-8")
                .body(EC2_FIXTURE);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = AwsConnectorConfig {
            profile: "prod".into(),
            region: "ap-southeast-1".into(),
        };

        let resources = super::inventory_with_endpoints(
            &client,
            &config,
            "aws-1",
            &server.url("/eks"),
            &server.url("/ec2"),
        )
        .await
        .unwrap();

        eks_list.assert();
        eks_describe.assert();
        ec2.assert();

        let cluster = resources
            .iter()
            .find(|resource| resource.resource_type == CloudResourceType::KubernetesCluster)
            .expect("a cluster");
        assert_eq!(cluster.provider, CloudProvider::Aws);
        assert_eq!(cluster.environment_id, "aws-1");
        assert_eq!(cluster.location, "ap-southeast-1");
        assert_eq!(cluster.health, CloudHealthState::Healthy);
        assert_eq!(cluster.status_detail, "ACTIVE");
        assert!(cluster.console_url.starts_with("https://"));
        assert!(cluster.cli_command.starts_with("aws eks"));
        assert!(cluster.cli_command.contains("--profile prod"));
        assert!(cluster.cli_command.contains("--region ap-southeast-1"));

        let instance = resources
            .iter()
            .find(|resource| resource.resource_type == CloudResourceType::ComputeInstance)
            .expect("an instance");
        assert_eq!(instance.provider, CloudProvider::Aws);
        assert_eq!(instance.name, "thalassaops-s10-fixture-ec2");
        assert_eq!(instance.location, "ap-southeast-1a");
        assert_eq!(instance.health, CloudHealthState::Healthy);
        assert_eq!(instance.status_detail, "running");
        assert!(
            !instance.status_detail.is_empty(),
            "provider status is preserved"
        );
    }

    #[tokio::test]
    async fn access_check_names_the_missing_permission_on_403() {
        let server = MockServer::start();
        let denied = server.mock(|when, then| {
            when.method("GET")
                .path("/eks/clusters")
                .query_param("maxResults", "1");
            then.status(403);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = AwsConnectorConfig {
            profile: "prod".into(),
            region: "ap-southeast-1".into(),
        };

        let environment =
            super::access_check_with_endpoint(&client, &config, "aws-1", &server.url("/eks")).await;

        denied.assert();
        assert_eq!(environment.access, CloudAccessState::PermissionDenied);
        assert_eq!(environment.remedy, "eks:ListClusters");
        assert_eq!(environment.account_label, "prod");
    }

    #[tokio::test]
    async fn access_check_names_the_login_command_on_401() {
        let server = MockServer::start();
        let expired = server.mock(|when, then| {
            when.method("GET")
                .path("/eks/clusters")
                .query_param("maxResults", "1");
            then.status(401);
        });
        let client =
            CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
        let config = AwsConnectorConfig {
            profile: "prod".into(),
            region: "ap-southeast-1".into(),
        };

        let environment =
            super::access_check_with_endpoint(&client, &config, "aws-1", &server.url("/eks")).await;

        expired.assert();
        assert_eq!(environment.access, CloudAccessState::SessionExpired);
        assert_eq!(environment.remedy, "aws sso login --profile prod");
    }

    #[tokio::test]
    async fn access_check_offers_the_login_command_when_no_credential_resolves() {
        let client = CloudClient::new(Arc::new(FakeCredentialProvider::no_credential())).unwrap();
        let config = AwsConnectorConfig {
            profile: "prod".into(),
            region: "ap-southeast-1".into(),
        };

        let environment = super::access_check(&client, &config, "aws-1").await;

        assert_eq!(environment.access, CloudAccessState::NoCredential);
        assert!(
            environment.remedy.contains("aws sso login"),
            "remedy: {}",
            environment.remedy
        );
    }
}
