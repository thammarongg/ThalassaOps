# Sprint 13 Signal Correlation Verification

Date: 2026-08-28

Branch: `thammarongg/sprint-13-signal-correlation`

Comparison base: `main` (`17af58d2597cd212dff6498dbc514cba0088ea0d`)

Reviewer: independent verification pass

## Executive result

The complete Sprint 13 branch was reviewed against `main`, including the Rust
normalization, source-record ledger, correlation engine, IPC authorization,
Rust/TypeScript contract guards, and React workspace. The deliverables are
implemented and traceable to source and tests, and the exit criterion is
credibly met: alerts, anomalies, and normalized vulnerability findings can be
correlated into explainable candidates while preserving the original source
record reference and evidence IDs.

The independent pass found and fixed source-identity collisions, source
evidence loss after SQLite reload, fabricated anomaly conditions, incomplete
candidate provenance validation, UI contract drift, non-deterministic target
ordering, a per-Signal evidence disclosure gap, and an over-broad numeric
privacy guard. All required gates pass at the end of this pass.

## Deliverable traceability

| Deliverable | Implementation | Verification |
| --- | --- | --- |
| Common signal envelope | `crates/thalassa-domain/src/lib.rs` (`Signal`, `SignalPayload`, `SourceRecordRef`, suppression and drill-down contracts); adapter seam in `src-tauri/src/correlation/adapters/mod.rs` | `crates/thalassa-domain/tests/signal_correlation_contracts.rs`, `src-tauri/tests/signal_adapters.rs`, adapter and IPC tests |
| Vulnerability/security finding envelope | `VulnerabilityFinding` with source, `FindingAsset`, severity, exploitability, optional CVSS, and evidence IDs; target and source matching are validated before projection | `crates/thalassa-domain/tests/signal_correlation_contracts.rs`, `src-tauri/tests/security_adapters.rs` (15 tests), source/evidence closure assertions |
| Trivy replay adapter | `src-tauri/src/correlation/adapters/trivy.rs`; package/image/path-aware source identity and honest missing-package dedup behavior | Trivy fixture mapping, repeated-package identity, mixed-deployment, malformed payload, and missing-package tests in `src-tauri/tests/security_adapters.rs` |
| Falco replay adapter | `src-tauri/src/correlation/adapters/falco.rs`; priority, runtime target and namespace identity parsing | Falco priority/target and namespace collision regression tests in `src-tauri/tests/security_adapters.rs` |
| Kyverno replay adapter | `src-tauri/src/correlation/adapters/kyverno.rs`; policy/rule/resource/path source identity | Kyverno policy-subject, repeated-resource revision, malformed schema and source-closure tests |
| OPA Gatekeeper replay adapter | `src-tauri/src/correlation/adapters/gatekeeper.rs`; constraint/resource/path source identity | Gatekeeper mapping, repeated-resource revision, malformed schema and source-closure tests |
| Complete source-record retention | `src-tauri/src/correlation/source_records.rs`, `src-tauri/migrations/0004_source_record_evidence.sql`, app database wiring; the complete redacted payload remains in the local ledger and evidence bodies survive restart | SQLite round-trip, tampered-evidence rejection, scope-conflict, cross-scope evidence rebinding, migration, and IPC ledger tests in `src-tauri/tests/signal_adapters.rs` and `src-tauri/tests/signal_ipc.rs` |
| Deduplication and event-time windows | `src-tauri/src/correlation/dedup.rs` and `window.rs`; source-aware keys, explicit event time, bounded half-open windows, watermark/lateness and late reopen semantics | `src-tauri/tests/signal_dedup.rs`, `src-tauri/tests/signal_grouping.rs`, domain window contract tests |
| Resource/service/deployment grouping | `src-tauri/src/correlation/grouping.rs` and `aggregate.rs`; exact typed target equality with deterministic candidate and reason ordering | `src-tauri/tests/signal_grouping.rs` exact-target, kind-separation, shuffle, duplicate-edge, candidate-anchor, and target-ordering tests |
| Topology grouping | `src-tauri/src/correlation/grouping.rs` with the Sprint 12 topology resolver; bounded paths and explicit source limitations | topology relation, depth-limit, failed-resolution, disconnected, and reordered-target tests in `src-tauri/tests/signal_grouping.rs` |
| Explainable correlation reasons | Typed `CorrelationReason` with exact association/probable structural qualification, signal IDs, target/path IDs and evidence IDs; reason coverage and target/path provenance are validated | domain reason-coverage/target tests, grouping tests, snapshot validation, and UI guard contract tests |
| Suppression and maintenance windows | `src-tauri/src/correlation/suppression.rs`; rule and half-open maintenance-window evaluation, retained IDs, policy version and candidate status precedence | `src-tauri/tests/signal_suppression.rs`, domain suppression contracts, and UI mismatch/status tests |
| Read-only IPC and capability enforcement | `src-tauri/src/app/correlation.rs` and `crates/thalassa-ipc`; exactly `correlation.snapshot` (`WorkspaceRead`) and `correlation.evidence` (`ResourceRead`) with command, capability, principal, membership, scope, policy and UI-egress checks | `src-tauri/tests/signal_ipc.rs`, app authorization/error tests, and `crates/thalassa-ipc/tests/contracts.rs` |
| Rust/TypeScript contract seam | `ui/contracts/ipc.ts` and `ui/contracts/guards.ts`; strict UUID, RFC3339/calendar, scope, window, watermark, status, evidence, provenance and privacy checks | `ui/src/correlation/correlation-contracts.test.ts`, full Vitest suite, and domain JSON/contract tests |
| Evidence-backed UI | `ui/src/correlation/CorrelationWorkspace.tsx`, `CandidateDetails.tsx`, CSS and locales; candidates expose expandable contributing Signals and a per-Signal evidence action | `ui/src/correlation/CorrelationWorkspace.test.tsx`, acceptance test, i18n parity test, and full Vitest suite |

## Defects found and fixed

The following are the coherent defect groups found during the independent
review. Commit subjects are included so the corrections remain traceable
without relying on a second diff interpretation.

### Source identity, normalization, and retention

- `5fa8a92 fix: preserve replay source identities` qualifies security targets
  by namespace, includes enough source fields in Trivy/Kyverno/Gatekeeper
  native identities to prevent same-CVE/policy collisions, leaves Trivy
  `dedup_key` absent when package identity is missing, and keeps malformed
  source records on the typed rejection path rather than panicking. It also
  rejects an anomaly fixture without an explicit source condition instead of
  inventing an operator/threshold.
- `de7b2de fix: retain and validate source evidence` adds the append-only
  evidence companion table and migration, wires the app projection to the
  SQLite ledger, restores evidence bodies after restart, preserves legacy
  source rows as unresolved when their old evidence bodies are unavailable,
  and validates persisted record/evidence redaction, digest, scope and
  identity invariants before reuse.
- The persisted ledger now rejects tampered evidence and out-of-scope
  identity/evidence rebinding. An invalid or unavailable source is reported
  as a typed source limitation; no source payload or evidence body is
  fabricated to fill a gap.

### Correlation and contract integrity

- `f4cbff4 fix: harden correlation contract boundaries` closes Rust/UI drift
  for backend-issued drill-down filter keys, source-kind/scope evidence
  binding, candidate evidence closure, reason coverage, reason target/path
  provenance, candidate status precedence, strict window/watermark/state
  rules, UUID and timestamp shapes, safe display identities, and numeric
  metric units. It also makes correlation target ordering canonical and adds
  regressions for malformed calendar timestamps and contract mismatches.
- `40bb97d fix: allow opaque correlation identifiers` corrects the privacy
  heuristic so generated `candidate:v1:` hashes are not mistaken for bare
  numeric account identifiers. The defect surfaced only when the full Rust
  workspace ran with a random workspace scope and was retained as an IPC
  regression check.
- `92a00c4 fix: satisfy replay adapter lint` removes the final Clippy warning
  from the namespace-qualified Falco target path without changing behavior.

### UI evidence disclosure and accessibility

- `99321fe fix: expose per-signal evidence disclosures` makes each candidate
  Signal a native expandable disclosure and adds an accessible per-Signal
  evidence action. The callback carries the Signal subject and its issued
  evidence IDs, while the candidate-level action remains available. English
  and Thai locale keys remain symmetric and the UI tests cover the updated
  action wiring.

## Security, privacy, capability, and causal language

Every normalized Signal retains a `SourceRecordRef` containing the original
source kind, native identity when safe, content digest and evidence IDs. The
complete post-policy source JSON and evidence JSON are retained in the local
SQLite ledger. A candidate therefore resolves through `candidate.signal_ids`
to the Signal, then to its source-record identity and evidence IDs; the
evidence command returns only IDs emitted by the current validated snapshot.
The raw source payload is deliberately not an arbitrary IPC lookup surface:
the two-command design exposes verified evidence references while keeping the
complete source record in the local ledger.

The representative trace is covered by
`candidate_members_retain_operational_and_security_source_evidence` and
`snapshot_retains_source_records_and_evidence_in_the_local_ledger` in
`src-tauri/tests/signal_ipc.rs`, by
`sqlite_source_record_store_round_trips_the_complete_retained_record` in
`src-tauri/tests/signal_adapters.rs`, and by the security adapter source-
closure tests. These cover an operational alert/anomaly and normalized
security finding members from a candidate and verify that every referenced
evidence ID resolves to the matching source kind and scope.

Both IPC commands are read-only and enforce capability, command name,
unbounded envelope scope, active principal/membership, workspace grant, role
permission, source-retention policy, audit-retention policy and UI-egress
policy. Evidence IDs, source identities, display text, fixture payloads and
reasons reject credentials, tokens, account/subscription identifiers, ARNs,
pagination cursors and unsafe control text. Generated digests, dedup keys,
candidate IDs and UUIDs are treated as opaque identifiers rather than being
classified as account numbers.

Correlation reasons say `exact_association` or `probable_structural`; they
identify the matching typed target or topology path and never claim causation.
Failed or ambiguous topology resolution is reported as a typed source
limitation and does not create a fallback candidate.

## Accessibility and localization

Candidate Signals are rendered as native `<details>` disclosures with
keyboard-reachable summaries and per-Signal evidence buttons. Existing focus
visibility and semantic button behavior are preserved. New correlation labels
are present in both `ui/src/locales/en.ts` and `ui/src/locales/th.ts`; locale
key parity is covered by the i18n test.

## Acceptance evidence

The full fixture-backed projection includes alert, anomaly, health-check and
security-finding inputs, with Trivy, Falco, Kyverno and OPA Gatekeeper source
values preserved. Candidates carry sorted Signal IDs, reasons, grouping
targets, late/suppression state, evidence closure and drill-down references;
the UI can expand each contributing Signal and request its backend-issued
evidence IDs.

Additional tests cover source-aware deduplication, explicit event-time
windows, half-open and maintenance boundaries, late reopening, exact
resource/service/deployment grouping, topology path qualification, stable
ordering under shuffled signals/targets, unknown or malformed source values,
cross-scope evidence rejection, policy/capability authorization, Rust/UI
contract parity, and i18n/accessibility wiring.

## Final gates

| Gate | Result |
| --- | --- |
| `npm run format:check` | PASS |
| `npm run lint` | PASS |
| `npm run typecheck` | PASS |
| `npm test` | PASS — 13 files, 107 tests |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo test --workspace` | PASS — 397 tests plus zero-test targets |

## Deliberately left open

- Sprint 13 remains fixture-backed by design. Live provider ingestion and
  network replay are outside this sprint; no live data is represented as a
  fabricated normalized signal.
- The UI-facing evidence command intentionally resolves only backend-issued
  evidence IDs from the current snapshot. Arbitrary native source-record
  retrieval is not added because it would bypass snapshot closure and the
  command’s read-only, bounded evidence contract; complete redacted source
  payloads remain available to the core through the local ledger.
- No known code, contract, policy, provenance, privacy, accessibility,
  localization or acceptance defect remains open after the final gates.
