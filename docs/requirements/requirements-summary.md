# ThalassaOps Requirements Summary

**Status:** Product requirements baseline  
**Updated:** 2026-08-24  
**Product:** ThalassaOps

## 1. Product vision

ThalassaOps is a local-first, cross-platform AIOps command center for DevOps, Platform Engineers and Cloud Engineers. It provides one operational experience across Kubernetes, VMs, bare metal, cloud providers, serverless, network systems and observability tools.

ThalassaOps does not initially replace Prometheus, Grafana, Loki or other telemetry backends. It connects the systems teams already use, normalizes their operational context and gives users a coherent path from signal to evidence-backed decision to governed action.

### Product promise

> Turn fragmented operational signals into an evidence-backed incident workflow, then help teams take safe and auditable action from one command center.

## 2. Problem statement

Operators currently switch between provider consoles, Kubernetes GUIs, CLI tools, monitoring, logging, tracing, ticketing, chat and AI assistants. Each provider and tool exposes a different workflow and vocabulary. This creates:

- Tool sprawl and context switching.
- Different troubleshooting procedures for AWS, Azure, GCP and on-premises systems.
- Slow correlation between alerts, logs, metrics, traces, deployments and topology.
- AI answers that may hallucinate, omit relevant context or consume excessive tokens.
- Risk of exposing secrets and sensitive production data to AI providers.
- Difficulty producing a shared incident narrative for responders and management.

## 3. Users and stakeholders

### Primary users

- DevOps Engineers
- Platform Engineers
- Cloud Engineers

### Secondary users

- Security Engineers reviewing incidents and vulnerabilities.
- Engineering Managers and technical stakeholders consuming incident summaries.
- Incident Commanders and service owners.

## 4. Organizational model

The default hierarchy is:

```text
Organization
└── Team
    └── Workspace
        └── Environment
            └── Resources and Integrations
```

A user or team may access multiple Organizations or Companies. A Workspace groups integrations, environments, incidents, policies, AI providers and operational history.

## 5. Operational scope

ThalassaOps targets:

- Kubernetes clusters and workloads.
- VMs and bare-metal systems.
- AWS, Azure and GCP. Huawei Cloud is deferred from the current scope.
- Serverless platforms.
- Network infrastructure, load balancers, DNS, gateways and firewalls.
- Prometheus, Grafana, Loki, OpenTelemetry and other observability systems.
- Source control, CI/CD, GitOps and deployment systems.
- Incident management, ITSM, collaboration and security systems.

The product may eventually support cluster and infrastructure provisioning, but provisioning should remain a distinct bounded context from incident investigation and operations.

## 6. Capability priorities

This is the canonical product priority list. The system requirements and sprint plan must preserve the same ordering, even when one delivery slice combines adjacent capabilities.

1. Cluster Management.
2. Monitoring, Logging and Tracing.
3. Anomaly Detection.
4. Alert Correlation.
5. Root Cause Analysis.
6. Troubleshooting Assistant.
7. Compliance and Security.
8. Automated Remediation under policy control.
9. Capacity, cost and reliability insights.
10. Extensible integrations, Skills, Plugins and MCP.

## 7. Core workflow

```text
Alert / Anomaly / User Report / Scheduled Check / Vulnerability / Manual Creation
        ↓
Normalize and correlate signals
        ↓
Estimate business impact and severity
        ↓
Curate, optimize and redact context
        ↓
AI investigation with scoped tools
        ↓
Evidence-backed findings and hypotheses
        ↓
Incident Card, recommendation or escalation
        ↓
Policy and approval decision
        ↓
Execute, verify, communicate and audit
```

Example: when a production workload enters `CrashLoopBackOff`, ThalassaOps should inspect resource state, events, logs, metrics, traces, recent changes and dependencies; reduce irrelevant data; protect secrets; explain probable causes with evidence; estimate business impact; propose a safe next step; and create or update an Incident if human coordination is required.

## 8. Incident requirements

### 8.1 Incident sources

- Alert.
- Anomaly.
- User report.
- Scheduled health check.
- Vulnerability finding.
- Manual incident creation.

Each source must have a producer before it is used by a later incident or AI exit criterion. The initial delivery plan provides anomaly and scheduled-check producers in Sprint 11 and normalized vulnerability-finding ingestion in Sprint 13.

### 8.2 Severity baseline

Severity is based on Business Impact and is separate from Urgency and Priority.

| Severity | Meaning |
|---|---|
| S1 Critical | Critical production outage, active credential compromise, confirmed customer-data exposure or destructive data loss |
| S2 Major | Major production capability unavailable or significantly degraded for a meaningful customer group, region or business process |
| S3 Moderate | Limited customer/internal impact with a workaround or material degradation of a non-critical capability |
| S4 Minor | Isolated, low-risk issue with no material business impact |
| S5 Informational | No current impact; observation, hygiene item, planned work or low-risk finding |

The highest matching impact dimension sets the initial severity. Severity can be upgraded or downgraded with an explanation and audit record.

### 8.3 Incident lifecycle

```text
Detected → Triage → Investigating → Mitigating → Monitoring → Resolved → Closed
                         ↑                 │
                         └── Reopened ─────┘
```

`Duplicate`, `False Positive`, `Suppressed` and `Cancelled` are dispositions rather than lifecycle statuses.

### 8.4 Incident workspace

Every incident should expose:

- Business impact and severity.
- Owner, team and incident roles.
- Timeline.
- Related alerts and correlated signals.
- Logs, metrics and traces.
- Topology and blast radius.
- Recent changes and deployments.
- Vulnerabilities and security context.
- AI findings, hypotheses and confidence.
- Evidence references and source queries.
- Proposed actions and approval state.
- Communications to Slack, Discord, Jira or PagerDuty.
- Audit history and post-incident follow-up.

## 9. AI requirements

### 9.1 Providers

- OpenAI API.
- Anthropic API.
- Google Gemini.
- Ollama.
- vLLM.
- Local models.
- OpenAI-compatible custom endpoints.
- Provider-neutral custom model interface.

### 9.2 Investigation behavior

The AI must:

- Use scoped tools rather than arbitrary shell access.
- Prefer curated evidence over raw context dumps.
- Show sources, timestamps, queries and relevant excerpts.
- Return findings, hypotheses, confidence and next steps in a structured form.
- State when evidence is missing or contradictory.
- Track context size, token usage and estimated cost where available.
- Distinguish observation, recommendation and mutation.
- Never act as the final authorization layer.

### 9.3 Extensibility

- User-defined system behavior through policy-managed instructions.
- Skills for reusable investigation knowledge and runbooks.
- Plugins for UI, integrations and domain capabilities.
- MCP adapters for interoperable tools and data.

## 10. Action and remediation requirements

Actions that change or directly affect clusters, pods, infrastructure or external systems are high-risk by default.

Every action must be classified as:

- `READ-ONLY`
- `MUTATING`
- `BLOCKED`
- `REQUIRES APPROVAL`

Risk classification and execution mode are separate fields. The risk class describes what an action can do; the execution mode describes how the policy permits it to run:

- `OBSERVE` — inspect or evaluate only.
- `RECOMMEND` — produce a proposed action for a human.
- `APPROVAL` — execute only after the required approval decision.
- `POLICY_AUTO` — execute a narrowly scoped, reversible mutation under an explicit policy; disabled by default and never authorized by the model itself.

`POLICY_AUTO` is only valid for a `MUTATING` action that passes resource, environment, blast-radius, cooldown, rollback and post-action verification checks. If any check fails, the action falls back to `REQUIRES APPROVAL` or is blocked.

Required safety features:

- Policy scope.
- Resource and environment scope.
- Dry-run where supported.
- Expected impact.
- Approval requirement.
- Rollback or recovery plan.
- Post-action verification.
- Full audit record.

## 11. Data and security requirements

### 11.1 Data handling classes

- Public.
- Internal.
- Confidential.
- Restricted.

### 11.2 Immutable restricted data

Secrets, passwords, API keys, tokens, cookies, private keys, Kubernetes Secret values, cloud credentials, database credentials, webhook secrets, encryption keys and regulated data must never be sent to Hosted AI providers.

### 11.3 Separate data policies

ThalassaOps must manage separate policies for:

- Sending to a model.
- Local storage.
- UI display.
- Export to integrations.
- Audit retention.

If classification, redaction or egress validation fails, external transmission must fail closed.

### 11.4 AI Assistant Log

The log should store model/provider identity, policy versions, source references, redaction decisions, context fingerprints, tool calls after redaction, output, confidence and proposed/executed actions. It must not store raw secrets or unredacted prompts by default.

## 12. Team and enterprise requirements

- User management.
- Organization and Team membership.
- Workspace access control.
- Roles and policies.
- SSO/OIDC.
- Shared incidents.
- Assignment and comments.
- Approval workflows.
- Audit logging.
- Integration access control.
- Environment and resource scoping.
- Policy versioning and rollback.

## 13. UX/UI requirements

- Hybrid Operations Console as the primary home.
- Incident Workspace as the primary deep-work surface.
- Dark mode as the default.
- Accessible status indicators that do not depend on color alone.
- Beginner-friendly summaries with expert escape hatches.
- Native links to Grafana and provider consoles.
- `⌘K` command palette.
- Embedded and external terminal support.
- Thai and English localization from the beginning.
- Configurable dashboard widgets without allowing dashboard customization to hide critical incidents.
- Evidence, policy and action state visible beside AI output.

## 14. Integration catalogue

### Foundation

Kubernetes, Prometheus, Alertmanager, Grafana, Loki, OpenTelemetry, GitHub/GitLab, Argo CD, Jira, Slack, AWS, Azure and GCP, plus Trivy, Falco, Kyverno and OPA Gatekeeper for initial normalized security-finding ingestion.

### Expansion

OpenSearch, Elasticsearch, OpenObserve, SigNoz, Jaeger, Tempo, PagerDuty, Discord, Jenkins, Flux, Microsoft Teams and additional security scanners beyond the initial finding sources.

### Enterprise and provider expansion

Vault, AWS Secrets Manager, Azure Key Vault, Google Secret Manager, cloud security services, network platforms, FinOps systems and additional ITSM systems. Huawei Cloud may be reconsidered in a future provider expansion.

## 15. Technical direction

- Tauri 2 desktop shell.
- Rust core for domain logic, connectors, local state, policy enforcement and action execution.
- Tokio for asynchronous work.
- kube-rs for Kubernetes access.
- OTLP/HTTP and OTLP/gRPC integration.
- SQLite for local metadata, cache, policies, incident history and audit state.
- React, TypeScript and Vite for the UI.
- Provider-neutral AI and tool registry.
- Secure IPC with capability-scoped commands.
- macOS-first with Windows and Linux cross-platform support.

## 16. Product boundaries

ThalassaOps should not initially:

- Replace Prometheus, Grafana, Loki or established telemetry backends.
- Send raw secrets or unrestricted production context to AI providers.
- Allow an AI model to authorize its own mutation.
- Force users to abandon native provider tools.
- Hide raw queries from expert users.

Provisioning remains a possible future bounded context, not part of the initial incident-control domain.

## 17. Open decisions remaining

- Organization-specific response targets, escalation rules and optional lifecycle extensions.
- Exact action allowlists and narrowly scoped `POLICY_AUTO` rules for each environment.
- Hosted AI data residency and provider-specific retention requirements.
- Boundary between Community Open Source and future Commercial/Enterprise features.
- Detailed connector SDK and packaged-plugin contract.

## 18. Licensing decision

The project license is **Apache License 2.0**. Apache-2.0 is the intended license for the core product, Rust crates, React packages, connector SDK and community extensions unless a separate compatible license is explicitly documented.

## 19. Recommended product sequence

1. Policy and identity foundation (Pre-Sprint–Sprint 2), followed by the full Policy Center governance surface in Sprint 20.
2. Operations Console and connector health.
3. Kubernetes, Prometheus, Alertmanager, Grafana, Loki and OpenTelemetry.
4. Incident lifecycle, evidence model and Incident Workspace.
5. AI investigation with redaction and context optimization.
6. GitHub/GitLab, Argo CD, Jira and Slack workflows.
7. Topology, anomaly detection and change correlation.
8. Approval-gated actions, verification and rollback.
9. Security, compliance, additional clouds and enterprise controls.
10. Provisioning and broader automation as a separate bounded context.
