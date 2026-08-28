# Sprint 13 Signal Normalization, Security Findings and Correlation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Normalize Alertmanager alerts, Prometheus anomalies, health-check observations and replayable Trivy, Falco, Kyverno and OPA Gatekeeper findings into the canonical source-preserving `Signal` contract, then emit deterministic, explainable, read-only correlation candidates with evidence closure.

**Architecture:** Extend the existing domain `Signal` and `EvidenceSourceKind` contracts once, retain every admitted post-policy source record in a local append-only ledger, and use a deep adapter seam for source-specific fixture parsing. A pure correlation module applies suppression, source-aware deduplication, explicit event-time windows, exact target grouping and the existing Sprint 12 topology resolver before a capability-scoped Tauri read projection reaches the Operations Console.

**Tech Stack:** Rust 2021, Tauri 2, Tokio-compatible existing modules, Serde, Chrono, SQLite/local-first state already in the repository, React 18, TypeScript, Vite, Vitest, Testing Library and the existing ThalassaOps design system.

**Spec:** docs/design/sprint-13-signal-correlation.md

## Global Constraints

- There is one type per concept. The existing `thalassa_domain::Signal` is the canonical common envelope. Do not create `SignalEnvelope`, `NormalizedSignal`, source-specific Signal aliases, a second finding type or a UI-only candidate model. Reuse `ResourceScope`, `EvidenceRef`, `EvidenceRedaction`, `DrillDownTarget`, `DrillDownReference`, `TimeWindow`, `NumberUnit`, `SourceStatus`, `ConsoleSeverity`, `HealthCheckOutcome` and the Sprint 12 topology types.
- Extend the existing `EvidenceSourceKind` enum with the explicit `trivy`, `falco`, `kyverno` and `opa_gatekeeper` wire values. Do not create a private security-source enum or use a free-form source string in a Signal, finding, reason or fixture.
- Rust numeric fields are `f64` and TypeScript numeric fields are `number`. Reject NaN, positive infinity and negative infinity with typed errors before IPC serialization. CVSS is finite and within `0.0..=10.0`; correlation metrics use finite `f64` with `NumberUnit::Count`, not the Sprint 11 string-valued `CriticalNumber`. Correlation windows are limited to 86,400 seconds and allowed lateness to 21,600 seconds; these bounds are validation limits, not fabricated source values.
- Signal kinds, states, finding severities, exploitability, candidate statuses, suppression states, window states and correlation reasons are typed enums. React maps stable wire values to English and Thai i18n keys; Rust never emits user-facing English sentences.
- Absent source data is `Option`/`null`, an explicit unavailable source status or an omitted record. Empty strings are never placeholders and fabricated timestamps, targets, severities, exploitability or native links are forbidden.
- The complete post-policy source record, including unknown fields, is retained in the local append-only source-record ledger. Normalized fields are typed indexes over that record, never a lossy flattening, paraphrase or replacement. Every candidate lists every contributing Signal ID, every Signal points to its source record and evidence IDs, and candidate/reason evidence is closed over the snapshot.
- Every displayed number, Signal, finding, reason, candidate, topology path and metric has verified evidence IDs and a typed drill-down reference. No returned ID may resolve outside the current workspace or to unverified evidence.
- No credential, token, ARN, account ID, subscription ID, authorization header, cookie, pagination cursor or credential reference may enter a normalized Signal, finding, reason, log, committed fixture, source record retained for correlation or serialized result. Safe identity validation rejects unsafe values instead of blanking them.
- Adapters in this sprint consume committed synthetic replay fixtures or already supplied provider-neutral Sprint 11 values. Do not provision infrastructure, run Terraform/OpenTofu, capture live cloud or cluster data, invoke a provider CLI, add an outbound network integration or add a source query path.
- Topology grouping delegates to the existing Sprint 12 engine in `src-tauri/src/topology/`. Do not reimplement graph traversal, ownership resolution, path confidence or topology node identity.
- Correlation emits candidates only. Do not add an Incident entity, IncidentStatus, disposition, responder role, assignment, notification, comment, incident write, mutation, remediation or incident lifecycle command. Reuse the Sprint 11 queue projection only as an existing read-only context if required.
- Correlation reasons are structural associations, not causal claims. `TopologyPath.kind` and topology correlation qualification remain `probable_structural`; never add or render `root_cause`, `caused_by`, `confirmed_dependency`, `proven_causal` or a probability score.
- New IPC commands are exactly `correlation.snapshot` (`WorkspaceRead`/`Read`) and `correlation.evidence` (`ResourceRead`/`Read`). There is no ingest, adapter-trigger, provider-query, correlation-write, correlation-act or maintenance-window mutation command.
- Every command follows the established authorization order: exact descriptor and capability, envelope scope, active membership/principal/workspace grant/role permission, request parsing and limits, source/local policy, adapter and projection work, evidence-ID validation, then `Ui` and `AuditLog` egress policy before serialization. Typed failures map to distinct existing `IpcErrorCode` values.
- Existing recursive masking and policy classification remain authoritative. Unparsed evidence is not marked masked; Restricted or unverified data fails closed. A policy denial never degrades a source to an unattributed or healthy candidate.
- Do not make adapter output or snapshots depend on wall-clock time, input order, background schedulers or provider calls. The fixture clock, explicit request evaluation time, sorted IDs and stable digest rules must produce byte-identical output for identical inputs and policy version.
- Run `npm ci` before any frontend gate. A gate that cannot run is blocked and must be reported; it is not a passing gate.
- The exact sprint exit criterion is: "Alerts, anomalies and normalized vulnerability findings can be correlated into explainable candidates without losing original source references."

## File map and parallel handoff

Task 2 is the synchronization point for domain and TypeScript contracts, IPC descriptors and copied security fixtures. After Task 2:

- the backend worker owns Tasks 3–7: `crates/thalassa-domain`, `crates/thalassa-ipc`, `src-tauri/src/correlation`, `src-tauri/src/app/correlation.rs`, migrations and Rust tests;
- the React worker owns the UI portion of Task 8: `ui/contracts/ipc.ts`, `ui/src/correlation`, Operations Console composition, locale files, styles and frontend tests; it consumes the copied fixture without importing Rust code; and
- Task 9 starts only after the contract, source, dedup/window, grouping, suppression, IPC and UI tests are green. No worker changes a field name, enum wire value, nullability rule, source identity rule or fixture ID without updating the design and copied fixture in the same change.

### Task 2: Define the canonical Signal/finding contracts and deterministic fixture catalog

**Files:**

- Modify: `crates/thalassa-domain/src/lib.rs` — evolve the existing `Signal`, extend `EvidenceSourceKind`, and add the canonical `SignalKind`, `SignalState`, `SignalTargetKind`, `SignalTarget`, `SourceRecordRef`, `SignalPayload`, `VulnerabilityFinding`, `FindingAssetKind`, `FindingAsset`, `FindingSeverity`, `Exploitability`, `CorrelationRequest`, `CorrelationWindowState`, `CorrelationWindow`, `CorrelationReasonKind`, `CorrelationQualification`, `CorrelationReason`, `CandidateStatus`, `CorrelationCandidate`, `CorrelationMetricKey`, `CorrelationMetric`, `CorrelationSummary`, `CorrelationSnapshot`, `CorrelationEvidenceRequest`, `SuppressionKind`, `SuppressionState`, `SuppressionRule`, `MaintenanceWindowReason` and `MaintenanceWindow` contracts from the design.
- Create: `crates/thalassa-domain/tests/signal_correlation_contracts.rs` — assert every enum wire value, full JSON shape, Option/null behavior, finite-number validation and evidence/drill-down invariants.
- Modify: `crates/thalassa-ipc/src/lib.rs` — add `correlation_snapshot_descriptor()` and `correlation_evidence_descriptor()` as the only command metadata source.
- Modify: `crates/thalassa-ipc/tests/contracts.rs` — assert command names, capabilities, permissions and descriptor scopes.
- Create: `src-tauri/src/correlation/mod.rs` — declare the correlation module and re-export domain contracts without introducing a second model.
- Create: `src-tauri/src/correlation/fixtures.rs` — define the internal replay fixture catalog, shared fixture clock and safe synthetic input values.
- Modify: `src-tauri/src/lib.rs` — export `pub mod correlation` for integration tests.
- Create: `docs/superpowers/fixtures/2026-08-28-capture/security/trivy.json` — synthetic Trivy vulnerability result.
- Create: `docs/superpowers/fixtures/2026-08-28-capture/security/falco.json` — synthetic Falco runtime event.
- Create: `docs/superpowers/fixtures/2026-08-28-capture/security/kyverno.json` — synthetic Kyverno policy report result.
- Create: `docs/superpowers/fixtures/2026-08-28-capture/security/gatekeeper.json` — synthetic OPA Gatekeeper violation.
- Modify: `ui/contracts/ipc.ts` — mirror the Rust wire contracts exactly, including four source values and correlation drill-down values.
- Create: `ui/src/correlation/correlation-fixtures.ts` — copied, typed fixture snapshot for frontend work.
- Create: `ui/src/correlation/correlation-contracts.test.ts` — copied-fixture field, enum, nullability and finite-number assertions.

**Interfaces:**

- Consumes: existing `Signal` call sites, `EvidenceSourceKind`, `ResourceScope`, `EvidenceRef`, `EvidenceRedaction`, `DrillDownTarget`, `DrillDownReference`, `TimeWindow`, `NumberUnit`, `SourceStatus`, `ConsoleSeverity`, `HealthCheckOutcome` and Sprint 12 `TopologyPath`.
- Produces: the exact domain and TypeScript shapes in the design, `correlation_snapshot_descriptor()`, `correlation_evidence_descriptor()`, `ReplayableSignalFixture`, `correlation_fixture_catalog()` and the fixed fixture clock `2026-08-28T09:00:00Z`.
- The Rust `Signal` remains the only common envelope. Its `SignalPayload` variant must agree with `Signal.kind`; security findings are nested in `SignalPayload::SecurityFinding` and cannot exist without `Signal.source_record`.

**Tests to add:**

- literal JSON assertions for every new enum, all four security source kinds, all signal payload variants, both commands and the correlation drill-down destination;
- round-trip serialization of a complete snapshot with an alert, anomaly, finding, source references, reason, topology path, candidate, metric, suppression state and late-arrival fields;
- explicit null assertions for absent observed/ingested time, native ID, revision, target selector, display name, artifact digest, severity, exploitability, CVSS and native URL;
- rejection before serialization for non-finite anomaly values, CVSS values outside `0.0..=10.0`, invalid windows, empty IDs, missing evidence and candidate IDs that do not resolve to snapshot Signals;
- TypeScript field-name/nullability parity against the copied snapshot and `number`/finite assertions for all numeric fields; and
- a fixture byte scan for credential, token, ARN, account, subscription and pagination-cursor keys or values before any adapter test runs.

- [ ] **Step 1: Write the failing contract tests**

Create `signal_correlation_contracts.rs` with representative assertions and one assertion for every enum member in the design:

```rust
#[test]
fn signal_and_correlation_wire_values_are_stable() {
    assert_eq!(serde_json::to_value(SignalKind::SecurityFinding).unwrap(), json!("security_finding"));
    assert_eq!(serde_json::to_value(SignalState::Observed).unwrap(), json!("observed"));
    assert_eq!(serde_json::to_value(EvidenceSourceKind::OpaGatekeeper).unwrap(), json!("opa_gatekeeper"));
    assert_eq!(serde_json::to_value(FindingSeverity::Critical).unwrap(), json!("critical"));
    assert_eq!(serde_json::to_value(Exploitability::KnownExploit).unwrap(), json!("known_exploit"));
    assert_eq!(serde_json::to_value(CorrelationReasonKind::TopologyRelation).unwrap(), json!("topology_relation"));
    assert_eq!(serde_json::to_value(CorrelationQualification::ProbableStructural).unwrap(), json!("probable_structural"));
    assert_eq!(serde_json::to_value(SuppressionKind::RuleAndMaintenanceWindow).unwrap(), json!("rule_and_maintenance_window"));
}
```

Add a full `CorrelationSnapshot` round trip and assertions that `Option` fields serialize as JSON `null`, all IDs are explicit, and `SignalPayload::SecurityFinding` carries a `VulnerabilityFinding` rather than a second Signal envelope.

- [ ] **Step 2: Run focused contract tests and record the expected failure**

Run:

```bash
cargo test -p thalassa-domain --test signal_correlation_contracts
cargo test -p thalassa-ipc --test contracts
```

Expected: FAIL because the new contract variants, types and descriptors do not yet exist.

- [ ] **Step 3: Add the domain contracts exactly once**

Evolve `Signal` from the Sprint 11 generic fields to the design-named source-preserving envelope. Add explicit Serde renames, `PartialEq` rather than `Eq` to values containing `f64`, and typed validation methods that reject non-finite numbers, missing evidence, invalid IDs and inconsistent payload/kind pairs. Preserve existing domain types instead of defining wrappers or aliases.

Add the four security variants to `EvidenceSourceKind` without changing the existing wire values. Use `FindingAsset.target` for a safe canonical target; do not add account, subscription, registry-credential or provider locator fields.

- [ ] **Step 4: Add the internal replay catalog and safe fixtures**

Define `ReplayableSignalFixture` with fixture key, source kind, workspace scope, recorded JSON, optional event/ingest times and admitted evidence. Keep the fixture clock explicit. Add the four synthetic source payloads with stable source IDs, safe image/package/policy/rule names, exact targets, evidence IDs and at least one unknown field in each record to prove retention. Include operational alert/anomaly and shared grouping fixtures in the catalog, not in committed provider captures.

Reject fixture keys/values that match the forbidden-data scanner. Do not load environment credentials, call a provider, or construct a URL from a fixture value.

- [ ] **Step 5: Add descriptors and mirror the contract**

Implement only:

```rust
pub fn correlation_snapshot_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "correlation",
        "snapshot",
        Capability::WorkspaceRead,
        Permission::Read,
    )
}

pub fn correlation_evidence_descriptor() -> CommandDescriptor {
    CommandDescriptor::new(
        "correlation",
        "evidence",
        Capability::ResourceRead,
        Permission::Read,
    )
}
```

Add exact snake_case TypeScript unions and object fields to `ui/contracts/ipc.ts`. Keep `Option` as `null` in the copied fixture, all metric/CVSS/anomaly values as `number`, and do not add React-only aliases.

- [ ] **Step 6: Run contract suites**

Run:

```bash
cargo test -p thalassa-domain --test signal_correlation_contracts
cargo test -p thalassa-ipc --test contracts
npm ci
npm test -- ui/src/correlation/correlation-contracts.test.ts
npm run typecheck
```

Expected: PASS, with Rust JSON and TypeScript fixture fields, enum strings, nullability and numeric types identical.

- [ ] **Step 7: Commit the synchronization point**

```bash
git add crates/thalassa-domain/src/lib.rs crates/thalassa-domain/tests/signal_correlation_contracts.rs crates/thalassa-ipc/src/lib.rs crates/thalassa-ipc/tests/contracts.rs src-tauri/src/correlation src-tauri/src/lib.rs docs/superpowers/fixtures/2026-08-28-capture/security ui/contracts/ipc.ts ui/src/correlation/correlation-fixtures.ts ui/src/correlation/correlation-contracts.test.ts
git commit -m "feat: define signal correlation contracts and fixtures"
```

**Acceptance criteria:**

- Rust and TypeScript expose one common `Signal` envelope and one finding payload with identical field names, enum wire values, nullability and numeric types.
- `EvidenceSourceKind` includes `trivy`, `falco`, `kyverno` and `opa_gatekeeper` without a private adapter source enum.
- Contract validation rejects non-finite numbers, invalid CVSS and evidence/reference invariant violations before serialization.
- The four fixtures are deterministic, synthetic, scope-bound, evidence-backed and contain no forbidden data, live response or credential material.
- The two descriptor functions are the only new command metadata and use `WorkspaceRead`/`Read` and `ResourceRead`/`Read` exactly.

### Task 3: Retain source records and normalize existing operational signals

**Files:**

- Create: `src-tauri/src/correlation/source_records.rs` — implement the local append-only source-record ledger, canonical masked JSON retention, source identity conflict detection and evidence closure.
- Create: `src-tauri/src/correlation/adapters/mod.rs` — define `SignalAdapter`, shared admission, source-safe identity and typed adapter errors.
- Create: `src-tauri/src/correlation/adapters/operational.rs` — normalize Sprint 11 `NormalizedAlert`, `AnomalySignal` and `HealthCheckResult` values without re-querying providers.
- Create: `src-tauri/migrations/0003_signal_records.sql` — add the local source-record table keyed by source kind/content digest/revision with append-only semantics and a secondary native-identity/revision conflict index.
- Modify: `src-tauri/src/correlation/mod.rs` — expose source retention and operational normalization to the aggregator while keeping provider-specific parsing private.
- Modify: `src-tauri/src/lib.rs` — register the migration/module using existing local-state conventions.
- Create: `src-tauri/tests/signal_adapters.rs` — source-record, operational mapping, scope, redaction and no-network tests.

**Interfaces:**

- Consumes: `NormalizedAlert`, `AnomalySignal`, `HealthCheckResult`, `ResourceScope`, existing evidence builders, existing masking/classification/policy modules and `ReplayableSignalFixture`.
- Produces: `SourceRecordStore::retain(...)`, `SignalAdapter::normalize(...)`, operational Signals with `SourceRecordRef`, stable Signal IDs, source status and verified evidence IDs.
- The adapter seam is:

```rust
pub trait SignalAdapter {
    fn source_kind(&self) -> EvidenceSourceKind;

    fn normalize(
        &self,
        fixture: &ReplayableSignalFixture,
        records: &mut SourceRecordStore,
    ) -> Result<Vec<Signal>, SignalAdapterError>;
}
```

`SourceRecordStore` accepts only already classified/masked local values, stores the complete post-policy JSON object or array including unknown fields, and never overwrites a retained row. A same-key conflicting payload returns `AmbiguousSourceIdentity`; byte-identical replay may reuse an index entry while preserving evidence reachability.

**Tests to add:**

- alert mapping to active/cleared Signal states, safe fingerprint/native identity, target resolution and source-record digest;
- anomaly mapping with finite observed/comparison values and typed condition, preserving rule/metric/query evidence in the source record;
- health-check mapping for healthy/degraded/unavailable/timed-out and skipped outcomes, with skipped records retained but excluded from active candidate edges;
- unknown source fields surviving mask-and-retain as structurally faithful JSON and producing a stable digest/reference;
- absent severity/time/target becoming `None` and source status rather than an empty string, fabricated target or inferred grouping key;
- scope containment, classification, redaction and evidence verification before ledger admission;
- conflicting native identity rejection, duplicate replay idempotence and complete source/evidence lookup; and
- an adapter call counter proving normalization performs no HTTP, provider CLI, credential lookup or recursive Tauri command.

- [ ] **Step 1: Write source-retention and operational adapter tests first**

Add a fixture containing an alert with an unknown `vendor_extension` object, an anomaly with finite values and a health check with an explicit skipped outcome. Assert that each emitted Signal carries its own source kind, content digest, evidence IDs and drill-down target, and that the retained JSON has the unknown field after masking. Add tests for an unverified EvidenceRef, an out-of-scope fixture and a conflicting `(source_kind, native_id, revision)` record.

- [ ] **Step 2: Run the focused adapter test and record the expected failure**

Run:

```bash
cargo test -p thalassaops --test signal_adapters
```

Expected: FAIL because the source ledger, migration and operational adapter seam do not exist.

- [ ] **Step 3: Add the append-only source-record migration/store**

Create the table with `source_kind`, optional `native_id`/`revision`, `content_digest`, scope, optional observed/ingested timestamps, redacted payload JSON, evidence IDs and retention metadata. Enforce uniqueness for the source-kind/content-digest/revision identity and, when a native ID is present, reject a different content digest under the same source-kind/native-ID/revision secondary index as a typed `AmbiguousSourceIdentity` error. Keep raw provider data out of logs and typed error details. Route local storage through the existing `EgressDestination::LocalStorage` policy and fail closed for Restricted/unverified data.

Canonicalize JSON only for digest/index comparison; retain the object/array shape and all unknown fields in the stored post-policy record. Do not flatten into a message or store a pre-policy copy.

- [ ] **Step 4: Implement shared adapter admission and operational mappings**

Implement masking/classification/evidence checks, safe identity validation, deterministic Signal ID derivation and payload/kind validation in `adapters/mod.rs`. Map `NormalizedAlert`, `AnomalySignal` and `HealthCheckResult` exactly as specified in the design. Keep unresolved targets explicit and prevent them from creating a candidate later. Record source status for malformed/out-of-scope/unverified inputs while preserving valid sources in the snapshot.

- [ ] **Step 5: Verify unknown-field fidelity and forbidden-data boundaries**

Run:

```bash
cargo test -p thalassaops --test signal_adapters source_record
cargo test -p thalassaops --test signal_adapters operational
cargo test -p thalassaops --test signal_adapters forbidden_data
```

Expected: PASS, including source JSON shape/digest stability, explicit absence, typed errors, no secret-like values and no outbound calls.

- [ ] **Step 6: Commit the source-adapter layer**

```bash
git add src-tauri/src/correlation/source_records.rs src-tauri/src/correlation/adapters src-tauri/src/correlation/mod.rs src-tauri/migrations/0003_signal_records.sql src-tauri/src/lib.rs src-tauri/tests/signal_adapters.rs
git commit -m "feat: retain source records and normalize operational signals"
```

**Acceptance criteria:**

- Every admitted operational Signal is a typed index over a retained, post-policy, structurally faithful source record; unknown fields and all source evidence remain reachable.
- Alert, anomaly and health-check values map without provider calls, unsafe defaults or lossy user-facing text. Missing data is explicit and cannot create a false grouping edge.
- The append-only ledger detects identity conflicts, retains deterministic duplicate references and uses existing classification, masking and local-storage policy checks.
- No credential, token, ARN, account/subscription identifier, cursor or raw provider error enters the Signal, source ledger, log or serialized output.
- Adapter errors and source-level unavailable states remain typed and never turn an invalid source into a healthy zero.

### Task 4: Implement replayable Trivy, Falco, Kyverno and OPA Gatekeeper adapters

**Files:**

- Create: `src-tauri/src/correlation/adapters/trivy.rs` — parse the synthetic Trivy result, retain all source fields and emit a container-image finding.
- Create: `src-tauri/src/correlation/adapters/falco.rs` — parse the synthetic Falco event and emit an exact runtime-resource finding.
- Create: `src-tauri/src/correlation/adapters/kyverno.rs` — parse the synthetic Kyverno policy report and emit a policy-subject/Kubernetes-resource finding.
- Create: `src-tauri/src/correlation/adapters/gatekeeper.rs` — parse the synthetic OPA Gatekeeper violation and emit a policy-subject/Kubernetes-resource finding.
- Modify: `src-tauri/src/correlation/adapters/mod.rs` — register the four adapters behind the shared `SignalAdapter` seam and shared admission checks.
- Modify: `src-tauri/src/correlation/fixtures.rs` — expose four source fixtures and a mixed operational/security correlation scenario.
- Create: `src-tauri/tests/security_adapters.rs` — source-specific mappings, malformed payload, identity, evidence and forbidden-data tests.
- Modify: `ui/src/correlation/correlation-fixtures.ts` — copy the four-source fixture snapshot used by the UI contract tests.

**Interfaces:**

- Consumes: one `ReplayableSignalFixture` per source, admitted evidence, current workspace scope and the shared source-record store.
- Produces: `Signal { kind: SecurityFinding, source: Trivy|Falco|Kyverno|OpaGatekeeper, payload: SecurityFinding { finding: VulnerabilityFinding { ... } } }` with a matching `SourceRecordRef` and evidence closure.
- The adapters expose source-specific identity extraction internally only. They return the provider-neutral `Signal` contract and never expose a source client, query, token or provider-native object to React.

**Tests to add:**

- Trivy: vulnerability ID/package/path/image identity, container-image target, explicit high severity, finite CVSS and source evidence;
- Falco: rule/event identity, runtime resource target, explicit priority mapping, event timestamp and evidence containing retained output fields;
- Kyverno: policy/rule/resource/path identity, policy-subject or Kubernetes-resource target, explicit severity and policy-report evidence;
- OPA Gatekeeper: constraint/template/resource/path identity, policy-subject or Kubernetes-resource target and violation evidence;
- `source` matching parent Signal and four-source-only validation for initial security findings;
- absent exploitability/CVSS staying `None`, explicit source unknown becoming `Some(Unknown)`, finite CVSS bounds and no inferred severity;
- unknown fields retained in each source record, all finding evidence linked to the parent Signal, and source native identity/digest stability;
- malformed/unsupported source schema, ambiguous target, unsafe identity, unverified evidence and out-of-scope source returning typed source status/error; and
- complete fixture/key/value scans for credentials, tokens, ARNs, account/subscription IDs and cursors.

- [ ] **Step 1: Write one failing contract test per adapter**

For each source, deserialize its committed fixture, call the adapter with a deterministic source-record store, and assert the fields that source contributes. Include an unknown field in every fixture and assert it is present in the retained JSON, not copied into a free-form reason. Add a test that changes only a source value unsupported by the adapter and proves the field remains in the ledger.

- [ ] **Step 2: Run focused security tests and record the expected failure**

Run:

```bash
cargo test -p thalassaops --test security_adapters
```

Expected: FAIL because the four adapters and finding mappings do not exist.

- [ ] **Step 3: Implement Trivy and Falco adapters**

In `trivy.rs`, derive a source-qualified stable identity from vulnerability ID, package/path and safe image/asset identity. Map scanner severity and finite CVSS; map exploitability only when explicitly provided. Use a `ContainerImage` asset target or a validated deployment/image target supplied by the fixture; never use a registry credential locator.

In `falco.rs`, derive identity from rule, exact runtime target and source event fingerprint while excluding event time from the logical key. Map explicit priority using the fixed typed table and keep absent exploitability absent. An ambiguous runtime target retains a safe source record and typed source status but rejects the finding because `FindingAsset.target` is required; it cannot receive a fabricated target or form a grouping claim.

- [ ] **Step 4: Implement Kyverno and Gatekeeper adapters**

In `kyverno.rs`, use policy/rule, exact namespace/kind/name and violation path. Map explicit policy-report severity and retain the complete report fields. In `gatekeeper.rs`, use constraint/template, exact resource identity and violation path, and map only a safe explicit source severity annotation. Both adapters reject fabricated targets and keep exploitability absent unless the source explicitly provides it.

- [ ] **Step 5: Validate the source-record/evidence closure**

Ensure each adapter retains before constructing typed facts, validates scope/classification/redaction/evidence, derives a deterministic Signal ID and validates `SignalPayload::SecurityFinding` against the parent source. Ensure all finding evidence IDs are present in the parent `Signal.evidence_ids` and every unknown source field remains in the ledger.

- [ ] **Step 6: Run source-specific suites and commit**

Run:

```bash
cargo test -p thalassaops --test security_adapters trivy
cargo test -p thalassaops --test security_adapters falco
cargo test -p thalassaops --test security_adapters kyverno
cargo test -p thalassaops --test security_adapters gatekeeper
cargo test -p thalassa-domain --test signal_correlation_contracts
```

```bash
git add src-tauri/src/correlation/adapters src-tauri/src/correlation/fixtures.rs src-tauri/tests/security_adapters.rs ui/src/correlation/correlation-fixtures.ts
git commit -m "feat: add replayable security signal adapters"
```

**Acceptance criteria:**

- All four committed fixtures normalize through one `SignalAdapter` interface into typed security findings with source, asset, severity, exploitability and evidence references.
- Every adapter retains its complete masked source record and unknown fields; no normalization path flattens or paraphrases the originating payload.
- Source/asset identities and evidence are stable, safe and workspace-scoped. Invalid or ambiguous source data fails typed admission rather than receiving a fabricated default.
- Severity and CVSS preserve source semantics, exploitability absence is honest, and no finding is promoted to an incident severity.
- Tests demonstrate no network, provider CLI, credential lookup, live capture or forbidden value in fixtures or serialized outputs.

### Task 5: Add source-aware deduplication and event-time correlation windows

**Files:**

- Create: `src-tauri/src/correlation/dedup.rs` — canonical masked identity tuples, source-qualified dedup keys, duplicate association handling and stable Signal/candidate anchors.
- Create: `src-tauri/src/correlation/window.rs` — `CorrelationRequest` validation, half-open membership, watermark/state calculation and late-arrival reopen/recompute behavior.
- Modify: `src-tauri/src/correlation/mod.rs` — compose deduplication and window assignment before grouping.
- Create: `src-tauri/tests/signal_dedup.rs` — source tuple, key, revision, cross-source and retention tests.
- Create: `src-tauri/tests/signal_windows.rs` — boundaries, missing/future event time, watermark states, late arrival and deterministic ordering tests.

**Interfaces:**

- Consumes: admitted canonical Signals, explicit `CorrelationRequest`, fixture clock, source kind/native identity/content digest and existing `TimeWindow`.
- Produces: optional `Signal.dedup_key`, deterministic `CorrelationWindow`, eligible Signal set, late Signal IDs, `CorrelationWindowState` and stable candidate anchors.
- The key format is `dedup:v1:<source-kind>:<signal-kind>:<stable-identity-digest>`. The digest is field-labelled and canonical after masking; the tuple is never exposed to React.

**Tests to add:**

- Alertmanager fingerprint, Prometheus rule/metric/condition/target, Trivy vulnerability/package/path/asset, Falco rule/target/event fingerprint, Kyverno policy/rule/resource/path, Gatekeeper constraint/template/resource/path and health schedule/probe tuples;
- key exclusion of event time, ingest time, evidence IDs, severity, state and free-form message;
- same source identity producing one association key, different content revisions both retained, cross-source equal text not deduplicated and missing identity resulting in `None`;
- conflicting native identity producing a typed error and no arrival-order selection;
- `[start,end)` membership including start and excluding end, explicit `evaluated_at`, 86,400-second window/21,600-second lateness bounds and watermark states `open`, `ready_to_finalize`, `finalized` and `reopened`;
- missing observed time retained but ineligible, future/out-of-range time retained but excluded, and late in-range ingestion reopening/recomputing the same candidate anchor with `late_signal_ids`; and
- input-order-independent keys, window states, Signal order and candidate anchor selection.

- [ ] **Step 1: Write key and window tests first**

Use two records that differ only in timestamp, evidence, severity, state and message and assert the stable key is unchanged. Use two source kinds with identical vulnerability text and assert distinct keys. Test exact timestamps at `start`, `end`, `evaluated_at`, `end + lateness` and one late ingestion after finalization.

- [ ] **Step 2: Run focused dedup/window tests and record the expected failure**

Run:

```bash
cargo test -p thalassaops --test signal_dedup
cargo test -p thalassaops --test signal_windows
```

Expected: FAIL because key construction and explicit window state do not exist.

- [ ] **Step 3: Implement canonical source-aware keys**

Sort identity tuple fields and serialize with explicit field labels before hashing. Use the source-native identity when safe and complete; otherwise use the exact safe target only where the source contract permits. Leave `dedup_key` absent when identity would require guesswork. Never hash or expose forbidden values. Preserve all Signals and source-record references even when the dedup index coalesces duplicate association edges.

- [ ] **Step 4: Implement request validation and half-open windows**

Parse RFC3339 timestamps, require `start < end`, `evaluated_at >= start`, `window <= 86,400` seconds, `allowed_lateness <= 21,600` seconds and explicit safe limits. Compute watermark as `evaluated_at - allowed_lateness_seconds`; derive `Open`, `ReadyToFinalize`, `Finalized` and `Reopened` from the request and prior finalization state. Do not call the wall clock or rewrite missing/future event time.

- [ ] **Step 5: Implement late-arrival recomputation**

If an in-range Signal is ingested after finalization, reopen the same window, rebuild its component, preserve the stable candidate anchor and add the late Signal ID. If an observed time is outside the range, retain the Signal locally and leave it to its correct window. If no observed time exists, retain explicit absence and do not group by ingestion time.

- [ ] **Step 6: Run suites and commit**

Run:

```bash
cargo test -p thalassaops --test signal_dedup
cargo test -p thalassaops --test signal_windows
cargo test -p thalassa-domain --test signal_correlation_contracts
```

```bash
git add src-tauri/src/correlation/dedup.rs src-tauri/src/correlation/window.rs src-tauri/src/correlation/mod.rs src-tauri/tests/signal_dedup.rs src-tauri/tests/signal_windows.rs
git commit -m "feat: add signal deduplication and correlation windows"
```

**Acceptance criteria:**

- Deduplication is source-qualified, deterministic, opaque at IPC, and never deletes or hides an originating Signal/source record/evidence reference.
- Time semantics are explicit and half-open, with correct watermark states, bounded inputs and no wall-clock/default timestamp behavior.
- Late in-range arrival reopens and recomputes a finalized window, preserves candidate identity and records the late Signal; out-of-range/missing-time Signals are retained but not forced into a group.
- Cross-source observations never deduplicate merely because their text or vulnerability identifier matches.
- All key/window errors are typed and safe to serialize.

### Task 6: Group by exact targets and Sprint 12 topology; emit explainable reasons

**Files:**

- Create: `src-tauri/src/correlation/grouping.rs` — exact Resource/Service/Deployment edge construction, component sorting and reason construction.
- Create: `src-tauri/src/correlation/aggregate.rs` — candidate IDs/status, Signal/reason/topology/evidence closure and snapshot assembly.
- Modify: `src-tauri/src/correlation/mod.rs` — provide the pure correlation orchestration boundary.
- Modify: `src-tauri/src/topology/mod.rs` — expose only the existing bounded topology resolver seam needed by correlation, without moving graph ownership.
- Create: `src-tauri/tests/signal_grouping.rs` — exact grouping, topology delegation, probable-structural reasons, closure and determinism tests.

**Interfaces:**

- Consumes: window-eligible Signals, dedup index, `CorrelationWindow`, existing Sprint 12 `TopologyBuilder`/path types, verified evidence and current `ResourceScope`.
- Produces: `TopologyCorrelationResolver`, `CorrelationReason`, `CorrelationCandidate`, `CorrelationSummary`, `CorrelationSnapshot` and candidate IDs stable across input order and late additions to an existing component.
- The only topology seam is:

```rust
pub trait TopologyCorrelationResolver {
    fn relation(
        &self,
        left: &SignalTarget,
        right: &SignalTarget,
        window: &CorrelationWindow,
    ) -> Result<Option<TopologyPath>, TopologyError>;
}
```

**Tests to add:**

- two Signals sharing the same exact Resource target produce one `SharedResource`/`ExactAssociation` reason;
- exact Service and Deployment target pairs produce distinct reason kinds and target references;
- same names, labels, timestamps, connector IDs or source kinds without exact target identity do not group;
- missing/ambiguous target never creates a group or inferred reason;
- a topology resolver stub receives bounded calls and returned Sprint 12 `TopologyPath` IDs/evidence pass through unchanged;
- topology reason uses `TopologyRelation`/`ProbableStructural` and has no causal/root-cause/probability field;
- disconnected, failed or depth-limited topology resolution produces no fallback edge and a typed SourceStatus limitation;
- candidate contains at least two distinct Signal IDs, every ID resolves to `snapshot.signals`, every reason ID is a candidate subset and every evidence union is closed;
- candidate IDs, ordering, grouping targets, reason ordering and metrics are stable under shuffled input and duplicate association edges; and
- non-finite topology/metric values and invalid references fail before a partial snapshot is serialized.

- [ ] **Step 1: Write failing target/group/reason tests**

Construct a fixture with an Alertmanager alert and Trivy finding on the same exact service/deployment chain, a Prometheus anomaly on a different service, and two targets connected only by a topology path. Assert exact shared-target reasons, a probable structural topology reason and absence of time-only grouping.

- [ ] **Step 2: Run focused grouping tests and record the expected failure**

Run:

```bash
cargo test -p thalassaops --test signal_grouping
```

Expected: FAIL because the grouping, resolver seam and candidate aggregation are not implemented.

- [ ] **Step 3: Build exact target association edges**

Filter by current workspace scope and window membership first. Index `(SignalTargetKind, target.id)` and connect only exact Resource, Service or Deployment pairs. Require a shared source-backed target and at least two distinct Signal IDs. Do not use labels, names, timestamps, source kind, query, connector or time proximity as a reason.

- [ ] **Step 4: Delegate topology relationships to Sprint 12**

Adapt the existing topology engine to `TopologyCorrelationResolver`. Pass only validated backend-issued target IDs and the bounded correlation window. Preserve path ID, direction, termination, confidence, kind and evidence; do not walk adjacency lists or resolve ownership in correlation. If the engine cannot produce a verified bounded path, return no topology edge and a typed source limitation.

- [ ] **Step 5: Emit structural reasons and deterministic candidates**

Create one reason per unique exact association or topology path. Use `ExactAssociation` for shared targets and `ProbableStructural` only for topology. Derive candidate IDs from window range, sorted grouping keys and the smallest stable dedup key or Signal ID. Sort all Signal IDs, targets, reasons, path IDs and evidence IDs before validation.

- [ ] **Step 6: Validate snapshot closure and metrics**

Build `CorrelationSummary` metrics as finite `f64` counts with evidence for exactly the records counted. Validate candidate-to-Signal, reason-to-candidate, topology-path and all evidence references. Fail closed with a typed internal error on invariant violations; never return a partial candidate.

- [ ] **Step 7: Run grouping suites and commit**

Run:

```bash
cargo test -p thalassaops --test signal_grouping
cargo test -p thalassaops --test signal_windows
cargo test -p thalassaops --test signal_adapters
```

```bash
git add src-tauri/src/correlation/grouping.rs src-tauri/src/correlation/aggregate.rs src-tauri/src/correlation/mod.rs src-tauri/src/topology src-tauri/tests/signal_grouping.rs
git commit -m "feat: correlate signals with explainable grouping reasons"
```

**Acceptance criteria:**

- Resource, Service and Deployment grouping uses exact scoped targets only; topology grouping delegates to Sprint 12 and does not reimplement traversal or ownership.
- Every candidate names every contributing Signal and every reason is structural, evidence-backed and subset-closed. No causal/root-cause language or field is introduced.
- Topology paths preserve Sprint 12 probable-structural qualification, path evidence and bounded traversal semantics.
- Candidate IDs, ordering, reasons, topology references and finite count metrics are deterministic and stable under shuffled fixture input.
- Invalid evidence, non-finite values, unresolved IDs and singleton/no-reason components fail closed or remain uncorrelated without fabricated output.

### Task 7: Add suppression and maintenance-window semantics

**Files:**

- Create: `src-tauri/src/correlation/suppression.rs` — rule matching, maintenance-window matching, policy-version/evaluation metadata and deterministic suppression state.
- Modify: `src-tauri/src/correlation/fixtures.rs` — add active maintenance, rule-only, both-match, mixed and all-suppressed fixture components.
- Modify: `src-tauri/src/correlation/aggregate.rs` — evaluate suppression before grouping, preserve suppressed Signals and apply candidate status precedence.
- Create: `src-tauri/tests/signal_suppression.rs` — rule/window boundary, retention, status and policy tests.
- Modify: `crates/thalassa-domain/tests/signal_correlation_contracts.rs` — suppression wire/null/evidence validation where domain coverage is missing.

**Interfaces:**

- Consumes: admitted Signals, `SuppressionRule`, `MaintenanceWindow`, current policy version, explicit evaluation time and exact target/scope selectors.
- Produces: `SuppressionState` with all matching rule/window IDs, `CandidateStatus::Suppressed`/`Provisional`/`Active`, preserved source/evidence/audit metadata and no mutation command.
- Rule matching requires enabled state, containing scope, equal optional source/kind selectors and an exact present target selector. Maintenance matching additionally requires observed event time in `[start,end)`.

**Tests to add:**

- enabled/disabled rule behavior, scope containment, source/kind/target selector equality and null-target match-all semantics;
- maintenance window start-inclusive/end-exclusive boundary, disabled window, missing observed time and out-of-scope target behavior;
- multiple matching rules/windows retained in sorted order and `RuleAndMaintenanceWindow` when both categories match;
- suppressed Signal retaining source kind/native identity/content digest/times/scope/targets/payload/dedup/evidence/matching IDs/evaluation/policy version;
- all-suppressed component emitted as `Suppressed`, mixed component retaining suppressed context but staying `Active`/`Provisional`, singleton suppressed Signal not becoming a candidate;
- late/reopened status taking precedence over active except when all Signals are suppressed, according to the design status rule;
- no IncidentDisposition, incident write or policy mutation path; and
- policy-version/evaluation metadata retained without raw payload or credentials in audit values.

- [ ] **Step 1: Write failing suppression tests**

Create exact-target and null-target rules, windows with Signals at start/end, a mixed component and an all-suppressed component. Assert every matching ID is returned and that no suppressed source record disappears from the snapshot or ledger.

- [ ] **Step 2: Run focused suppression tests and record the expected failure**

Run:

```bash
cargo test -p thalassaops --test signal_suppression
```

Expected: FAIL because matching and candidate status logic are not implemented.

- [ ] **Step 3: Implement pure rule/window matching**

Match scope via existing `ResourceScope::contains`; compare optional selectors exactly; preserve all matching IDs sorted. Use half-open maintenance intervals and never use ingestion time when observed time is absent. Validate policy IDs/version, scope, timestamps and safe selector values before evaluation.

- [ ] **Step 4: Preserve records and assign status**

Evaluate suppression after source admission and before grouping. Keep the complete Signal, source reference, evidence IDs, dedup key and SuppressionState. Emit all-suppressed components for explainability. Apply deterministic precedence: all-suppressed → `Suppressed`; otherwise any late Signal in an open/reopened window → `Provisional`; otherwise → `Active`.

- [ ] **Step 5: Verify policy and incident boundaries**

Run a forbidden-data and command-surface scan proving suppression inputs come only from local policy/fixture state, no UI command creates/edits definitions, no IncidentDisposition is serialized and no raw source payload enters audit metadata.

- [ ] **Step 6: Run suites and commit**

Run:

```bash
cargo test -p thalassaops --test signal_suppression
cargo test -p thalassaops --test signal_grouping
cargo test -p thalassa-domain --test signal_correlation_contracts
```

```bash
git add src-tauri/src/correlation/suppression.rs src-tauri/src/correlation/fixtures.rs src-tauri/src/correlation/aggregate.rs src-tauri/tests/signal_suppression.rs crates/thalassa-domain/tests/signal_correlation_contracts.rs
git commit -m "feat: preserve signal suppression and maintenance context"
```

**Acceptance criteria:**

- Suppression and maintenance matching are exact, bounded, half-open and policy-versioned; all matches remain visible as typed IDs.
- Suppressed Signals retain their original source references, complete post-policy payload, evidence, dedup identity and candidate context.
- All-suppressed, mixed and late/reopened statuses are deterministic and explainable; singleton suppression does not invent a candidate.
- Suppression never becomes an incident disposition, changes source severity, deletes evidence or authorizes mutation.
- Rules/windows and audit metadata contain no forbidden identifiers or raw source payload.

### Task 8: Expose capability-scoped correlation IPC and build the localized read-only UI

**Files:**

- Create: `src-tauri/src/app/correlation.rs` — implement `correlation_snapshot` and `correlation_evidence` Tauri handlers using the exact descriptors and established authorization/policy order.
- Modify: `src-tauri/src/app/mod.rs` or existing command registration — register only the two correlation read commands.
- Modify: `src-tauri/src/main.rs` or existing invoke handler list — expose the two commands without adding an ingest/write/act command.
- Create: `src-tauri/tests/signal_ipc.rs` — descriptor, capability, scope, membership, role, policy, evidence-ID and typed-error tests.
- Modify: `ui/contracts/ipc.ts` — verify the exact request/response contracts and command names.
- Create: `ui/src/correlation/CorrelationWorkspace.tsx` — explicit request lifecycle and composition.
- Create: `ui/src/correlation/CandidateList.tsx` — typed candidate rows and status/source summaries.
- Create: `ui/src/correlation/CandidateDetails.tsx` — reasons, source Signals, suppression and late state.
- Create: `ui/src/correlation/CorrelationEvidencePanel.tsx` — backend-issued evidence-ID lookup and trusted-link handling.
- Create: `ui/src/correlation/CorrelationWorkspace.test.tsx` — request, loading, empty, error, keyboard and evidence interaction tests.
- Create: `ui/src/correlation/correlation.acceptance.test.tsx` — fixture journey covering mixed operational/security candidate, topology reason and suppression.
- Modify: `ui/src/OperationsConsole.tsx` — add signal/correlation summary entry point using existing console patterns.
- Modify: `ui/src/shell.tsx` — add correlation navigation only if the existing shell needs a dedicated read-only surface; do not add an Incident route.
- Modify: `ui/src/locales/en.ts` and `ui/src/locales/th.ts` — identical locale key structures for typed signal/finding/reason/status/suppression/error labels.
- Modify: `ui/src/styles.css` — evidence, status, keyboard-focus and color-independent state styling.

**Interfaces:**

- `correlation.snapshot`: `WorkspaceRead`/`Read`, unbounded envelope resolved to current workspace, `CorrelationRequest` → `CorrelationSnapshot`.
- `correlation.evidence`: `ResourceRead`/`Read`, unbounded envelope resolved to current workspace, `CorrelationEvidenceRequest` containing only backend-issued IDs → `EvidenceRef[]`.
- Handlers reject malformed ranges/IDs, duplicate or cross-workspace evidence IDs, inactive membership, principal mismatch, missing grant, role denial, unverified/restricted source data and UI/AuditLog policy denial with distinct existing `IpcErrorCode` values.
- React consumes the exact wire contract. It does not submit provider URLs/queries, native IDs, source selectors or maintenance definitions; it only submits explicit time window/evaluation/lateness and issued evidence IDs.

**Tests to add:**

- Rust descriptor names/capabilities/permissions and handler authorization order;
- missing capability, permission, active membership, principal, workspace grant and envelope-scope failures mapping to `PERMISSION_DENIED`;
- malformed request/range/non-finite input/duplicate IDs mapping to `INVALID_REQUEST`;
- unknown/cross-workspace/unverified evidence mapping to `NOT_FOUND` or `POLICY_DENIED` as specified;
- source malformed/unavailable, UI egress, LocalStorage and AuditLog policy failures with distinct typed codes;
- output scans proving no source payload/query/credential/ARN/account/subscription/cursor leaks into errors or serialized results;
- UI sends an explicit complete request and renders deterministic candidate/source order;
- every candidate expands to every contributing Signal, source reference, reason and evidence ID, with source evidence opened only by `correlation.evidence`;
- shared target reasons, topology reason shown as “probable structural relationship” and no causal/root-cause wording in rendered text;
- late/reopened, suppressed, maintenance-window, mixed/all-suppressed, missing metric and unavailable-source states;
- finite `number` metric rendering, omitted metric as unavailable rather than zero, source/query/time-window/excerpt/masked/unparsed evidence states and trusted HTTPS native-link guard;
- keyboard navigation, focus visibility, screen-reader labels, text status in addition to color and equal English/Thai locale key sets; and
- no Incident route/model/write and no outbound/provider call in the acceptance fixture journey.

- [ ] **Step 1: Write failing IPC contract tests**

Add `signal_ipc.rs` tests that use the existing authorization fixtures to exercise both descriptors and each failure order. Assert the handlers validate IDs against the current snapshot, return complete evidence closure and never expose raw payload or dynamic provider text in an error.

- [ ] **Step 2: Run focused IPC tests and record the expected failure**

Run:

```bash
cargo test -p thalassaops --test signal_ipc
```

Expected: FAIL because the two handlers and command registration do not exist.

- [ ] **Step 3: Implement read-only correlation handlers**

Mirror `operations.snapshot` and `topology.evidence` authorization conventions exactly. Construct descriptors from `thalassa-ipc`, check command/capability/scope/membership/principal/grant/permission, parse and bound the request, evaluate LocalStorage/source policy, build the snapshot, validate evidence IDs, evaluate Ui/AuditLog policy and return typed sanitized errors. Register only `correlation_snapshot` and `correlation_evidence`.

`correlation_evidence` must resolve the complete request before returning anything, reject empty/duplicate/unknown/cross-workspace/unverified IDs, and never accept a raw native ID, query, URL or source selector.

- [ ] **Step 4: Build the React candidate/evidence view from the fixture**

Compose the view from `CorrelationSnapshot`. Show status, source kind, safe native identity when present, target, typed severity/exploitability, reasons, qualification, suppression IDs/policy version, late/reopened state and every contributing Signal. Use `Signal.evidence_ids`, reason evidence, candidate evidence and topology path evidence as issued IDs only.

Render missing metrics as unavailable and all values as numeric at the contract boundary. Use locale keys for every user-visible enum. Render `probable_structural` as a structural qualification and do not render causal synonyms.

- [ ] **Step 5: Add evidence panel and Operations Console entry point**

Call `correlation.evidence` only with IDs from the validated snapshot. Reuse existing EvidenceRef fields and masking/unparsed states; open only trusted existing HTTPS native links. Add an Operations Console signal/correlation entry point with keyboard focus and no Incident model/route.

- [ ] **Step 6: Add locale/accessibility and acceptance tests**

Provide identical English/Thai keys for signal kinds, source kinds, finding fields, reasons, qualifications, candidate/window/suppression statuses, source/policy errors and evidence states. Add accessible labels/focus order, text status indicators and responsive evidence presentation without using color as the only channel.

- [ ] **Step 7: Run backend/frontend gates and commit**

Run:

```bash
cargo test -p thalassaops --test signal_ipc
npm ci
npm test -- ui/src/correlation/correlation-contracts.test.ts ui/src/correlation/CorrelationWorkspace.test.tsx ui/src/correlation/correlation.acceptance.test.tsx
npm run typecheck
npm run lint
```

```bash
git add src-tauri/src/app/correlation.rs src-tauri/src/app src-tauri/src/main.rs src-tauri/tests/signal_ipc.rs ui/contracts/ipc.ts ui/src/correlation ui/src/OperationsConsole.tsx ui/src/shell.tsx ui/src/locales/en.ts ui/src/locales/th.ts ui/src/styles.css
git commit -m "feat: expose read-only signal correlation workspace"
```

**Acceptance criteria:**

- Only `correlation.snapshot` and `correlation.evidence` cross IPC, with the exact capabilities, permissions, scope behavior, authorization order and typed error mapping.
- The UI renders the exact Rust contract from copied fixtures and live IPC responses without a second model, fabricated zeros/defaults or source query construction.
- Operators can expand every candidate to every contributing Signal and reach each original evidence reference; source retention and unknown fields remain backend-reachable through verified references.
- Exact and probable-structural reasons, late/reopened and suppression states are localized and accessible, and no causal/root-cause claim appears.
- Tests prove masking, redaction, Restricted-data fail-closed behavior, trusted-link handling, keyboard/accessibility behavior and no forbidden value/command leak.

### Task 9: Run complete regression, fixture acceptance and release verification

**Files:**

- Create: `src-tauri/tests/signal_correlation_acceptance.rs` — one end-to-end fixture journey across operational adapters, four security adapters, retention, dedup, windows, topology, suppression and snapshot closure.
- Create: `ui/src/correlation/correlation.release.test.tsx` — final UI journey asserting source references remain reachable through the rendered candidate.
- Create: `docs/superpowers/reports/2026-08-28-sprint-13-verification.md` — command results, fixture evidence and the exact exit-criterion observation.
- Modify only when a verified defect is found: the affected Sprint 13 source/test/doc file; do not broaden scope or alter unrelated Sprint 11/12 behavior.

**Interfaces:**

- Consumes: the complete fixture catalog, canonical domain contracts, adapters, source ledger, dedup/window/grouping/topology/suppression aggregator, two IPC handlers and copied React contract.
- Produces: a deterministic snapshot containing at least one Alertmanager alert, one Prometheus anomaly, one normalized vulnerability finding, shared/exact and topology reasons, complete evidence closure, a late/reopened example and a suppressed source that remains reachable.
- Verification report records commands and outcomes only; it must not include credentials, raw provider bodies, forbidden IDs or unmasked Restricted data.

**Tests to add:**

- end-to-end candidate with Alertmanager + Prometheus + one security finding and all Signal/source/evidence references resolving;
- one fixture each for Trivy, Falco, Kyverno and OPA Gatekeeper with source, asset, severity/exploitability behavior and evidence;
- source-record unknown-field retention, digest/native identity, cross-source dedup distinction and no source-reference loss after candidate aggregation;
- half-open window boundary, finalized-window late arrival/reopen with stable candidate ID and out-of-range/missing-time retention;
- Resource/Service/Deployment exact reasons, topology path passthrough/probable-structural label and no causal claim;
- rule and maintenance suppression, all/mixed statuses and preserved policy/evidence metadata;
- full IPC authorization/policy/error matrix and UI evidence drill-down/locale/accessibility journey;
- deterministic byte-equality from shuffled input and repeated fixture runs; and
- forbidden-data scans over committed fixtures, serialized Rust snapshots, IPC errors, UI fixture JSON and verification report.

- [ ] **Step 1: Write the full fixture acceptance tests**

Build the full snapshot at fixture clock `2026-08-28T09:00:00Z` with an explicit bounded request. Assert the candidate includes an Alertmanager alert, a Prometheus anomaly and at least one normalized security finding, lists every contributing Signal ID and resolves every Signal/source/evidence reference. Assert one topology relationship is `probable_structural`, one source is suppressed while retained, and no Incident entity/write appears.

- [ ] **Step 2: Run focused end-to-end tests and record any failure**

Run:

```bash
cargo test -p thalassaops --test signal_correlation_acceptance
npm test -- ui/src/correlation/correlation.release.test.tsx
```

If a test fails, trace it to the owning task and fix the narrowest source/test contract; do not bypass an evidence, policy, scope, boundary or forbidden-data assertion.

- [ ] **Step 3: Run Rust formatting, tests and static checks**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Confirm no existing Sprint 11/12 test count or behavior regressed and no provider/network command was added.

- [ ] **Step 4: Run frontend install, tests, types, lint and format gate**

Run:

```bash
npm ci
npm test
npm run typecheck
npm run lint
npm run format:check
```

The required `npm run format:check` must pass. If it fails on an existing unrelated path, record the exact path and cause; do not claim the gate passed.

- [ ] **Step 5: Run policy, forbidden-data and scope audits**

Use repository search and the existing test scanners to verify:

- no `SignalEnvelope`, `NormalizedSignal`, private source enum, incident model, incident write, mutation/remediation, provider query, CLI invocation, Terraform/OpenTofu execution or new network integration was introduced;
- no secret-like key/value, credential, token, ARN, account ID, subscription ID, cursor, authorization header or raw provider error exists in fixtures, source records, logs, reasons, errors, snapshots or report;
- no correlation reason contains causal/root-cause/proven language or probability claims;
- only `correlation.snapshot` and `correlation.evidence` are new IPC commands and their capabilities/policies remain exact; and
- source references, evidence IDs, topology path evidence and candidate Signal IDs are closed and reachable.

- [ ] **Step 6: Write the verification report and inspect the diff**

Record the successful commands, fixture IDs, snapshot/candidate/reference checks and the exact exit criterion in `docs/superpowers/reports/2026-08-28-sprint-13-verification.md`. Run `git diff --check`, inspect the full diff for field/enum/nullability drift and verify only intended Sprint 13 files changed.

- [ ] **Step 7: Commit the verified implementation**

```bash
git add src-tauri/tests/signal_correlation_acceptance.rs ui/src/correlation/correlation.release.test.tsx docs/superpowers/reports/2026-08-28-sprint-13-verification.md
git commit -m "test: verify sprint 13 signal correlation"
```

Do not push or merge. Report the commit, test commands and exact format-gate result to the coordinator.

**Acceptance criteria:**

- The complete fixture journey demonstrates that alerts, anomalies and normalized vulnerability findings form explainable candidates and every original source reference remains reachable.
- Rust and frontend tests, type checks, lint, formatting and `npm run format:check` pass with no reduced coverage or skipped evidence/policy assertions.
- Suppression, maintenance, late-arrival, topology and cross-source dedup behavior remains deterministic and source-preserving in the integrated snapshot.
- Forbidden-data, no-network, no-mutation, no-incident-lifecycle and exact IPC command-surface audits pass.
- The verification report contains no sensitive data and states the sprint exit criterion verbatim.

## Exit criterion

The sprint is complete only when the validated fixture snapshot and its UI evidence controls demonstrate:

> "Alerts, anomalies and normalized vulnerability findings can be correlated into explainable candidates without losing original source references."
