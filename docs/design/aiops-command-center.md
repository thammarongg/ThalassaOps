# ThalassaOps UX/UI Spec — AIOps Command Center

**Status:** Authoritative UX/UI spec — replaces `ux-ui-concept.md` in full
**Source:** Claude Design project `e51ce2dd-9138-45bb-8bab-70d2c5b6de50`,
file `AIOps Command Center.dc.html` (imported via the `claude_design` MCP)
**Adopted:** 2026-09-10, by explicit user decision (see
[Product-model changes](#product-model-changes-from-the-old-spec) — this
was a deliberate override of the prior spec, not an oversight)

This document describes the product as the Claude Design mockup shows it:
navigation, page layouts, content model, interaction rules, and visual
system. Where the mockup's data model is simplified or silent relative to
the prior spec, this document states the new model and calls out the
change explicitly rather than leaving it implicit.

## Provenance and what this replaces

`docs/design/ux-ui-concept.md` is deprecated as of this document. It is
kept in the repository for history — several sprint-plan docs cite it by
line number — but no longer describes what gets built. Any future spec
change happens in this document.

## Navigation and information architecture

Four sidebar groups, eleven views:

| Group | Views |
|---|---|
| Operate | Home (Command Center), Alerts, Incidents |
| Investigate | Topology, Metrics, Logs, AI insights |
| Automate | Runbooks, Agents |
| Govern | Reports, Settings |

This four-group structure was already adopted in the shell (Phase 1,
2026-09-10) as a reasonable, non-conflicting evolution of the old spec's
flat list; this document makes it the spec of record instead of an
unwritten addition. `Area` ids in `ui/src/shell.tsx` map to these views
one-to-one except: `commandCenter`→Home, `incidents`→Incidents (list) +
an Incident detail view reached by opening a row. `correlation`,
`changes`, `vulnerability`, `environments`, `integrations`, `policies`,
`audit` are current-product concepts with no mockup equivalent — see
[Divergence](#divergence-from-the-current-implementation).

## Global application shell

- **Top bar** (`--color-nav`, darker than panels): brand mark, ⌘K search
  field, language toggle, notifications bell, terminal/embedded-shell
  icon. (Shipped 2026-09-10.)
- **Sidebar**: collapsible, icon-only when collapsed, grouped per the
  table above, each item showing an active-state highlight and optional
  badge (e.g. an alert count). (Shipped 2026-09-10.)
- **⌘K command palette**: modal overlay (not a drawer), fuzzy-matches
  navigation items and a fixed command list: go to alerts, acknowledge
  selected alerts, run the rollback runbook for the open incident, open
  topology, search logs, switch theme, switch language. Each command
  shows an icon and a keyboard hint (e.g. `G then A`, `⌥R`, `⌘⇧L`).
- **Ask AI**: a **drawer**, not a persistent third column. Opened from a
  button in the page header or from "Ask a follow-up" inside an incident.
  Renders a message thread (question/answer bubbles), quick-reply chips
  contextual to the current view, and a text input. Closing it returns
  the main content to full width.
- **Status bar** (bottom, 26px): agents-online count, ingest rate,
  correlation percentage, last-sync timestamp, build/version string.
  New in this spec; the old shell had no bottom status bar.
- **Scope/workspace switcher**: the mockup does not show one (it has a
  single implicit environment). The current shell's
  Organization/Team/Workspace/Environment switcher stays — this is a
  current-product concept, kept per
  [Divergence](#divergence-from-the-current-implementation), not part of
  the mockup import.

## Home (Command Center)

Two layout variants exist in the source (`homeLayout`: `widgets` |
`focus`); **`widgets` is the adopted default**. `focus` is documented
below as an open variant, not built unless requested.

### `widgets` layout (default)

1. **KPI strip** — four cards: Open incidents, Mean time to acknowledge,
   Mean time to resolve, Alerts suppressed. Each shows a headline value,
   a delta vs. a prior period, and a one-line note.
2. **Customizable widget grid** (drag-to-reorder, resize, hide/show, with
   a "Customize layout" toggle and a "reset to default" action):
   - **Active incidents** — table of open incidents (severity, title,
     service, age), linking to the Incident view.
   - **Service health** — per-service 12-cell sparkline strip (last 24h)
     plus an SLA percentage.
   - **Signal volume** — stacked bar chart, raw alerts vs. correlated
     count, last 24h.
   - **AI insights** — up to three insight cards (see
     [AI insights view](#ai-insights)), each linking to the full list.

### `focus` layout (open variant, not built)

A single large "top incident" card (severity, live-impact pulse, key
stats, an inline AI probable-cause panel, and three quick actions:
acknowledge, run rollback, open incident) above a compact grid of the
remaining open incidents. Useful for a single-incident-dominant on-call
view; not scheduled.

## Alerts

Two triage modes (`triageMode`: `inbox` | `board`); **`inbox` is the
adopted default**. `board` is an open variant.

- **Filter bar**: severity/status filter chips with counts, a free-text
  filter field, a noise-reduction on/off toggle, and a bulk-acknowledge
  action.
- **`inbox` mode**: a dense table — checkbox, severity badge, alert title
  (with an optional "AI: root/symptom/dup" tag), source, first-seen time,
  state (`New`/`Acknowledged`/`Investigating`/`Suppressed`), and a
  correlated-count column. Row opacity is reduced for suppressed alerts.
- **`board` mode** (open variant): a four-column kanban
  (New/Acknowledged/Investigating/Suppressed), each card showing
  severity, first-seen time, title, source, and an AI correlation badge.

## Incidents (list)

- **Filter chips** with counts: Open, Mine, Needs update, Resolved · 7d.
- **Summary strip**: five numbers — total open, SEV1 active, updates
  overdue, unassigned, alerts rolled up.
- **Incident cards**, one per row: severity color bar, id, severity
  badge, status badge (`Triage`/`Investigating`/`Identified`/
  `Monitoring`), affected-service tags, title, one-line impact
  statement, most recent update (or an explicit "no update yet" state),
  commander/owner with initials avatar (or an "unassigned — claim"
  affordance), elapsed time, a "next update in Nm" or "update overdue"
  chip, and the rolled-up alert count.
- **Declare incident** action, top right.
- **Empty state**: explicit "nothing open here" message, not a bare
  blank table, when a filter matches nothing.

## Incident (detail)

- **Header card**: severity + status badges, id/opened-time/owner line,
  title, impact line, and two actions (Acknowledge, Run rollback runbook)
  — plus a stat row (arbitrary key/value pairs, e.g. users affected,
  error rate, p99).
- **Tabs**: Timeline, Alerts (with a live count), Metrics, Changes,
  Postmortem.
- **Timeline tab**: a vertical event list — time, colored dot, event
  text, and an optional monospace metadata block per entry (e.g. a diff
  or a stack trace excerpt).
- **Right column** (three stacked cards):
  1. **Probable cause** — an AI panel with a confidence number, one
     narrative paragraph, and an "ask a follow-up" affordance that opens
     the AI drawer.
  2. **Blast radius** — a list of affected services with a colored
     health dot and state label.
  3. **Similar past incidents** — id, one-line description, and a
     percentage match score, each linking to that incident.

## Topology

- **Scope chips** (e.g. environment/domain filters) plus a status legend
  (down/degraded/healthy).
- **Node-edge graph**: absolutely-positioned service nodes (name, status
  dot, one-line metadata) connected by colored/dashed edges indicating
  edge health (ok/warn/bad). Rendered as inline SVG lines behind
  positioned HTML node cards, not a graph library.

## Metrics

- **Query bar**: a PromQL-style expression field plus a time-range
  selector (5m/1h/6h/24h/7d style buttons).
- **Chart grid**: 2-up cards, each with a title, current value, a
  one-line context (e.g. threshold), an SVG area+line sparkline with a
  threshold reference line, and a hover crosshair + tooltip showing the
  value and timestamp at the cursor position.

## Logs

- **Facet sidebar** (200px): checkbox filter groups (e.g. by service,
  level) with per-value counts.
- **Query bar**: a LogQL-style free-text field with a result-count hint.
- **Histogram**: a 40-bar volume-over-time strip above the log table.
- **Log table**: time, level (colored), service, message — monospace,
  dense, striped on hover.

## AI insights

- **Card grid** (auto-fit, ~330px min column): each card has a kind badge
  (`Correlation` / `Anomaly` / `Forecast`, colored + iconized), a
  confidence number, a one-line title, a supporting sentence, and a row
  of informal tags (e.g. an incident id, a service name — not a
  structured evidence-source list; see
  [Product-model changes](#product-model-changes-from-the-old-spec)).

## Runbooks

- **Pending-approval banner** (shown when one exists): title, runbook
  id + related incident id + requester, and two actions — Dry run,
  Approve and run.
- **Runbook table**: name + step count, a trigger-type badge
  (`Automatic` / `Needs approval` / `Manual`), 30-day run count, success
  rate, and last-run time.

## Agents

- **Fleet KPI row**: online/degraded/offline counts, versions-in-use
  count.
- **Collector table**: host, collector kind, version, CPU, lag, and a
  colored state dot + label.

## Reports

- **SLA cards** (auto-fit grid): service name, current value vs. target,
  a progress bar, and an error-budget note (`healthy` /
  `NN% consumed` / `exhausted`).
- **Incidents-per-week chart**: stacked bars (critical/warning/info) over
  a 12-week window.

## Settings

- **Integrations panel** (left, wide): one row per connector — badge
  (initials/abbreviation), name, detail line, status text, and a toggle
  switch.
- **Preferences panel** (right): theme selector, language selector, and
  a list of boolean preference toggles (e.g. desktop notifications for
  SEV1, offline incident caching, global hotkeys, sound on new critical
  alert).
- **About card**: version, runtime versions (Tauri/rustc/React), local
  cache size, and an "update ready" note when applicable.

## Visual system

See `docs/superpowers/plans/2026-09-10-aiops-command-center-visual-rebrand.md`
for the palette/typography decision and mechanism (a 7-token CSS variable
substitution) — not duplicated here. Summary: IBM Plex Sans/Sans Thai +
IBM Plex Mono, dark-only, orange (`#ff9900`) brand accent, tinted-pill
status indicators.

### `data-props` variants and adopted defaults

The source file exposes editor-configurable variants; this spec adopts
one value per axis as the product default and marks the rest open:

| Axis | Options | Adopted | Status |
|---|---|---|---|
| `platform` | macos / windows | — | **Neither** — no fake window chrome; `decorations` is unset in `src-tauri/tauri.conf.json`, so the OS draws the real title bar on both platforms. |
| `navMode` | sidebar / services | sidebar | services mega-menu not built (redundant with sidebar groups) |
| `homeLayout` | widgets / focus | widgets | focus documented above, open |
| `triageMode` | inbox / board | inbox | board documented above, open |
| `startTheme` | system / dark / light | dark | light theme not built; product is dark-only today |
| `startLang` | en / th | en | both fully localized |
| `noiseReduction` | boolean | true | — |

## Interaction rules

- `⌘K` opens the command palette from anywhere; `Escape` closes any open
  overlay (palette, AI drawer, dropdown menus).
- Respect reduced-motion settings; keep animations (pulse, fade-in)
  meaningful rather than decorative.
- Support Thai and English UI strings from the start; no hard-coded
  domain text.
- Never use color alone for status — every status indicator pairs a
  color with a symbol and/or a text label (this rule survives from the
  old spec; the mockup itself follows it via badge text + color).
- Read-only views (Metrics, Logs, Topology, Reports, Settings) contain no
  mutating actions. Mutating actions (Acknowledge, Run rollback,
  Approve and run, connector enable/disable) are visually distinct button
  styles (filled accent vs. outlined) but do not carry the old spec's
  formal `READ-ONLY`/`MUTATING`/`BLOCKED`/`REQUIRES APPROVAL` label —
  see [Product-model changes](#product-model-changes-from-the-old-spec).

## Product-model changes from the old spec

`ux-ui-concept.md` treated several things as hard product rules that this
mockup does not encode. Per explicit user decision on 2026-09-10, this
spec drops them rather than layering them onto the mockup's simpler
model:

- **Severity/priority split dropped.** The old spec required severity
  (`S1–S5`, business impact) and derived priority (`P1–P5`, operational)
  to always be shown as separate fields. The mockup has a single
  `SEV1`/`SEV2`/`SEV3` severity field and no priority concept. This spec
  adopts `SEV1–3` only.
  **Reversed 2026-09-11:** both fields are kept — see the
  [governance reconciliation design](../superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md)
  Section 1. Priority already renders on the Operations Console queue
  row; priority on the `Incident` entity is a backend backlog item.
- **AI evidence/confidence/budget disclosure dropped as a formal
  contract.** The old spec required every AI response to expose finding,
  confidence, evidence references, sources queried, data
  omitted/redacted, context/token budget, next step, and
  read-only-vs-mutation status. The mockup's AI insight cards and
  probable-cause panel show a confidence number and prose only — evidence
  is referenced informally in the prose ("Three pods were OOMKilled...")
  and via loosely-typed tags, not a structured, required evidence-link
  list. This spec adopts the mockup's lighter model: confidence number +
  narrative + informal tags/links, no mandatory structured disclosure.
  **Reversed 2026-09-11:** the compact card stays as the default, with a
  "Show evidence" disclosure carrying the full contract — reconciliation
  design Section 2.
- **Terminal/action risk classification dropped.** The old spec required
  every shown command to carry a `READ-ONLY`/`MUTATING`/`BLOCKED`/
  `REQUIRES APPROVAL` label, plus a separate policy-mode label
  (`OBSERVE`/`RECOMMEND`/`APPROVAL`/`POLICY_AUTO`). The mockup instead
  gives runbooks a simple trigger type (`Automatic`/`Needs approval`/
  `Manual`) and a two-button pattern (`Dry run`, `Approve and run`) for
  anything pending approval. This spec adopts the mockup's model.
  **Reversed 2026-09-11:** the trigger-type text is replaced by a
  risk-class pill and an execution-mode pill — reconciliation design
  Section 3.
- **Evidence Tide Line signature element dropped.** No equivalent exists
  in the mockup; the Incident timeline tab's vertical event list replaces
  it.
  **Reversed 2026-09-11:** kept, and deferred to the Sprint 19
  `IncidentNarrative` rewrite rather than dropped — reconciliation design
  Section 5.
- **Redaction-disclosure UI dropped as a spec requirement.** The old spec
  required visible "sensitive fields redacted: N" messaging. Not present
  in the mockup. (This does not mean redaction itself is dropped from the
  backend — see Divergence below; only the UI requirement to surface a
  count is dropped from this spec.)
  **Reversed 2026-09-11:** a conditional `sensitive fields redacted: N`
  line is required on AI-response and command-output surfaces —
  reconciliation design Section 4.

## Divergence from the current implementation

This document describes the target design. It does not, by itself,
change backend contracts, Rust domain types, or already-shipped Sprint
behavior. The following are known gaps between this spec and
`crates/`/`ui/contracts/ipc.ts` as of 2026-09-10, listed so they are
reconciled deliberately rather than discovered mid-implementation:

- `ConsoleSeverity`/`Severity` types are `s1`–`s5` throughout
  `ui/contracts/ipc.ts`, `ui/src/design-system/components.tsx`
  (`severityTone`), and both locale files (`severity.s1`…`severity.s5`).
  This spec's `SEV1–3` model requires either a mapping layer or a
  contract change — not decided here.
- `IncidentQueueItem.priority` and the domain's derived-priority handling
  exist in the Rust incident domain (Sprint 15/16) and are now spec-less
  per the dropped severity/priority split.
- `CriticalNumberLink` (`OperationsConsole`) hard-requires an
  `evidence_ids` list before rendering a number as a clickable
  drill-down, and the evidence panel shows redaction/parse-state per
  item (Sprint 11–16). This is stricter than the new spec's model and is
  **not** being relaxed by this document — it is existing, shipped
  governance behavior in a different part of the product (evidence
  drill-down) than the AI insight cards this spec describes. Whether to
  loosen it to match the new AI-insights model, or keep it as a stricter
  standard specifically for numeric drill-downs, is an open decision.
- `AiProviderPanel` and the audit store (Sprint 17) implement
  provider-level governance (fallback order, request auditing) with no
  mockup equivalent — kept as-is; out of this spec's scope.
- Kubernetes manifest masking and Loki log masking (Sprint 6, 9) surface
  a redaction banner in the UI today. This spec's dropped
  "redaction-disclosure UI" rule applies to the AI-response/terminal
  surfaces the old spec described, not to this existing, unrelated
  masking UI — not touched by this document.
- `docs/planning/sprint-plan.md` Sprints 18 (context optimization and
  redaction), 19 (read-only AI investigation), 20 (Policy Center), 21
  (approval and action framework), and 22 (terminal and runbook
  workflows) were scoped against the old spec's governance model. They
  need reconciliation against this document before implementation —
  **this is a separate approval gate**, following the same design→
  approval→plan pattern every prior sprint used. Not done as part of
  this document.
