# ThalassaOps System Requirements — Working Baseline

**Status:** Requirements discovery in progress  
**Updated:** 2026-08-24

## 1. Product problem

Operators currently move between Kubernetes GUIs, provider consoles, CLI tools, monitoring systems, log systems, incident tools and AI chat. Each provider exposes different operational concepts and manuals. This increases investigation time, causes context switching and makes it difficult to maintain a reliable incident narrative.

ThalassaOps should act as an all-in-one operational command center without requiring a new metrics or logs backend. It should unify access, context and workflow while allowing users to keep their existing data planes.

## 2. Target users

### Primary users

- DevOps Engineer
- Platform Engineer
- Cloud Engineer

### Secondary users

- Security Engineer reviewing incidents or vulnerabilities
- Engineering Manager or incident stakeholder consuming Incident Cards and status updates

## 3. Operational scope

The target operating landscape includes:

- Kubernetes
- VM and bare-metal environments
- AWS, Azure and GCP
- Serverless and network environments
- Prometheus, Grafana, Loki and other log platforms
- OpenTelemetry
- GitHub and GitLab
- Argo CD and CI/CD systems
- Jira or related issue boards
- Slack and Discord

## 3.1 Integration catalogue

Integrations should be grouped by operational purpose. The following catalogue is a candidate target set, not a requirement to implement all connectors at once.

### Core infrastructure and control

- Kubernetes API and kubeconfig
- AWS: EKS, EC2, CloudWatch, CloudTrail and IAM context
- Azure: AKS, Virtual Machines, Azure Monitor and Activity Logs
- GCP: GKE, Compute Engine, Cloud Monitoring, Cloud Logging and Audit Logs
- VM and bare-metal access through agent, SSH or provider APIs
- Serverless: AWS Lambda, Azure Functions and Google Cloud Functions/Run
- Network: cloud network APIs, load balancers, DNS, gateways and firewall/security-group context

### Observability and telemetry

- Prometheus
- Alertmanager
- Grafana
- Loki
- OpenTelemetry Collector and OTLP
- Elasticsearch / Elastic Observability
- OpenSearch
- OpenObserve
- SigNoz
- Jaeger and Grafana Tempo for tracing
- Fluent Bit or Fluentd for log routing context

### Delivery and change intelligence

- GitHub and GitHub Actions
- GitLab and GitLab CI/CD
- Argo CD
- Jenkins
- Flux CD
- Docker registries and image metadata
- Terraform / OpenTofu plan and state metadata

### Incident, collaboration and work management

- PagerDuty
- Jira and Jira Service Management
- GitHub Issues and GitLab Issues
- Slack
- Discord
- Microsoft Teams
- Email and webhook endpoints

### Security and compliance

- Trivy
- Falco
- Kyverno
- OPA Gatekeeper
- AWS Security Hub / GuardDuty
- Microsoft Defender for Cloud
- Google Security Command Center
- Vulnerability scanners and policy engines through a connector contract

### Secrets and identity context

- HashiCorp Vault
- AWS Secrets Manager
- Azure Key Vault
- Google Secret Manager
- OIDC / SSO identity providers

### AI providers

- OpenAI API
- Anthropic API
- Google Gemini
- Ollama
- vLLM
- OpenAI-compatible custom endpoints
- Local model runtimes exposed through a provider contract

### Suggested integration sequencing

**Foundation:** Kubernetes, Prometheus, Alertmanager, Grafana, Loki, OpenTelemetry, GitHub/GitLab, Argo CD, Jira, Slack and AWS/Azure/GCP, plus Trivy, Falco, Kyverno and OPA Gatekeeper for initial normalized security-finding ingestion.  
**Expansion:** OpenSearch/Elastic, PagerDuty, Discord, Jenkins, Flux, Tempo/Jaeger, additional security scanners and cloud-native security services.  
**Provider and enterprise expansion:** Microsoft Teams, Vault/Key Vault/Secret Manager, advanced network providers, FinOps and additional ITSM systems. Huawei Cloud is deferred and out of the current product scope.

## 4. Capability priorities

The canonical priority order is defined in [Requirements Summary §6](requirements-summary.md#6-capability-priorities) and is repeated here for implementation traceability:

1. Cluster Management
2. Monitoring, Logging and Tracing
3. Anomaly Detection
4. Alert Correlation
5. Root Cause Analysis
6. Troubleshooting Assistant
7. Compliance and Security
8. Automated Remediation under policy control
9. Capacity, cost and reliability insights
10. Extensible integrations, Skills, Plugins and MCP

Other AIOps and DevOps capabilities remain relevant but should be sequenced around these priorities.

## 5. Core operational workflow

```text
Alert / Anomaly / User Report / Scheduled Check / Vulnerability / Manual Creation
        ↓
Normalize and correlate
        ↓
Assign severity 1–5
        ↓
Curate, optimize and redact context
        ↓
AI investigation with scoped tools
        ↓
Evidence-backed findings and hypotheses
        ↓
Recommend action or create Incident Card
        ↓
Policy and approval decision
        ↓
Execute, verify, record and communicate
```

Example: a `CrashLoopBackOff` on a workload should cause ThalassaOps to inspect relevant Kubernetes status, events, logs, recent changes and related telemetry; reduce unnecessary context; identify likely causes with evidence; present severity and impact; and either propose a safe action or create an Incident Card for escalation.

## 6. Functional requirements — confirmed direction

### 6.1 Command center

- Show status across connected environments from one workspace.
- Navigate from an environment or resource into monitoring, logging, activity, incidents and AI investigation history.
- Support native-tool links, including Grafana external links.
- Provide search, filtering, saved views and a command palette.
- Provide an integrated terminal surface or terminal handoff for expert workflows.

### 6.2 Incident operations

- Receive signals from multiple sources.
- Accept alerts, anomalies, user reports, scheduled health checks, vulnerability findings and manual incident creation.
- Correlate related alerts into incidents.
- Classify incidents with severity 1–5.
- Maintain a timeline of signals, changes, findings, actions and communications.
- Generate a management-readable Incident Card.
- Support shared incidents, assignment, comments, status updates and audit history.
- Create or update Jira issues and related boards.
- Send incident updates to Slack or Discord.

### 6.3 AI investigation

- Accept manual questions, alerts, anomalies, scheduled checks and vulnerability findings as investigation triggers.
- Use provider-neutral model access for OpenAI, Anthropic, Gemini, Ollama, vLLM, local models and custom providers.
- Query only scoped, relevant tools and sources.
- Optimize context through filtering, deduplication, summarization and sensitive-data redaction.
- Return structured findings, hypotheses, confidence, evidence and recommended actions.
- Expose an AI Assistant Log for explainability, cost awareness and audit.
- Support user-defined system behavior through policies, Skills, Plugins and MCP.

### 6.4 Actions and remediation

- Support read-only investigation operations.
- Support proposed operational changes.
- Support approval-gated execution for permitted mutations.
- Support central settings for action classes and autonomy level.
- Keep risk class (`READ-ONLY`, `MUTATING`, `BLOCKED`, `REQUIRES APPROVAL`) separate from execution mode (`OBSERVE`, `RECOMMEND`, `APPROVAL`, `POLICY_AUTO`).
- Permit `POLICY_AUTO` only for narrowly scoped, reversible, low-blast-radius mutations explicitly enabled by policy; default to disabled and fall back to approval when checks fail.
- Preserve dry-run, expected impact, verification and audit information where supported.
- Do not use the model itself as the authorization layer.
- Treat direct cluster or pod changes and commands as high-risk mutations by default.
- Clearly classify command and action surfaces as `READ-ONLY`, `MUTATING`, `BLOCKED` or `REQUIRES APPROVAL`.

## 7. Non-functional requirements — initial direction

- Local-first operation and useful behavior when remote connectivity is degraded.
- Enterprise-ready path for team management, SSO, roles, policies and shared operational history.
- Strong protection for kubeconfig, cloud credentials, API keys, logs and sensitive telemetry.
- Cross-provider abstraction without hiding provider-specific escape hatches.
- Explainable AI outputs with source evidence and timestamps.
- Predictable token use and configurable model/provider budgets.
- Accessible dark-mode-first UI suitable for dense technical workflows.
- Thai and English localization from the beginning, with a path to additional languages.
- Full auditability of user actions, AI tool calls, approvals and mutations.
- Extensible integration model for connectors, plugins, Skills and MCP.
- Read-only capacity, cost and reliability insights are in the initial product scope; full FinOps-system integrations remain an expansion path.

## 8. Product-shaping benchmark observations

PagerDuty's current AIOps materials reinforce several requirements already identified: normalize and enrich events, suppress/group noise, surface recent changes and probable origin, provide a live operations console and connect diagnosis/remediation to incident response. ([PagerDuty AIOps](https://support.pagerduty.com/main/docs/aiops), [Automated Event Management](https://www.pagerduty.com/use-cases/automated-event-management/), [Event Orchestration](https://support.pagerduty.com/main/docs/event-orchestration))

A practitioner example on LinkedIn describes an AI incident responder that uses read-only Kubernetes commands, analyzes logs/events/deployment history and posts a structured RCA to Slack. This is useful as a workflow reference, not as a validated requirement or architecture decision. ([LinkedIn practitioner example](https://www.linkedin.com/pulse/now-live-ai-incident-responder-autonomous-investigation-production-t5psc))

## 9. Remaining open decisions

The following are the only unresolved product decisions in this baseline:

1. Organization-specific response targets, escalation rules and optional lifecycle extensions.
2. Exact action allowlists and narrowly scoped `POLICY_AUTO` rules for each environment.
3. Hosted AI data residency and provider-specific retention requirements.
4. Boundary between Community Open Source and future Commercial/Enterprise features.
5. Detailed connector SDK and packaged-plugin contract.

The hybrid Operations Console home, six incident sources, embedded plus external terminal, baseline redaction/fail-closed behavior, initial integration tiers, deferred provisioning and deferred Huawei Cloud scope are resolved elsewhere in this document and are not open questions.

## 10. Accepted policy baseline

The canonical baseline is defined in [Operational Policy Baseline](../policies/operational-policy-baseline.md). It establishes:

- S1–S5 severity based on business impact.
- Separate Severity, Urgency and Priority concepts.
- Detected → Triage → Investigating → Mitigating → Monitoring → Resolved → Closed lifecycle.
- Reopened transitions and separate dispositions such as Duplicate, False positive and Suppressed.
- Immutable blocking of secrets and credentials from hosted AI providers.
- Fail-closed behavior when classification, redaction or egress validation cannot be completed.
- Separate policies for model transmission, local storage, UI display, integration export and audit retention.
- Versioned, testable and reversible Policy Center configuration.

## 11. Licensing decision

ThalassaOps will use **Apache License 2.0** as the project license. The repository should include the official Apache-2.0 license text and SPDX metadata when the application scaffold is created.
