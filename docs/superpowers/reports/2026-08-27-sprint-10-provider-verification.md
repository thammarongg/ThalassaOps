# Sprint 10 provider verification

Date: 2026-08-27
Task: Sprint 10, Task 1
Scope: provider authentication APIs, inventory endpoints, response shapes, and live wire captures

This report was prepared after reading the Sprint 10 plan (Global Constraints and Task 1), the Sprint 10 design, and ADR 0006. It records provider documentation and crate-source verification separately from live response captures. No product code was written.

## Gate summary

| Task 1 step | Result |
| --- | --- |
| Auth crates and exact credential/signing APIs | Verified for the pinned versions below. azure_identity needs a design decision because its desktop credential path invokes Azure CLI. gcp_auth can also fall back to gcloud. |
| Six call families | Verified. The EKS cluster family is two HTTP operations (list names, then describe each name), so seven HTTP responses are captured below. |
| EC2 response format | Resolved: live EC2 responded with Content-Type text/xml;charset=UTF-8 and an XML error body. A JSON EC2 mapper would be incorrect. |
| Azure VM power state | Resolved: default List All representation does not include runtime instanceView.statuses; use statusOnly=true in the list request, or make a per-VM instance-view request. The recommended path keeps it in one list call. |
| Real response bodies | One real body was captured for each of the seven HTTP operations. The environment had no usable AWS, Azure, or GCP session, so all captures are provider error responses rather than successful resource pages. These are wire samples, not mapper fixtures. Authenticated 200 bodies remain a required follow-up before Tasks 7–9 copy fixtures. |
| cargo build --workspace | PASS. Run from src-tauri/ after adding the four dependencies; cold build finished in 53.39s with exit code 0, and a clean incremental rerun also finished with exit code 0. |

## 1. Auth crates

All four names resolve to the official crates. Versions are pinned exactly in src-tauri/Cargo.toml; the lockfile records the complete resolved graph.

| Crate | Verified version | Last crates.io release | Maintenance evidence | Exact API result |
| --- | ---: | --- | --- | --- |
| aws-config | 1.11.0 | 2026-08-20 | Smithy Rust upstream had a commit on 2026-08-27. | aws_config::defaults(...).profile_name(...).region(...).load().await yields SdkConfig; its credential provider's provide_credentials().await yields AWS Credentials. |
| aws-sigv4 | 1.5.1 | 2026-07-08 | Same active Smithy Rust upstream. | aws_sigv4::http_request::sign(signable_request, &signing_params) yields signing instructions and a signature; apply instructions to the reqwest request before sending. |
| azure_identity | 1.0.0 | 2026-05-12 | Azure SDK for Rust upstream had a commit on 2026-08-27. | AzureCliCredential::new(...) followed by TokenCredential::get_token(&["https://management.azure.com/.default"], None).await yields an Azure AccessToken. |
| gcp_auth | 0.12.7 | 2026-06-22 | djc/gcp_auth upstream had a commit on 2026-08-10. | gcp_auth::provider().await yields a TokenProvider; provider.token(&["https://www.googleapis.com/auth/cloud-platform"]).await yields an OAuth token. |

### AWS

The verified aws-config call is:

    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::v2023_11_09())
        .profile_name(profile)
        .region(aws_config::Region::new(region.to_owned()))
        .load()
        .await;
    let provider = sdk_config
        .credentials_provider()
        .expect("AWS credentials provider");
    let credentials = provider.provide_credentials().await?;

credentials exposes the access key, secret key, and optional session token. The provider must remain transient. aws-config may resolve profiles, SSO, or configured credential-process providers internally; that is the delegated auth boundary in the plan, and no resolved value may be persisted or logged.

aws-sigv4 does not resolve credentials. It signs a request after receiving an AWS identity made from those transient credentials:

    let identity = aws_credential_types::Credentials::new(
        credentials.access_key_id(),
        credentials.secret_access_key(),
        credentials.session_token().map(str::to_owned),
        None,
        "thalassaops",
    );
    let params = aws_sigv4::sign::v4::SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name(service) // EKS or EC2
        .time(std::time::SystemTime::now())
        .settings(aws_sigv4::http_request::SigningSettings::default())
        .build()?
        .into();
    let signable = aws_sigv4::http_request::SignableRequest::new(
        "GET", uri, headers.iter(), aws_sigv4::http_request::SignableBody::Bytes(&[]),
    )?;
    let (instructions, _signature) = aws_sigv4::http_request::sign(signable, &params)?.into_parts();
    instructions.apply_to_request_http1x(&mut request);

The AWS API docs identify EKS's API model/version as 2017-11-01; it is not an api-version query parameter. EC2's query API requires Version=2016-11-15 in the URL.

### Azure

The direct credential call that accepts the configured tenant selector is:

    let credential = azure_identity::AzureCliCredential::new(Some(
        azure_identity::AzureCliCredentialOptions {
            tenant_id: Some(tenant_id.to_owned()),
            subscription: Some(subscription_id.to_owned()),
            ..Default::default()
        },
    ))?;
    let token = azure_core::credentials::TokenCredential::get_token(
        &credential,
        &["https://management.azure.com/.default"],
        None,
    )
    .await?;
    let bearer = token.token.secret();

DeveloperToolsCredential::new(None) is the convenient fallback, but it tries AzureCliCredential and then AzureDeveloperCliCredential. Both paths execute a provider CLI. That conflicts with ADR 0006 and the plan's explicit “ThalassaOps does not execute provider CLIs” constraint. azure_identity is maintained, but it is not usable as an ADR-compliant ambient desktop credential provider without a coordinator decision (for example, an approved external token handoff or a change to the no-CLI constraint). No client secret alternative was introduced because the plan forbids storing cloud credentials.

### GCP

The verified ambient ADC call is:

    let provider = gcp_auth::provider().await?;
    let token = provider
        .token(&["https://www.googleapis.com/auth/cloud-platform"])
        .await?;
    let bearer = token.as_str();

gcp_auth::provider() checks GOOGLE_APPLICATION_CREDENTIALS, the local ADC file, metadata credentials, and finally a gcloud auth print-access-token fallback. ADC is compatible with the no-secret-storage boundary; the gcloud fallback is another ADR consideration if “does not execute provider CLIs” applies to auth-crate internals as well.

### Auth sources

- [aws-config 1.11.0](https://crates.io/crates/aws-config/1.11.0) and [AWS config docs](https://docs.rs/aws-config/1.11.0/aws_config/)
- [aws-sigv4 1.5.1](https://crates.io/crates/aws-sigv4/1.5.1) and [AWS SigV4 HTTP request docs](https://docs.rs/aws-sigv4/1.5.1/aws_sigv4/http_request/index.html)
- [azure_identity 1.0.0](https://crates.io/crates/azure_identity/1.0.0), [Azure Identity for Rust](https://github.com/Azure/azure-sdk-for-rust), and [Azure Identity README](https://raw.githubusercontent.com/Azure/azure-sdk-for-rust/main/sdk/identity/azure_identity/README.md)
- [gcp_auth 0.12.7](https://crates.io/crates/gcp_auth/0.12.7), [gcp_auth upstream](https://github.com/djc/gcp_auth), and [gcp_auth README](https://raw.githubusercontent.com/djc/gcp_auth/master/README.md)

## 2. Provider calls

The six resource call families are represented by seven HTTP operations because EKS must list names and then describe each name to obtain status. Every URL below is an internally selected fixed endpoint; selectors (region, subscriptionId, and project) come from connector configuration and are not discovered from the machine.

| Operation | URL template and method | Required query/API version | Response and pagination | Health/status |
| --- | --- | --- | --- | --- |
| AWS EKS ListClusters | GET https://eks.{region}.amazonaws.com/clusters | Optional maxResults (1–100), opaque nextToken, optional include=all; service API model 2017-11-01 (not a URL query) | application/json; opaque nextToken cursor | Absent from the name list |
| AWS EKS DescribeCluster | GET https://eks.{region}.amazonaws.com/clusters/{name} | No query API version; same EKS 2017-11-01 service model | application/json; no pagination | cluster.status is present; health.issues is also present when returned |
| AWS EC2 DescribeInstances | GET https://ec2.{region}.amazonaws.com/ | Required Action=DescribeInstances and Version=2016-11-15; use MaxResults and opaque NextToken for bounded pages; filters are optional | text/xml (live wire confirmed); token pagination via nextToken | reservationSet.item.instancesSet.item.instanceState.name is present |
| Azure AKS managed clusters | GET https://management.azure.com/subscriptions/{subscriptionId}/providers/Microsoft.ContainerService/managedClusters | Required api-version=2026-05-01 | application/json; value plus nextLink URL pagination | properties.provisioningState, powerState, and status are schema fields; do not assume every field is emitted for every resource |
| Azure Compute VM List All | GET https://management.azure.com/subscriptions/{subscriptionId}/providers/Microsoft.Compute/virtualMachines | Required api-version=2026-03-01; recommended statusOnly=true to include runtime status; optional $filter and $expand=instanceView constraints apply | application/json; value plus nextLink URL pagination | Default list omits runtime instanceView.statuses; statusOnly=true includes runtime status in the list response, avoiding a second call |
| GCP GKE ListClusters | GET https://container.googleapis.com/v1/projects/{project}/locations/-/clusters | No query required beyond /v1/ and parent path; projectId and zone query parameters are deprecated | application/json; clusters and missingZones; no pagination token in ListClustersResponse | cluster.status and statusMessage are present |
| GCP Compute aggregated instances | GET https://compute.googleapis.com/compute/v1/projects/{project}/aggregated/instances | Optional maxResults (bounded), opaque pageToken, and returnPartialSuccess=true | application/json; nextPageToken token pagination; instances grouped by scope under items | instance.status and statusMessage are present |

Required read permissions are eks:ListClusters, eks:DescribeCluster, and ec2:DescribeInstances for AWS; Microsoft.ContainerService/managedClusters/read and Microsoft.Compute/virtualMachines/read for Azure; and container.clusters.list and compute.instances.list for GCP.

### Important design escalations

1. EC2 is XML, not JSON. The permitted choices from Task 1 are (a) a narrowly scoped XML dependency for the EC2 mapper, (b) aws-sdk-ec2 for that call, or (c) a JSON-returning alternative. The coordinator must choose; this report adds no mapper or parser.
2. Azure VM status is absent from the default list representation. Use statusOnly=true in the bounded list request if accepted by the selected API version. Otherwise, the design must allow a second instance-view request per VM; this report does not silently assume the default list has power state.
3. azure_identity's available desktop credential flow executes az/azd, conflicting with the no-provider-CLI rule. gcp_auth has a similar gcloud fallback. The coordinator must decide whether the auth boundary permits those crate internals or whether a different token handoff is required.
4. Successful authenticated 200 resource bodies could not be captured in this environment. The real captures below prove endpoint reachability and media/error formats, but they are not valid resource fixtures. Tasks 7–9 must wait for authenticated captures or explicitly remain blocked; documentation examples must not be copied as fixture bodies.

### Provider API sources

- [EKS ListClusters](https://docs.aws.amazon.com/eks/latest/APIReference/API_ListClusters.html)
- [EKS DescribeCluster](https://docs.aws.amazon.com/eks/latest/APIReference/API_DescribeCluster.html)
- [EC2 DescribeInstances](https://docs.aws.amazon.com/AWSEC2/latest/APIReference/API_DescribeInstances.html)
- [EC2 Query API](https://docs.aws.amazon.com/ec2/latest/devguide/Query-Requests.html)
- [Azure AKS managed-clusters list, API 2026-05-01](https://learn.microsoft.com/en-us/rest/api/aks/managed-clusters/list?view=rest-aks-2026-05-01)
- [Azure VM List All, API 2026-03-01](https://learn.microsoft.com/en-us/rest/api/compute/virtual-machines/list-all?view=rest-compute-2026-03-01)
- [Azure VM instance view](https://learn.microsoft.com/en-us/rest/api/compute/virtual-machines/instance-view?view=rest-compute-2024-07-01)
- [GKE ListClusters](https://docs.cloud.google.com/kubernetes-engine/docs/reference/rest/v1/projects.locations.clusters/list)
- [GKE ListClustersResponse](https://docs.cloud.google.com/kubernetes-engine/docs/reference/rest/v1/ListClustersResponse)
- [Compute Engine aggregated instances](https://docs.cloud.google.com/compute/docs/reference/rest/v1/instances/aggregatedList)

## 3. Live response captures

Capture date: 2026-08-27. Requests used placeholder selectors and no authorization so that no cloud secret, token, or credential reference entered this report. Request IDs and any credential-related headers are intentionally omitted. The bodies below are copied from live provider responses, with only placeholder subscription/identifier text normalized.

### AWS EKS ListClusters

Request: GET https://eks.us-east-1.amazonaws.com/clusters?maxResults=1
Observed: HTTP 403, Content-Type: application/json

    {"message":"Missing Authentication Token"}

This confirms the endpoint returned JSON over the wire; it does not establish the successful clusters page shape.

### AWS EKS DescribeCluster

Request: GET https://eks.us-east-1.amazonaws.com/clusters/thlassa-verification-placeholder
Observed: HTTP 403, Content-Type: application/json

    {"message":"Missing Authentication Token"}

### AWS EC2 DescribeInstances

Request: GET https://ec2.us-east-1.amazonaws.com/?Action=DescribeInstances&Version=2016-11-15&MaxResults=1
Observed: HTTP 400, Content-Type: text/xml;charset=UTF-8

    <?xml version="1.0" encoding="UTF-8"?>
    <Response><Errors><Error><Code>MissingParameter</Code><Message>The request must contain the parameter AWSAccessKeyId</Message></Error></Errors><RequestID>[REDACTED]</RequestID></Response>

This is the required assumption resolution: EC2 Query API responses are XML, including errors.

### Azure AKS managed clusters (pinned API)

Request: GET https://management.azure.com/subscriptions/<SUBSCRIPTION_ID>/providers/Microsoft.ContainerService/managedClusters?api-version=2026-05-01
Observed: HTTP 404, Content-Type: application/json

    {"error":{"code":"SubscriptionNotFound","message":"The subscription '<SUBSCRIPTION_ID>' could not be found."}}

The live probe used an intentionally invalid subscription to avoid guessing an operator's subscription. This capture uses the currently documented 2026-05-01 API version.

### Azure Compute VM List All (pinned API, status-only)

Request: GET https://management.azure.com/subscriptions/<SUBSCRIPTION_ID>/providers/Microsoft.Compute/virtualMachines?api-version=2026-03-01&statusOnly=true
Observed: HTTP 404, Content-Type: application/json

    {"error":{"code":"SubscriptionNotFound","message":"The subscription '<SUBSCRIPTION_ID>' could not be found."}}

This capture uses the subscription-level List All operation and statusOnly=true, the recommended request that includes runtime state in a successful list response. No successful default-list body is treated as evidence that power state exists.

### GCP GKE ListClusters

Request: GET https://container.googleapis.com/v1/projects/thalassaops-verification/locations/-/clusters
Observed: HTTP 401, Content-Type: application/json

    {
      "error": {
        "code": 401,
        "message": "Request is missing required authentication credential. Expected OAuth 2 access token, login cookie or other valid authentication credential. See https://developers.google.com/identity/sign-in/web/devconsole-project.",
        "status": "UNAUTHENTICATED",
        "details": [
          {
            "@type": "type.googleapis.com/google.rpc.ErrorInfo",
            "reason": "CREDENTIALS_MISSING",
            "domain": "googleapis.com",
            "metadata": {
              "method": "google.container.v1.ClusterManager.ListClusters",
              "service": "container.googleapis.com"
            }
          }
        ]
      }
    }

### GCP Compute aggregated instances

Request: GET https://compute.googleapis.com/compute/v1/projects/thalassaops-verification/aggregated/instances?maxResults=1&returnPartialSuccess=true
Observed: HTTP 401, Content-Type: application/json

    {
      "error": {
        "code": 401,
        "message": "Request is missing required authentication credential. Expected OAuth 2 access token, login cookie or other valid authentication credential. See https://developers.google.com/identity/sign-in/web/devconsole-project.",
        "status": "UNAUTHENTICATED",
        "details": [
          {
            "@type": "type.googleapis.com/google.rpc.ErrorInfo",
            "reason": "CREDENTIALS_MISSING",
            "domain": "googleapis.com",
            "metadata": {
              "method": "google.compute.v1.InstancesAggregatedList",
              "service": "compute.googleapis.com"
            }
          }
        ]
      }
    }

## 4. Fixture boundary

The captures above are the only response bodies this environment could obtain from the real provider URLs. AWS reported an expired local session, Azure CLI was not installed, and GCP had no active account or ADC file. The report therefore intentionally does not include invented successful 200 JSON/XML bodies; provider documentation was used only to verify field names, media types, pagination, and status placement.

Before a mapper fixture is created, record an authenticated successful body for each operation, redact request IDs/resource identifiers as needed, and copy that exact body into the local fixture. In particular, retain an EC2 XML body and an Azure VM list body obtained with statusOnly=true (or document the second-call response if the coordinator rejects statusOnly).

## 5. Dependency/build verification

Added to src-tauri/Cargo.toml:

    aws-config = { version = "=1.11.0" }
    aws-sigv4 = { version = "=1.5.1" }
    azure_identity = { version = "=1.0.0" }
    gcp_auth = { version = "=0.12.7" }

Command required by Task 1: cargo build --workspace from src-tauri/. Result: PASS, exit code 0, finished in 53.39s on 2026-08-27; a clean incremental rerun finished in 1.33s with exit code 0. The build compiled all four pinned crates and completed the thalassaops workspace build.
