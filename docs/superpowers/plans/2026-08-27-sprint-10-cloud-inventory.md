# Sprint 10 Cloud Inventory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show Kubernetes clusters and compute instances from AWS, Azure and GCP together in one Environment view, with the provider boundary visible and each environment's read access confirmed before its resources are listed.

**Architecture:** Credential resolution is delegated to each provider's own auth crate behind a single `CloudCredentialProvider` trait; every request after the credential runs on a ThalassaOps-owned adapter that is GET-only, redirect-free, timeout-bounded and error-sanitized. Three thin mappers translate provider responses into one `CloudResource` model, so React never learns a provider exists. `app.rs` is split into per-domain command modules first, so the new commands land in a structure that can hold them.

**Tech Stack:** Rust 2021, Tauri 2, Tokio, `reqwest` with Rustls TLS, `httpmock`, SQLite/keyring, React 18, TypeScript, Vitest, Testing Library.

**Spec:** `docs/superpowers/specs/2026-08-26-sprint-10-cloud-inventory-design.md`. Read it before Task 1; this plan argues from it and does not restate its reasoning.

**ADR:** `docs/adr/0006-integration-transport-policy.md` is binding on every task.

## Global Constraints

- Every command added this sprint requires `Capability::EnvironmentRead`, matching `kubernetes_inventory`. Connector configuration continues to require `ConnectorAct`.
- Every request is a fixed, internally selected HTTP GET. Redirects stay disabled, the timeout stays bounded, and failures return a sanitized service or status message with no response body, authorization header or credential reference.
- ThalassaOps stores **no** cloud credential. No task may add a keychain entry, and `credential_configured` is always `false` for the three cloud kinds. A cloud connector carrying a `credential_value` is a configuration error.
- Connector configuration holds only non-secret selectors. One connector is one environment: one AWS profile, one Azure subscription, or one GCP project.
- Resolved credentials and signed authorization headers are used transiently and never persisted, logged or serialized.
- Do not enumerate the machine's available profiles, subscriptions or projects. The operator types the selector. Discovery plus inference has been refused since Sprint 5.
- Every enum crossing the IPC boundary — `CloudProvider`, `CloudResourceType`, `CloudAccessState`, `CloudHealthState` — declares explicit `#[serde(rename = ...)]` values, has a Rust test asserting its exact serialized JSON, and its React fixture is copied from that asserted shape, never from what the UI reads.
- ThalassaOps does not execute provider CLIs. `cli_command` is a generated string for the operator to copy. The Tauri capability set stays at `core:default` and `shell:allow-open`.
- Keep English and Thai locale objects structurally identical, preserve keyboard access and focus styles, and add no live-infrastructure dependency. All tests use local mock endpoints and fixtures.
- Do not add metrics, logs, audit trails, serverless, networking, storage, databases, IAM principals, cost data, provisioning, static credential mode, or any resource type beyond the managed Kubernetes cluster and the compute instance.
- Run `npm ci` before any frontend gate. A gate that cannot be run is a **blocked task, not a passing one** — say so and escalate. Sprint 9 lost two rounds to frontend gates reported as `BLOCKED exit 127`.

---

### Task 1: Verify every provider assumption before writing a mapper

This task writes no product code. Its output is knowledge and pinned dependencies. The spec deliberately records two unverified assumptions; this task resolves them while a wrong answer is still cheap.

**Files:**
- Create: `docs/superpowers/reports/2026-08-27-sprint-10-provider-verification.md`
- Modify: `src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: nothing.
- Produces: the verification report, and compiling dependency entries every later task builds on.

- [ ] **Step 1: Resolve the auth crates**

For each of `aws-config`, `aws-sigv4`, `azure_identity` and `gcp_auth`, record in the report: the current version, the last release date, whether it is maintained, and the exact API call that yields a credential or a signed request. If a crate does not exist under that name, find the actual official equivalent and record what you used instead.

- [ ] **Step 2: Record the six calls**

For each call below, record the full URL template, the HTTP method, the required query parameters including the pinned API version, the response content type, the pagination style (cursor, token or page), and whether resource health or status is present in the response.

| Provider | Kubernetes cluster | Compute instance |
| --- | --- | --- |
| AWS | EKS `ListClusters`, then `DescribeCluster` | EC2 `DescribeInstances` |
| Azure | `Microsoft.ContainerService/managedClusters` | `Microsoft.Compute/virtualMachines` |
| GCP | `container.googleapis.com/v1/projects/{project}/locations/-/clusters` | `compute.googleapis.com/compute/v1/projects/{project}/aggregated/instances` |

Two answers are expected to be awkward and must be stated explicitly rather than assumed:

1. `DescribeInstances` is expected to return **XML**, not JSON.
2. An Azure virtual machine's power state is expected to be **absent** from the default list representation, needing an instance-view or status-only request.

- [ ] **Step 3: Capture one real response body per call**

Save a redacted sample of each response into the report. Every fixture in Tasks 7, 8 and 9 is copied from these samples. A fixture invented from documentation is how a serde/UI contract mismatch became a Sprint 8 blocker.

- [ ] **Step 4: Escalate anything that changes the design**

If `DescribeInstances` returns XML, if an auth crate is unusable, or if a health value needs a second call, **stop and report it** with your recommendation. Do not improvise a fix. The spec names the permitted options for the XML case: a scoped XML dependency for that one mapper, `aws-sdk-ec2` for that one call, or a JSON-returning alternative. Each is contained by the mapper and auth boundaries — but the choice is the coordinator's.

- [ ] **Step 5: Add the dependencies and prove they build**

Add the resolved crates to `src-tauri/Cargo.toml` under `[dependencies]`.

Run: `cargo build --workspace`
Expected: PASS. Nothing uses the crates yet; this only proves they resolve and compile together.

- [ ] **Step 6: Commit**

```bash
git add docs/superpowers/reports/2026-08-27-sprint-10-provider-verification.md src-tauri/Cargo.toml Cargo.lock
git commit -m "docs: verify sprint 10 provider APIs and pin auth dependencies"
```

---

### Task 2: Split `app.rs` into per-domain command modules with no behaviour change

`app.rs` is 3,248 lines carrying 21 commands in one `impl AppState` block. The cloud commands need somewhere to live that a reviewer can read.

**Files:**
- Create: `src-tauri/src/app/mod.rs`, `src-tauri/src/app/connectors.rs`, `src-tauri/src/app/observability.rs`, `src-tauri/src/app/kubernetes.rs`
- Delete: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/main.rs`, `src-tauri/src/lib.rs` if either names the module path

**Interfaces:**
- Consumes: nothing.
- Produces: the `app::` module tree. `AppState`, `BootstrapState` and every command keep their exact current names, signatures and visibility.

- [ ] **Step 1: Record the baseline**

Run: `cargo test --workspace 2>&1 | grep "^test result"`
Write down every count. This is the contract for the whole task.

- [ ] **Step 2: Move the types and shared helpers into `app/mod.rs`**

Move `BootstrapState`, `AppState`, `HealthResponse`, `ContextResponse`, `KubernetesConnectorRequest`, `KubernetesPodRequest`, the constructors `open` and `open_with_credential_store`, the `health` and `context` commands, and every shared authorization or policy helper. Declare the submodules:

```rust
mod connectors;
mod kubernetes;
mod observability;
```

Rust permits one `impl AppState` to be spread across modules in the same crate, so each submodule opens its own `impl AppState { ... }` block. Nothing becomes `pub` that was not `pub` before.

- [ ] **Step 3: Move each command group into its module**

- `app/connectors.rs` — `connector_list`, `connector_add`, `connector_enable`, `connector_disable`, `connector_remove`, `connector_test`, `connector_diagnose`
- `app/observability.rs` — `prometheus_query`, `prometheus_query_range`, `loki_query_range`, `tempo_trace`, `tempo_health`, `alertmanager_alerts`, `grafana_health`, `grafana_link`
- `app/kubernetes.rs` — `kubernetes_command`, `kubernetes_inventory`, `kubernetes_pod_logs`, `kubernetes_pod_events`, `kubernetes_resource_manifest`

Move the tests with the code they cover. Change no function body.

- [ ] **Step 4: Prove behaviour is unchanged**

Run: `cargo test --workspace 2>&1 | grep "^test result"`
Expected: **identical counts to Step 1.**

If a test needs editing to pass, the split changed behaviour. Fix the split — do not adjust the test. That rule is the entire safety net for this task.

- [ ] **Step 5: Run the full Rust gates**

Run: `cargo fmt --all -- --check` then `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS both.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/app src-tauri/src/main.rs
git rm src-tauri/src/app.rs
git commit -m "refactor: split app commands into per-domain modules"
```

---

### Task 3: Define the shared cloud model and pin its serialized shape

**Files:**
- Create: `src-tauri/src/cloud/mod.rs`, `src-tauri/src/cloud/model.rs`
- Modify: `src-tauri/src/main.rs` to declare `mod cloud;`

**Interfaces:**
- Consumes: nothing.
- Produces: `CloudProvider`, `CloudResourceType`, `CloudHealthState`, `CloudAccessState`, `CloudEnvironment`, `CloudResource`, and the kind constants `AWS_CONNECTOR_KIND`, `AZURE_CONNECTOR_KIND`, `GCP_CONNECTOR_KIND`.

- [ ] **Step 1: Write the failing serialization test**

```rust
#[test]
fn cloud_enums_serialize_to_their_documented_wire_values() {
    assert_eq!(serde_json::to_value(CloudProvider::Aws).unwrap(), json!("aws"));
    assert_eq!(serde_json::to_value(CloudProvider::Azure).unwrap(), json!("azure"));
    assert_eq!(serde_json::to_value(CloudProvider::Gcp).unwrap(), json!("gcp"));

    assert_eq!(
        serde_json::to_value(CloudResourceType::KubernetesCluster).unwrap(),
        json!("kubernetes_cluster")
    );
    assert_eq!(
        serde_json::to_value(CloudResourceType::ComputeInstance).unwrap(),
        json!("compute_instance")
    );

    assert_eq!(serde_json::to_value(CloudHealthState::Healthy).unwrap(), json!("healthy"));
    assert_eq!(serde_json::to_value(CloudHealthState::Degraded).unwrap(), json!("degraded"));
    assert_eq!(serde_json::to_value(CloudHealthState::Unavailable).unwrap(), json!("unavailable"));
    assert_eq!(serde_json::to_value(CloudHealthState::Unknown).unwrap(), json!("unknown"));

    assert_eq!(serde_json::to_value(CloudAccessState::Confirmed).unwrap(), json!("confirmed"));
    assert_eq!(serde_json::to_value(CloudAccessState::NoCredential).unwrap(), json!("no_credential"));
    assert_eq!(
        serde_json::to_value(CloudAccessState::SessionExpired).unwrap(),
        json!("session_expired")
    );
    assert_eq!(
        serde_json::to_value(CloudAccessState::PermissionDenied).unwrap(),
        json!("permission_denied")
    );
    assert_eq!(serde_json::to_value(CloudAccessState::Unavailable).unwrap(), json!("unavailable"));
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p thalassaops cloud::model::`
Expected: FAIL — module `cloud` does not exist.

- [ ] **Step 3: Define the types**

```rust
pub const AWS_CONNECTOR_KIND: &str = "aws";
pub const AZURE_CONNECTOR_KIND: &str = "azure";
pub const GCP_CONNECTOR_KIND: &str = "gcp";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CloudProvider {
    #[serde(rename = "aws")]
    Aws,
    #[serde(rename = "azure")]
    Azure,
    #[serde(rename = "gcp")]
    Gcp,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CloudResourceType {
    #[serde(rename = "kubernetes_cluster")]
    KubernetesCluster,
    #[serde(rename = "compute_instance")]
    ComputeInstance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CloudHealthState {
    #[serde(rename = "healthy")]
    Healthy,
    #[serde(rename = "degraded")]
    Degraded,
    #[serde(rename = "unavailable")]
    Unavailable,
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CloudAccessState {
    #[serde(rename = "confirmed")]
    Confirmed,
    #[serde(rename = "no_credential")]
    NoCredential,
    #[serde(rename = "session_expired")]
    SessionExpired,
    #[serde(rename = "permission_denied")]
    PermissionDenied,
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudEnvironment {
    pub connector_id: String,
    pub provider: CloudProvider,
    /// The configured selector, shown verbatim: AWS profile, Azure
    /// subscription, or GCP project.
    pub account_label: String,
    pub location: String,
    pub access: CloudAccessState,
    /// Empty when access is Confirmed. Otherwise the operator's remedy: a
    /// copyable login command, or the name of the missing permission.
    pub remedy: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CloudResource {
    pub provider: CloudProvider,
    pub environment_id: String,
    pub resource_type: CloudResourceType,
    pub id: String,
    pub name: String,
    pub location: String,
    pub health: CloudHealthState,
    /// The provider's own status string, unmodified.
    pub status_detail: String,
    pub console_url: String,
    pub cli_command: String,
}
```

- [ ] **Step 4: Run the test and the whole suite**

Run: `cargo test -p thalassaops cloud::model::` then `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/cloud src-tauri/src/main.rs
git commit -m "feat: add the shared cloud resource model"
```

---

### Task 4: Add the credential seam

The trait is both the containment boundary for an unsuitable auth crate and the seam that lets every later test run without a provider login on the machine.

**Files:**
- Create: `src-tauri/src/cloud/auth/mod.rs`, `src-tauri/src/cloud/auth/aws.rs`, `src-tauri/src/cloud/auth/azure.rs`, `src-tauri/src/cloud/auth/gcp.rs`
- Modify: `src-tauri/src/cloud/mod.rs`

**Interfaces:**
- Consumes: the dependencies pinned in Task 1.
- Produces: `CloudAuthError`, `CloudCredentialProvider`, `AwsCredentialProvider::new(profile, region)`, `AzureCredentialProvider::new(tenant_id)`, `GcpCredentialProvider::new()`, and `FakeCredentialProvider` for tests.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn fake_provider_authorizes_and_can_report_a_missing_credential() {
    let client = reqwest::Client::new();

    let ok = FakeCredentialProvider::authorized("Bearer test-token");
    let request = ok
        .authorize(client.get("http://example.test/x"))
        .await
        .expect("authorized");
    let built = request.build().unwrap();
    assert_eq!(built.headers()["authorization"], "Bearer test-token");

    let missing = FakeCredentialProvider::no_credential();
    let error = missing
        .authorize(client.get("http://example.test/x"))
        .await
        .expect_err("must fail");
    assert!(matches!(error, CloudAuthError::NoCredential { .. }));
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p thalassaops cloud::auth::`
Expected: FAIL — module `auth` does not exist.

- [ ] **Step 3: Define the error and the trait**

`NoCredential` and `Rejected` must stay distinct variants. The preflight classifies them into different operator remedies, and collapsing them would erase the difference between "you are not logged in" and "your login was refused".

```rust
#[derive(Debug, thiserror::Error)]
pub enum CloudAuthError {
    /// No credential could be resolved at all: no SSO cache, no az login
    /// session, no application default credentials.
    #[error("no credential available")]
    NoCredential { login_command: String },
    /// A credential was resolved but the provider refused it.
    #[error("credential rejected")]
    Rejected { login_command: String },
    /// Signing or token exchange failed for a reason that is not the
    /// operator's to fix.
    #[error("credential resolution failed")]
    Failed,
}

#[async_trait::async_trait]
pub trait CloudCredentialProvider: Send + Sync {
    async fn authorize(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, CloudAuthError>;
}
```

`login_command` is the copyable remedy, for example `aws sso login --profile prod`, `az login --tenant <tenant>`, or `gcloud auth application-default login`.

- [ ] **Step 4: Implement the fake**

```rust
pub struct FakeCredentialProvider {
    outcome: Result<String, CloudAuthError>,
}

impl FakeCredentialProvider {
    pub fn authorized(header: &str) -> Self {
        Self { outcome: Ok(header.to_string()) }
    }
    pub fn no_credential() -> Self {
        Self {
            outcome: Err(CloudAuthError::NoCredential {
                login_command: "aws sso login --profile test".into(),
            }),
        }
    }
}

#[async_trait::async_trait]
impl CloudCredentialProvider for FakeCredentialProvider {
    async fn authorize(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::RequestBuilder, CloudAuthError> {
        match &self.outcome {
            Ok(header) => Ok(request.header("authorization", header)),
            Err(error) => Err(match error {
                CloudAuthError::NoCredential { login_command } => {
                    CloudAuthError::NoCredential { login_command: login_command.clone() }
                }
                CloudAuthError::Rejected { login_command } => {
                    CloudAuthError::Rejected { login_command: login_command.clone() }
                }
                CloudAuthError::Failed => CloudAuthError::Failed,
            }),
        }
    }
}
```

- [ ] **Step 5: Implement the three real providers**

Use the exact crate APIs recorded in the Task 1 report. Each provider resolves a credential and either signs the request (AWS) or attaches a bearer token (Azure, GCP). A resolution that finds nothing returns `NoCredential` with that provider's login command; a resolution that finds a credential the provider later refuses is classified by the client in Task 5, not here.

Never log, persist or serialize a resolved credential or a signed header.

- [ ] **Step 6: Run and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add src-tauri/src/cloud/auth src-tauri/src/cloud/mod.rs
git commit -m "feat: add the cloud credential provider seam"
```

---

### Task 5: Build the shared cloud HTTP adapter

**Files:**
- Create: `src-tauri/src/cloud/client.rs`
- Modify: `src-tauri/src/cloud/mod.rs`

**Interfaces:**
- Consumes: `CloudCredentialProvider`, `CloudAuthError`.
- Produces: `CloudClientError`, `CloudClient::new(provider: Arc<dyn CloudCredentialProvider>) -> Result<Self, CloudClientError>`, `CloudClient::get_json<T: DeserializeOwned>(&self, url: Url) -> Result<T, CloudClientError>`, and `CloudClient::get_paginated<T, F>(&self, first: Url, next: F) -> Result<Vec<T>, CloudClientError>`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn get_json_sends_the_authorization_from_the_provider() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET").path("/things").header("authorization", "Bearer t");
        then.status(200).json_body(json!({ "value": 1 }));
    });
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let body: serde_json::Value =
        client.get_json(Url::parse(&server.url("/things")).unwrap()).await.unwrap();
    assert_eq!(body["value"], json!(1));
    mock.assert();
}

#[tokio::test]
async fn a_missing_credential_surfaces_before_any_request_is_sent() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET").path("/things");
        then.status(200).json_body(json!({}));
    });
    let client =
        CloudClient::new(Arc::new(FakeCredentialProvider::no_credential())).unwrap();
    let error = client
        .get_json::<serde_json::Value>(Url::parse(&server.url("/things")).unwrap())
        .await
        .expect_err("must fail");
    assert!(matches!(error, CloudClientError::Auth(CloudAuthError::NoCredential { .. })));
    assert_eq!(mock.hits(), 0, "no request may be sent without a credential");
}

#[tokio::test]
async fn provider_errors_carry_only_a_status_code_and_never_the_body() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method("GET").path("/things");
        then.status(403).body("AccessDenied: user arn:aws:iam::123:user/secret is not authorized");
    });
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let error = client
        .get_json::<serde_json::Value>(Url::parse(&server.url("/things")).unwrap())
        .await
        .expect_err("must fail");
    assert!(matches!(error, CloudClientError::ProviderError(403)));
    let rendered = format!("{error}");
    assert!(!rendered.contains("arn:aws:iam"), "response body must not leak: {rendered}");
    assert!(!rendered.contains("secret"), "response body must not leak: {rendered}");
}

#[tokio::test]
async fn redirects_are_not_followed() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method("GET").path("/start");
        then.status(302).header("location", "/elsewhere");
    });
    let followed = server.mock(|when, then| {
        when.method("GET").path("/elsewhere");
        then.status(200).json_body(json!({}));
    });
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let _ = client
        .get_json::<serde_json::Value>(Url::parse(&server.url("/start")).unwrap())
        .await;
    assert_eq!(followed.hits(), 0);
}

#[tokio::test]
async fn get_paginated_follows_pages_until_the_next_link_is_absent() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method("GET").path("/items").query_param("page", "1");
        then.status(200).json_body(json!({ "items": [1, 2], "next": "2" }));
    });
    server.mock(|when, then| {
        when.method("GET").path("/items").query_param("page", "2");
        then.status(200).json_body(json!({ "items": [3], "next": null }));
    });
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let first = Url::parse(&server.url("/items?page=1")).unwrap();
    let base = server.url("/items");
    let all: Vec<i64> = client
        .get_paginated(first, |body: &serde_json::Value| {
            let items = body["items"].as_array()?.iter().filter_map(|v| v.as_i64()).collect();
            let next = body["next"]
                .as_str()
                .and_then(|token| Url::parse(&format!("{base}?page={token}")).ok());
            Some((items, next))
        })
        .await
        .unwrap();
    assert_eq!(all, vec![1, 2, 3]);
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops cloud::client::`
Expected: FAIL — `CloudClient` does not exist.

- [ ] **Step 3: Implement the client**

```rust
#[derive(Debug, thiserror::Error)]
pub enum CloudClientError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error(transparent)]
    Auth(#[from] CloudAuthError),
    #[error("request failed")]
    RequestFailed,
    #[error("provider error: {0}")]
    ProviderError(u16),
    #[error("response format error")]
    MalformedResponse,
}
```

Build the inner `reqwest::Client` exactly as `observability/client.rs` does — `.timeout(Duration::from_secs(10))` and `.redirect(reqwest::redirect::Policy::none())`. `get_json` builds a GET, hands it to the credential provider, executes it, maps a non-success status to `ProviderError(status)` **without reading the body**, and deserializes on success. `get_paginated` calls the supplied closure to extract this page's items and the next URL, and stops when the closure returns no next URL. Cap the page follow at 50 pages and return what has been collected, so a provider that loops cannot hang the app.

- [ ] **Step 4: Run and commit**

Run: `cargo test -p thalassaops cloud::client::` then `cargo test --workspace`
Expected: PASS.

```bash
git add src-tauri/src/cloud/client.rs src-tauri/src/cloud/mod.rs
git commit -m "feat: add the shared cloud http adapter with pagination"
```

---

### Task 6: Register the three connector kinds and route the connection test to the preflight

**Files:**
- Modify: `src-tauri/src/connectors.rs` — manifests near `:204-238`, `manifest_for` at `:605`, `validate_add_request` at `:270`, `run_connection_test` at `:444`
- Create: `src-tauri/src/cloud/preflight.rs` — `classify_access` only
- Modify: `src-tauri/src/cloud/model.rs` — the three config structs live here, beside the model they configure
- Modify: `src-tauri/src/cloud/mod.rs`

**Interfaces:**
- Consumes: `CloudClient`, `CloudClientError`, `CloudAuthError`, `CloudAccessState`, the kind constants.
- Produces: `aws_manifest()`, `azure_manifest()`, `gcp_manifest()`, `AwsConnectorConfig { profile, region }`, `AzureConnectorConfig { subscription_id, tenant_id }`, `GcpConnectorConfig { project_id }`, and `classify_access(result: &Result<(), CloudClientError>) -> (CloudAccessState, String)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn cloud_manifests_declare_read_only_capabilities() {
    let aws = aws_manifest();
    assert!(aws.can_read("aws.inventory", "inventory"));
    assert!(aws.can_read("aws.access_check", "access_check"));
    assert!(azure_manifest().can_read("azure.inventory", "inventory"));
    assert!(gcp_manifest().can_read("gcp.inventory", "inventory"));
}

#[test]
fn a_cloud_connector_may_not_carry_a_credential() {
    let request = AddConnectorRequest {
        kind: "aws".into(),
        display_name: "Prod".into(),
        config_metadata: json!({ "profile": "prod", "region": "ap-southeast-1" }),
        credential_value: Some("AKIA-should-not-be-here".into()),
    };
    assert!(matches!(
        validate_add_request(&request),
        Err(ConnectorError::InvalidConfiguration(_))
    ));
}

#[test]
fn cloud_selectors_are_required() {
    for (kind, config) in [
        ("aws", json!({ "profile": "", "region": "ap-southeast-1" })),
        ("azure", json!({ "subscription_id": "s", "tenant_id": "" })),
        ("gcp", json!({ "project_id": "  " })),
    ] {
        let request = AddConnectorRequest {
            kind: kind.into(),
            display_name: "X".into(),
            config_metadata: config,
            credential_value: None,
        };
        assert!(
            matches!(validate_add_request(&request), Err(ConnectorError::InvalidConfiguration(_))),
            "{kind} must reject a blank selector"
        );
    }
}

#[test]
fn access_classification_maps_each_failure_to_its_own_remedy() {
    let (state, remedy) = classify_access(&Err(CloudClientError::Auth(
        CloudAuthError::NoCredential { login_command: "aws sso login --profile prod".into() },
    )));
    assert_eq!(state, CloudAccessState::NoCredential);
    assert_eq!(remedy, "aws sso login --profile prod");

    let (state, _) = classify_access(&Err(CloudClientError::ProviderError(401)));
    assert_eq!(state, CloudAccessState::SessionExpired);

    let (state, _) = classify_access(&Err(CloudClientError::ProviderError(403)));
    assert_eq!(state, CloudAccessState::PermissionDenied);

    let (state, _) = classify_access(&Err(CloudClientError::ProviderError(500)));
    assert_eq!(state, CloudAccessState::Unavailable);

    let (state, remedy) = classify_access(&Ok(()));
    assert_eq!(state, CloudAccessState::Confirmed);
    assert!(remedy.is_empty());
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops connectors::` then `cargo test -p thalassaops cloud::preflight::`
Expected: FAIL — the manifests and `classify_access` do not exist.

- [ ] **Step 3: Add the manifests**

```rust
pub fn aws_manifest() -> ConnectorManifest {
    ConnectorManifest::new(AWS_CONNECTOR_KIND, "AWS", "0.1.0")
        .with_capability(ConnectorCapability::read("aws.inventory", ["inventory"]))
        .with_capability(ConnectorCapability::read("aws.access_check", ["access_check"]))
}

pub fn azure_manifest() -> ConnectorManifest {
    ConnectorManifest::new(AZURE_CONNECTOR_KIND, "Azure", "0.1.0")
        .with_capability(ConnectorCapability::read("azure.inventory", ["inventory"]))
        .with_capability(ConnectorCapability::read("azure.access_check", ["access_check"]))
}

pub fn gcp_manifest() -> ConnectorManifest {
    ConnectorManifest::new(GCP_CONNECTOR_KIND, "GCP", "0.1.0")
        .with_capability(ConnectorCapability::read("gcp.inventory", ["inventory"]))
        .with_capability(ConnectorCapability::read("gcp.access_check", ["access_check"]))
}
```

Add all three to `manifest_for`.

- [ ] **Step 4: Add the `validate_add_request` arms**

Three separate arms, following the `KUBERNETES_CONNECTOR_KIND` arm rather than the shared observability arm, because the three config shapes differ from each other. Each deserializes its own struct, rejects a selector that is blank after trimming, rejects a present `credential_value`, and returns the normalized config.

- [ ] **Step 5: Implement `classify_access` and route the connection test**

```rust
pub fn classify_access(result: &Result<(), CloudClientError>) -> (CloudAccessState, String) {
    match result {
        Ok(()) => (CloudAccessState::Confirmed, String::new()),
        Err(CloudClientError::Auth(CloudAuthError::NoCredential { login_command })) => {
            (CloudAccessState::NoCredential, login_command.clone())
        }
        Err(CloudClientError::Auth(CloudAuthError::Rejected { login_command })) => {
            (CloudAccessState::SessionExpired, login_command.clone())
        }
        Err(CloudClientError::ProviderError(401)) => (CloudAccessState::SessionExpired, String::new()),
        Err(CloudClientError::ProviderError(403)) => (CloudAccessState::PermissionDenied, String::new()),
        _ => (CloudAccessState::Unavailable, String::new()),
    }
}
```

The `401` and `403` arms are filled in with a provider-specific remedy by the mappers in Tasks 7–9, which know the login command and the permission name. `run_connection_test` gains a cloud branch that runs the same preflight and maps `Confirmed` to outcome `"healthy"` and everything else to `"unavailable"` with the remedy as the message, so "test connection" and "check access" can never disagree.

- [ ] **Step 6: Run and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add src-tauri/src/connectors.rs src-tauri/src/cloud
git commit -m "feat: register the aws, azure and gcp connector kinds"
```

---

### Task 7: Implement the AWS mapper

**Files:**
- Create: `src-tauri/src/cloud/aws.rs`
- Modify: `src-tauri/src/cloud/mod.rs`

**Interfaces:**
- Consumes: `CloudClient`, `AwsConnectorConfig`, `CloudResource`, `classify_access`.
- Produces: `pub async fn inventory(client: &CloudClient, config: &AwsConnectorConfig, connector_id: &str) -> Result<Vec<CloudResource>, CloudClientError>` and `pub async fn access_check(client: &CloudClient, config: &AwsConnectorConfig, connector_id: &str) -> CloudEnvironment`.

- [ ] **Step 1: Write the failing tests**

Use the response samples captured in the Task 1 report as the mock bodies. The assertions below are the contract regardless of the exact body shape.

```rust
#[tokio::test]
async fn inventory_maps_eks_clusters_and_ec2_instances_into_the_shared_model() {
    let server = MockServer::start();
    // Mock the EKS and EC2 endpoints using the Task 1 response samples.
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let config = AwsConnectorConfig { profile: "prod".into(), region: "ap-southeast-1".into() };

    let resources = inventory(&client, &config, "aws-1").await.unwrap();

    let cluster = resources
        .iter()
        .find(|r| r.resource_type == CloudResourceType::KubernetesCluster)
        .expect("a cluster");
    assert_eq!(cluster.provider, CloudProvider::Aws);
    assert_eq!(cluster.environment_id, "aws-1");
    assert_eq!(cluster.location, "ap-southeast-1");
    assert!(cluster.console_url.starts_with("https://"));
    assert!(cluster.cli_command.starts_with("aws eks"));
    assert!(cluster.cli_command.contains("--profile prod"));

    let instance = resources
        .iter()
        .find(|r| r.resource_type == CloudResourceType::ComputeInstance)
        .expect("an instance");
    assert_eq!(instance.provider, CloudProvider::Aws);
    assert!(!instance.status_detail.is_empty(), "the provider's own status is preserved");
}

#[tokio::test]
async fn access_check_names_the_missing_permission_on_403() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method("GET");
        then.status(403).body("AccessDenied");
    });
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let config = AwsConnectorConfig { profile: "prod".into(), region: "ap-southeast-1".into() };

    let environment = access_check(&client, &config, "aws-1").await;

    assert_eq!(environment.access, CloudAccessState::PermissionDenied);
    assert!(environment.remedy.contains("eks:ListClusters"), "remedy: {}", environment.remedy);
    assert_eq!(environment.account_label, "prod");
}

#[tokio::test]
async fn access_check_offers_the_login_command_when_no_credential_resolves() {
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::no_credential())).unwrap();
    let config = AwsConnectorConfig { profile: "prod".into(), region: "ap-southeast-1".into() };

    let environment = access_check(&client, &config, "aws-1").await;

    assert_eq!(environment.access, CloudAccessState::NoCredential);
    assert!(environment.remedy.contains("aws sso login"), "remedy: {}", environment.remedy);
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops cloud::aws::`
Expected: FAIL — module `aws` does not exist.

- [ ] **Step 3: Implement `inventory`**

Call the endpoints exactly as recorded in the Task 1 report. Map each cluster and instance into `CloudResource`, translating the provider's status into `CloudHealthState` while keeping the raw string in `status_detail`. Build `console_url` from the documented console URL shape and `cli_command` as a read-only command carrying `--profile` and `--region`, following `kubernetes.rs::kubectl_command`. If the Task 1 report found `DescribeInstances` returns XML, use the approach the coordinator approved in Task 1 Step 4 — do not choose one now.

- [ ] **Step 4: Implement `access_check`**

Attempt `ListClusters` with the smallest page the API allows, pass the result through `classify_access`, and fill the empty remedy for the `401` and `403` arms with `aws sso login --profile <profile>` and `eks:ListClusters` respectively.

- [ ] **Step 5: Run and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add src-tauri/src/cloud/aws.rs src-tauri/src/cloud/mod.rs
git commit -m "feat: add the aws inventory and access check mapper"
```

---

### Task 8: Implement the Azure mapper

**Files:**
- Create: `src-tauri/src/cloud/azure.rs`
- Modify: `src-tauri/src/cloud/mod.rs`

**Interfaces:**
- Consumes: `CloudClient`, `AzureConnectorConfig`, `CloudResource`, `classify_access`.
- Produces: `pub async fn inventory(...)` and `pub async fn access_check(...)`, same signatures as Task 7 with `AzureConnectorConfig`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn inventory_maps_aks_clusters_and_virtual_machines_into_the_shared_model() {
    let server = MockServer::start();
    // Mock the ARM endpoints using the Task 1 response samples.
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let config = AzureConnectorConfig {
        subscription_id: "sub-1".into(),
        tenant_id: "tenant-1".into(),
    };

    let resources = inventory(&client, &config, "azure-1").await.unwrap();

    let cluster = resources
        .iter()
        .find(|r| r.resource_type == CloudResourceType::KubernetesCluster)
        .expect("a cluster");
    assert_eq!(cluster.provider, CloudProvider::Azure);
    assert_eq!(cluster.environment_id, "azure-1");
    assert!(!cluster.location.is_empty());
    assert!(cluster.console_url.starts_with("https://"));
    assert!(cluster.cli_command.starts_with("az aks"));

    let vm = resources
        .iter()
        .find(|r| r.resource_type == CloudResourceType::ComputeInstance)
        .expect("a vm");
    assert_eq!(vm.provider, CloudProvider::Azure);
    assert!(vm.cli_command.starts_with("az vm"));
    assert!(!vm.status_detail.is_empty(), "the provider's own status is preserved");
}

#[tokio::test]
async fn access_check_names_the_missing_permission_on_403() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method("GET");
        then.status(403).body("AuthorizationFailed");
    });
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let config = AzureConnectorConfig {
        subscription_id: "sub-1".into(),
        tenant_id: "tenant-1".into(),
    };

    let environment = access_check(&client, &config, "azure-1").await;

    assert_eq!(environment.access, CloudAccessState::PermissionDenied);
    assert!(
        environment.remedy.contains("Microsoft.ContainerService"),
        "remedy: {}",
        environment.remedy
    );
    assert_eq!(environment.account_label, "sub-1");
}

#[tokio::test]
async fn access_check_offers_the_login_command_when_no_credential_resolves() {
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::no_credential())).unwrap();
    let config = AzureConnectorConfig {
        subscription_id: "sub-1".into(),
        tenant_id: "tenant-1".into(),
    };

    let environment = access_check(&client, &config, "azure-1").await;

    assert_eq!(environment.access, CloudAccessState::NoCredential);
    assert!(environment.remedy.contains("az login"), "remedy: {}", environment.remedy);
}
```

The permission asserted in the 403 test is the Azure action recorded in the Task 1 report, for example `Microsoft.ContainerService/managedClusters/read`.

Add one Azure-specific test:

```rust
#[tokio::test]
async fn virtual_machine_health_comes_from_the_status_carrying_call() {
    // The default VM list may not carry power state. This test asserts the
    // mapper reaches whichever call the Task 1 report identified as carrying
    // it, and that a VM whose power state is unknown maps to
    // CloudHealthState::Unknown rather than being silently reported healthy.
    let server = MockServer::start();
    // Mock the list call and, if Task 1 found one is needed, the
    // instance-view or status-only call, using the captured samples.
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let config = AzureConnectorConfig {
        subscription_id: "sub-1".into(),
        tenant_id: "tenant-1".into(),
    };

    let resources = inventory(&client, &config, "azure-1").await.unwrap();
    let vm = resources
        .iter()
        .find(|r| r.resource_type == CloudResourceType::ComputeInstance)
        .expect("a vm");
    assert_ne!(
        vm.health,
        CloudHealthState::Healthy,
        "a vm with no reported power state must not be assumed healthy"
    );
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops cloud::azure::`
Expected: FAIL — module `azure` does not exist.

- [ ] **Step 3: Implement `inventory`**

Call the ARM endpoints with the `api-version` pinned in the Task 1 report. If that report found power state absent from the list representation, make the additional call it identified. An absent or unrecognized power state maps to `CloudHealthState::Unknown` — never to `Healthy`.

- [ ] **Step 4: Implement `access_check`**

Same shape as Task 7, with `az login --tenant <tenant_id>` as the login remedy.

- [ ] **Step 5: Run and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add src-tauri/src/cloud/azure.rs src-tauri/src/cloud/mod.rs
git commit -m "feat: add the azure inventory and access check mapper"
```

---

### Task 9: Implement the GCP mapper

**Files:**
- Create: `src-tauri/src/cloud/gcp.rs`
- Modify: `src-tauri/src/cloud/mod.rs`

**Interfaces:**
- Consumes: `CloudClient`, `GcpConnectorConfig`, `CloudResource`, `classify_access`.
- Produces: `pub async fn inventory(...)` and `pub async fn access_check(...)`, same signatures as Task 7 with `GcpConnectorConfig`.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn inventory_maps_gke_clusters_and_compute_instances_into_the_shared_model() {
    let server = MockServer::start();
    // Mock the container and compute endpoints using the Task 1 response samples.
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let config = GcpConnectorConfig { project_id: "my-project".into() };

    let resources = inventory(&client, &config, "gcp-1").await.unwrap();

    let cluster = resources
        .iter()
        .find(|r| r.resource_type == CloudResourceType::KubernetesCluster)
        .expect("a cluster");
    assert_eq!(cluster.provider, CloudProvider::Gcp);
    assert_eq!(cluster.environment_id, "gcp-1");
    assert!(!cluster.location.is_empty());
    assert!(cluster.console_url.starts_with("https://"));
    assert!(cluster.cli_command.starts_with("gcloud container clusters"));

    let instance = resources
        .iter()
        .find(|r| r.resource_type == CloudResourceType::ComputeInstance)
        .expect("an instance");
    assert_eq!(instance.provider, CloudProvider::Gcp);
    assert!(instance.cli_command.starts_with("gcloud compute instances"));
    assert!(!instance.status_detail.is_empty(), "the provider's own status is preserved");
}

#[tokio::test]
async fn access_check_names_the_missing_permission_on_403() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method("GET");
        then.status(403).body("PERMISSION_DENIED");
    });
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let config = GcpConnectorConfig { project_id: "my-project".into() };

    let environment = access_check(&client, &config, "gcp-1").await;

    assert_eq!(environment.access, CloudAccessState::PermissionDenied);
    assert!(
        environment.remedy.contains("container.clusters.list"),
        "remedy: {}",
        environment.remedy
    );
    assert_eq!(environment.account_label, "my-project");
}

#[tokio::test]
async fn access_check_offers_the_login_command_when_no_credential_resolves() {
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::no_credential())).unwrap();
    let config = GcpConnectorConfig { project_id: "my-project".into() };

    let environment = access_check(&client, &config, "gcp-1").await;

    assert_eq!(environment.access, CloudAccessState::NoCredential);
    assert!(
        environment.remedy.contains("gcloud auth application-default login"),
        "remedy: {}",
        environment.remedy
    );
}
```

The permission asserted in the 403 test is the GCP permission recorded in the Task 1 report.

Add one GCP-specific test, because the aggregated instance list is shaped differently from the other two providers:

```rust
#[tokio::test]
async fn aggregated_instance_list_flattens_zones_and_skips_empty_scopes() {
    let server = MockServer::start();
    // The aggregated list returns a map of scope -> { instances } where many
    // scopes carry no instances at all. Mock at least one populated zone and
    // one empty scope, using the Task 1 sample.
    let client = CloudClient::new(Arc::new(FakeCredentialProvider::authorized("Bearer t"))).unwrap();
    let config = GcpConnectorConfig { project_id: "my-project".into() };

    let resources = inventory(&client, &config, "gcp-1").await.unwrap();
    let instances: Vec<_> = resources
        .iter()
        .filter(|r| r.resource_type == CloudResourceType::ComputeInstance)
        .collect();
    assert!(!instances.is_empty(), "the populated zone must be flattened into the list");
    assert!(
        instances.iter().all(|i| !i.location.is_empty()),
        "each instance keeps the zone it came from"
    );
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops cloud::gcp::`
Expected: FAIL — module `gcp` does not exist.

- [ ] **Step 3: Implement `inventory`**

Call the two endpoints as recorded in Task 1. Flatten the aggregated instance response into a flat list, carrying each instance's zone into `location` and skipping scopes with no instances.

- [ ] **Step 4: Implement `access_check`**

Same shape as Task 7, with `gcloud auth application-default login` as the login remedy.

- [ ] **Step 5: Run and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add src-tauri/src/cloud/gcp.rs src-tauri/src/cloud/mod.rs
git commit -m "feat: add the gcp inventory and access check mapper"
```

---

### Task 10: Expose the two IPC commands

**Files:**
- Create: `src-tauri/src/app/cloud.rs`
- Modify: `src-tauri/src/app/mod.rs`, `src-tauri/src/main.rs`, `ui/contracts/ipc.ts`

**Interfaces:**
- Consumes: the three mappers, `CloudClient`, the three config structs.
- Produces: the `cloud_access_check` and `cloud_inventory` Tauri commands, and their TypeScript contract types.

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn cloud_commands_require_environment_read() {
    let state = test_state();
    let envelope = CommandEnvelope {
        request_id: uuid::Uuid::new_v4().to_string(),
        command: "cloud_inventory".into(),
        capability: Capability::ResourceRead, // wrong capability on purpose
        scope: unbounded_scope(),
        payload: json!({ "connector_id": "aws-1" }),
    };
    assert!(matches!(state.cloud_inventory(envelope).await, IpcResult::Err { .. }));
}

#[tokio::test]
async fn cloud_inventory_rejects_an_inactive_membership() {
    let state = test_state_with_inactive_membership();
    let envelope = valid_cloud_envelope("cloud_inventory", "aws-1");
    assert!(matches!(state.cloud_inventory(envelope).await, IpcResult::Err { .. }));
}

#[tokio::test]
async fn cloud_inventory_rejects_an_unknown_connector() {
    let state = test_state();
    let envelope = valid_cloud_envelope("cloud_inventory", "does-not-exist");
    assert!(matches!(state.cloud_inventory(envelope).await, IpcResult::Err { .. }));
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops app::cloud::`
Expected: FAIL — `cloud_inventory` does not exist.

- [ ] **Step 3: Add the `cloud_command` helper**

Mirror `kubernetes_command` in `app/kubernetes.rs`: build the descriptor, then reject the call unless the command name, capability, scope, membership status and policy all pass — before any provider is contacted.

```rust
let descriptor = CommandDescriptor::new(
    "cloud",
    verb,
    Capability::EnvironmentRead,
    thalassa_domain::Permission::Read,
);
```

The helper resolves the connector by id, reads its kind, constructs the matching credential provider and `CloudClient`, and dispatches to the matching mapper. This is the only place that maps a kind to a provider.

- [ ] **Step 4: Add the two commands and register them**

`cloud_access_check` returns `CloudEnvironment`; `cloud_inventory` returns `Vec<CloudResource>`. Register both in `main.rs` alongside the existing commands.

- [ ] **Step 5: Mirror the contract types in TypeScript**

Add `CloudProvider`, `CloudResourceType`, `CloudHealthState`, `CloudAccessState`, `CloudEnvironment` and `CloudResource` to `ui/contracts/ipc.ts`, copying the string unions from the values asserted in Task 3 Step 1 — not from what the UI reads.

- [ ] **Step 6: Run and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add src-tauri/src/app/cloud.rs src-tauri/src/app/mod.rs src-tauri/src/main.rs ui/contracts/ipc.ts
git commit -m "feat: expose the cloud access check and inventory commands"
```

---

### Task 11: Build the Environment workspace

**Files:**
- Create: `ui/src/EnvironmentWorkspace.tsx`, `ui/src/environment/EnvironmentPanel.tsx`, `ui/src/environment/ResourceTable.tsx`, `ui/src/environment/AccessBanner.tsx`
- Modify: `ui/src/shell.tsx`, `ui/src/locales/en.ts`, `ui/src/locales/th.ts`, `ui/src/styles.css`, `ui/src/shell.test.tsx`

**Interfaces:**
- Consumes: `cloud_access_check`, `cloud_inventory`, and the contract types from Task 10.
- Produces: the Environment route.

Build this as separate panels from the first commit. Sprint 9 had to split `ObservabilityWorkspace.tsx` after it grew too large; do not repeat that.

- [ ] **Step 1: Write the failing acceptance test**

```tsx
test("shows three cloud environments with provider boundaries and keeps healthy ones visible when one session expires", async () => {
  const user = userEvent.setup();
  // aws-1 and gcp-1 are confirmed; azure-1 has no credential.
  const invoke = vi.fn().mockImplementation((name: string, args?: {
    envelope?: { payload?: { connector_id?: string } };
  }) => {
    if (name === "system_context") return Promise.resolve({ ok: true, value: context });
    if (name === "connector_list")
      return Promise.resolve({ ok: true, value: [awsConnector, azureConnector, gcpConnector] });
    if (name === "cloud_access_check") {
      const id = args?.envelope?.payload?.connector_id;
      return Promise.resolve({ ok: true, value: accessFixtures[id!] });
    }
    if (name === "cloud_inventory") {
      const id = args?.envelope?.payload?.connector_id;
      return Promise.resolve({ ok: true, value: inventoryFixtures[id!] ?? [] });
    }
    return Promise.resolve({ ok: true, value: {} });
  });

  render(
    <I18nProvider>
      <Shell invoke={invoke} />
    </I18nProvider>
  );

  await user.click(screen.getByRole("button", { name: "Environments" }));

  // All three environments are present, each labelled with its provider.
  expect(await screen.findByText("AWS")).toBeInTheDocument();
  expect(screen.getByText("Azure")).toBeInTheDocument();
  expect(screen.getByText("GCP")).toBeInTheDocument();

  // The healthy environments still render their resources.
  expect(await screen.findByText("prod-eks")).toBeInTheDocument();
  expect(await screen.findByText("prod-gke")).toBeInTheDocument();

  // The failed environment shows the copyable remedy and hides only its own
  // resources.
  expect(screen.getByText(/az login/)).toBeInTheDocument();
  expect(screen.queryByText("prod-aks")).not.toBeInTheDocument();
});
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `npm ci` then `npm test -- shell.test.tsx`
Expected: FAIL — there is no Environments route.

- [ ] **Step 3: Build `AccessBanner`**

Renders the preflight state. On any state other than `confirmed`, show the remedy with a copy button and suppress that environment's resource lists. Never let one environment's failure unmount another's.

- [ ] **Step 4: Build `ResourceTable`**

One row per `CloudResource`: name, type, location, health, the provider's `status_detail`, a console deep link opened with `open` from `@tauri-apps/plugin-shell`, and a copy action for `cli_command`.

- [ ] **Step 5: Build `EnvironmentPanel` and `EnvironmentWorkspace`**

The panel owns one environment: its provider badge, its `AccessBanner`, and its `ResourceTable`. The workspace lists one panel per cloud connector and fetches each environment independently, so a slow or failing provider never blocks the others.

- [ ] **Step 6: Add locale keys and styles**

Add every new string to both `en.ts` and `th.ts`, keeping the two objects structurally identical. Add the provider badge and banner styles to `styles.css`.

- [ ] **Step 7: Run the frontend gates and commit**

Run: `npm test`, `npm run typecheck`, `npm run lint`, `npm run build`, `npm run format:check`
Expected: PASS all five.

```bash
git add ui/src ui/contracts
git commit -m "feat: add the cross-cloud environment workspace"
```

---

### Task 12: Complete regression, security and acceptance verification

**Files:**
- Create: `docs/superpowers/reports/2026-08-27-sprint-10-verification.md`

- [ ] **Step 1: Run every Rust gate from the repository root**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: PASS all three, with no test count lower than the Task 2 baseline.

- [ ] **Step 2: Run every frontend gate under Node 24**

```bash
npm ci
npm run format:check
npm run lint
npm run typecheck
npm run build
npm test
```
Expected: PASS all six.

`npm ci` is the first step for a reason. In Sprint 9 these gates were reported as `BLOCKED exit 127` across two rounds because `node_modules` was absent, and two acceptance tests plus a real type error shipped uncaught. **A gate you did not run is not a gate that passed.** If one is genuinely blocked, escalate — do not mark this task done.

- [ ] **Step 3: Audit the diff for leaks and scope**

```bash
git diff --check
git diff main...HEAD
```

Confirm by reading: no credential, token, signed authorization header or provider response body reaches a log, a diagnostic, a React fixture or a serialized `IpcResult`; no keychain entry was added; `src-tauri/capabilities/default.json` still lists only `core:default` and `shell:allow-open`; and no resource type outside the two in scope was added.

- [ ] **Step 4: Execute the fixture acceptance journey**

Three environments, one per provider, one in a failed-access state. Confirm the Environment view shows all three with correct provider badges, renders clusters and instances for the healthy two, and shows the copyable remedy for the third without hiding the others.

- [ ] **Step 5: Record results and hand off**

Write the report with the real output summary of every command in Steps 1 and 2 — actual counts and outcomes, not a claim that they passed. Do not merge to main and do not push; the coordinator is the final approver.

```bash
git add docs/superpowers/reports/2026-08-27-sprint-10-verification.md
git commit -m "docs: record sprint 10 verification results"
```
