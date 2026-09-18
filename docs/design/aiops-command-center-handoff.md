# Handoff: AIOps Command Center (ThalassaOps)

## Overview
A desktop web console for an AIOps / observability platform: an on-call operator monitors service health, triages alert noise, works incidents with AI-assisted root-cause suggestions, and runs remediation runbooks. The design covers a full app shell (desktop window chrome, global top bar, primary nav, command palette, AI side panel) plus twelve views.

Primary user: NOC / SRE on-call engineer at a Thai enterprise (bilingual EN/TH), working on a 1440×900+ desktop, often for hours at a time — hence a dense, dark-first, monospace-numeric UI.

## About the Design Files
The files in this bundle are **design references created in HTML** — an interactive prototype showing intended look, layout, and behavior. They are **not production code to copy directly**.

The task is to **recreate these designs in the target codebase's existing environment** (React, Vue, SwiftUI, native, etc.) using its established component library, routing, state management, and styling patterns. If no environment exists yet, choose the most appropriate framework for the project and implement the designs there.

`AIOps Command Center.dc.html` opens in any browser — run it and click through before implementing. `support.js` is only the prototype runtime that renders the file; it has no production value and should not be ported.

## Fidelity
**High-fidelity (hifi).** Final colors, typography, spacing, radii, shadows, iconography, empty/active states, and interaction behavior are all specified. Recreate the UI pixel-perfectly using the codebase's existing libraries and patterns. All exact values are listed in **Design Tokens** below; every value used in the prototype comes from that token set.

Note: the visual language is an enterprise-console style (AWS-Console-adjacent: neutral greys, orange accent, blue links, dense tables). If the target codebase already has a design system, map these tokens onto it rather than hard-coding hexes.

---

## App Shell

Root is a full-viewport column, `height: 100vh`, `overflow: hidden` — the shell never scrolls; only the content region does. Base type: 14px / 20px, `IBM Plex Sans`.

### 1. Desktop window chrome (prop: `platform` = `macos` | `windows`)
32px tall bar, background `--nav`, 1px bottom hairline `rgba(255,255,255,.06)`. macOS: three 12px traffic-light dots left. Windows: minimize/maximize/close glyphs right. Purely decorative framing for the mock — **omit in a real web app**; keep it only if shipping in Electron/Tauri.

### 2. Global top bar (~48px)
Left → right:
- **Product mark + name** ("ThalassaOps"), 15px/600.
- **Scope picker** — click-to-open popover, 30px pill, `IBM Plex Mono` 12.5px, format `site / cluster` with a `--fg3` slash. Popover lists sites (mono 12px/600 + grey meta line) each with clusters (7px status dot, mono name, 11px meta). Selecting a cluster re-scopes every view.
- **Search field** — flexible width, 13px placeholder, trailing `⌘K` key hint chip (mono 11px, 1px border `rgba(255,255,255,.18)`, radius 3px, padding 1px 5px). Opens the command palette.
- **Right cluster** — environment/on-call status, notification bell with count badge, theme toggle (dark/light), language toggle (EN/TH), AI panel toggle, then avatar.
- **User menu** — popover: name 15px/700, email 12px `--fg2`, handle + on-call status mono 11px `--fg3`; then rows of 15px stroke icons + 13px label + mono 11px keyboard hint.

### 3. Primary nav (prop: `navMode` = `sidebar` | `services`)
Twelve destinations, each with icon, label, and one-line description: **Home, Alerts, Incidents, Topology, Metrics, Logs, Insights, Runbooks, Agents, Reports, Settings** (+ Incident detail as a child of Incidents).
- `sidebar`: persistent left rail on `--nav`, collapsible (`sidebarOpen`), active item marked with accent left edge + raised background; count badges right-aligned.
- `services`: a "Services" mega-menu launcher in the top bar instead of a rail — content region gets the full width.

Active state resolution: `incidentDetail` keeps **Incidents** active.

### 4. Content region
`flex: 1; overflow-y: auto; padding: 0 24px 24px`. Each view begins with a header block: breadcrumb (with a middle crumb only in incident detail), page title, and a grey sub-line, then the view body. Vertical rhythm between blocks is 12–16px.

### 5. Command palette (⌘K / Ctrl+K, Esc to close)
Centered overlay over a scrim, `fadein` animation (translateY −4px → 0, opacity 0 → 1). Grouped results: navigate-to-view, incident actions, scope switches. Keyboard-first.

### 6. AI side panel (`aiOpen`)
Right-docked panel in the `--ai` violet family. Streaming-style assistant messages, suggested-action chips, and confidence figures rendered in mono (`confidence 0.87`). Section eyebrows: 10px/700, uppercase, letter-spacing .5px, color `--ai`.

### 7. Footer hint bar
11px mono `#5f6b78`: `A ack · R resolve · Ctrl+K palette`.

---

## Screens / Views

### Home / Overview (prop: `homeLayout` = `widgets` | `focus`)
Purpose: at-a-glance state of the estate; the operator's default landing view.

- **KPI row** — `grid-template-columns: repeat(auto-fit, minmax(210px, 1fr)); gap: 16px`. Each card: `--surface`, 1px `--border`, radius 8px, `--shadow`, label 12px/500 `--fg2`, value 28px/32px/700 mono with `letter-spacing: -.6px` and semantic color, delta 12px/600 in `--ok`/`--crit`.
- **Widget grid** — four rearrangeable widgets on a 3-column grid: `incidents` (span 2), `health` (span 1), `signal` (span 2), `insights` (span 1). In `editing` mode each widget header shows a control cluster: `‹` move left, `›` move right, a span cycler (1 → 2 → 3 → 1), and hide. Hidden widgets are recoverable from the edit affordance.
  - **Active incidents** — dense rows: title 14px/500 + mono 11px `--fg3` sub-line, service, age (mono 12px right side).
  - **Service health** — per-service row: name, inline 7-segment sparkline of status blocks, SLA % right-aligned in mono 11px, colored by threshold.
  - **Signal volume** — 24h bar chart, bars colored by severity, axis labels `00:00 / 06:00 / 12:00 / 18:00 / now` in mono 10px `--fg3`.
  - **AI insights** — stacked cards: kind eyebrow (10px/700 uppercase `--ai`) + mono confidence + 12px/17px body.
- `focus` layout: drops the widget grid for a single-column "what needs me now" stack (critical incidents + AI next-actions), for smaller screens or a war-room display.

### Alerts
Purpose: triage raw signal into incidents.
- Filter segmented control: `All / Critical / Warning / Info` — one bordered pill group (1px `--border2`, radius 4px, `overflow: hidden`), active segment gets `--surface3` fill and 600 weight; 13px labels, 6px 13px padding.
- Noise-reduction toggle (prop `noiseReduction`, default on) collapses duplicate alerts into groups; a grouped alert shows a `×N` count.
- Table columns: severity dot + title, source (mono 12px), first seen (mono 12px), status (19px pill, 1px border in status color, radius 10px, 11px/600), grouped count (mono 12px). Row hover raises background to `--surface2`.
- Multi-select (`selectedAlerts`) reveals a bulk action bar: Acknowledge (`A`), Resolve (`R`), Create incident, Silence.

### Incidents (list)
Two modes (prop `triageMode`):
- `inbox` — dense list grouped by severity, same row grammar as Alerts.
- `board` — kanban columns (`Triage / Investigating / Mitigating / Resolved`): column header = 8px status dot + 12px/700 uppercase label + mono 11px count; cards show severity, id, age (all mono 10px), 12px/500 title, and mono 11px `service · owner`.

### Incident detail
Purpose: single pane of glass for one incident (`INC-4821`).
- **Header card** — `border-top: 3px solid <severity>`, radius 8px, padding 16px 18px. Severity chip (22px tall, `--critbg` on `--crit`, radius 3px, 11px/700 uppercase), mono meta `INC-4821 · opened 14m ago`, live indicator: 8px dot with `pulse` 1.6s animation.
- **Stat strip** — key/value pairs: key 11px/600 uppercase `--fg2` letter-spacing .5px; value 18px/700 mono in semantic color (duration, affected services, error rate, customers impacted).
- **AI probable cause** — `--aibg` panel: sparkle icon, `PROBABLE CAUSE` eyebrow in `--ai`, mono `confidence 0.87`, then 13px/20px body. Below it: suggested runbook with a primary action.
- **Related signals** — cards with severity + mono id + mono age, 12px/500 title, mono 11px `service · owner`.
- **Timeline** — chronological events (alert fired, auto-ack, agent action, human comment), each with actor, mono timestamp, and body.

### Topology
Service dependency graph, scoped to the current site/cluster. Nodes colored by health, edges by traffic; filter chips above for layer (edge / app / data). Selecting a node reveals its dependents and open alerts.

### Metrics
Chart grid (latency p50/p95/p99, error rate, saturation, throughput) with a shared time-range control and mono axis labels. Charts are simple line/area renderings — implement with the codebase's charting library.

### Logs
`grid-template-columns: 200px minmax(0, 1fr); gap: 14px; align-items: start`.
- Left: saved-query / facet card (`--surface`, radius 8px, padding 12px 14px 14px).
- Right: query bar prefilled with `service="checkout" level>=error` (mono) plus a virtualized log stream — mono 12px rows, level-colored gutter, expandable rows.

### Insights
Feed of AI findings (anomaly, correlation, prediction, cost) — each with kind eyebrow, mono confidence, body, and accept/dismiss actions.

### Runbooks
Purpose: automation catalog.
- Lead card: `border-left: 3px solid var(--accent)`, radius 8px, padding 15px 18px — the recommended runbook for the active incident, with a run action.
- Below: runbook list with owner, last run, success rate, and step count.

### Agents
Automation agents (auto-ack, noise clustering, auto-remediation) with enable/disable state, scope, action counts, and last activity.

### Reports
Post-incident and SLA reporting: MTTA/MTTR trends, incident volume by service, exportable summaries.

### Settings
Scoped preferences: theme, language, notification routing, integrations, on-call schedule.

---

## Interactions & Behavior
- **Navigation** — flat `screen` value; `incidentDetail` is reached from an incident row or the palette and shows a middle breadcrumb back to Incidents.
- **Keyboard** — `⌘K`/`Ctrl+K` toggles the palette; `Esc` closes palette, services menu, user menu, and scope popover; `A` acknowledges, `R` resolves the focused alert/incident.
- **Popovers** — scope, services, and user menus are click-open, Esc/outside-click close, mutually exclusive, and use the `fadein` keyframe (120–160ms ease-out is right).
- **Theme** — dark/light via a `data-theme="dark"` attribute on the root element; token values swap wholesale (see below). Initial value from prop `startTheme` (`system` resolves via `prefers-color-scheme`).
- **Language** — EN/TH; all strings come from a single translation map. Thai text uses `IBM Plex Sans Thai` in the same stack. Thai runs longer than English — every label container must tolerate ~30% growth (the prototype uses `min-width: 0` + `text-overflow: ellipsis` on flex children throughout).
- **Widget editing** — reorder (swap with neighbor), resize (span cycles 1→2→3), hide/show. Persist per user.
- **Animations** — `pulse` (opacity 1 → .35, 1.6s infinite) for live/critical indicators; `fadein` for popovers and panels; `wave` (scale .55 → 1.5, opacity .75 → 0) for the radiating ping behind an active alert dot. No other motion.
- **Hover** — rows lift to `--surface2`; icon buttons get a `--surface3` square, radius 3px; links use `--link` and underline on hover.
- **Empty states** — every list has one (e.g. `incEmpty` for the incident widget): centered 13px `--fg2` line plus the relevant primary action.
- **Responsive** — desktop-only design (min 1280px). KPI row auto-fits; below ~1100px collapse the sidebar to icons and drop widget spans to 1.

## State Management
Prototype state, as a guide to the real store:
- `screen` — active route (`home | alerts | incident | incidentDetail | topo | metrics | logs | insights | runbooks | agents | reports | settings`).
- `theme` (`dark | light`), `lang` (`en | th`) — user preferences, persisted.
- `scopeSite`, `scopeCluster` — current scope; every data query is scoped by these.
- `aiOpen`, `paletteOpen`, `servicesOpen`, `userOpen`, `scopeOpen`, `sidebarOpen` — UI chrome, non-persisted except `sidebarOpen`.
- `incidentId` — subject of the detail view.
- `alertFilter` (`all | critical | warning | info`), `selectedAlerts: string[]`, `noiseReduction: boolean`.
- `logQuery: string`.
- `editing: boolean` and `widgets: { id, span, on }[]` — dashboard layout, persisted per user.

Data the real app must fetch, all scope-filtered: KPI summary, alert stream (with grouping metadata), incident list + single incident (stats, related signals, timeline), service health + SLA, 24h signal histogram, AI insights (kind, confidence, text, target), topology graph, metric series, log stream, runbooks, agents. Alerts/incidents/timeline want live updates (stream or poll).

## Design Tokens

### Colors — light (`:root`)
```
--bg #f2f3f3   --surface #ffffff  --surface2 #fafbfc  --surface3 #f0f2f3
--border #eaeded  --border2 #d5dbdb
--fg #161d26   --fg2 #687078  --fg3 #8b949c
--nav #161d26  --navfg #f7f8f8  --navsub #a3adb8
--accent #ff9900  --link #0972d3
--crit #d13212  --critbg #fdf3f1   --warn #b7791f  --warnbg #fdf7ec
--ok #037f0c    --okbg #f0f8f1     --info #0972d3  --infobg #f0f7ff
--ai #7d4bd6    --aibg #f6f2fd
--shadow 0 1px 4px rgba(0,7,22,.10)
```

### Colors — dark (`[data-theme="dark"]`, the default)
```
--bg #0c1116   --surface #161d26  --surface2 #1b232e  --surface3 #212b38
--border #2a3542  --border2 #3a4757
--fg #f2f3f3   --fg2 #9aa7b4  --fg3 #7a8794
--nav #000716  --navfg #f7f8f8  --navsub #96a3b0
--accent #ff9900  --link #539fe5
--crit #ff7c70  --critbg #2a1614   --warn #f0b429  --warnbg #2a2113
--ok #5dd47a    --okbg #12241a     --info #539fe5  --infobg #12212f
--ai #b18cf0    --aibg #1f1830
--shadow 0 1px 4px rgba(0,0,0,.45)
```
Extra literal used on the footer hint: `#5f6b78`. Nav-surface text literals: `#e6eaee`, `#a3adb8`, `#8d99a6`. Nav hairlines/fills: `rgba(255,255,255,.06)` / `.18`.

**Repository addition (2026-09-18), not yet in Claude Design:** `--control-edge #6e7378` — the boundary of every control. `--border2` is 1.79:1 on `--surface`, below WCAG 1.4.11's 3:1, so controls use this instead. No light value is defined; adopting the light theme needs one computed against `--surface #ffffff`. See [the control-edge plan](../superpowers/plans/2026-09-18-control-edge-contrast.md).

### Typography
Families (Google Fonts, weights 400/500/600/700):
- UI: `"IBM Plex Sans", "IBM Plex Sans Thai", Arial, sans-serif`
- Numerics, ids, timestamps, code, key hints: `"IBM Plex Mono", monospace` (400/500/600)

Scale (size / line-height / weight):
| Use | Value |
| --- | --- |
| KPI value | 28 / 32 / 700, ls −.6px, mono |
| Stat value | 18 / — / 700, mono |
| Popover name | 15 / — / 700 |
| Product mark | 15 / — / 600 |
| Body base | 14 / 20 / 400 |
| Row title | 14 / — / 500 |
| Controls, menu rows, filters | 13 / — / 400–600 |
| Secondary text, table cells, meta | 12 / — / 400–500 (mono where numeric) |
| Section label | 12 / — / 700, uppercase, ls .3px |
| Sub-lines, hints, ids | 11 / — / 400–600 (mono) |
| Status pill | 11 / — / 600–700, ls .3–.5px |
| Eyebrow (AI, stat keys) | 10 / — / 700, uppercase, ls .4–.5px |
| Chart axis | 10 / — / 400, mono |

### Spacing
4px base. Used: 1, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14, 16, 18, 24. Card padding 12–18px; content gutter 24px; grid/stack gaps 12–16px; inline gaps 5–12px.

### Radii
`3px` icon buttons + severity chips · `4px` inputs, pills, control groups · `6px` scrollbar thumb · `8px` cards and panels · `10px` status pills · `50%` dots and avatars.

### Elevation & borders
One shadow only: `--shadow`. Borders: 1px `--border` on cards, 1px `--control-edge` on controls/inputs (the export says `--border2`; see the repository addition above). Accent edges: `border-top: 3px solid <severity>` (incident header), `border-left: 3px solid var(--accent)` (recommended runbook).

### Fixed dimensions
Window chrome 32px · top bar 48px · scope pill 30px · segmented control 32px (6px 13px padding) · severity chip 22px · status pill 19px · icon button 22×20px · status dot 7–8px · nav icon 14–15px.

## Assets
- **Fonts** — IBM Plex Sans, IBM Plex Sans Thai, IBM Plex Mono (Google Fonts; self-host in production).
- **Icons** — all icons are hand-authored inline SVG paths on a 24×24 viewBox, `fill: none`, `stroke: currentColor`, `stroke-width: 1.6–2`, round caps/joins. They live in the `const I = { … }` map near the top of the script block in the HTML file (home, alert, incident, topo, metrics, logs, ai, runbooks, agents, reports, settings, search, sparkle, chevrons). Substitute the codebase's icon set if it has one with a matching 1.5–2px stroke weight — do not mix in filled icons.
- **Imagery** — none. No photography, no illustration, no emoji.

## Screenshots
`screenshots/` — dark theme, EN, `sidebar` nav, captured at ~924px wide (below the 1280px target, so some columns are tighter than intended; trust the HTML prototype at 1440×900 for true proportions):

| File | View |
| --- | --- |
| `01-home.png` | Home / widget dashboard |
| `02-alerts.png` | Alerts triage |
| `03-incidents.png` | Incidents list |
| `04-incident-detail.png` | Incident detail (INC-4821) |
| `05-ai-panel.png` | AI assistant panel open |
| `06-command-palette.png` | Command palette (⌘K) |
| `07-topology.png` | Topology |
| `08-metrics.png` | Metrics |
| `09-logs.png` | Logs |
| `10-ai-insights.png` | AI insights |
| `11-runbooks.png` | Runbooks |

Not captured: Agents, Reports, Settings, light theme, Thai locale, `focus` home layout, `board` triage mode — open the HTML prototype and toggle the props to see them.

## Files
- `AIOps Command Center.dc.html` — the complete prototype: all twelve views, the shell, the palette, the AI panel, both themes, both languages, and the layout/nav/triage variations. Template markup first, then the icon map, translation maps, mock data, and the logic class.
- `support.js` — prototype runtime only. Do not port.

Variation switches exposed on the prototype (change them at the top of the script's `data-props` defaults, or via the tooling that renders the file):
`platform` macos|windows · `navMode` sidebar|services · `homeLayout` widgets|focus · `triageMode` inbox|board · `startTheme` system|dark|light · `startLang` en|th · `noiseReduction` boolean.
