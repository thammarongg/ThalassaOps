# Sprint 9 Logs and Traces Design

**Status:** Approved design
**Date:** 2026-08-26
**Sprint:** 9 — Loki and OpenTelemetry

## Goal

Add logs and traces to the investigation path Sprint 8 established, so a
responder who opens an alert can read the matching logs and the trace behind a
log line without reconstructing the time window by hand.

## Scope

- Loki range queries against a configured Loki endpoint.
- Tempo trace retrieval by trace ID and a Tempo health probe.
- Backend log masking by sensitive field name, with an explicit warning when a
  log line cannot be parsed and therefore cannot be masked.
- Conservative log-to-trace correlation from explicit log fields only.
- A single workspace time context that drives the metric, log and trace panels.
- Optional non-secret multi-tenancy metadata sent as `X-Scope-OrgID`.

## Non-goals

- Receiving pushed OTLP telemetry. ThalassaOps opens no listener and stores no
  telemetry; it reads from configured backends only.
- Jaeger, or any trace backend other than Tempo.
- Loki label discovery, Tempo trace search, saved queries and log alerting.
- Span attributes beyond the explicit allow list below, flame graphs and log
  line virtualization.
- Value-pattern redaction (regex over unstructured text) and policy-driven
  redaction rules, which belong to Policy Center in Sprint 20.
- New Grafana link targets, including a logs datasource UID.

## Architecture

Sprint 9 adds provider modules to the Sprint 8 observability module tree and
changes the shared HTTP adapter in exactly one place: an optional tenant
header. Every other guarantee of that adapter — GET only, redirects disabled,
bounded timeout, transient credential use, sanitized failures — is reused
unchanged rather than reimplemented.

```text
React Observability route
        │  one TimeContext drives every panel
        ▼
AppState authorization and policy checks
        │
        ▼
Shared observability HTTP adapter (+ optional X-Scope-OrgID)
  ├── Prometheus mapper            (Sprint 8, unchanged)
  ├── Alertmanager mapper          (Sprint 8, unchanged)
  ├── Grafana link builder         (Sprint 8, unchanged)
  ├── Loki mapper  → shared masking module
  └── Tempo mapper
        │
        ▼
Configured native endpoint (GET only)
```

`observability/masking.rs` becomes the single owner of the sensitive-field-name
deny list. The Sprint 7 manifest masking in `kubernetes.rs` currently carries
its own copy of that list; this sprint moves both callers onto the shared list.
Two lists would drift, and the leak would open in whichever one was forgotten.

## Connector configuration

Two new connector kinds, `loki` and `tempo`, reuse the Sprint 8 configuration
shape and add one optional non-secret field:

```json
{
  "base_url": "https://loki.example.test",
  "auth_mode": "none | bearer | basic",
  "username": "optional-basic-auth-user",
  "tenant_id": "optional-tenant"
}
```

`tenant_id` is metadata, not a credential: it is stored in SQLite with the rest
of the configuration and never in the OS keychain. When present, the adapter
sends it as `X-Scope-OrgID`. When absent, no such header is sent. Connector
kinds that do not support multi-tenancy never receive the header even if the
field is somehow present.

## Contracts and data flow

### Loki

`loki.query_range` is a `ResourceRead` command reading
`GET /loki/api/v1/query_range`. Its request carries a connector ID, a LogQL
expression, the requested time window and a line limit. The backend validates a
non-blank expression, `start <= end`, and a positive limit bounded by a server
side maximum (default 200). Direction is fixed to `backward`.

The backend never synthesizes a LogQL query. The UI pre-fills one from an
alert's labels, and the user may edit it before it is sent.

Each returned entry is normalized as:

```rust
pub struct LogEntry {
    pub timestamp_ns: String,
    pub line: String,
    pub parsed: bool,
    pub masked: bool,
    pub fields: Option<BTreeMap<String, String>>,
    pub trace_id: Option<String>,
}
```

`timestamp_ns` keeps Loki's nanosecond value verbatim as a string; converting it
to a float would silently lose precision. A line that parses as a JSON object
yields `parsed: true` with its `fields` populated. A line that does not parse
yields `parsed: false` and the original text, and is counted in the result's
`unparsed_count` so the UI can say plainly that those lines were not masked.
`masked` is true only when at least one value was actually replaced, so a
parsed line carrying nothing sensitive reports `parsed: true, masked: false`.

`trace_id` is populated only from an explicit `trace_id`, `traceID` or
`traceparent` field of a parsed object. No regular expression scans unstructured
text for identifier-shaped substrings, and no correlation is inferred from
timing or proximity.

### Tempo

`tempo.trace` is a `ResourceRead` command reading `GET /api/traces/{traceID}`.
The trace ID must be exactly 32 characters matching `[0-9a-f]`, the W3C Trace
Context format, and is validated before it is placed in the URL path. Uppercase
hexadecimal, shortened IDs and any other input are rejected, and a rejected ID
never reaches the provider. `tempo.health` probes Tempo's fixed readiness endpoint.

A trace is normalized into span summaries carrying `trace_id`, `span_id`,
`parent_span_id`, `name`, `service_name`, `start_time_unix_nano`,
`duration_nano` and `status`.

Span attributes are returned through an explicit allow list, never a deny list.
Sensitive content in attributes usually hides in the value under an innocuous
key — a token in an `http.url` query string, literals in a `db.statement`, a
bearer header under `http.request.header.authorization` — so a name-based deny
list would pass all of them through while labelling the result masked. The
allow list inverts that default: an unrecognized key is dropped.

Sprint 9 allows exactly these OpenTelemetry semantic-convention keys, whose
meaning and value shape are defined by the specification rather than by the
instrumented application:

```text
http.status_code   http.method   http.route
rpc.service        rpc.method    db.system
exception.type     otel.status_description
```

`db.system` names the database engine and is not `db.statement`. Adding a key to
this list is a deliberate change with its own test, not a configuration option.

### IPC contract rule

Every enum crossing the IPC boundary declares explicit `#[serde(rename = ...)]`
values, is covered by a Rust test asserting its exact serialized JSON, and its
React fixture is copied from that asserted shape. Sprint 8 shipped a contract
mismatch that every test passed over because the fixture was written to match
what the UI read rather than what the backend emitted. This rule exists to make
that failure mode impossible to repeat silently.

## UI behavior

The `Observability` route keeps its Sprint 8 panels and gains two more. A single
`TimeContext` of `{ start, end, source }` lives at the workspace level:
selecting an alert sets it from `startsAt` to `endsAt`, using the present moment
as the end of a still-firing alert. Editing the range switches `source` to
`manual` and the workspace states visibly that the window no longer follows the
alert. No panel keeps a time range of its own; that single context is what makes
the exit criterion true rather than merely convenient.

`ObservabilityWorkspace.tsx` is split into `ui/src/observability/` with
`AlertsPanel`, `MetricsPanel`, `LogsPanel`, `TracePanel`, `GrafanaPanel` and
`TimeRangeControl`, leaving the workspace file as a composition root holding
only the time context and the current selection. The file is already long
enough that adding two panels in place would make it unreadable, and this is the
file the sprint works in.

The log panel renders entries with their stream labels, timestamp and line, a
banner naming the number of unmasked unparsed lines, and the loading, empty,
unavailable and malformed states in Thai and English. An entry carrying a
`trace_id` shows a control that opens the trace panel; when no entry in the
window carries one, the trace panel says so explicitly instead of rendering an
empty frame.

The trace panel renders spans as an accessible table ordered parent before
child, with indentation for depth and a duration column.

## Safety and error handling

- Every Sprint 9 request is a GET to a fixed provider path.
- Masking runs in Rust before serialization. A field value the deny list covers
  cannot reach React, a log fixture, a diagnostic or a test snapshot.
- Trace IDs are validated before URL construction.
- `limit`, time range and direction are bounded and validated server side; the
  UI may validate too, but backend validation is authoritative.
- `X-Scope-OrgID` is sent only when configured, and never carries a credential.
- Command-envelope, membership, capability, scope and both egress-policy checks
  (`ExternalIntegration` before the request, `Ui` before returning) are
  mandatory for all three new commands.
- Provider failures return a sanitized service or status message, never a
  response body, an authorization header or a credential reference.

## Verification and acceptance

Rust tests use local mock HTTP servers to verify:

- exact GET paths, query parameters and authorization headers for Loki and
  Tempo, including the presence and absence of `X-Scope-OrgID`;
- nanosecond timestamp preservation and stream label mapping;
- masking of covered fields, the unparsed path, and `unparsed_count`;
- `trace_id` extraction from explicit fields only, including negative cases for
  identifier-shaped text in unparsed lines;
- trace ID validation rejecting non-hexadecimal and wrong-length input;
- span summary mapping, and that an attribute outside the allow list — including
  `http.url`, `db.statement` and a custom application key — never appears in a
  serialized span;
- the full authorization and policy table from Sprint 8 applied to all three new
  commands, plus a success case proving no credential or credential reference
  appears in a serialized result.

React tests use fixtures copied from the asserted Rust shapes to verify the
whole journey: select an alert, see the time context set from it, run a log
query, see the masking banner, open a trace from a log entry, and read the span
table — plus localization, keyboard access and the honest empty and error
states.

Codex implements under Orca orchestration supervision; Claude performs an
independent review and QA pass, running `cargo test --workspace`,
`cargo clippy --workspace --all-targets -- -D warnings`, and the frontend test,
typecheck, lint, build and format checks under Node 24.

The Sprint 9 exit criterion is satisfied only when the fixture journey works
without live infrastructure, with no mutating request, and with the log and
trace panels bound to the same time context as the metric panel.
