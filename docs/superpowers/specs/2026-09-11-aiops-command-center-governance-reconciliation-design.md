# AIOps Command Center — Governance Reconciliation Design

**Status:** Approved design (pending user's file review)
**Date:** 2026-09-11
**Type:** UX/UI specification amendment (no backend/contract change)

## Provenance

This document amends `docs/design/aiops-command-center.md`, the UX/UI spec
produced from a Claude Design mockup (handoff bundle: `AIOps Command Center.dc.html`
+ `README.md`, delivered 2026-09-10). That spec replaced the product's original
UX/UI concept (`docs/design/ux-ui-concept.md`) with the mockup's visual system
(IBM Plex Sans/Mono, orange-accent enterprise-console palette) and, per an
earlier decision recorded in
`docs/superpowers/plans/2026-09-10-aiops-command-center-spec-deprecation.md`,
also dropped five product rules the mockup does not encode natively:

1. Severity (`S1–S5`, business impact) shown separately from derived Priority (`P1–P5`, operational).
2. A mandatory structured disclosure on every AI response: finding, confidence, evidence references, sources queried, data omitted/redacted, context/token budget, next step, and read-only-vs-mutation status.
3. A risk-classification label (`READ-ONLY`/`MUTATING`/`BLOCKED`/`REQUIRES APPROVAL`) plus a separate execution-mode label (`OBSERVE`/`RECOMMEND`/`APPROVAL`/`POLICY_AUTO`) on every shown command/action.
4. Visible "sensitive fields redacted: N" messaging on AI-response and terminal surfaces.
5. The Evidence Tide Line signature timeline element.

All five are current requirements in `docs/requirements/requirements-summary.md`
(§8.2, §9.2, §10) and `docs/requirements/system-requirements.md` (§6.2–6.4). On
2026-09-11 the user reviewed that gap against the requirements baseline and
decided: **keep all five product rules; adopt the new visual system.** This
document is the reconciliation — how each rule is expressed in the new visual
language without reverting to the old spec's layout.

## Goal

Define, view by view, how the five governance elements above render inside the
Claude Design visual system, so that Sprint 16 (Incident Workspace UI follow-up),
Sprint 18 (context optimization/redaction) and Sprint 19 (read-only AI
investigation) have a single, non-contradictory target spec to implement
against.

## Non-goals

- Re-litigating the visual system itself (colors, type, spacing, component
  shapes from the Claude Design handoff are accepted as-is).
- Implementing Sprint 18/19 backend (context classification, redaction
  engine, AI investigation contract). This document specifies UI surface only;
  the data these surfaces need does not exist yet outside `evidence_ids` +
  `confidence`, which already exist in shipped contracts (see per-section notes).
- Changing already-shipped, working redaction UI (Kubernetes manifest masking,
  Loki log masking — Sprint 6/9). Untouched, as already noted in
  `docs/design/aiops-command-center.md`'s "Divergence" section.
- Resolving `CriticalNumberLink`'s stricter evidence-gating vs. this spec's
  lighter AI-card model — remains an explicitly open decision, unchanged by
  this document.

## Reconciliation pattern

Two options were considered:

- **Always-expanded** — show all governance fields inline, unconditionally.
  Rejected: recreates the density that motivated the visual redesign in the
  first place, especially on AI cards (8 fields per card) and incident rows.
- **Compact-inline + expandable disclosure (adopted)** — fields cheap to show
  as a small chip/line render inline, always visible, no click required.
  Fields that are inherently verbose (lists, multi-line detail) sit behind a
  one-click expand on the same card, using the mockup's own
  progressive-disclosure vocabulary (widget-edit controls, popovers). This
  satisfies requirements-summary.md §13 — "Evidence, policy and action state
  visible beside AI output" — without contradicting the mockup's density goals.

## Section 1 — Severity and Priority

**Applies to:** Incident header (detail view), Incident inbox rows, Incident
kanban cards.

`ConsoleSeverity` (`S1`–`S5`) and `ConsolePriority` (`P1`–`P5`) already exist as
separate fields in `ui/contracts/ipc.ts` (`IncidentQueueItem.severity`,
`IncidentQueueItem.priority`) — no contract change.

- **Incident header card:** existing 22px severity chip (`--critbg` on
  `--crit`-family tone) stays as the primary signal. Add a second, smaller
  chip immediately to its right for Priority, styled neutral
  (`--surface3` background, `--fg2` text, same 3px radius) so it reads as
  contextual rather than competing with severity's alarm coloring.
- **Inbox rows / kanban cards:** append `P{n}` to the existing mono
  10–11px meta line (`service · owner`, id, age) that both list styles
  already render. No new component; same type scale.
- **Alerts table:** unchanged — no Priority column. Priority is an Incident-
  level concept per requirements-summary.md §8.2; an Alert does not carry one
  until correlated into an Incident.

## Section 2 — AI response disclosure contract

**Applies to:** Home "AI insights" widget, Incident detail "AI probable
cause" panel, AI side panel, Insights view feed cards.

The compact card (sparkle icon, kind eyebrow, mono `confidence 0.N`, prose)
from the Claude Design handoff is kept as the always-visible default — this
already satisfies the "beside AI output" requirement for the two cheapest
fields (finding via prose, confidence via the existing mono treatment).

Add a text link "Show evidence" (13px, `--link` color, underline-on-hover —
the handoff's existing link style) at the bottom of every such card. Expanding
it reveals key/value rows using the same key style as the Incident detail stat
strip (11px/600 uppercase `--fg2`), but with a smaller 12–13px value instead of
the stat strip's 18px/700 mono — the stat strip's size is tuned for a handful
of headline numbers, not a multi-row evidence list:

| Field | Rendering |
|---|---|
| Evidence references | List of `evidence_ids`, each a clickable drill-down link reusing the mechanism `CriticalNumberLink` (Operations Console, shipped Sprint 11) already uses |
| Sources queried | Plain list |
| Data omitted/redacted | Count + reason text; renders nothing if count is 0 |
| Context/token budget | Mono, e.g. `1,240 / 8,000 tokens` |
| Next step | Short text |
| Read-only vs. mutation status | The risk-classification chip from Section 3, shown only when the next step is an action |

This is new UI surface — the backing data (finding text beyond confidence,
sources queried, token budget, next step) does not exist in any shipped
contract yet. It is the target shape Sprint 18/19 must produce, not a
retrofit of existing Sprint 11/17 code.

## Section 3 — Action risk classification and execution mode

**Applies to:** Runbooks view (cards and list rows today; any future embedded-
terminal command surface in Sprint 22).

The handoff's existing trigger-type text (`Automatic`/`Needs approval`/
`Manual`) is **replaced**, not supplemented — it duplicates the meaning of the
execution-mode label below and showing both would read as two answers to one
question. In its place, two small pills, reusing the handoff's existing
status-pill style (19px, radius 10px, 1px border in the tone color, 11px/600
text) with no new pill component:

- Risk-class pill: `READ-ONLY` (`--ok`), `MUTATING` (`--warn`), `BLOCKED`
  (`--crit`), `REQUIRES APPROVAL` (`--info`).
- Execution-mode pill: `OBSERVE` / `RECOMMEND` / `APPROVAL` / `POLICY_AUTO` —
  neutral tone (`--surface3` / `--fg2`), since this communicates policy
  configuration, not risk severity.

Both pills sit where the old trigger-type text was, in the same card/row
position, so no layout restructuring is needed beyond the label swap.

## Section 4 — Redaction disclosure line

**Applies to:** Any AI-response card (Section 2) and any terminal/command
output surface (Section 3, Sprint 22).

A single conditional line, 11px mono, `--fg3`: `sensitive fields redacted: N`.
(Not the literal `#5f6b78` used specifically for the shell's footer hint bar —
that is a one-off value for that one bar per the handoff's token README;
`--fg3` is the right token for a muted line inside a card or output block.)
Rendered only
when `N > 0`; a clean response or output shows nothing, so this never adds
clutter to the common case. On terminal/command surfaces it sits directly
above the Section 3 pill row.

Kubernetes manifest masking and Loki log masking (Sprint 6/9, shipped) already
carry their own redaction banner and are explicitly out of scope — this
section does not touch them, matching the note already in
`docs/design/aiops-command-center.md`'s Divergence section.

## Section 5 — Evidence Tide Line

**Applies to:** Incident detail timeline.

Replaces the handoff's plain vertical event list with a restyled version of
the same concept from the old spec (`ux-ui-concept.md`: "a thin, calm
timeline that shows how signals, changes, hypotheses and actions move through
an incident"). Same event data and per-event grammar (actor, mono timestamp,
body) as the handoff's Timeline — this is a rendering change, not a new data
requirement:

- Thin rail using `--border2`, with event markers along it instead of a plain
  stacked list.
- Latest/active event marker in `--accent` (orange).
- Alert-origin event markers colored by the alert's severity tone.

## Design tokens (for reference — unchanged from the Claude Design handoff)

Colors, type scale, spacing, radii and fixed dimensions are as specified in
`design_handoff_aiops_command_center/README.md` (delivered 2026-09-10,
`--bg`/`--surface`/`--crit`/`--warn`/`--ok`/`--info`/`--ai` families, IBM Plex
Sans/Sans Thai/Mono, 4px spacing base, 3/4/6/8/10px radii). This document
introduces no new token values — every element above is styled from the
existing set.

## Divergence carried forward (unchanged by this document)

From `docs/design/aiops-command-center.md`'s existing Divergence section,
still open and not resolved here:

- Whether `CriticalNumberLink`'s stricter evidence-gating (Sprint 11–16)
  should loosen to match the AI-card model in Section 2, or remain a stricter
  standard specific to numeric drill-downs.
- `AiProviderPanel` and the Sprint 17 audit store's provider-level governance
  (fallback order, request auditing) — kept as-is, no mockup/spec equivalent
  needed.

## Follow-up documentation corrections (out of this document, tracked here)

Three documents from the 2026-09-10 session need a short correction note (not
a rewrite) once this design is approved, since their "drop the governance
rules" premise was reversed on 2026-09-11:

- `docs/design/ux-ui-concept.md` — its "Superseded 2026-09-10" banner should
  clarify that only its *visual* language (tokens, typography) is superseded;
  its governance content (severity/priority split, Evidence Tide Line,
  interaction rules) is reinstated by this document.
- `docs/design/aiops-command-center.md` — needs the five sections above folded
  in as amendments (cross-referencing this document) so it remains the single
  current UX/UI spec, rather than two documents a future reader has to
  reconcile themselves.
- `docs/superpowers/plans/2026-09-10-aiops-command-center-spec-deprecation.md`
  — append a dated note recording that the "drop governance rules" decision
  was reversed on 2026-09-11, per this project's existing convention of
  keeping decision history rather than deleting it.

These corrections are implementation-phase work (they live in the
`claude/aiops-command-center-4e1759` worktree, where the mockup import and
Phase 1 shell code already exist) and are not made by this document itself.

## Testing

- Visual/interaction: each of the five reconciled elements gets a
  presence/absence check (e.g., redaction line absent at N=0, present at N>0)
  in the relevant view's existing test file (`shell.test.tsx`,
  `TopologyWorkspace.test.tsx`, `ChangeTimeline.test.tsx` pattern already in
  use).
- No new IPC contract to test in this document's scope — Section 2's fields
  are Sprint 18/19 work.
