# ThalassaOps UX/UI Concept — Operations Console

**Status:** Concept for requirement validation  
**Updated:** 2026-08-24

## Design thesis

ThalassaOps should feel like a calm command bridge over a noisy operational ocean. The interface should answer three questions in order:

1. What is affected right now?
2. What evidence explains it?
3. What can I safely do next?

The product should not present a wall of dashboards. It should present an operational narrative that starts with impact and lets an expert dive into raw signals when needed.

## Primary information architecture

```text
Organization
└── Team
    └── Workspace
        └── Environment
            ├── Cluster / VM / Bare Metal / Cloud / Serverless / Network
            ├── Signals
            ├── Incidents
            ├── Changes
            ├── Vulnerabilities
            └── Policies and Actions
```

## Global application shell

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│  ThalassaOps  [Org ▾] [Team ▾] [Workspace ▾] [Environment ▾]  ⌘K  AI  Terminal │
├──────────────┬─────────────────────────────────────────────────┬─────────────┤
│ Command      │                                                 │ AI / Help   │
│ Center       │                 Primary work area               │ Desk        │
│              │                                                 │             │
│ Incidents    │                                                 │ Context     │
│ Environments │                                                 │ budget      │
│ Observability│                                                 │ Sources     │
│ Changes      │                                                 │ Findings    │
│ Vulnerability│                                                 │             │
│ Automations  │                                                 │             │
│ Integrations │                                                 │             │
│ Policies     │                                                 │             │
│ Audit        │                                                 │             │
└──────────────┴─────────────────────────────────────────────────┴─────────────┘
```

The right panel is contextual rather than permanently dominant. It can collapse into a drawer, open as a helpdesk conversation or be replaced by evidence details.

## Home: Operations Console

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Operations Console                                      Last sync: 12 sec ago │
│ 12 environments · 4 providers · 2 teams                          ⌘K Search    │
├───────────────────┬────────────────────┬─────────────────────────────────────┤
│ SERVICE HEALTH     │ ACTIVE INCIDENTS   │ CHANGE & ANOMALY STREAM              │
│ 98.7% healthy      │ S1 / P1  1         │ 14:32 deploy/payment-api v42        │
│ 3 degraded         │ S2 / P2  3         │ 14:29 CPU anomaly · prod-cluster-1  │
│ 1 unavailable      │ S3 / P3  8         │ 14:18 new vulnerability · 2 assets  │
├───────────────────┴────────────────────┬─────────────────────────────────────┤
│ INCIDENT QUEUE                          │ ENVIRONMENT MAP                      │
│                                        │                                     │
│ S1 / P1 Checkout API                    │ AWS / prod-us-east                   │
│ CrashLoopBackOff · 8 min                │ GCP / prod-asia                      │
│ Customer impact: high                   │ Azure / staging                      │
│ [Open investigation]                    │ Bare metal / legacy                  │
│                                        │                                     │
│ S2 / P2 Payment latency                 │ click environment → resource map   │
│ probable origin: payment-db             │                                     │
├────────────────────────────────────────┴─────────────────────────────────────┤
│ AI INVESTIGATION QUEUE                                                       │
│ 3 recommendations waiting for review · 2 context jobs running · 0 mutations  │
└──────────────────────────────────────────────────────────────────────────────┘
```

The home view is optimized for triage, not deep analysis. The most important visual hierarchy is business impact and incident urgency, not raw CPU or memory charts. `S1–S5` is severity based on business impact; `P1–P5` is the separately derived operational priority.

## Incident Workspace

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ S1 Critical · P1  Checkout API failing           [Investigating] [Share] [...]│
│ Customer impact: High · 8 min · AWS/prod-us-east · Owner: Platform Team       │
├──────────────────────────────┬──────────────────────────────┬────────────────┤
│ INCIDENT NARRATIVE            │ EVIDENCE                      │ ACTIONS        │
│                              │                              │                │
│ 14:24 Alert received         │ 01 Kubernetes Events          │ Restart pod    │
│ 14:25 AI investigation       │ 02 Logs: OOMKilled             │ [Dry run]      │
│ 14:26 deploy detected        │ 03 Metrics: memory +42%       │                │
│ 14:28 hypothesis updated     │ 04 Change: release v42         │ Rollback       │
│                              │ [Open native source]           │ [Approval req.]│
│ Probable origin              │                              │                │
│ payment-api deployment v42   │ AI confidence: 87%             │ Policy         │
│                              │ Evidence coverage: 4/5         │ Allowed        │
│ Suggested next step          │ Sensitive fields redacted: 12 │ 2 approvals    │
│ verify memory limit change   │                              │                │
├──────────────────────────────┴──────────────────────────────┴────────────────┤
│ SIGNALS  Logs · Metrics · Traces · Events · Changes · Vulnerabilities         │
├──────────────────────────────────────────────────────────────────────────────┤
│ AI Assistant / Helpdesk                                                        │
│ “Why did this incident start?”                                                 │
│ [Ask]  Context: Curated · 24% budget · 8 sources · No mutations               │
└──────────────────────────────────────────────────────────────────────────────┘
```

The incident workspace keeps the AI answer beside the evidence and action controls. An AI finding without an evidence link is incomplete and should not be displayed as a root-cause conclusion. Severity (`S1–S5`) and derived priority (`P1–P5`) are shown as separate fields; one must not be used as a label for the other.

## AI Helpdesk interaction

The assistant should not be a generic chat window. Each response should expose:

- Finding or hypothesis
- Confidence
- Evidence references
- Sources queried
- Data omitted or redacted
- Context/token budget
- Suggested next step
- Whether the answer is read-only or proposes a mutation

Example response:

```text
Probable cause · 87% confidence

payment-api release v42 increased memory usage. Three pods were OOMKilled
within 6 minutes of deployment. The deployment changed the image and memory
request but not the memory limit.

Evidence  [K8s Events] [Pod Logs] [Deployment Diff] [Prometheus]
Missing context  Recent application trace data is unavailable.
Next step  Compare v41 and v42 memory behavior.
Action  Read-only investigation; no mutation proposed.
```

## Terminal interaction

Support both modes:

1. Embedded terminal in a resizable bottom drawer.
2. External terminal handoff with a generated, copyable command and context link.

Every command shown by ThalassaOps should display its risk classification: `READ-ONLY`, `MUTATING`, `BLOCKED`, or `REQUIRES APPROVAL`. If execution is governed by a policy mode, show `OBSERVE`, `RECOMMEND`, `APPROVAL` or `POLICY_AUTO` separately.

## Visual direction

### Palette

- Abyss `#071A2B` — main application background
- Deep Water `#0E2C3D` — panels and navigation
- Sea Glass `#DCEFF0` — primary text and surfaces
- Reef Cyan `#53D7E8` — focus, active links and telemetry
- Kelp `#50D18C` — healthy and verified
- Amber `#E4B65A` — warning and approval pending
- Coral `#FF6B6B` — critical impact and blocked action

### Typography

- UI and headings: Manrope
- Telemetry, identifiers and commands: IBM Plex Mono
- Avoid all-caps for long labels; use sentence case and short operational verbs.

### Signature element

The signature visual is the **Evidence Tide Line**: a thin, calm timeline that shows how signals, changes, hypotheses and actions move through an incident. It replaces decorative wave graphics with a functional metaphor tied to ThalassaOps' ocean identity.

## Interaction rules

- Use impact and severity before infrastructure detail.
- Show severity (`S1–S5`) separately from derived priority (`P1–P5`) wherever both are available.
- Never use color alone to communicate severity or status.
- Keep read-only investigation visually separate from mutation.
- Show confirmation and expected impact before approval.
- Keep raw queries and native-tool links available to expert users.
- Provide keyboard navigation and `⌘K` command palette.
- Support Thai and English UI strings from the beginning; avoid hard-coded text in domain components.
- Respect reduced-motion settings and keep animations meaningful.

## Inspiration analysis

The supplied screenshots are treated as visual references only. No screenshot or extracted asset is part of the project.

### Patterns worth learning

#### ServiceNow AIOps

- Enterprise application shell with global navigation, workspace context, search and administration.
- Dashboard tabs separate operational concerns such as events, agent health and HLA/service performance.
- KPI cards show a headline number, comparison to a previous period and a drill-down affordance.
- Large analytical views are useful for leadership and operational trend analysis.

**ThalassaOps adaptation:** use the shell and context-switching discipline, but replace generic dashboard sprawl with a curated Operations Console and an incident-first drill-down.

#### BigPanda

- Strong incident-centric split view: incident list on the left, selected incident details on the right.
- Severity, priority, affected applications, sources and ownership are visible without opening multiple pages.
- AI analysis is presented inside the incident rather than as a disconnected chatbot.
- Tabs such as Overview, Alerts, Topology and Changes create a clear investigation path.

**ThalassaOps adaptation:** this is the strongest reference for the Incident Workspace. Add Evidence, AI Assistant Log, Context Budget and Policy/Approval as first-class tabs or panels.

#### AppDynamics and LogicMonitor

- Domain views group signals by applications, services, hosts, databases and networks.
- Topology and dependency views help users move from a symptom to a possible origin.
- Cards can combine status, trend, ranking and a small visualization without forcing a full dashboard page.
- Dark operational views work well when many signals must be scanned quickly.

**ThalassaOps adaptation:** use a resource hierarchy and topology layer, but preserve a common operational vocabulary across AWS, Azure, GCP, Kubernetes and bare metal. Huawei Cloud is deferred from the current scope.

#### IBM Cloud Pak for AIOps

- Onboarding and connection status are visible early.
- AIOps model management, data/tool connections, incidents and automation are separate navigational concepts.
- Dark mode with restrained cards can support a technical command-center feel.

**ThalassaOps adaptation:** make connector health, model/provider health and policy state visible before users trust AI recommendations.

### Patterns to avoid copying directly

- A dashboard made only of gauges and charts without a clear next action.
- Provider-specific tabs that force users to learn a different interface for every cloud.
- Large enterprise navigation trees that hide the active incident.
- AI summaries without source evidence, confidence, context budget or data-redaction status.
- Dense multicolor charts that communicate status only through color.

## Refined UX structure

```text
Global shell
  ├── Organization / Team / Workspace / Environment switcher
  ├── Global search and ⌘K command palette
  ├── AI Helpdesk
  ├── Embedded or external Terminal
  └── User, language, policy and connector status

Operations Console
  ├── Impact and health summary
  ├── Incident queue
  ├── Noise and correlation trends
  ├── Change and anomaly stream
  ├── Environment/resource overview
  └── AI investigation queue

Incident Workspace
  ├── Overview / narrative
  ├── Alerts and correlated signals
  ├── Evidence
  ├── Topology
  ├── Changes
  ├── Vulnerabilities
  ├── AI Assistant Log
  ├── Proposed actions and approvals
  └── Audit and communications
```

The resulting product should feel familiar to enterprise operations users while remaining more focused, explainable and cross-provider than the reference dashboards.
