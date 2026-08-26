# Sprint 9 Logs and Traces Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a responder move from an Alertmanager alert to the matching Loki logs and the Tempo trace behind a log line, without rebuilding the time window by hand.

**Architecture:** Reuse the Sprint 8 observability HTTP adapter unchanged except for one optional tenant header. Add `loki` and `tempo` provider modules beside the existing Prometheus, Alertmanager and Grafana mappers, and move the Sprint 7 sensitive-field-name list into a shared masking module both manifest masking and log masking call. React gains a single workspace time context that drives every panel.

**Tech Stack:** Rust 2021, Tauri 2, Tokio, `reqwest` with Rustls TLS, `httpmock`, SQLite/keyring, React 18, TypeScript, Vitest, Testing Library.

**Spec:** `docs/superpowers/specs/2026-08-26-sprint-9-logs-and-traces-design.md`.

## Global Constraints

- Preserve the existing connector lifecycle and the command-envelope, membership, capability, scope and dual egress-policy checks. Every new read command requires `ResourceRead`; connector configuration continues to require `ConnectorAct`.
- Every Sprint 9 provider request is a fixed, internally selected HTTP GET. Redirects stay disabled, the timeout stays bounded, and failures return a sanitized service or status message with no response body, authorization header or credential reference.
- `tenant_id` is non-secret metadata stored in `config_metadata`, never in the OS keychain. It is sent as `X-Scope-OrgID` only when configured, and only for `loki` and `tempo`.
- Masking runs in Rust before serialization. A value covered by the shared deny list must not reach a React fixture, a diagnostic, a log, or a serialized `IpcResult`.
- Log-to-trace correlation uses only an explicit `trace_id`, `traceID` or `traceparent` field of a parsed JSON log object. No regular expression scans unstructured text for identifier-shaped substrings.
- A Tempo trace ID must be exactly 32 characters matching `[0-9a-f]`. Uppercase hexadecimal, other lengths and any other input are rejected before URL construction.
- Span attributes are returned through the explicit allow list in Task 4 only. An unrecognized attribute key is dropped.
- Every enum crossing the IPC boundary declares explicit `#[serde(rename = ...)]` values, has a Rust test asserting its exact serialized JSON, and its React fixture is copied from that asserted shape — never from what the UI reads.
- Keep English and Thai locale objects structurally identical, preserve keyboard access and focus styles, and add no live-infrastructure dependency. All acceptance tests use local mock endpoints and fixtures.
- Do not add Loki label discovery, Tempo search, saved queries, flame graphs, log virtualization, value-pattern redaction, or new Grafana link targets.

---

### Task 1: Extract the shared sensitive-field masking module

**Files:**
- Create: `src-tauri/src/observability/masking.rs`
- Modify: `src-tauri/src/observability/mod.rs`
- Modify: `src-tauri/src/kubernetes.rs:231-237`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub const REDACTED: &str`, `pub fn sensitive_key(key: &str) -> bool`, and `pub fn mask_json_object(object: &mut serde_json::Map<String, serde_json::Value>) -> bool` returning whether any value was replaced.

- [ ] **Step 1: Write the failing test in the new module**

```rust
#[test]
fn sensitive_key_matches_the_sprint_7_semantics() {
    for key in ["password", "API_KEY", "client_secret", "authToken", "credential"] {
        assert!(sensitive_key(key), "{key}");
    }
    for key in ["namespace", "message", "level", "duration_ms"] {
        assert!(!sensitive_key(key), "{key}");
    }
}

#[test]
fn mask_json_object_replaces_only_sensitive_values() {
    let mut object = serde_json::json!({ "msg": "hello", "api_key": "sk-live-1" })
        .as_object()
        .unwrap()
        .clone();
    assert!(mask_json_object(&mut object));
    assert_eq!(object["msg"], serde_json::json!("hello"));
    assert_eq!(object["api_key"], serde_json::json!(REDACTED));
}
```

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test -p thalassaops masking::`
Expected: FAIL — module `masking` does not exist.

- [ ] **Step 3: Create the module with the Sprint 7 semantics moved verbatim**

Move the existing constant and matcher out of `kubernetes.rs` without changing behaviour. Substring matching is deliberate: over-masking is the safe direction, and each masked entry reports `masked: true` so a reader always knows a replacement happened.

```rust
pub const REDACTED: &str = "<REDACTED>";

pub fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    ["password", "secret", "token", "key", "credential"]
        .iter()
        .any(|needle| key.contains(needle))
}

pub fn mask_json_object(object: &mut serde_json::Map<String, serde_json::Value>) -> bool {
    let mut masked = false;
    for (key, value) in object.iter_mut() {
        if sensitive_key(key) {
            *value = serde_json::Value::String(REDACTED.into());
            masked = true;
        }
    }
    masked
}
```

- [ ] **Step 4: Point `kubernetes.rs` at the shared module**

Delete the local `REDACTED` and `sensitive_key` and import them from `crate::observability::masking`. Leave every manifest traversal function in `kubernetes.rs` untouched. Declare `pub mod masking;` in `observability/mod.rs`.

- [ ] **Step 5: Prove the Sprint 7 masking is unchanged**

Run: `cargo test -p thalassaops kubernetes::` then `cargo test --workspace`
Expected: PASS, including `masking_redacts_secret_and_sensitive_metadata_but_preserves_name` and `masking_redacts_sensitive_deployment_container_environment_values`. These two tests are the safety net for this move; if either fails, the extraction changed behaviour and must be corrected rather than the test adjusted.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/observability/masking.rs src-tauri/src/observability/mod.rs src-tauri/src/kubernetes.rs
git commit -m "refactor: share the sensitive field deny list across masking callers"
```

---

### Task 2: Add tenant metadata and the Loki and Tempo connector kinds

**Files:**
- Modify: `src-tauri/src/observability/mod.rs`
- Modify: `src-tauri/src/observability/client.rs`
- Modify: `src-tauri/src/connectors.rs:204-230` (manifests), `:256` (`validate_add_request`), `:426-460` (`run_connection_test`)

**Interfaces:**
- Consumes: `ObservabilityClient`, `ObservabilityConnectorConfig` from Sprint 8.
- Produces: `pub const LOKI_CONNECTOR_KIND: &str = "loki";`, `pub const TEMPO_CONNECTOR_KIND: &str = "tempo";`, `ObservabilityConnectorConfig.tenant_id: Option<String>`, `pub fn loki_manifest() -> ConnectorManifest`, `pub fn tempo_manifest() -> ConnectorManifest`.

- [ ] **Step 1: Write the failing adapter tests**

```rust
#[tokio::test]
async fn sends_scope_org_id_when_tenant_is_configured() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET").path("/ready").header("X-Scope-OrgID", "team-a");
        then.status(200).body("ready");
    });
    let connector = tenant_connector(&server.url(""), "loki", Some("team-a"));
    let client = ObservabilityClient::new(&connector, &InMemoryCredentialStore::default()).unwrap();
    let request = client.prepare_get(client.build_url("/ready").unwrap()).unwrap();
    client.execute_empty(request).await.unwrap();
    mock.assert();
}

#[tokio::test]
async fn omits_scope_org_id_for_non_tenant_kinds_and_when_absent() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method("GET").path("/ready").matches(|req| {
            !req.headers.iter().flatten().any(|(name, _)| name.eq_ignore_ascii_case("x-scope-orgid"))
        });
        then.status(200).body("ready");
    });
    let connector = tenant_connector(&server.url(""), "prometheus", Some("team-a"));
    let client = ObservabilityClient::new(&connector, &InMemoryCredentialStore::default()).unwrap();
    let request = client.prepare_get(client.build_url("/ready").unwrap()).unwrap();
    client.execute_empty(request).await.unwrap();
    mock.assert();
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops observability::client::`
Expected: FAIL — `tenant_id` is not part of the configuration.

- [ ] **Step 3: Add the field and the header**

Add `pub tenant_id: Option<String>` to `ObservabilityConnectorConfig`. In `ObservabilityClient::new`, keep the tenant only when the connector kind is `loki` or `tempo`; in `prepare_get`, add `X-Scope-OrgID` when a tenant is present. `validate` rejects a `tenant_id` that is present but blank after trimming.

- [ ] **Step 4: Add the manifests and validation**

```rust
pub fn loki_manifest() -> ConnectorManifest {
    ConnectorManifest::new(LOKI_CONNECTOR_KIND, "Loki", "0.1.0")
        .with_capability(ConnectorCapability::read("loki.query_range", ["query_range"]))
}

pub fn tempo_manifest() -> ConnectorManifest {
    ConnectorManifest::new(TEMPO_CONNECTOR_KIND, "Tempo", "0.1.0")
        .with_capability(ConnectorCapability::read("tempo.trace", ["trace"]))
        .with_capability(ConnectorCapability::read("tempo.health", ["health"]))
}
```

Extend `manifest_for` and `validate_add_request` so both kinds validate through `ObservabilityConnectorConfig` exactly as the Sprint 8 kinds do.

- [ ] **Step 5: Extend the connection-test probe path**

`run_connection_test` currently maps Grafana to `/api/health` and the other observability kinds to `/-/ready`. Map `loki` and `tempo` to `/ready`. Keep the failure message sanitized — no error text interpolation.

- [ ] **Step 6: Run the tests and commit**

Run: `cargo test --workspace`
Expected: PASS.

```bash
git add src-tauri/src/observability/mod.rs src-tauri/src/observability/client.rs src-tauri/src/connectors.rs
git commit -m "feat: add loki and tempo connector kinds with optional tenant metadata"
```

---

### Task 3: Implement Loki range queries with masked log entries

**Files:**
- Create: `src-tauri/src/observability/loki.rs`
- Modify: `src-tauri/src/observability/mod.rs`, `src-tauri/src/app.rs`, `src-tauri/src/main.rs`, `ui/contracts/ipc.ts`

**Interfaces:**
- Consumes: `ObservabilityClient`, `masking::{mask_json_object, sensitive_key, REDACTED}`.
- Produces: `LogEntry`, `LogStream`, `LogSourceReference`, `LokiQueryResult`, `LokiQueryRangeRequest`, `pub async fn query_range(client: &ObservabilityClient, request: LokiQueryRangeRequest) -> Result<LokiQueryResult, LokiError>`, and `AppState::loki_query_range`.

- [ ] **Step 1: Write the failing provider tests**

Use an `httpmock` fixture shaped like a real Loki response, asserting `GET /loki/api/v1/query_range` with `query`, `start`, `end`, `limit` and `direction=backward`:

```json
{"status":"success","data":{"resultType":"streams","result":[
  {"stream":{"namespace":"prod","pod":"api-0"},
   "values":[["1735689600000000001","{\"msg\":\"boom\",\"api_key\":\"sk-live-1\",\"trace_id\":\"4bf92f3577b34da6a3ce929d0e0e4736\"}"],
             ["1735689600000000002","plain text line with api_key=sk-live-2"]]}]}}
```

Assert: the first entry has `parsed: true`, `masked: true`, `fields["api_key"] == "<REDACTED>"`, `trace_id == Some("4bf92f...")`, and `timestamp_ns == "1735689600000000001"` as a string; the second has `parsed: false`, `masked: false`, its original text preserved, `trace_id == None`, and `unparsed_count == 1`. Add a negative test that a `traceparent`-shaped substring inside an unparsed line yields `trace_id: None`.

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops observability::loki::`
Expected: FAIL — module `loki` does not exist.

- [ ] **Step 3: Define the contract types**

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogEntry {
    pub timestamp_ns: String,
    pub line: String,
    pub parsed: bool,
    pub masked: bool,
    pub fields: Option<BTreeMap<String, String>>,
    pub trace_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogStream {
    pub labels: BTreeMap<String, String>,
    pub entries: Vec<LogEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogSourceReference {
    pub connector_id: String,
    pub query: String,
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LokiQueryResult {
    pub streams: Vec<LogStream>,
    pub source: LogSourceReference,
    pub unparsed_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LokiQueryRangeRequest {
    pub connector_id: String,
    pub query: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub limit: u32,
}
```

Mirror all of these in `ui/contracts/ipc.ts` with identical field names.

- [ ] **Step 4: Implement validation and mapping**

Reject a blank `query`, `start > end`, `limit == 0`, and `limit > MAX_LOG_LINES` where `pub const MAX_LOG_LINES: u32 = 200;`. Send `direction=backward`. A `status` other than `"success"`, a missing `data`, a `resultType` other than `"streams"`, or a malformed value tuple is a sanitized typed failure — never silently synthesized data.

For each value tuple, keep the timestamp string verbatim. Parse the line as a JSON object; on success run `mask_json_object`, rebuild `line` from the masked object, set `parsed: true` and `masked` to whatever the masker returned, and take `trace_id` from `trace_id`, `traceID` or `traceparent` in that order. On parse failure keep the original text, set `parsed: false, masked: false, fields: None, trace_id: None`, and increment `unparsed_count`.

- [ ] **Step 5: Add the command and handler**

Add `AppState::loki_query_range` following the Sprint 8 command body exactly: validate the `loki.query_range` descriptor, require `ResourceRead`, reject a bounded scope and an inactive membership, reject a disabled connector as `ConnectorUnavailable` and a wrong-kind connector as `NotFound`, check `ExternalIntegration` before the request and `Ui` before returning. Register `loki_query_range` in `main.rs`.

- [ ] **Step 6: Add the app-level tests**

Cover wrong command, wrong capability, bounded scope, inactive membership, disabled connector, wrong-kind connector, policy denial, and a success case asserting the serialized `IpcResult` contains neither the fixture credential nor `credential_reference` nor `sk-live-`.

- [ ] **Step 7: Run and commit**

Run: `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, no warnings.

```bash
git add src-tauri/src/observability/loki.rs src-tauri/src/observability/mod.rs src-tauri/src/app.rs src-tauri/src/main.rs ui/contracts/ipc.ts
git commit -m "feat: add loki range queries with backend log masking"
```

---

### Task 4: Implement Tempo trace retrieval and health

**Files:**
- Create: `src-tauri/src/observability/tempo.rs`
- Modify: `src-tauri/src/observability/mod.rs`, `src-tauri/src/app.rs`, `src-tauri/src/main.rs`, `ui/contracts/ipc.ts`

**Interfaces:**
- Consumes: `ObservabilityClient`.
- Produces: `SpanSummary`, `TraceResult`, `TraceSourceReference`, `TempoTraceRequest`, `pub fn validate_trace_id(value: &str) -> Result<(), TempoError>`, `pub async fn trace(...)`, `pub async fn health(...)`, `AppState::tempo_trace`, `AppState::tempo_health`.

- [ ] **Step 1: Write the failing tests**

Assert `validate_trace_id` accepts exactly 32 lowercase hexadecimal characters and rejects a 16-character ID, a 32-character uppercase ID, a 31- or 33-character ID, and any value containing `/`, `.` or `%`. Assert with `httpmock` that `tempo.trace` issues `GET /api/traces/4bf92f3577b34da6a3ce929d0e0e4736` and that a rejected ID produces no HTTP request at all. Assert `tempo.health` issues `GET /ready`.

Add a mapping test whose fixture span carries `http.status_code`, `http.url`, `db.statement` and `app.customer_email`, and assert the serialized span contains `http.status_code` and contains none of the other three anywhere in its JSON.

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test -p thalassaops observability::tempo::`
Expected: FAIL — module `tempo` does not exist.

- [ ] **Step 3: Define the contract types and the allow list**

```rust
pub const ALLOWED_SPAN_ATTRIBUTES: [&str; 8] = [
    "http.status_code",
    "http.method",
    "http.route",
    "rpc.service",
    "rpc.method",
    "db.system",
    "exception.type",
    "otel.status_description",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SpanSummary {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub service_name: String,
    pub start_time_unix_nano: String,
    pub duration_nano: String,
    pub status: String,
    pub attributes: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraceSourceReference {
    pub connector_id: String,
    pub trace_id: String,
    pub endpoint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TraceResult {
    pub trace_id: String,
    pub spans: Vec<SpanSummary>,
    pub source: TraceSourceReference,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TempoTraceRequest {
    pub connector_id: String,
    pub trace_id: String,
}
```

Mirror both in `ui/contracts/ipc.ts`.

- [ ] **Step 4: Implement validation and mapping**

Validate the trace ID before building the URL. Map OTLP resource spans to `SpanSummary`, taking `service_name` from the resource attribute `service.name`, keeping nanosecond values as strings, and copying only allow-listed attributes. An unrecognized attribute key is dropped without comment. A malformed payload is a sanitized typed failure.

- [ ] **Step 5: Add the commands, handlers and app-level tests**

Each command validates its own `tempo.<verb>` descriptor, requires `ResourceRead`, rejects a bounded scope and an inactive membership with `PermissionDenied`, rejects a disabled connector as `ConnectorUnavailable` and a wrong-kind connector as `NotFound`, checks `ExternalIntegration` before the request and `Ui` before returning, and returns `PolicyDenied` when either egress check fails. Register `tempo_trace` and `tempo_health` in `main.rs`. Repeat the whole authorization table for both commands, plus a success case asserting the serialized `IpcResult` contains neither the fixture credential nor `credential_reference`.

- [ ] **Step 6: Run and commit**

Run: `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, no warnings.

```bash
git add src-tauri/src/observability/tempo.rs src-tauri/src/observability/mod.rs src-tauri/src/app.rs src-tauri/src/main.rs ui/contracts/ipc.ts
git commit -m "feat: add tempo trace retrieval with an allow-listed span attribute set"
```

---

### Task 5: Split the Observability workspace with no behaviour change

**Files:**
- Create: `ui/src/observability/AlertsPanel.tsx`, `MetricsPanel.tsx`, `GrafanaPanel.tsx`, `TimeRangeControl.tsx`, `timeContext.ts`
- Modify: `ui/src/ObservabilityWorkspace.tsx`, `ui/src/shell.tsx`, `ui/src/shell.test.tsx`

**Interfaces:**
- Consumes: the existing `ObservabilityWorkspace` props.
- Produces: `TimeContext = { start: string; end: string; source: "alert" | "manual" }` exported from `ui/src/observability/timeContext.ts`, plus the panel components above, each taking its data and the shared `timeContext` as props.

- [ ] **Step 1: Run the existing suite and record the baseline**

Run: `npm test`
Expected: PASS. This task must end with the same tests passing and no assertion edited except import paths.

- [ ] **Step 2: Move each panel into its own file**

Move the existing alert list, metric query panel and Grafana panel out of `ObservabilityWorkspace.tsx` verbatim, changing only what an extraction forces: props in, imports out. Do not rename a label, reorder a control, or alter a query while moving it.

- [ ] **Step 3: Introduce the shared time context**

Add `timeContext.ts` with the type above and a `timeContextFromAlert(alert, now)` helper returning `{ start: alert.starts_at, end: alert.state === "resolved" && alert.ends_at ? alert.ends_at : now, source: "alert" }`. Hold the context in `ObservabilityWorkspace` and pass it to `MetricsPanel` in place of that panel's own range state.

- [ ] **Step 4: Add `TimeRangeControl`**

One labelled start/end control that reports edits upward and switches `source` to `"manual"`. When `source` is `"manual"`, the workspace renders a localized line stating the window no longer follows the selected alert.

- [ ] **Step 5: Confirm behaviour is unchanged and commit**

Run: `npm test`, `npm run typecheck`, `npm run lint`, `npm run build`, `npm run format:check`
Expected: PASS. The Sprint 8 fixture journey test must still pass without changing its assertions.

```bash
git add ui/src/observability ui/src/ObservabilityWorkspace.tsx ui/src/shell.tsx ui/src/shell.test.tsx
git commit -m "refactor: split the observability workspace and share one time context"
```

---

### Task 6: Deliver the logs and traces panels

**Files:**
- Create: `ui/src/observability/LogsPanel.tsx`, `ui/src/observability/TracePanel.tsx`
- Modify: `ui/src/ObservabilityWorkspace.tsx`, `ui/src/locales/en.ts`, `ui/src/locales/th.ts`, `ui/src/styles.css`, `ui/src/shell.test.tsx`

**Interfaces:**
- Consumes: `LokiQueryResult`, `TraceResult` from `ui/contracts/ipc.ts`; `TimeContext` from Task 5.
- Produces: no exports other than the two components.

- [ ] **Step 1: Write the failing fixture journey test**

Copy the fixtures from the shapes asserted in Task 3 Step 1 and Task 4 Step 1 — not from what the components read. The test selects a firing alert, asserts the time context is set from it, runs a log query, asserts a masked value renders as `<REDACTED>`, asserts the banner names one unmasked unparsed line, clicks the trace control on the entry carrying a trace ID, and asserts the span table renders the service name, duration and `http.status_code`. Add a second test where no entry carries a trace ID and the trace panel states that explicitly.

- [ ] **Step 2: Run it and confirm it fails**

Run: `npm test -- shell.test.tsx`
Expected: FAIL — the logs panel does not exist.

- [ ] **Step 3: Build `LogsPanel`**

Pre-fill the LogQL input from the selected alert's labels using the Sprint 8 rule — `namespace` plus exactly one of `pod`, `service` or `deployment` — as `{namespace="x", pod="y"}`, and leave it editable. Never send a query the user cannot see. Render stream labels, timestamp and line, plus loading, empty, unavailable and malformed states.

- [ ] **Step 4: Add the honest masking banner**

When `unparsed_count > 0`, render a `role="status"` line stating how many lines could not be parsed and were therefore not masked. Do not soften it, and do not hide the lines.

- [ ] **Step 5: Build `TracePanel`**

Open on a trace control click, call `tempo_trace`, and render spans as a table ordered parent before child with an indentation column for depth, plus name, service, duration, status and the allow-listed attributes. When the window has no trace ID at all, say so instead of rendering an empty table.

- [ ] **Step 6: Add matching locale keys and styles**

Add every new key to `en.ts` and `th.ts` in the same object shape, and add CSS only for these two panels.

- [ ] **Step 7: Run and commit**

Run: `npm test`, `npm run typecheck`, `npm run lint`, `npm run build`, `npm run format:check`
Expected: PASS.

```bash
git add ui/src/observability ui/src/ObservabilityWorkspace.tsx ui/src/locales ui/src/styles.css ui/src/shell.test.tsx
git commit -m "feat: add the logs and traces investigation panels"
```

---

### Task 7: Complete regression, security and acceptance verification

**Files:**
- Modify only if a verification defect requires a minimal, in-scope fix: files above.

- [ ] **Step 1: Run the full Rust gates from the repository root**

Run: `cargo test --workspace` then `cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS with no warnings. Fix only failures caused by Sprint 9 work and rerun the exact failing command first.

- [ ] **Step 2: Run the full frontend gates under Node 24**

Run: `node -v` (must report v24.x), then `npm test`, `npm run typecheck`, `npm run lint`, `npm run build`, `npm run format:check`
Expected: PASS. Do not bypass a test, mute a lint, or weaken a type to make the suite pass.

- [ ] **Step 3: Audit the diff for leaks and scope**

Run `git diff main...HEAD` and confirm: no credential, `Authorization` value or `credential_reference` in any serialized shape; `X-Scope-OrgID` sent only for `loki` and `tempo`; every provider call a GET against a local mock in tests; no Loki label discovery, Tempo search or new Grafana target; span attributes limited to `ALLOWED_SPAN_ATTRIBUTES`.

- [ ] **Step 4: Execute the fixture acceptance journey**

Alert selected → time context set from the alert → Loki logs rendered with masking and the unparsed banner → trace opened from an explicit log `trace_id` → span table rendered — all three panels reading the same time context, with no live service and no mutating request.

- [ ] **Step 5: Record results and hand off**

Record the exact verification commands and their output in the completion report. Leave the branch committed and unpushed for the independent review; do not self-approve a failed or partial exit criterion.
