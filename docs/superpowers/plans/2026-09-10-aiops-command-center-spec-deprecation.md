# ThalassaOps UX/UI spec — full replacement, not just visual

Follows `2026-09-10-aiops-command-center-shell-phase1.md` (shell + Home
structure) and `2026-09-10-aiops-command-center-visual-rebrand.md`
(palette/typography). Both of those were scoped narrowly at the time —
"keep the spec's product rules, adopt the mockup's visuals." The user
then asked, in a separate turn, to deprecate `docs/design/ux-ui-concept.md`
entirely and replace it with the Claude Design mockup. This doc records
that larger decision.

## Decision

Before touching the doc, surfaced the actual conflict rather than
assuming "replace" meant "reskin": the mockup's content model is missing
several things the old spec treated as hard product rules — a
severity/priority split (`S1–S5` vs `P1–P5`), a mandatory AI-response
disclosure contract (confidence + evidence + sources + redaction + context
budget), and a terminal risk-classification taxonomy
(`READ-ONLY`/`MUTATING`/`BLOCKED`/`REQUIRES APPROVAL` +
`OBSERVE`/`RECOMMEND`/`APPROVAL`/`POLICY_AUTO`). Also asked whether the
old spec's persistent right-hand AI panel survives, since the mockup uses
a drawer instead.

User's answer, both questions: adopt the mockup's model as the new
product truth — drop the governance rules the mockup doesn't encode, and
use the drawer-based AI pattern.

## What this task is, and isn't

**Is:** replace the authoritative UX/UI spec document. New doc:
`docs/design/aiops-command-center.md`. Old doc: deprecated in place with
a banner, content kept for history (cited by line number in
`docs/design/sprint-16-incident-workspace.md`,
`docs/design/sprint-17-ai-provider-gateway.md`, and two
`docs/superpowers/plans/*.md` files — grepped first, confirmed before
deciding to keep rather than delete/rename).

**Is not:** a code change. `ui/`, `contracts/`, `crates/`, and
`docs/planning/sprint-plan.md` are untouched by this task. The new spec's
"Divergence from the current implementation" section enumerates every
place shipped code now disagrees with the new spec (severity types,
`IncidentQueueItem.priority`, `CriticalNumberLink`'s evidence-gating,
Sprint 17's AI provider governance, existing manifest/log masking UI) so
those are reconciled deliberately later, not discovered mid-task.

## Extraction method

The mockup's per-view markup (10 `sc-if` blocks beyond Home) and the full
state script (`aiThread`, `aiChips`, palette command list, `INCSTATUS`,
alert-state colors, runbook trigger types, insight-card confidence
values) were read in full from the scratchpad copy made during the
original design import — not reconstructed from the mock data arrays
alone. This matters because the data arrays (`INCIDENTS`, `ALERTS`) don't
show, e.g., that AI insight cards *do* carry a confidence number (cycling
through a fixed array in the script) but *don't* require a structured
evidence-source list — a distinction only visible in the render script,
and the exact boundary the new spec needed to state precisely.

## What survived from the old spec, unchanged

- Never use color alone for status.
- `⌘K` command palette, keyboard navigation, reduced-motion respect.
- Thai/English localization from the start, no hard-coded domain text.
- The general shape of "impact before infrastructure detail" — the new
  spec's Home KPI strip and incident-first Operate group serve the same
  goal via the mockup's own layout, not a separate rule.

## Next step (explicit, not done here)

`docs/planning/sprint-plan.md` Sprints 18–22 (context optimization and
redaction, read-only AI investigation, Policy Center, approval and
action framework, terminal and runbook workflows) were scoped against the
old spec's governance model, most of which this document just dropped.
Reconciling those sprints against `docs/design/aiops-command-center.md`
is its own design-approval gate — the same pattern every prior sprint in
this project used (design doc → user approval → task plan). Not started
as part of this doc.

## 2026-09-11 — decision reversed

The "drop the governance rules" call in this document's "Decision" section
(severity/priority split, AI evidence/confidence/budget disclosure,
terminal/action risk classification, redaction disclosure, Evidence Tide
Line) was reversed on 2026-09-11: all five rules are kept. See
`docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md`
for how each one is expressed inside the new visual system. This
document's account of the 2026-09-10 decision is kept as-is above — it is
what was decided that day, not what stands today.
