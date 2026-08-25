# ThalassaOps Product Sprint Plan

**Plan type:** Product-grade desktop application  
**Cadence:** 2 weeks per delivery sprint  
**Core plan:** 1-week Pre-Sprint + 28 delivery sprints / approximately 57 weeks  
**Target:** Production-ready macOS-first release with Windows/Linux path

## Planning assumptions

- This is a product build, not an MVP-only exercise.
- macOS is the primary platform; Windows and Linux receive cross-platform validation.
- Current cloud scope is AWS, Azure and GCP. Huawei Cloud is deferred.
- ThalassaOps integrates existing telemetry backends instead of creating a new metrics/logs backend.
- Apache License 2.0 is the project license.
- The default UX is a Hybrid Operations Console with an Incident Workspace.
- AI is evidence-first, context-optimized and policy-governed.
- Direct cluster/pod mutations are high-risk and require explicit policy handling.
- Identity and policy foundations are established before connector and AI work; the full Policy Center administration surface is delivered later.

## Sprint cadence

Each two-week sprint should include:

### Week 1 — Build

- Confirm sprint goal and acceptance criteria.
- Implement the smallest coherent vertical slice.
- Add unit and contract tests with the feature.
- Update domain/API documentation.

### Week 2 — Integrate and prove

- Integrate with adjacent modules.
- Run targeted tests, security checks and UX review.
- Demo the working slice with a realistic operational scenario.
- Run an independent review agent.
- Record decisions, risks and follow-up work.

Every sprint ends with a demonstrable artifact, not only merged code.

## Phase 0 — Product and architecture alignment

### Pre-Sprint — Product contract and delivery setup

**Goal:** Make the product boundary and engineering workflow explicit before implementation.

**Deliverables:**

- Requirements baseline and domain glossary.
- Severity, Incident Status and Data Redaction policy baseline.
- Identity and policy scope contract: Organization, Team, Workspace and Environment.
- Product success metrics and non-goals.
- Local Git repository connected to GitHub.
- Branch, commit, review and release conventions.
- Apache-2.0 license file and SPDX metadata.
- Initial risk register.

**Exit criteria:** The team can explain what ThalassaOps owns, what it integrates and what it will not do in the first product release.

### Sprint 1 — Architecture and domain contracts

**Goal:** Define the boundaries that let Rust core, React UI and connectors evolve independently.

**Deliverables:**

- Workspace hierarchy: Organization, Team, Workspace, Environment.
- Local principal, membership and resource-scope model for a single-user secure workspace, with an enterprise-compatible identity shape.
- Domain entities: Resource, Signal, Incident, Evidence, Hypothesis, Action, Policy and Audit.
- Policy contract and baseline policy runtime, including immutable secret protection and fail-closed egress enforcement.
- Rust/React IPC contract conventions.
- Connector capability model.
- Error and permission vocabulary.
- Architecture Decision Records for local-first state, secure IPC and provider-neutral AI.

**Exit criteria:** Domain contracts can be tested without a live cloud provider.

## Phase 1 — Desktop foundation and visual system

### Sprint 2 — Tauri/Rust/React application shell

**Goal:** Boot a production-shaped desktop application on macOS.

**Deliverables:**

- Tauri 2 shell.
- Rust workspace and React/TypeScript/Vite UI.
- Dev, test and build commands.
- Secure IPC boundary.
- SQLite initialization and migration strategy.
- Secure local workspace bootstrap, local administrator identity and policy-store migrations.
- CI for formatting, linting, type checks and tests.

**Exit criteria:** A signed development build opens on macOS and executes a tested Rust-to-React health call.

### Sprint 3 — Design system and localization foundation

**Goal:** Establish the ThalassaOps visual language and Thai/English support.

**Deliverables:**

- Dark-mode-first design tokens.
- Typography, spacing, status, severity and focus states.
- Reusable cards, tables, tabs, drawers, timelines, command surfaces and empty states.
- Thai/English translation catalog.
- Accessibility baseline and keyboard focus behavior.

**Exit criteria:** Screens can be composed from shared components without hard-coded user-facing strings.

### Sprint 4 — Global shell and navigation

**Goal:** Make the application navigable as a command center.

**Deliverables:**

- Organization/Team/Workspace/Environment switchers.
- Left navigation and favorites.
- Global search and `⌘K` command palette.
- Notification center.
- Connector, policy and model status indicators.
- Embedded terminal drawer and external terminal handoff shell.

**Exit criteria:** A user can navigate between all planned product areas using mouse and keyboard.

**Milestone:** Foundation Demo.

## Phase 2 — Connection and read-only operations

### Sprint 5 — Connector registry and health

**Goal:** Make integrations first-class product objects.

**Deliverables:**

- Connector registry.
- Credential reference model using OS keychain integration.
- Connection test and health state.
- Capability discovery.
- Per-connector logs and last-sync information.
- Safe failure and retry behavior.

**Exit criteria:** A connector can be added, tested, disabled and diagnosed without exposing credentials.

### Sprint 6 — Kubernetes read-only foundation

**Goal:** Build the first deep operational connector.

**Deliverables:**

- Cluster discovery from kubeconfig.
- Nodes, namespaces, workloads, services and pods.
- Events, status conditions and resource relationships.
- Read-only logs and resource detail views.
- RBAC-aware capability detection.

**Exit criteria:** A user can inspect a cluster and move from a failing pod to its events, logs and owning workload.

### Sprint 7 — Kubernetes resource workspace

**Goal:** Turn Kubernetes data into a coherent resource investigation experience.

**Deliverables:**

- Resource hierarchy and topology edges.
- Pod/workload health summaries.
- YAML/manifest viewer with sensitive-field masking.
- Resource search and filtering.
- Native `kubectl`/provider-console handoff links.
- Read-only command classification.

**Exit criteria:** The UI supports a complete read-only investigation of CrashLoopBackOff, OOMKilled and Pending scenarios.

### Sprint 8 — Prometheus, Alertmanager and Grafana

**Goal:** Connect metrics and alerts to resources.

**Deliverables:**

- Prometheus connection and query execution.
- Alertmanager alert ingestion.
- Alert labels and resource mapping.
- Query/result model with timestamps and source references.
- Grafana dashboard and Explore deep links.
- Basic time-series and alert panels.

**Exit criteria:** An alert can be opened from ThalassaOps and traced to metrics, resource scope and native Grafana context.

### Sprint 9 — Loki and OpenTelemetry

**Goal:** Add logs and traces to the same investigation path.

**Deliverables:**

- Loki query integration.
- OpenTelemetry/OTLP metadata integration.
- Trace/log correlation where identifiers are available.
- Structured log viewer with field masking.
- Time-window alignment across signals.

**Exit criteria:** A user can move from an alert to logs and traces without reconstructing the time window manually.

### Sprint 10 — AWS, Azure and GCP inventory

**Goal:** Provide cross-cloud environment visibility without building provisioning yet.

**Deliverables:**

- AWS account/resource discovery.
- Azure subscription/resource discovery.
- GCP project/resource discovery.
- Cloud resource health and activity metadata.
- Cloud credential scopes and read-only permission checks.
- Provider-native deep links.

**Exit criteria:** Multiple cloud environments appear in one Environment view with clear provider boundaries and health status.

**Milestone:** Read-only Operations Alpha.

## Phase 3 — Operations Console and correlation

### Sprint 11 — Operations Console

**Goal:** Build the primary home experience.

**Deliverables:**

- Business-impact-first health summary.
- Active incident queue.
- Alert and anomaly summary.
- Rule-based anomaly signal producer using metric thresholds/rate-of-change fixtures.
- Scheduled health-check producer with interval, scope, timeout, cooldown and audit metadata.
- Recent change stream.
- Environment status overview.
- Configurable but curated dashboard widgets.
- Drill-down from every critical number.

**Exit criteria:** A user can open the application and understand what needs attention within 30 seconds.

### Sprint 12 — Resource and service topology

**Goal:** Show dependencies and blast radius.

**Deliverables:**

- Service/resource graph.
- Ownership and team mapping.
- Upstream/downstream impact.
- Topology filtering by Environment, Team and Incident.
- Graph-to-evidence navigation.

**Exit criteria:** An incident can show affected resources and probable dependency paths.

### Sprint 13 — Signal normalization, security findings and correlation

**Goal:** Normalize operational and security signals before correlation turns them into meaningful incident candidates.

**Deliverables:**

- Common signal envelope.
- Vulnerability/security finding envelope with source, asset, severity, exploitability and evidence references.
- Initial Trivy, Falco, Kyverno and OPA Gatekeeper adapters or replayable contract fixtures.
- Deduplication keys.
- Correlation windows.
- Grouping by resource, service, deployment and topology.
- Explainable correlation reasons.
- Suppression and maintenance-window support.

**Exit criteria:** Alerts, anomalies and normalized vulnerability findings can be correlated into explainable candidates without losing original source references.

### Sprint 14 — Change intelligence

**Goal:** Connect operational problems to recent changes.

**Deliverables:**

- GitHub/GitLab integration.
- Argo CD integration.
- Deployment and configuration change events.
- Change timeline.
- Change-to-signal correlation.
- Diff and native source links.

**Exit criteria:** A user can identify what changed before an incident and inspect the supporting source/diff.

## Phase 4 — Incident and AI investigation

### Sprint 15 — Incident domain and lifecycle

**Goal:** Implement the canonical incident model.

**Deliverables:**

- Detected, Triage, Investigating, Mitigating, Monitoring, Resolved, Closed and Reopened states.
- Duplicate, False Positive, Suppressed and Cancelled dispositions.
- Severity and Business Impact model.
- Ownership and responder roles.
- Incident timeline and audit events.

**Exit criteria:** Incidents can be created from alerts, anomalies, user reports, scheduled health checks, vulnerability findings and manual reports, then progress through a validated state machine.

### Sprint 16 — Incident Workspace

**Goal:** Build the primary deep-work UX.

**Deliverables:**

- Split incident list/detail view.
- Incident narrative.
- Evidence panel.
- Alerts, topology, changes and vulnerability tabs.
- Shareable Incident Card.
- Comments, assignment and status updates.

**Exit criteria:** A responder can manage an incident from any supported source through resolution without leaving the workspace for basic coordination, including a vulnerability finding with evidence in the vulnerability tab.

### Sprint 17 — AI provider gateway

**Goal:** Provide one safe abstraction over hosted and local models.

**Deliverables:**

- Provider registry.
- OpenAI, Anthropic, Gemini and OpenAI-compatible provider adapters.
- Ollama/vLLM/local provider path.
- Model capability metadata.
- Request budgets and timeout/cancellation.
- Provider health and cost metadata.

**Exit criteria:** The same investigation contract can run against hosted or local providers without changing the UI.

### Sprint 18 — Context optimization and redaction

**Goal:** Protect data and control AI context quality/cost.

**Deliverables:**

- Public/Internal/Confidential/Restricted classification.
- Immutable secret detection and blocking.
- Drop, mask, hash, truncate and aggregate actions.
- Policy-runtime-backed send/store/display/export/audit separation; no mutable redaction behavior is hard-coded in this sprint.
- Context selection, deduplication and summarization.
- Redaction preview and policy failure behavior.

**Exit criteria:** A test corpus containing secrets and noisy telemetry is redacted correctly and never leaves the application when policy validation fails.

### Sprint 19 — Read-only AI investigation

**Goal:** Deliver evidence-backed troubleshooting for core incidents.

**Deliverables:**

- Tool registry with capability scopes.
- Kubernetes analyzers for CrashLoopBackOff, OOMKilled, Pending, probe failure and image pull failure.
- Prometheus/Loki/OTel evidence retrieval.
- Structured findings, hypotheses and confidence.
- Evidence citations and missing-context warnings.
- AI Assistant Log.

**Exit criteria:** For fixture incidents, the assistant produces a replayable, evidence-backed investigation without mutating infrastructure.

**Milestone:** Read-only AI Beta.

## Phase 5 — Governance and controlled action

### Sprint 20 — Policy Center

**Goal:** Deliver the full Policy Center governance surface on top of the identity and policy runtime established in Sprints 1–2.

**Deliverables:**

- Policy hierarchy and inheritance.
- Organization, Team, Workspace and Environment scope selection using the existing identity model.
- Baseline presets.
- Effective-policy preview.
- Severity simulation.
- Incident transition validation.
- Redaction test payloads.
- Action risk-class and execution-mode matrix, including disabled-by-default `POLICY_AUTO` rules.
- Versioning, rollback and audit.

**Exit criteria:** The local administrator identity from Sprint 2 can safely change a policy at an Organization, Team, Workspace or Environment scope, preview its effect and roll it back; multi-user membership and SSO remain Sprint 24 work.

### Sprint 21 — Approval and action framework

**Goal:** Support governed mutations without giving the AI authorization power.

**Deliverables:**

- Action registry.
- Read-only, Mutating, Blocked and Requires Approval classification.
- Separate execution modes: Observe, Recommend, Approval and disabled-by-default Policy Auto.
- Approval requests and approver roles.
- Dry-run output.
- Expected impact and rollback plan.
- Post-action verification.
- Action audit records.
- Narrowly scoped, reversible `POLICY_AUTO` path with resource/environment allowlists, cooldown, rollback and approval fallback.

**Exit criteria:** A user can propose a safe action, obtain approval, execute it and verify the result with a complete audit trail.

### Sprint 22 — Terminal and runbook workflows

**Goal:** Connect expert workflows without weakening governance.

**Deliverables:**

- Embedded terminal.
- External terminal handoff.
- Command preview and classification.
- Copyable generated commands.
- Skills and runbook registry.
- Allowlisted command patterns.
- Terminal audit and sensitive-output masking.

**Exit criteria:** Experts can use terminal workflows while ThalassaOps records classification, scope and policy state.

### Sprint 23 — Jira, Slack, Discord and PagerDuty

**Goal:** Close the communication and work-management loop.

**Deliverables:**

- Jira/JSM Incident Card creation and updates.
- Slack incident channel/thread updates.
- Discord notifications.
- PagerDuty incident/status mapping.
- Communication templates.
- Management summary generation from evidence.

**Exit criteria:** An incident can be coordinated internally and reflected in external systems without losing source and audit links.

**Milestone:** Controlled Action Beta.

## Phase 6 — Team, security and scale

### Sprint 24 — Multi-organization team access

**Goal:** Make the local-first product team-ready.

**Deliverables:**

- Organization, Team and Workspace membership.
- Roles and resource scopes.
- Shared incidents and comments.
- Approval delegation.
- SSO/OIDC foundation.
- Session and device management.

**Exit criteria:** Multiple users can collaborate while seeing only authorized environments and actions.

### Sprint 25 — Security posture, compliance and application hardening

**Goal:** Ingest customer security/compliance posture and harden the application for enterprise evaluation.

**Deliverables:**

- OS keychain integration.
- Secure secret storage.
- IPC capability restrictions.
- Signed policy and audit events.
- Dependency/license scanning.
- Threat model and security test suite.
- Data retention and deletion controls.
- AWS Security Hub/GuardDuty, Microsoft Defender for Cloud and Google Security Command Center finding adapters.
- Compliance posture aggregation and evidence-linked vulnerability/security views.

**Exit criteria:** Security review finds no known critical path allowing secret leakage or unauthorized mutation.

### Sprint 26 — Performance, resilience, scale and operational insights

**Goal:** Validate operation with large environments and 1000+ microservices while exposing read-only capacity, cost and reliability insights.

**Deliverables:**

- Large-resource list virtualization.
- Query cancellation and backpressure.
- Local cache and offline/degraded mode.
- Connector retry and rate limiting.
- Performance fixtures for alerts, logs, metrics and topology.
- Investigation latency and token-cost benchmarks.
- Capacity trends, reliability/error-budget indicators and provider cost metadata where available.
- Explicit boundary for full FinOps-system integrations, which remain post-release expansion.

**Exit criteria:** The application remains usable with target-scale fixtures and handles provider/API degradation visibly.

**Milestone:** Enterprise Evaluation Candidate.

## Phase 7 — Release engineering and product quality

### Sprint 27 — macOS production packaging and UX polish

**Goal:** Prepare a release candidate that feels like a real product.

**Deliverables:**

- macOS signing and notarization pipeline.
- Installer and update strategy.
- Crash reporting with privacy controls.
- Accessibility review.
- Thai/English copy review.
- Empty, error, loading and degraded-state polish.
- Onboarding and first-connection wizard.

**Exit criteria:** New users can install, connect a safe read-only environment and understand the product without developer assistance.

### Sprint 28 — Release candidate, beta feedback and launch

**Goal:** Ship the first production-ready release.

**Deliverables:**

- Release candidate build.
- End-to-end test suite.
- Upgrade and rollback test.
- Documentation and troubleshooting guide.
- Security and privacy documentation.
- Open Source contribution guide.
- Release notes and known limitations.
- GA release decision.

**Exit criteria:** Release checklist is complete, critical issues are closed or explicitly accepted, and a repeatable build can be produced from CI.

**Milestone:** Production Release.

## Backlog — deferred candidates

Items below are not scheduled into a sprint number yet. They are scoped enough to pull into
an existing sprint (most likely Sprint 25) or split into their own sprint once there is
concrete customer demand or the sprint sequence above is renumbered/extended. Keep this
section short — a scope draft per candidate, not a running wishlist.

### External secret manager backends (HashiCorp Vault, AWS Secrets Manager) — optional

**Origin:** User question during Sprint 7 review (2026-08-25): can ThalassaOps use Vault or
AWS Secrets Manager instead of (or alongside) the OS keychain for storing connector
credentials, as an opt-in choice.

**Goal:** Let an installation back its `CredentialStore` with a centrally managed secret
service instead of the local OS keychain, for teams that already run Vault or AWS Secrets
Manager and want rotation/audit/central revocation — without changing the default,
local-first behavior for everyone else.

**Why it fits:** `CredentialStore` (src-tauri/src/connectors.rs) is already a trait with a
single implementation, `OsKeychainCredentialStore` (`keyring` crate), plus an
`InMemoryCredentialStore` test double. Adding `VaultCredentialStore` and
`AwsSecretsManagerCredentialStore` behind the same trait requires no change to any call
site — every connector already goes through `CredentialStore::set/has/delete`.

**Deliverables (draft):**

- `VaultCredentialStore` (HashiCorp Vault KV v2, token or AppRole auth) and
  `AwsSecretsManagerCredentialStore` implementing the existing `CredentialStore` trait.
- Per-installation (not per-connector) backend selection, defaulting to
  `OsKeychainCredentialStore` — this is an installation-wide choice, not a per-connector
  toggle, to avoid splitting one workspace's credentials across inconsistent trust
  boundaries.
- The credential needed to reach Vault/AWS itself (a Vault token, an AWS credential/role)
  still has to be stored somewhere — store that root credential in the OS keychain rather
  than a config file, so enabling this feature never means writing a plaintext secret to
  disk.
- Outbound calls to Vault/AWS Secrets Manager go through the existing egress policy path
  (`EgressDestination`, `DataClass`) like every other external connector — this is a new
  egress destination, not a bypass.
- Connection health/diagnostics surfaced the same way connector health already is (reuse
  the existing connector diagnose/test pattern rather than inventing a parallel one).
- Documentation covering the tradeoff explicitly: enabling this trades local-first
  isolation for centralized rotation/audit, and requires network reachability to the
  secret service for the app to unlock any stored connector credential.

**Exit criteria (draft):** A workspace can be configured to use Vault or AWS Secrets Manager
as its credential backend instead of the OS keychain; existing OS-keychain-backed
workspaces are unaffected by default; a security review finds no path where a connector
credential is written to disk in plaintext under either backend.

**Explicitly out of scope for this candidate:** per-connector backend mixing, other secret
managers (Azure Key Vault, GCP Secret Manager, etc. — add only if asked for), and any
change to how credentials are used once retrieved (masking/redaction rules in
`kubernetes.rs` and elsewhere are unaffected).

## Milestones at a glance

| Milestone | Sprint | Outcome |
|---|---:|---|
| Foundation Demo | 4 | Desktop shell, design system, navigation and terminal surfaces |
| Read-only Operations Alpha | 10 | Kubernetes, metrics, logs, traces and three-cloud inventory |
| Read-only AI Beta | 19 | Evidence-backed, redacted investigation without mutations |
| Controlled Action Beta | 23 | Policy, approval, terminal and external incident workflows |
| Enterprise Evaluation Candidate | 26 | Team access, security hardening and scale validation |
| Production Release | 28 | Signed, documented and repeatable product release |

## Subagent operating model

Subagents are useful, but they should be placed around clear boundaries. They should not all edit the same files or independently decide architecture.

### Required roles

#### 1. Product/Domain Architect

Owns:

- Requirement interpretation.
- Domain model and contracts.
- Cross-module decisions.
- ADRs and scope control.

This role should stay with the primary agent and human owner for important decisions.

#### 2. UX/UI Designer

Owns:

- Screen flows.
- Design system.
- Accessibility and localization.
- UX acceptance criteria.

Best used heavily in Sprints 3–4, 11, 16, 20, 22, 27 and 28.

#### 3. Rust Core Implementer

Owns:

- Domain services.
- Connectors.
- Secure IPC.
- SQLite/local state.
- Policy and action execution.

Best used in Sprints 2, 5–10, 15, 17–18, 20–22 and 25–26.

#### 4. React UI Implementer

Owns:

- Shell and information architecture.
- Tables, timelines, charts and topology views.
- Incident Workspace.
- Policy Center UI.
- Localization and keyboard interaction.

Best used in Sprints 3–4, 7–9, 11–16, 19–23 and 27.

#### 5. Integration Specialist

Owns:

- Provider/API adapters.
- Authentication and capability discovery.
- Rate limits, retries and source links.
- Contract fixtures for integrations.

Best used in Sprints 5–10, 13–14, 23, 25 and 26.

#### 6. AI and Safety Specialist

Owns:

- Model gateway.
- Tool registry.
- Context optimization.
- Redaction.
- Evidence contract.
- AI evaluation fixtures.

Best used in Sprints 17–22 and 25–26.

#### 7. QA and Verification Agent

Owns:

- Test strategy.
- Contract and integration tests.
- Scenario fixtures.
- Regression checks.
- Accessibility, performance and release gates.

This agent should review every sprint, not only the final release.

#### 8. Security and Release Agent

Owns:

- Dependency/license checks.
- Secret-leak tests.
- Threat-model verification.
- macOS signing/notarization.
- Packaging and release checklist.

Best used from Sprint 2 onward in a light capacity, then heavily in Sprints 25–28.

## Recommended subagent layout per sprint

Use this pattern:

```text
Primary Agent / Human Owner
        │
        ├── Implementer Agent A: scoped feature
        ├── Implementer Agent B: independent adjacent feature
        ├── Review Agent: spec + code quality
        └── QA Agent: tests, integration and acceptance
```

Use parallel subagents only when tasks do not share mutable files or interfaces. For example, a React dashboard view and an independent connector fixture can proceed in parallel after their contracts are fixed. Rust domain contracts and React consumers should proceed sequentially when they share IPC schemas.

## Rules for coding, review and checking

1. One implementer owns one scoped task.
2. Implementers do not review their own work.
3. Every task receives an independent spec/code review.
4. Every sprint has a QA/checking pass with realistic operational fixtures.
5. Security-sensitive changes receive a dedicated security review.
6. The primary agent integrates shared contracts and resolves conflicts.
7. No subagent merges directly to the release branch.
8. Do not dispatch many agents to edit the same core files in parallel.
9. Keep an implementation ledger with task, branch, reviewer, tests and decision records.
10. Prefer small vertical slices over completing an entire technical layer before testing it.

## Recommended staffing

### Solo builder plus subagents

- 1 primary owner.
- 2–4 active implementer agents per sprint depending on independence.
- 1 reviewer agent per completed task.
- 1 QA/security agent at the end of each sprint.
- Expected calendar: approximately 12–16 months including stabilization and feedback.

### Small human team plus subagents

- 1 product/architecture owner.
- 1 Rust engineer.
- 1 React/UX engineer.
- 1 integrations/QA engineer.
- Subagents for documentation, test generation, review and connector scaffolding.
- Expected calendar: approximately 12–16 months at the stated two-week cadence, including stabilization and feedback. A compressed 8–12 month path requires parallel delivery streams and/or explicit scope reduction; it does not represent 28 sequential two-week sprints.

Subagents can increase throughput, but they do not remove the need for human decisions about security, product scope, architecture, UX quality and production readiness.

## Recommended first execution slice

Do not begin with AI or automated remediation. Establish the policy guard and identity foundation first; the first execution slice should be:

1. Pre-Sprint product/repository setup.
2. Sprint 1 identity, policy and domain contracts.
3. Sprint 2 secure local desktop shell.
4. Sprint 3 design system.
5. Sprint 4 navigation and workspace context.
6. Sprint 5 connector registry.
7. Sprint 6 Kubernetes read-only flow.

At the end of this slice, ThalassaOps should already open as a credible desktop product and let a user connect to a Kubernetes environment and inspect it safely.
