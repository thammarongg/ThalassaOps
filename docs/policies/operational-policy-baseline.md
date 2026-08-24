# ThalassaOps Operational Policy Baseline

**Status:** Accepted baseline for product design  
**Updated:** 2026-08-24  
**Scope:** Severity Matrix, Incident Lifecycle, Data Redaction and Action Autonomy

This policy is the safe default shipped with ThalassaOps. Organizations may customize it through Policy Center, but customization must not weaken the immutable secret-protection rules.

## 1. Policy model

Policies are configuration data, not hard-coded behavior.

```text
System Baseline (immutable safety rules)
        ↓
Organization Policy
        ↓
Team Policy
        ↓
Workspace Policy
        ↓
Environment / Integration Policy
```

Resolution rules:

1. The most specific applicable policy supplies the value.
2. A lower-level policy may restrict a higher-level policy but may not weaken an immutable safety rule.
3. A deny decision wins over an allow decision for data egress and mutation.
4. Every policy change is versioned, attributed, timestamped and reversible.
5. A policy must support preview, validation and an effective-policy explanation before activation.

## 2. Severity Matrix

### 2.1 Severity semantics

Severity measures **business impact**, not technical excitement. The lower the number, the more severe the incident.

Priority and urgency are separate concepts:

- **Severity:** how much impact the incident has or could have.
- **Urgency:** how quickly the impact is expanding or how quickly action is required.
- **Priority:** the operational ordering derived from severity, urgency and policy.

The AI may propose a severity with evidence. A responder or policy must be able to confirm or override it.

### 2.2 Baseline levels

| Level | Name | Business impact | Default response target | Examples |
|---|---|---|---|---|
| S1 | Critical | Critical production service is unavailable or materially harming customers, business continuity, security or data integrity | Engage immediately; first acknowledgement within 5 minutes; updates at least every 15 minutes | Full checkout outage, confirmed customer-data exposure, active credential compromise, destructive data corruption |
| S2 | Major | Important production capability is unavailable or severely degraded for a significant customer group, region or business process | Acknowledge within 15 minutes; updates at least every 30 minutes | Payment unavailable in one region, sustained high error rate for a major service, critical vulnerability actively exploited but contained |
| S3 | Moderate | Limited customer or internal impact with a workaround, or meaningful degradation of a non-critical capability | Acknowledge within 1 business hour; updates at least every 4 hours | One service degraded, partial availability issue, repeated workload failures with a working fallback |
| S4 | Minor | No material customer impact; isolated resource issue, low-risk degradation or routine operational defect | Triage within 1 business day | One non-production pod failing, isolated node warning, capacity trend below intervention threshold |
| S5 | Informational | No current service impact; observation, hygiene item, planned work or low-risk finding | Track in backlog or scheduled work | Early capacity signal, informational vulnerability with no exposure, planned maintenance observation |

### 2.3 Impact dimensions

The highest matching dimension sets the initial severity. One S1 condition is sufficient for S1.

- **Availability:** total outage, critical function failure, partial degradation or no outage.
- **Customer reach:** all customers, significant segment/region, limited segment, internal only or none.
- **Business criticality:** Tier 0/mission-critical, Tier 1/important, Tier 2/supporting or non-production.
- **Data integrity:** confirmed loss/corruption, suspected integrity issue, recoverable inconsistency or none.
- **Security and privacy:** active compromise, confirmed exposure, credible suspected exposure, policy violation or none.
- **Financial/contractual impact:** material revenue loss, SLA/contract breach, limited impact or none.
- **Trajectory:** expanding rapidly, stable, reducing or unknown.

Safety rules:

- Confirmed active credential compromise, customer-data exposure or destructive data loss is at least S1.
- A rapidly expanding production incident with unknown scope is at least S2 until triage proves otherwise.
- A vulnerability is not automatically S1; exploitability, exposure, asset criticality and evidence of active exploitation determine severity.
- A severity may be upgraded or downgraded as evidence changes. The reason and actor must be recorded.

### 2.4 Priority calculation

ThalassaOps should not collapse severity and priority into one field.

```text
Priority = policy(severity, urgency, service criticality, customer reach, time window)
```

The initial product preset maps S1/S2 to urgent handling, but an Organization can define business-hour, maintenance-window and escalation differences without changing the underlying severity.

## 3. Incident Status State Machine

### 3.1 Canonical statuses

```text
Detected
   ↓ acknowledge / triage
Triage
   ↓ investigation starts
Investigating
   ↓ mitigation action begins
Mitigating
   ↓ symptoms improve; verification starts
Monitoring
   ↓ recovery verified
Resolved
   ↓ post-incident review / retention rule
Closed
```

Reopening is allowed from `Monitoring`, `Resolved` or `Closed` when the incident recurs or verification fails:

```text
Monitoring / Resolved / Closed → Reopened → Investigating
```

### 3.2 Status meanings

| Status | Meaning | Required information |
|---|---|---|
| Detected | A trigger created or suggested an incident | Trigger source, timestamp, affected scope if known |
| Triage | A responder is validating impact and ownership | Initial severity, business impact, owner, duplicate check |
| Investigating | Evidence is being gathered and hypotheses tested | Timeline, queries, evidence, current hypothesis |
| Mitigating | A temporary or permanent change is being applied | Action, approval, expected impact, executor |
| Monitoring | Symptoms improved but stability is not yet verified | Verification window, success criteria, watch owner |
| Resolved | Service recovered and the responder confirms the immediate problem is mitigated | Resolution summary, evidence, customer impact end time |
| Closed | Required review, communication and follow-up records are complete | Post-incident notes, follow-up tasks, closure actor |
| Reopened | A resolved/closed incident requires active work again | Reopen reason, new evidence or recurrence signal |

### 3.3 Dispositions separate from status

These are not lifecycle statuses because they describe classification rather than operational phase:

- Duplicate
- False positive
- Suppressed
- Cancelled
- Informational

An incident may be `Closed` with a `Duplicate` disposition. This preserves lifecycle reporting while allowing alert correlation to remain explainable.

### 3.4 Required incident roles

For S1/S2 incidents, the system should support explicit assignment of:

- Incident Commander
- Technical Lead
- Communications Lead
- Approver or Change Owner
- Stakeholders / Subscribers

Small teams may assign one person to multiple roles. Roles should be optional for S3–S5.

## 4. Data Redaction Policy

### 4.1 Data handling classes

| Class | Meaning | Default external AI behavior |
|---|---|---|
| Public | Safe for general sharing | Allowed |
| Internal | Operational data without direct secrets or regulated content | Allowed only by provider/workspace policy |
| Confidential | Source code, topology detail, customer identifiers, private logs | Redact or allow only to approved providers |
| Restricted | Secrets, credentials, private keys, tokens, regulated data or raw sensitive payloads | Never send to external AI |

### 4.2 Immutable restricted data

The following must never be sent to a hosted AI provider and must never be written into an unredacted AI Assistant Log:

- Passwords and password-like values
- API keys, access tokens and refresh tokens
- Bearer headers, cookies and session identifiers
- Private keys, certificates with private material and signing keys
- Kubernetes Secret values and kubeconfig credentials
- Cloud provider credentials
- Database credentials and connection strings containing credentials
- Webhook signing secrets
- Encryption keys
- Unmasked payment-card or regulated personal data

These rules cannot be disabled through normal Organization, Team or Workspace settings.

### 4.3 Configurable redaction actions

For configurable data classes, the administrator may choose:

- `DROP`: remove the field or record completely.
- `MASK`: replace with a stable label such as `<REDACTED:EMAIL>`.
- `HASH`: create a deterministic non-reversible identifier for correlation where appropriate.
- `TRUNCATE`: preserve only a safe prefix, suffix or bounded length.
- `AGGREGATE`: send counts, ranges or distributions instead of raw records.
- `ALLOW`: permit only for a named provider, model, workspace and purpose.

Secrets must use `DROP` or non-reversible `MASK`; they must not use reversible encryption as a substitute for redaction in model context.

### 4.4 Processing pipeline

```text
Connector data
  ↓
Classify source and fields
  ↓
Detect secrets and sensitive patterns
  ↓
Apply allowlist and redaction rules
  ↓
Deduplicate, summarize and minimize context
  ↓
Validate egress policy
  ↓
Preview / record redaction decision
  ↓
Send only approved context to the model
```

If classification, redaction or policy validation fails, external transmission must fail closed. The user should see the reason and a safe recovery path.

### 4.5 Separate policies

ThalassaOps must not use one generic “privacy setting.” It needs separate policies for:

1. `send_to_model`
2. `store_locally`
3. `display_in_ui`
4. `export_to_integration`
5. `retain_for_audit`

Example: a log line may be displayed locally after masking, excluded from hosted AI, retained for 24 hours in encrypted local storage and summarized before being posted to Jira.

### 4.6 AI Assistant Log requirements

The AI Assistant Log should retain:

- Provider and model identity
- Policy version
- Redaction rule version
- Sources and evidence identifiers
- Redacted context fingerprint
- Token/cost estimate where available
- Tool calls and results after redaction
- Model output and confidence
- Actions proposed or executed

It must not retain raw secrets or unredacted prompts by default.

### 4.7 Local model exception

Local models may receive more `Confidential` data when the Workspace policy permits it, but immutable `Restricted` data remains blocked by default. A local model is not automatically trusted merely because it runs on the user's machine.

## 5. Action risk and autonomy policy

Action risk and execution mode are separate policy dimensions.

### 5.1 Risk classes

- `READ-ONLY` — does not change the target or external system.
- `MUTATING` — can change the target or external system.
- `BLOCKED` — forbidden in the current scope.
- `REQUIRES APPROVAL` — cannot execute until the required human approval is recorded.

### 5.2 Execution modes

- `OBSERVE` — inspect or evaluate only.
- `RECOMMEND` — produce a proposed action for a human.
- `APPROVAL` — execute after the policy-defined approval decision.
- `POLICY_AUTO` — execute a narrowly scoped, reversible `MUTATING` action under an explicit policy.

`POLICY_AUTO` is disabled by default. It is valid only when the policy specifies the allowed resource and environment scope, blast-radius limit, cooldown, rollback or recovery behavior, and post-action verification. The model may propose an action, but it never authorizes it. If a precondition, policy check or verification step fails, execution falls back to `REQUIRES APPROVAL` or `BLOCKED`.

The four risk classes remain the stable UI and audit vocabulary; execution mode is recorded separately in the action and audit record.

## 6. Policy Center UX

Policy Center should provide:

- Preset selection
- Scope inheritance view
- Effective-policy preview
- Test payload and redaction preview
- Severity simulation
- Incident transition validation
- Action permission matrix with risk class and execution mode, including disabled-by-default `POLICY_AUTO` rules
- Version history and rollback
- Export/import and policy-as-code
- Audit trail

## 7. Rationale and references

This baseline follows the principle that severity represents impact, while priority and urgency drive operational ordering. It also follows the incident-management emphasis on coordination, communication, control, mitigation and recovery, and the telemetry-security principle of data minimization and explicit redaction. ([Atlassian severity levels](https://www.atlassian.com/incident-management/kpis/severity-levels), [PagerDuty incident priority](https://support.pagerduty.com/main/docs/incident-priority), [PagerDuty incidents](https://support.pagerduty.com/main/docs/incidents), [Google SRE Incident Management Guide](https://sre.google/resources/practices-and-processes/incident-management-guide/), [OpenTelemetry handling sensitive data](https://opentelemetry.io/docs/security/handling-sensitive-data/))
