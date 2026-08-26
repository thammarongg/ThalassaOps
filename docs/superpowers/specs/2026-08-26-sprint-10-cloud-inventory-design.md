# Sprint 10 Cloud Inventory Design

**Status:** Approved design
**Date:** 2026-08-26
**Sprint:** 10 — AWS, Azure and GCP inventory

## Goal

Let an operator see Kubernetes clusters and compute instances from AWS, Azure
and GCP together in one Environment view, with the provider boundary visible
and each environment's read access confirmed before any resource list is shown.

The sprint proves the cross-cloud abstraction against three genuinely different
authentication stacks. Resource breadth is deliberately thin; the abstraction is
the deliverable.

## Scope

- Three connector kinds — `aws`, `azure`, `gcp` — configured with non-secret
  selectors only.
- Two resource types per provider: the managed Kubernetes cluster and the
  compute instance.
- A read-access preflight per environment that names the missing permission or
  the expired session instead of failing opaquely.
- Provider console deep links and copyable provider CLI commands.
- An Environment workspace that groups resources by environment with an explicit
  provider badge.
- A shared cloud HTTP adapter with pagination, reused by all three providers.
- Splitting `app.rs` into per-domain command modules, with no behaviour change.

## Non-goals

- Provisioning, mutation or any write to a cloud provider. Every command this
  sprint adds is `ResourceRead`.
- Metrics and logs: CloudWatch, Azure Monitor, Cloud Monitoring, CloudTrail,
  Activity Logs and Cloud Audit Logs. Sprint 8 and 9 own the observability path;
  cloud-native equivalents are backlog.
- Serverless (Lambda, Azure Functions, Cloud Functions/Run), networking, load
  balancers, DNS and firewall/security-group context.
- Any resource type beyond the two named above, including storage, databases and
  IAM principals.
- Storing cloud credentials. See "Credentials" below — this sprint adds no new
  secret to the OS keychain.
- Enumerating the machine's available profiles, subscriptions or projects.
  Discovery plus inference has been refused consistently since Sprint 5; the
  operator types the selector.
- Cost data. FinOps is a later phase.

## Architecture

### Transport policy

This sprint establishes a rule the remaining sprints inherit, recorded as
ADR 0006:

> Delegate credentials. Own the wire. Reserve full SDKs for protocols whose
> types are the domain model.

Cloud credential resolution is delegated to the providers' own auth crates,
because SSO session handling, signature construction and token refresh are the
one part of this integration where a hand-rolled implementation is both
dangerous and permanently in maintenance. Everything after the credential —
URL construction, the request itself, pagination, error handling — stays on a
ThalassaOps-owned adapter, because that is where the project's auditable
guarantees live: GET only, redirects disabled, bounded timeout, failures
sanitized to a status code with no response body.

`kube` remains the right choice for Kubernetes under the same rule rather than
as an exception to it: the Kubernetes API is genuinely complex, and
`k8s-openapi` types *are* the domain model for Sprint 6 and 7. Cloud inventory
reads are paginated JSON list calls and do not meet that bar.

### Module layout

```text
src-tauri/src/
  app/
    mod.rs            AppState, BootstrapState, shared authorization helper
    connectors.rs     connector_* commands            (moved, unchanged)
    observability.rs  prometheus/loki/tempo/alertmanager/grafana (moved)
    kubernetes.rs     kubernetes_* commands           (moved, unchanged)
    cloud.rs          cloud_* commands                (new)
  cloud/
    client.rs         shared adapter: GET, no redirect, timeout, pagination
    auth/
      mod.rs          trait CloudCredentialProvider
      aws.rs          aws-config + aws-sigv4
      azure.rs        azure_identity
      gcp.rs          gcp_auth
    aws.rs
    azure.rs
    gcp.rs            provider mappers into the shared model
    model.rs          CloudEnvironment, CloudResource
```

`app.rs` is 3,248 lines carrying 21 commands in a single `impl AppState` block.
Adding three providers to it would produce a file no reviewer can hold in
context. Rust allows one `impl` to be spread across modules within a crate, so
the split moves command bodies without changing the type, the command names,
the authorization path or the serialized contracts. It is the same
no-behaviour-change move Sprint 9 applied to `ObservabilityWorkspace.tsx`, and
the existing test suite is the safety net: if a test changes, the split changed
behaviour and the split is wrong.

`observability/` is not touched. `cloud/` borrows its shape, not its code —
the authentication models have nothing in common, and a forced abstraction over
both would obscure each.

### Authentication seam

```rust
#[async_trait]
pub trait CloudCredentialProvider {
    async fn authorize(
        &self,
        request: RequestBuilder,
    ) -> Result<RequestBuilder, CloudAuthError>;
}
```

Every provider's credential handling sits behind this one method. This is a
deliberate containment boundary: the maturity of `azure_identity` and
`gcp_auth` has not yet been verified, and if either proves unsuitable, the
fallback — using that provider's full SDK for that provider only — is a change
to one file behind a stable interface rather than a redesign.

## Credentials

ThalassaOps stores no cloud credential. Connector configuration holds only
non-secret selectors:

```json
{
  "aws":   { "profile": "prod", "region": "ap-southeast-1" },
  "azure": { "subscription_id": "...", "tenant_id": "..." },
  "gcp":   { "project_id": "my-project" }
}
```

All three values are stored in SQLite with the rest of the configuration.
`credential_configured` is always `false` for these connector kinds, and no
keychain entry is created.

The credential itself is resolved at request time from the operator's existing
machine state by the auth crates: an AWS profile or SSO session, an `az login`
session, or Google application default credentials. A cloud engineer's existing
short-lived session is therefore what grants access, which is both the workflow
they already have and a materially better security position than a long-lived
access key pasted into a desktop application. Static credential entry is
explicitly deferred to backlog rather than designed now.

The cost of this choice is that an unauthenticated or expired session is a
normal, expected state rather than an error case. The preflight below exists to
make that state actionable.

## Read-access preflight

Each environment is checked before its resources are listed. The check is the
real list call with the smallest page the provider allows, and the result is
classified:

| Outcome | Reported as |
| --- | --- |
| Success | Access confirmed; resource lists render. |
| Unauthenticated or expired session | The provider's re-login command, verbatim and copyable, for example `aws sso login --profile prod`. |
| Authorization denied | The specific permission the call required, for example `eks:ListClusters`. |
| Anything else | Sanitized service or status message, no response body. |

The preflight deliberately does not call the providers' policy-simulation APIs
(`iam:SimulatePrincipalPolicy` and its equivalents). Those calls require IAM
read permissions that a correctly scoped read-only role generally does not
hold, so a preflight built on them would report failure for exactly the
well-configured principals it is meant to reassure. Attempting the real call is
both cheaper and a more honest answer to "can this environment be read".

## Contracts and data flow

```text
React Environment route
        │
        ▼
AppState authorization and policy checks   (ResourceRead)
        │
        ▼
cloud/client.rs   GET only · no redirects · bounded timeout · pagination
        │  request signed or bearer-authorized by CloudCredentialProvider
        ▼
Provider native endpoint
        │
        ▼
Provider mapper → CloudResource
```

### Shared model

```rust
pub struct CloudEnvironment {
    pub connector_id: String,
    pub provider: CloudProvider,
    pub account_label: String,   // the configured selector, shown verbatim:
                                 // AWS profile, Azure subscription, GCP project
    pub location: String,
    pub access: CloudAccessState,
}

pub struct CloudResource {
    pub provider: CloudProvider,
    pub environment_id: String,
    pub resource_type: CloudResourceType,
    pub id: String,
    pub name: String,
    pub location: String,
    pub health: CloudHealthState,
    pub status_detail: String,
    pub console_url: String,
    pub cli_command: String,
}
```

`CloudHealthState` is a new typed enum, and the repository has no shared health
type to reuse: `ConnectorSummary.health_state` is a bare `String` written
directly by the connection test, and `kubernetes.rs` classifies pod health with
its own logic. This sprint introduces a typed enum for cloud resources and
deliberately does not retrofit the other two — consolidating three health
vocabularies is real work that belongs in its own change, not smuggled into a
cloud sprint. Its variant names reuse the existing wording (`healthy`,
`degraded`, `unavailable`, `unknown`) so the three converge on vocabulary now
and can converge on a type later.

`status_detail` carries the provider's own status string unmodified, for
operators who need the native term rather than the normalized one.

The mappers are the only code that knows a provider exists. React reads
`CloudResource` and renders a provider badge; it contains no provider-specific
logic.

### Calls

| Provider | Kubernetes cluster | Compute instance |
| --- | --- | --- |
| AWS | EKS `ListClusters`, then `DescribeCluster` per cluster | EC2 `DescribeInstances` |
| Azure | `Microsoft.ContainerService/managedClusters` | `Microsoft.Compute/virtualMachines` |
| GCP | `container.googleapis.com/v1/projects/{project}/locations/-/clusters` | `compute.googleapis.com/compute/v1/projects/{project}/aggregated/instances` |

Every provider API is version-pinned — an AWS action version, an Azure
`api-version` query parameter, a GCP `/v1/` path — and each provider guarantees
backward compatibility within that version. New provider features arrive as new
fields or new endpoints; unknown fields are ignored during deserialization, so a
provider adding a feature cannot break an existing call. Surfacing a new feature
is product work regardless of transport, and is not this sprint's concern.

### Known unverified detail

Five of the six calls are expected to return JSON. `DescribeInstances` uses the
EC2 query protocol, which returns XML. This has not been confirmed against the
live API and must not be assumed during implementation.

Task 1 of the implementation plan verifies, before any mapper is written:

1. The exact request shape and response content type of all six calls.
2. That `aws-config`, `aws-sigv4`, `azure_identity` and `gcp_auth` exist at
   usable versions, are maintained, and can produce a credential for the flows
   above.
3. Whether pagination is cursor, token or page based per call.

If `DescribeInstances` does return XML, the options are a scoped XML dependency
for that one mapper, `aws-sdk-ec2` for that one call, or a JSON-returning
alternative API. All three are contained by the mapper and auth boundaries.
Sprint 9 lost a round to a problem found at the end; this task exists so this
one is found at the start.

### Deep links and CLI handoff

`console_url` is a constructed provider console URL, opened through the
existing `shell:allow-open` permission. `cli_command` is a generated command
string presented for the operator to copy and run themselves.

ThalassaOps does not execute provider CLIs. This follows the decision already
made in `kubernetes.rs`, where `kubectl_command` builds a string and the test
`topology_and_kubectl_commands_are_read_only` asserts it stays inert. The
application holds cloud access; granting it subprocess execution would widen
that boundary for no read-path benefit, and the Tauri capability set stays at
`core:default` and `shell:allow-open`.

### IPC contract rule

Every enum crossing the IPC boundary — `CloudProvider`, `CloudResourceType`,
`CloudAccessState`, `CloudHealthState` — declares explicit
`#[serde(rename = ...)]` values, has a
Rust test asserting its exact serialized JSON, and its React fixture is copied
from that asserted shape rather than from what the UI happens to read. This rule
exists because a serde/UI contract mismatch was a Sprint 8 blocker.

## UI behavior

A new Environment workspace, built as separate panels from the first commit
rather than as one file to be split later.

- Resources group by environment; every environment shows a provider badge, so
  the provider boundary is explicit rather than inferred from resource names.
- Each environment renders its preflight state first. A failed preflight shows
  the remedy — the re-login command or the missing permission — and suppresses
  that environment's resource lists without affecting the others.
- Each resource row offers its console deep link and a copy action for its CLI
  command.
- English and Thai locale objects stay structurally identical, and keyboard
  access and focus styles are preserved.

One environment failing must never blank the view. Providers are independent,
and a GCP session expiring is not a reason to hide healthy AWS resources.

## Safety and error handling

- Every command added this sprint requires `ResourceRead`; connector
  configuration continues to require `ConnectorAct`.
- Every request is a fixed, internally selected GET. Redirects stay disabled and
  the timeout stays bounded.
- Failures return a sanitized service or status message. No response body, no
  authorization header and no credential reference reaches a log, a diagnostic,
  a React fixture or a serialized `IpcResult`.
- Resolved credentials and signed authorization headers are used transiently and
  never persisted, logged or serialized.
- Provider selectors — profile, subscription, tenant and project identifiers —
  are non-secret configuration and may appear in the UI.

## Verification and acceptance

- All provider interactions are tested against local `httpmock` endpoints. No
  test requires a live cloud account or a configured CLI.
- Response fixtures are copied from the real shapes captured in Task 1.
- The `app.rs` split is proved by the existing suite passing unchanged. A test
  that needs editing means the split altered behaviour.
- Acceptance journey: three environments configured, one per provider, with one
  in an expired-session state. The Environment view shows all three with correct
  provider badges, renders clusters and instances for the two healthy ones, and
  shows the copyable re-login command for the third without hiding the others.
- Gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  -- -D warnings`, `cargo test --workspace`, `npm ci`, `npm run format:check`,
  `npm run lint`, `npm run typecheck`, `npm run build`, `npm test`,
  `git diff --check`. A gate that cannot be run is a blocked task, not a passing
  one.
