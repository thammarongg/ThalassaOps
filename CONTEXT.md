# ThalassaOps Domain Context

## Product intent

ThalassaOps is a command center for DevOps, Platform Engineers and Cloud Engineers who operate Kubernetes, infrastructure and observability systems across multiple providers and tools. Its central value is to turn fragmented operational signals into a shared, evidence-backed operational workflow.

The product is intended to be local-first, team-capable and enterprise-extensible. It should connect to existing systems rather than replace established telemetry backends.

## Canonical glossary

### Command Center

The primary ThalassaOps workspace for viewing system status, investigating operational problems, coordinating responders and controlling approved actions across connected environments.

### Workspace

A logical scope containing connected environments, integrations, users, policies, incidents and operational history. A workspace may represent a personal setup, a team or an enterprise boundary.

### Environment

An operational target or provider context, such as a Kubernetes cluster, VM estate, bare-metal fleet, cloud account, serverless platform or network environment.

### Resource

An object inside an environment that can produce signals or be investigated or acted upon. Examples include clusters, nodes, namespaces, workloads, pods, services, VMs, deployments, cloud resources and network components.

### Signal

An operational observation originating from a connected system. Signals include alerts, metrics, logs, traces, events, activity records, deployment changes and vulnerability findings.

### Alert

An individual notification emitted by an integration. An alert is not automatically an incident; multiple alerts may be correlated into one incident.

### Incident

A correlated operational problem that requires investigation, coordination, communication or action. An incident has impact, severity, status, timeline, related signals, evidence, hypotheses and actions.

### Incident Trigger

The provenance that explains why a responder explicitly created an incident. A trigger is an alert, anomaly, user report, scheduled health check, vulnerability finding or manual report; a correlation candidate may help select triggers but is not itself a trigger.

### Incident Disposition

A classification explaining how an incident was ultimately understood, separate from its operational status. Canonical dispositions are Duplicate, False Positive, Suppressed, Cancelled and Informational.

### Incident Timeline Event

An immutable, actor-attributed record of an incident creation or change. Timeline events preserve the reason, policy version and before/after state needed for audit and later incident reconstruction.

### Incident Responder Role

An explicit responsibility assigned to a principal for one incident, such as Owner, Incident Commander, Technical Lead, Communications Lead, Approver, Change Owner or Stakeholder. One principal may hold multiple roles.

### Incident Card

A concise, shareable representation of an incident for responders and management. It contains the problem summary, severity, affected scope, business or service impact, current status, evidence-backed findings, initial mitigation and next steps.

### Evidence

A source-backed fact used to support an observation, hypothesis or action. Evidence must retain its source, resource scope, timestamp or time range, query or retrieval context and a relevant excerpt or structured result.

### Investigation

The process of moving from a signal or user question to a set of evidence-backed findings, hypotheses, confidence levels and recommended next steps.

### AIOps Assistant

The AI-assisted investigation and operations capability. It gathers relevant context through scoped tools, reduces and redacts data before model use, produces structured findings and may propose actions according to policy.

### Context Optimization

The deliberate process of selecting, deduplicating, summarizing, ranking and redacting operational data before it is sent to an AI model. Its goals are evidence quality, sensitive-data protection, latency and token-cost control.

### AI Assistant Log

A user-visible record of an AI investigation, including the question or trigger, selected context, tools consulted, model/provider, output, evidence references, confidence, cost or token information and policy decisions.

### Action

An operation that changes or requests a change in an environment or external system. Examples include restarting a workload, scaling a deployment, rolling back a release, applying a manifest, triggering a pipeline, creating a Jira issue or posting to Slack. Every action has a risk class (`READ-ONLY`, `MUTATING`, `BLOCKED` or `REQUIRES APPROVAL`) and an independent execution mode (`OBSERVE`, `RECOMMEND`, `APPROVAL` or `POLICY_AUTO`).

### Policy

The centrally managed set of rules that determines which users, AI capabilities, tools, resources and action types are allowed, denied or require approval.

### Approval

An explicit human authorization for a proposed action. Approval may depend on user role, resource scope, severity, environment, action risk and required number of approvers.

### Skill

A versioned operational knowledge or runbook package that teaches the assistant how to investigate or respond to a class of problems. A Skill does not grant execution privilege by itself.

### Plugin

An installed product extension that adds a UI surface, integration or domain capability through approved ThalassaOps contracts. A Plugin may expose capabilities, but policy and capability scopes remain the authorization boundary.

### MCP

An interoperability adapter for exposing external tools and data sources to an AI assistant. MCP is a transport/interoperability boundary, not an authorization boundary; every exposed capability remains constrained by ThalassaOps policy and capability scopes.

### Severity

A 1–5 classification describing business impact, where S1 is highest impact. Severity is distinct from urgency and priority; the baseline uses availability, customer reach, business criticality, data integrity, security/privacy, financial impact and trajectory.

### Incident Status

The canonical operational lifecycle: Detected → Triage → Investigating → Mitigating → Monitoring → Resolved → Closed, with Reopened allowed when verification fails or the incident recurs.

### Data Redaction Policy

Rules governing whether data may be sent to a model, stored locally, displayed, exported or retained for audit. Secret and credential protection is immutable and external transmission fails closed when classification or redaction cannot be verified.

### Policy Center

The product surface where administrators manage versioned Severity, Incident, AI, Data Redaction, Action and Integration policies across Organization, Team, Workspace and Environment scopes.

### Autonomy

The permitted level of AI participation in an operation: observe, recommend, act with approval or act under a narrowly scoped policy. Automatic mutation is not the default product behavior.

## Confirmed product boundaries

- ThalassaOps should connect Kubernetes, VM, bare metal, cloud, serverless, network and observability systems through integrations.
- It should support Prometheus, Grafana, Loki or other log systems, OpenTelemetry, GitHub/GitLab, Argo CD, Jira, Slack/Discord and AWS/Azure/GCP as important integrations.
- The current cloud scope is AWS, Azure and GCP; Huawei Cloud is deferred.
- It should not build a replacement metrics or logs backend as its initial product direction.
- It should offer native summaries and operational views while preserving links to native tools such as Grafana.
- It should support local-first usage while leaving room for team and enterprise deployment.
- It should support multi-provider AI, including hosted providers and local models, with a custom provider option.
- The project license is Apache License 2.0.
- Team features include users, roles, policies, SSO, shared incidents and audit history.
- The default organizational hierarchy is Organization → Team → Workspace → Environment. A user or team may access multiple Organizations or Companies.
- The primary home experience is a hybrid Operations Console: health overview, active incidents, changes, anomalies and environment status, with drill-down into an Incident Workspace.
- Incident sources include alerts, anomalies, user reports, scheduled checks, vulnerabilities and manual creation.
- Incident severity is primarily based on business impact and follows the S1–S5 baseline in the Operational Policy Baseline.
- Severity, urgency and priority are separate fields; user-facing incident surfaces show both severity (S1–S5) and derived priority when available.
- Terminal access must support both an embedded terminal and external terminal handoff.
- The initial UI must support Thai and English and be designed for localization.
- Provisioning is deferred from the initial incident-control release and remains a separate future bounded context.
- The Skill/Plugin/MCP boundary is defined in the glossary above; detailed SDK contracts remain open.
- Reference images and pasted reference text are design/research material only and are not project assets.

## Remaining open domain decisions

- Organization-specific response targets, escalation rules and optional lifecycle extensions.
- Exact action allowlists and narrowly scoped `POLICY_AUTO` rules for each environment.
- Hosted AI data residency and provider-specific retention requirements.
- Boundary between Community Open Source and future Commercial/Enterprise features.
- Detailed connector SDK and packaged-plugin contract.
