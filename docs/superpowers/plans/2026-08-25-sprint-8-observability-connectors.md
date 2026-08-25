# Sprint 8 Observability Connectors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a read-only, local-first investigation path from an Alertmanager alert to its explicit Kubernetes resource reference, Prometheus evidence, and user-initiated native Grafana context.

**Architecture:** Keep the existing connector registry as the lifecycle and credential-reference owner. Add a shared Rust observability HTTP adapter that is the only code allowed to validate endpoints, retrieve a secret transiently, apply authentication, issue GET requests, and sanitize provider failures. Provider modules map Prometheus, Alertmanager, and Grafana protocols into stable response types; `AppState` remains the authorization and egress-policy boundary; React owns only presentation and explicit external navigation.

**Tech Stack:** Rust 2021, Tauri 2, Tokio, `reqwest` with Rustls TLS, `httpmock`, SQLite/keyring, React 18, TypeScript, Vitest, Testing Library, `@tauri-apps/plugin-shell`.

**Spec:** `docs/superpowers/specs/2026-08-25-sprint-8-observability-connectors-design.md`.

## Global Constraints

- Preserve the existing connector lifecycle, command-envelope, membership, capability, scope, and policy checks. New read commands require `ResourceRead`; connector configuration continues to require `ConnectorAct`.
- Support only `none`, Bearer token, and Basic authentication. OAuth, custom CA/TLS configuration, certificate workflows, credential rotation, mutations, datasource/dashboard discovery, saved queries, logs, and traces are out of scope.
- Secrets may exist only as a transient Rust local value while an adapter builds an authorization header. The sole IPC-request exception is the existing write-only `connector_add.credential_value` input that transfers a user-supplied credential to the OS keychain. Secrets must otherwise never appear in SQLite, connector summaries, diagnostics, provider errors, IPC responses or later observability requests, logs, test snapshots, reactive React state, URLs, or translation strings. The credential input is an uncontrolled password field, read only at submit time and cleared immediately.
- Every provider request is a fixed, internally selected HTTP GET endpoint. Reject invalid/non-HTTP(S) URLs, embedded URL userinfo, query/fragment base URLs, malformed auth configuration, missing required credentials, and unsupported connector kinds before any request. Disable redirect following, set an explicit timeout, and return only sanitized service/status failures.
- Gate the outbound request with `EgressDestination::ExternalIntegration` and the result returned to React with `EgressDestination::Ui`; never pass a credential to either egress request. Existing immutable-secret protection remains fail-closed.
- Alert-to-resource mapping is deliberately conservative: only an exact `namespace` label plus exactly one of exact `pod`, `service`, or `deployment` labels yields a reference. No synonym, fuzzy match, or guessed Kubernetes relation is allowed.
- Grafana URLs are generated only from explicit configured IDs and explicit UI context. The UI may call `open()` only in a user click handler and must not execute a shell command.
- Keep English and Thai localization keys structurally identical, preserve keyboard access and focus styles, and add no untested live-infrastructure dependency. All acceptance tests use local mock endpoints and fixtures.

---

### Task 1: Establish the secure observability connector foundation

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/connectors.rs`
- Create: `src-tauri/src/observability/mod.rs`
- Create: `src-tauri/src/observability/client.rs`
- Modify: `src-tauri/src/app.rs`

- [ ] Write failing Rust tests first for all three connector kinds, their manifests, validation of `none`/Bearer/Basic metadata, rejection of bad URLs/embedded credentials/invalid username combinations, and keychain retrieval without secret serialization.
- [ ] Add `reqwest` with JSON and Rustls TLS support, then create `observability` as a dedicated module tree. Export constants for the three connector kinds and a shared configuration type:

  ```rust
  pub const PROMETHEUS_CONNECTOR_KIND: &str = "prometheus";
  pub const ALERTMANAGER_CONNECTOR_KIND: &str = "alertmanager";
  pub const GRAFANA_CONNECTOR_KIND: &str = "grafana";

  pub struct ObservabilityConnectorConfig {
      pub base_url: String,
      pub auth_mode: ObservabilityAuthMode,
      pub username: Option<String>,
  }
  ```

- [ ] Give `prometheus_manifest`, `alertmanager_manifest`, and `grafana_manifest` read-only capabilities; extend `connectors::manifest_for` and `validate_add_request` to use them. Require `credential_value` to be absent for `none` and non-empty for Bearer/Basic; require `username` only for Basic.
- [ ] Extend `CredentialStore` with a backend-only `get(&self, reference) -> Result<Option<String>, ConnectorError>` implementation for the OS keychain and `InMemoryCredentialStore`. Keep credential references private to connector-store functions; do not add them to `ConnectorSummary` or any IPC contract.
- [ ] Implement the shared client in `client.rs`: parse a canonical base URL, join only fixed relative provider paths, use `reqwest::Client` with redirects disabled and a bounded timeout, select the exact `Authorization` header, send only GET, and translate transport/status/JSON errors to sanitized domain errors with no response body.
- [ ] Refactor connection-test persistence so an observability probe can reuse the existing health/log history update without exposing credentials. Route observability test probes through the shared client and have `AppState::connector_test` authorize `ExternalIntegration` before the probe.
- [ ] Run the focused module tests. Confirm a database row, `ConnectorSummary`, `ConnectorDiagnostics`, serialized `IpcResult`, and an adapter error cannot contain a fixture secret.

### Task 2: Implement Prometheus query and range evidence contracts

**Files:**
- Create: `src-tauri/src/observability/prometheus.rs`
- Modify: `src-tauri/src/observability/mod.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `ui/contracts/ipc.ts`

- [ ] Write failing `httpmock` tests that assert `GET /-/ready`, `GET /api/v1/query`, and `GET /api/v1/query_range`; test absent, Bearer, and Basic authorization headers and verify the query parameters exactly.
- [ ] Define stable serializable Rust response types and matching TypeScript types. Preserve labels, numeric timestamps, sample values, requested range, and source provenance without persisting the query:

  ```rust
  pub struct MetricSample { pub timestamp: f64, pub value: String }
  pub struct MetricSeries { pub labels: BTreeMap<String, String>, pub samples: Vec<MetricSample> }
  pub struct MetricSourceReference { pub connector_id: String, pub query: String, pub endpoint: String }
  pub struct PrometheusQueryResult { pub series: Vec<MetricSeries>, pub source: MetricSourceReference }
  ```

- [ ] Map Prometheus `vector` results to one sample per series and `matrix` results to all returned samples. Treat an error status, missing `data`, unsupported result shape, non-numeric timestamp, or malformed tuple as a sanitized typed failure; do not silently synthesize data.
- [ ] Add request types for instant and range queries. Require a nonblank PromQL expression; validate RFC 3339 start/end timestamps, `start <= end`, positive bounded `step_seconds`, and an allowed connector before calling the provider.
- [ ] Add async `AppState::prometheus_query` and `AppState::prometheus_query_range` methods plus Tauri handlers `prometheus_query` and `prometheus_query_range`. Each must validate the exact `prometheus.<verb>` descriptor, require `ResourceRead`, verify active membership/unbounded workspace scope, check `ExternalIntegration` before the request and `Ui` before returning the result.
- [ ] Add app-level tests for wrong command/capability/scope, disabled or wrong-kind connectors, a policy denial, and a successful response that carries the connector ID and query source reference without a credential.

### Task 3: Implement Alertmanager ingestion and safe Kubernetes mapping

**Files:**
- Create: `src-tauri/src/observability/alertmanager.rs`
- Modify: `src-tauri/src/observability/mod.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `ui/contracts/ipc.ts`

- [ ] Write failing adapter tests using `/api/v2/alerts` fixtures for firing and resolved alerts, no matching labels, a valid one-kind match, and ambiguous/multiple resource-kind labels. Assert every mock expects GET and never accepts a mutation method.
- [ ] Normalize alerts into a serializable contract retaining `fingerprint`, `status.state`, `startsAt`, `endsAt`, labels, annotations, optional `generatorURL`, a source reference, and a `ResourceReference` result. Preserve provider evidence verbatim except for type normalization; do not redact or infer normal labels.
- [ ] Implement `resolve_resource_reference(labels)` using only `namespace` with exactly one of `pod`, `service`, or `deployment`. Return an explicit unresolved state and reason for missing namespace, missing target, or multiple targets. Unit-test that labels such as `app`, `job`, `instance`, `kubernetes_pod_name`, and label-value heuristics never create a reference.
- [ ] Add `alertmanager.alerts` as an async `ResourceRead` command and Tauri handler. Reuse the same descriptor/membership/scope and dual egress-policy checks as Prometheus; verify its connector kind before dispatch.
- [ ] Add `AppState` tests for fixture ingestion, enforced authorization/policy failures, and JSON serialization proving response evidence does not contain a connector secret or credential reference.

### Task 4: Implement Grafana health and native-context link construction

**Files:**
- Create: `src-tauri/src/observability/grafana.rs`
- Modify: `src-tauri/src/observability/mod.rs`
- Modify: `src-tauri/src/app.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `ui/contracts/ipc.ts`

- [ ] Write failing unit and `httpmock` tests for Grafana health GET/auth, a valid Dashboard URL, a valid Explore URL, URL encoding of PromQL and time range, missing configured dashboard/datasource IDs, and rejection of unsupported link targets.
- [ ] Probe Grafana health through its fixed health endpoint using the shared adapter. Return only the normalized health fields required by the UI and sanitized errors on malformed/unavailable responses.
- [ ] Add an explicit `GrafanaLinkRequest` containing connector ID, target (`dashboard` or `explore`), selected query, and start/end timestamps. Build a Dashboard URL only when a supplied or configured `default_dashboard_uid` exists; build an Explore URL only when `datasource_uid` exists. Never discover IDs and never include a credential.
- [ ] Add `grafana.health` and `grafana.link` `ResourceRead` methods/handlers with the same authorization and egress policy path. The link command validates configuration and returns a URL but never opens it server-side.
- [ ] Test command-envelope rejection, connector kind/disabled rejection, no-credential serialization, and source-link correctness after URL parsing rather than brittle raw-string comparison.

### Task 5: Let Integrations configure the Sprint 8 connector types safely

**Files:**
- Modify: `ui/src/shell.tsx`
- Modify: `ui/contracts/ipc.ts`
- Modify: `ui/src/locales/en.ts`
- Modify: `ui/src/locales/th.ts`
- Modify: `ui/src/styles.css`
- Modify: `ui/src/shell.test.tsx`

- [ ] Add failing React tests for selecting Prometheus, Alertmanager, or Grafana in Integrations; submitting only valid non-secret metadata; passing Bearer/Basic credentials exactly once to `connector_add`; and rendering the existing test/diagnose lifecycle after creation.
- [ ] Replace the fixture-only add affordance with an accessible connector form that retains the fixture option and adds the three supported observability kinds. The form collects display name, base URL, auth mode, Basic username, Grafana datasource UID, and default dashboard UID as applicable.
- [ ] Keep the credential field uncontrolled (`type="password"` with a ref). Include its value only in the one `connector_add` payload, then clear the DOM input; never put it in `useState`, a URL, a notification, or a rendered diagnostic. Omit `credential_value` entirely for `none`.
- [ ] Add visible HTTPS guidance that allows HTTP loopback/development endpoints but makes no claim that HTTP is equally secure. Surface backend invalid/unavailable results as localized, sanitized status text.
- [ ] Keep tables and form controls labeled, keyboard reachable, and usable at the existing responsive breakpoints. Add matching English/Thai keys in the same object shape and CSS only for this route/form.

### Task 6: Deliver the Observability investigation workspace

**Files:**
- Modify: `ui/src/shell.tsx`
- Modify: `ui/contracts/ipc.ts`
- Modify: `ui/src/locales/en.ts`
- Modify: `ui/src/locales/th.ts`
- Modify: `ui/src/styles.css`
- Modify: `ui/src/shell.test.tsx`

- [ ] Write failing fixture-driven React tests for the complete path: open Observability, choose an Alertmanager connector, select a firing alert with a valid resource reference, run a Prometheus range query, inspect label/timestamp/value evidence, request a Grafana link, and open it only after clicking the link control.
- [ ] Replace the Observability `EmptyState` with an `ObservabilityWorkspace` component that loads enabled observability connectors and presents three separately labelled panels: Alertmanager alerts, Prometheus instant/range query evidence, and Grafana native context.
- [ ] Carry the selected alert's immutable labels and resolved/unresolved resource reference into the visible metric context. Display resource kind/name/namespace when resolved and the explicit unresolved reason otherwise; do not add a guessed Kubernetes navigation link.
- [ ] Render metric series as accessible evidence (series labels plus timestamp/value rows) with loading, empty, unavailable, and malformed-result states. Validate the UI's time range before invoking the backend, but rely on backend validation as authoritative.
- [ ] Request Dashboard/Explore URLs through `grafana_link`; hide each unavailable affordance rather than guessing configuration. Use `open(url)` only inside its click event, show no raw secret or authorization data, and give each button an unambiguous localized accessible name.
- [ ] Add English and Thai fixture assertions, keyboard traversal checks, and a test that the route does not invoke an external-navigation API during render/load.

### Task 7: Complete regression, security, and acceptance verification

**Files:**
- Modify only if a verification defect requires a minimal, in-scope fix: files above

- [ ] Run `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` from the repository root. Fix only failures caused by Sprint 8 work and rerun the exact failing command first.
- [ ] Run `npm test`, `npm run typecheck`, `npm run lint`, `npm run build`, and `npm run format:check`. Do not bypass a test, mute a lint, or weaken a type solely to make the suite pass.
- [ ] Inspect `git diff` and targeted serialized fixtures for secrets, `Authorization`, `credential_reference`, unexpected methods, redirect behavior, and scope expansion. Confirm all native endpoint tests use local mock servers.
- [ ] Execute the fixture acceptance journey end-to-end: Alertmanager firing alert → exact resource reference or explicit unresolved result → Prometheus series with source reference → Grafana Dashboard/Explore URL returned → user click opens native context. Confirm no mutation request and no live service is required.
- [ ] Record the exact verification commands and results in the implementation handoff/review notes. Leave the branch ready for Codex's independent code review and QA; do not self-approve a failed or partial exit criterion.
