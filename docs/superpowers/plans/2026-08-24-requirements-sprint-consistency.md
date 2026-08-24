# Requirements and Sprint Consistency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the requirements, policy, UX, ADR and sprint plan documents executable as one consistent baseline before Sprint 1.

**Architecture:** Keep `docs/requirements/requirements-summary.md` as the canonical product-priority and decision summary. Keep `docs/policies/operational-policy-baseline.md` and ADR-0001 authoritative for policy semantics, while the sprint plan maps each required producer and consumer to a delivery sprint. Keep the four existing action risk classes and add a separate execution-mode field for narrowly policy-authorized automation.

**Tech Stack:** Markdown documentation, ADRs, repository-local cross-reference checks with `rg`/shell.

**Spec:** `CONTEXT.md`, `docs/requirements/requirements-summary.md`, `docs/requirements/system-requirements.md`, `docs/policies/operational-policy-baseline.md`, `docs/design/ux-ui-concept.md`, `docs/adr/0001-policy-as-data.md`, `docs/planning/sprint-plan.md`.

## Global Constraints

- Current cloud scope remains AWS, Azure and GCP; Huawei Cloud stays deferred.
- The canonical incident lifecycle remains Detected → Triage → Investigating → Mitigating → Monitoring → Resolved → Closed, with Reopened and separate dispositions.
- Severity remains S1–S5 and distinct from urgency and priority.
- Immutable secret protection and fail-closed egress cannot be weakened by configuration.
- The four risk classes remain READ-ONLY, MUTATING, BLOCKED and REQUIRES APPROVAL.
- Policy-bounded automatic mutation is disabled by default and never lets the model authorize itself.
- Provisioning remains outside the initial incident-control release.
- No application code or scaffold is created by this documentation consistency change.

### Task 1: Establish the early identity and policy foundation

**Files:**
- Modify: `docs/planning/sprint-plan.md`
- Modify: `docs/requirements/requirements-summary.md`
- Modify: `docs/adr/0001-policy-as-data.md`

- [x] Add Organization/Team/Workspace/Environment identity entities, local principal/bootstrap and policy contract/runtime to Sprint 1–2.
- [x] Clarify that Sprint 20 delivers the full Policy Center governance surface on top of the earlier runtime.
- [x] Clarify that mutable policy is never hard-coded; the runtime may precede the admin UI.

### Task 2: Schedule every declared incident producer

**Files:**
- Modify: `docs/planning/sprint-plan.md`
- Modify: `docs/requirements/requirements-summary.md`
- Modify: `docs/requirements/system-requirements.md`

- [x] Add rule-based anomaly detection and scheduled health-check producers to Sprint 11.
- [x] Add normalized vulnerability-finding ingestion and initial security-source adapters to Sprint 13.
- [x] Make Sprint 15/16 exit criteria reference the scheduled producers and normalized findings.

### Task 3: Close capability-priority and action-model gaps

**Files:**
- Modify: `docs/planning/sprint-plan.md`
- Modify: `docs/requirements/requirements-summary.md`
- Modify: `docs/requirements/system-requirements.md`
- Modify: `CONTEXT.md`
- Modify: `docs/policies/operational-policy-baseline.md`
- Modify: `docs/design/ux-ui-concept.md`

- [x] Synchronize the 10-item capability priority list across the two requirements documents.
- [x] Add capacity/cost/reliability insights to the planned release scope and explicitly defer full FinOps integrations.
- [x] Define execution_mode separately from action risk_class and schedule a disabled-by-default POLICY_AUTO path.
- [x] Distinguish customer security/compliance posture ingestion from application hardening.
- [x] Update UX examples to show severity and priority separately.

### Task 4: Normalize resolved and remaining decisions

**Files:**
- Modify: `CONTEXT.md`
- Modify: `docs/requirements/requirements-summary.md`
- Modify: `docs/requirements/system-requirements.md`

- [x] Define the Skill/Plugin/MCP boundary.
- [x] Mark provisioning, home screen, incident sources, terminal shape and baseline data-safety behavior as resolved/deferred.
- [x] Make the remaining open-decision lists identical in scope and wording.

### Task 5: Fix plan hygiene and verify consistency

**Files:**
- Modify: `docs/planning/sprint-plan.md`
- Modify: `docs/superpowers/plans/2026-08-24-requirements-sprint-consistency.md`

- [x] Add the missing Sprint 26 milestone marker.
- [x] Reconcile calendar estimates with the fixed two-week cadence and explain when compression requires parallel streams or scope reduction.
- [x] Run cross-document searches and inspect the final diff for contradictions.
