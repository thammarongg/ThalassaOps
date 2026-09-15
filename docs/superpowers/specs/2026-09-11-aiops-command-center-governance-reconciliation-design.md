# AIOps Command Center — Governance Reconciliation Design

**Status:** Approved design (pending user's file review)
**Date:** 2026-09-11
**Type:** UX/UI specification amendment (no backend/contract change)

## Execution note

This document is written and committed on `claude/review-requirements-spec-thai-89f240`.
The code it describes lives on `claude/aiops-command-center-4e1759` (a sibling
worktree of the same repository). The implementation plan must bring this
document's commits onto that branch (cherry-pick, not a rewrite) before any
task starts — otherwise the branch being implemented has no committed record
of the spec it's following.

## Provenance

This document amends `docs/design/aiops-command-center.md`, the UX/UI spec
produced from a Claude Design mockup (handoff bundle: `AIOps Command Center.dc.html`
+ `README.md`, delivered 2026-09-10). That spec replaced the product's original
UX/UI concept (`docs/design/ux-ui-concept.md`, removed 2026-09-15 — its
surviving principles are in the spec's "Design principles") with the mockup's visual system
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

**Correction (2026-09-11, post-approval):** verified against the actual
shipped code in the `claude/aiops-command-center-4e1759` worktree. Two
different types carry Incident data, and only one of them has Priority:

- `IncidentQueueItem` (Operations Console dashboard row) has both
  `severity: ConsoleSeverity` and `priority: ConsolePriority | null` — and
  **already renders both** (`ui/src/OperationsConsole.tsx:428-429`:
  `<StatusIndicator severity={...} />` followed by
  `{item.priority && <span className="operations-priority">{item.priority}</span>}`).
  No task needed here; this requirement is already met.
- `Incident` (the full domain entity `IncidentWorkspace` renders —
  `ui/contracts/ipc.ts:1081-1101`) has `derived_severity` and
  `severity_override` but **no `priority` field at all**. Showing Priority on
  the Incident detail header/list/kanban would require a backend/contract
  change, which is out of this document's declared non-goals. Adding
  `priority` to the `Incident` domain entity is tracked as a backlog item
  (see "Backlog" section below), not part of this design.

The Incident-detail-view bullets originally in this section (header chip,
inbox/kanban `P{n}`) are removed — they described work this document cannot
scope without a contract change.

**Alerts table:** unchanged — no Priority column, unaffected by the above.
Priority is an Incident-level concept per requirements-summary.md §8.2; an
Alert does not carry one until correlated into an Incident.

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

**Deferred to Sprint 19 (correction, 2026-09-11).** The component this would
restyle, `ui/src/incident/IncidentNarrative.tsx`, carries its own doc comment
at line 28: *"a bare count... Sprint 19 rewrites the narrative anyway (design
15)."* It currently renders as a `<Table>` (shared `design-system/components`
table), not an event-card list — restyling it into a rail-based Tide Line now
means building throwaway structure a known, already-scheduled rewrite
replaces. This section's rail/marker design (thin rail, `--accent` marker for
the latest event, severity-toned markers for alert-origin events) is the
target for Sprint 19 to implement directly, not an interim restyle.

Plain color-token consistency for `IncidentNarrative` (so it doesn't look
visually stale next to the rest of a restyled Incident Workspace) is still in
scope — see "Phase 2" below — but no structural rail/timeline change happens
before Sprint 19.

## Phase 2 — Restyle Incident Workspace to the new visual system

**Added 2026-09-11, after user request to expand this document's scope
beyond documentation-only changes.** `docs/superpowers/plans/2026-09-10-aiops-command-center-shell-phase1.md`
scoped Phase 1 to shell chrome + Home only, explicitly leaving
`IncidentWorkspace` and its children unrestyled ("keep rendering inside the
new shell unchanged"). Phase 2 closes that gap for the six `IncidentWorkspace`
child components confirmed clean of any "will be rewritten" marker
(`IncidentList`, `IncidentSummaryCard`, `IncidentTabs`, `IncidentEvidencePanel`,
`IncidentCommentThread`, `IncidentActions` — grepped for "Sprint 19",
"rewrite", "design 15", "throwaway": no matches, unlike `IncidentNarrative`
above). These render real Sprint 15/16 backend data through existing,
passing tests — restyling them is not mock-driven UI work.

**Scope:** visual token restyle only (colors, spacing already-in-use,
typography inherited from the Phase 1 font change). No markup restructuring,
no new interaction, no behavior change. Existing test files
(`IncidentList.test.tsx`, `IncidentSummaryCard.test.tsx`, `IncidentTabs.test.tsx`,
`IncidentEvidencePanel.test.tsx`, `IncidentCommentThread.test.tsx`,
`IncidentActions.test.tsx`, `IncidentWorkspace.test.tsx`) must keep passing
unmodified — the same rule Phase 1 applied to `shell.test.tsx`.

### Token gap

`ui/src/styles.css` defines 13 color custom properties (the "ocean" naming:
`--color-abyss`, `--color-nav`, `--color-deep-water`, `--color-sea-glass`,
`--color-reef-cyan`, `--color-info(-bg)`, `--color-kelp(-bg)`,
`--color-amber(-bg)`, `--color-coral(-bg)`) — a subset of the ~25 the Claude
Design handoff README specifies. This document's Sections 1–4 above cite
handoff names (`--crit`, `--ok`, `--surface2`, `--fg2`, etc.) directly; the
table below is the authoritative mapping from handoff name to the actual (or
newly added) project variable, so an implementer never has to guess:

| Handoff token | Dark value | Project variable | Status |
|---|---|---|---|
| `--bg` | `#0c1116` | `--color-abyss` | exists |
| `--nav` | `#000716` | `--color-nav` | exists |
| `--surface` | `#161d26` | `--color-deep-water` | exists |
| `--fg` | `#f2f3f3` | `--color-sea-glass` | exists |
| `--accent` | `#ff9900` | `--color-reef-cyan` | exists — **name is misleading, the value is orange, not cyan; do not rename as part of this plan, just don't be surprised by it** |
| `--ok` / `--okbg` | `#5dd47a` / `#12241a` | `--color-kelp` / `--color-kelp-bg` | exists |
| `--warn` / `--warnbg` | `#f0b429` / `#2a2113` | `--color-amber` / `--color-amber-bg` | exists |
| `--crit` / `--critbg` | `#ff7c70` / `#2a1614` | `--color-coral` / `--color-coral-bg` | exists |
| `--info` / `--infobg` | `#539fe5` / `#12212f` | `--color-info` / `--color-info-bg` | exists |
| `--link` | `#539fe5` | `--color-info` | exists — handoff's dark-theme `--link` and `--info` are the same hex; reuse `--color-info`, no new variable |
| `--fg2` | `#9aa7b4` | `--color-fog` | **used but never defined** — referenced 12 times in `ui/src/incident/incident.css`, missing from `:root`. Defining it is a real bug fix, not new scope. |
| `--fg3` | `#7a8794` | `--color-mist` (new) | to add |
| `--surface2` | `#1b232e` | `--color-deep-water-2` (new) | to add |
| `--surface3` | `#212b38` | `--color-deep-water-3` (new) | to add |
| `--border` | `#2a3542` | `--color-border` (new) | to add |
| `--border2` | `#3a4757` | `--color-border-strong` (new) | to add |
| `--ai` / `--aibg` | `#b18cf0` / `#1f1830` | `--color-violet` / `--color-violet-bg` (new) | to add — no current use (Section 2 is blocked), added now so the token set is complete for Sprint 18/19 |
| `--shadow` | `0 1px 4px rgba(0,0,0,.45)` | `--shadow-card` (new) | to add |
| `--navfg` / `--navsub` | `#f7f8f8` / `#96a3b0` | `--color-nav-fg` / `--color-nav-sub` (new) | to add |

### Non-goals for Phase 2

- `IncidentNarrative`'s Tide Line rail (Section 5, deferred to Sprint 19).
- Kanban/board mode for the incident list — no existing concept in
  `IncidentList`, not requested.
- Priority on the Incident detail view (Section 1 correction above) —
  backend follow-up, not scoped here.
- Sections 2, 3, 4 — still blocked on Sprint 18 (redaction), Sprint 19 (AI
  investigation) and Sprint 22 (terminal/runbooks) backend respectively.

## Design tokens

Colors, type scale, spacing, radii and fixed dimensions are as specified in
[`docs/design/aiops-command-center-handoff.md`](../../design/aiops-command-center-handoff.md) (delivered 2026-09-10). The
handoff's token *values* are unchanged by this document — the "Token gap"
table under Phase 2 above is the authoritative name mapping from handoff
token to the actual (or newly added) project CSS variable; use that table,
not the handoff README's names directly, when implementing.

## Divergence carried forward (unchanged by this document)

From `docs/design/aiops-command-center.md`'s existing Divergence section,
still open and not resolved here:

- Whether `CriticalNumberLink`'s stricter evidence-gating (Sprint 11–16)
  should loosen to match the AI-card model in Section 2, or remain a stricter
  standard specific to numeric drill-downs.
- `AiProviderPanel` and the Sprint 17 audit store's provider-level governance
  (fallback order, request auditing) — kept as-is, no mockup/spec equivalent
  needed.

## Backlog (not implemented by this document or its plan)

- **Priority on the Incident domain entity.** `Incident` (`ui/contracts/ipc.ts:1081`)
  has no `priority` field; only the dashboard-level `IncidentQueueItem` does.
  Adding it (Rust domain type, migration, IPC contract, then the Section-1
  display this document originally proposed for the detail view) is a
  backend task for a future sprint, not UI-only work.
- **Sections 2, 3, 4** (AI disclosure, action risk/execution pills, redaction
  line) — blocked on Sprint 18/19/22 backend, as stated throughout.
- **Section 5 / Evidence Tide Line** — blocked on the Sprint 19
  `IncidentNarrative` rewrite already noted in that component's own code
  comment.
- **Kanban/board mode** for the incident list — no existing concept, not
  requested by any current requirement.

## Follow-up documentation corrections (out of this document, tracked here)

**Done 2026-09-15.** All three corrections below were made. `ux-ui-concept.md`
was afterwards removed outright, with its surviving principles moved into
`aiops-command-center.md`.

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

- **Phase 2 (buildable now):** no new assertions — the existing test files
  for the six restyled components (`IncidentList.test.tsx`,
  `IncidentSummaryCard.test.tsx`, `IncidentTabs.test.tsx`,
  `IncidentEvidencePanel.test.tsx`, `IncidentCommentThread.test.tsx`,
  `IncidentActions.test.tsx`, `IncidentWorkspace.test.tsx`) must keep passing
  unmodified, since this is a token-only restyle with no markup or behavior
  change. Verify visually with a Vite dev server + Browser pane screenshot,
  same as Phase 1.
- **Sections 2/3/4 and the Tide Line:** no tests to write yet — there is no
  component or contract for them until Sprint 18/19/22.
