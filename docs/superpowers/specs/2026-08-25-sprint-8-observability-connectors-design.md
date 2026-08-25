# Sprint 8 Observability Connectors Design

**Status:** Approved design  
**Date:** 2026-08-25  
**Sprint:** 8 — Prometheus, Alertmanager and Grafana

## Goal

Connect Prometheus metrics, Alertmanager alerts and Grafana context to the
existing local-first Kubernetes investigation flow. A responder must be able to
open an alert in ThalassaOps, see its resource scope and supporting metric
series, then open the corresponding native Grafana context.

## Scope

- Prometheus connection testing and instant/range query execution.
- Alertmanager read-only alert ingestion.
- Conservative Kubernetes resource references derived from alert labels.
- A normalized metric/alert result contract with timestamps and source
  references.
- Grafana health testing plus Dashboard and Explore deep links.
- Basic time-series and alert panels in the existing `Observability` route.
- Authentication modes: none, Bearer token and Basic authentication.

## Non-goals

- Replacing Prometheus, Alertmanager or Grafana.
- OAuth, custom CA/TLS configuration, certificate-management workflows or
  credential rotation.
- Mutating Alertmanager operations such as creating silences or acknowledging
  alerts.
- Grafana dashboard or datasource discovery.
- Saved queries, dashboards, incident correlation, logs or traces.

## Architecture

Sprint 8 uses a shared observability HTTP adapter rather than three independent
Tauri command implementations. The adapter owns URL validation, authentication,
sanitized failures and read-only HTTP execution. Provider modules own only their
protocol-specific request and response mapping:

```text
React Observability route
        │ IPC command envelope
        ▼
AppState authorization and policy checks
        │
        ▼
Shared observability HTTP adapter
  ├── Prometheus query/query_range mapper
  ├── Alertmanager alerts mapper + resource-reference resolver
  └── Grafana health/link builder
        │
        ▼
Configured native endpoint (GET only)
```

The existing connector registry remains the lifecycle owner. It stores only
non-sensitive metadata in SQLite and uses the OS keychain for the single
credential value. `CredentialStore` gains a backend-only retrieval operation so
the HTTP adapter can read the secret transiently. The existing write-only
`connector_add.credential_value` input is the sole IPC-request exception needed
to put a user-supplied secret into the keychain; no secret may enter a Rust
response type, a later observability IPC request, diagnostic log, persistent or
reactive UI state.

## Connector configuration

All three connector kinds use non-sensitive metadata with this common shape:

```json
{
  "base_url": "https://observability.example.test",
  "auth_mode": "none | bearer | basic",
  "username": "optional-basic-auth-user"
}
```

`username` is valid only for `basic`. The token or password is supplied through
the existing `credential_value` input and stored in the OS keychain under the
connector credential reference. A connector configured with `none` has no
credential reference. Configuration accepts HTTP(S) endpoints; the UI warns
when the configured endpoint is not HTTPS, while loopback and local development
remain usable.

Grafana adds optional, non-sensitive metadata:

```json
{
  "datasource_uid": "prometheus-main",
  "default_dashboard_uid": "service-overview"
}
```

Neither field is inferred. Missing optional fields disable only the affected
link affordance.

## Contracts and data flow

### Prometheus

`prometheus.query` and `prometheus.query_range` are `ResourceRead` commands.
Their requests include a connector ID, PromQL expression and the requested
time window. The backend validates the window before sending a GET request to
the Prometheus query endpoint. Its result preserves the original labels,
timestamps and values, and adds a source reference containing the connector ID
and query identity.

The UI offers a basic query surface and time-series panel. It displays the
backend result without reinterpreting timestamps or synthesizing missing data.

### Alertmanager

`alertmanager.alerts` is a `ResourceRead` command that reads `/api/v2/alerts`.
Each normalized alert retains fingerprint, state, start/end timestamps, labels,
annotations and generator URL when supplied by Alertmanager.

The resource resolver derives a `ResourceReference` only from explicit known
labels: `namespace` plus one of `pod`, `service` or `deployment`. It marks a
reference unresolved when those labels are absent or ambiguous. Unmatched
labels remain evidence and must not be converted into guessed Kubernetes
relationships.

### Grafana

`grafana.health` checks the configured Grafana endpoint. `grafana.link` returns
a Dashboard or Explore URL from an explicit link context: configured base URL,
optional datasource UID, optional dashboard UID, the selected PromQL query and
time range. The UI opens the returned URL only after an explicit user click.

Grafana links are source context, not a replacement for Grafana rendering.

## UI behavior

The existing `Integrations` route remains responsible for adding, testing and
diagnosing connectors. The `Observability` route adds three panels:

1. An Alertmanager alert list with state, labels, timestamps and resource
   reference or unresolved status.
2. A Prometheus query/range panel with a basic time-series rendering.
3. Native Grafana Dashboard and Explore link controls when their explicit
   configuration is available.

Selecting an alert carries its resource reference and labels into the metric
context. The route supports loading, empty, unavailable and malformed-response
states in Thai and English. External navigation is user initiated; no shell
command execution is introduced in this sprint.

## Safety and error handling

- All Sprint 8 native endpoint calls are GET requests.
- Connector credentials are retrieved only inside the backend adapter and are
  never serialized.
- Connector failures expose a sanitized service/status message, not response
  bodies, authorization headers or credential references.
- Malformed configuration and malformed provider payloads return the existing
  typed IPC errors.
- Existing command-envelope, membership and policy checks remain mandatory for
  every new command.
- Query input is treated as operational input, not executable local code; it is
  passed only to the configured Prometheus endpoint.

## Verification and acceptance

Gemini implements test-first. Rust tests use local mock HTTP servers to verify:

- config validation for every supported auth mode;
- exact GET methods and expected authorization headers;
- Prometheus timestamp, value and source-reference mapping;
- Alertmanager alert parsing and conservative resource-reference mapping;
- Grafana health and deep-link construction;
- absence of credential values from SQLite rows, connector summaries,
  diagnostics and serialized IPC results.

React tests use fixture responses to verify alert selection, metric display,
Grafana-link availability, loading/error states, localization and keyboard
accessible controls.

Codex performs an independent review and QA pass after Gemini's implementation:

- inspect the full diff for scope, authentication and secret-handling defects;
- run the Rust workspace tests and Clippy with warnings denied;
- run frontend tests, typecheck, lint and production build;
- confirm the complete fixture journey: alert → resource reference → metric
  series → native Grafana context.

The Sprint 8 exit criterion is satisfied only when that fixture journey works
without live infrastructure and without any mutating endpoint call.
