# AIOps Command Center — visual rebrand (supersedes Phase 1's palette)

Follows `2026-09-10-aiops-command-center-shell-phase1.md`. That doc's
*structure* decisions (grouped sidebar, real-IPC-only Home, deferred
views) still stand. This doc records a scope change the user made
explicitly after Phase 1 shipped: **adopt the Claude Design mockup's
visual identity wholesale**, superseding the palette/typography in
`docs/design/ux-ui-concept.md`.

**Superseded by a later, larger decision:** the same day, the user asked
to deprecate `ux-ui-concept.md` entirely, not just its palette/typography.
The authoritative spec is now
`docs/design/aiops-command-center.md`; `ux-ui-concept.md` carries a
deprecation banner and is kept only for history. This doc's palette
mechanism and verification below are still accurate and unaffected —
only the "which doc is the spec of record" framing changed.

## Decision

Asked the user directly because the mockup and the already-approved spec
disagreed on visual identity (ocean palette + Manrope vs. AWS-console
palette + IBM Plex Sans), and disagreed on whether that mockup should
override a documented, approved decision. User chose: adopt the mockup's
style wholesale (this doc), and keep Phase 1's sidebar grouping (not in
the original spec's flat-list sketch, but no longer in question).

## Mechanism

Every component in `ui/src` already renders color and type through 7
`--color-*` custom properties and `--font-ui`/`--font-mono` — verified by
grepping every `.css` file for hex/rgb literals outside `styles.css`'s own
`:root` block (none found; `ai/ai.css`, `change/change.css`,
`correlation/correlation.css`, `incident/incident.css`,
`topology/topology.css` are all token-only). So the "wholesale" adoption
is a token-value substitution in `ui/src/styles.css`, not a
component-by-component rewrite — every workspace (Incidents, Topology,
Observability, Correlation, Environment, Integrations, AI provider panel)
reskins for free, including `OperationsConsole`, which Phase 1
deliberately left alone.

Two corrections to a naive 1:1 value swap:

- **Nav vs. panel split**: the mockup's most recognizable trait is the top
  bar being *darker* than card surfaces. A flat rename would have made
  `.shell-header` lighter than the page. Added `--color-nav: #000716`
  for the header; `--color-deep-water` (now `#161d26`) stays the sidebar/
  card tone, matching the mockup's `--surface`.
- **Informational vs. warning collision**: `--color-reef-cyan` is now
  `#ff9900` (brand accent, matches the mockup's use for nav/focus/logo).
  Mapping `.indicator--informational` to it would have put S5/informational
  one hue-step from `--color-amber` (`#f0b429`) — a near-miss that breaks
  "never use color alone" when the two sit side by side. Added
  `--color-info: #539fe5` (the mockup's `--link`/`--info`) instead.

Status indicators also gained a tinted-pill treatment
(background + text, `border-radius: 3px`) instead of text-only color,
matching the mockup's alert/severity chips.

## What did NOT change

- **Light theme**: the mockup is light-first with a dark toggle; only the
  dark values were adopted. The product stays single-theme (matches the
  user's approved answer — palette + typography, not a theme system — and
  avoids building an unrequested toggle).
- **Fake window chrome**: still dropped. `decorations` is unset in
  `src-tauri/tauri.conf.json`, so the OS draws the real title bar; this is
  a functional constraint, independent of the visual-identity question.
- **Per-view layouts**: this is a systemic reskin, not a rebuild of each
  screen to match the mockup's specific page compositions. Views the
  mockup shows with no backend today (Alerts triage, Metrics, Logs,
  standalone AI insights, Runbooks, Agents, Reports) remain scheduled on
  their existing sprints (19, 22, 26, …) per `docs/planning/sprint-plan.md`
  — "replace completely" is satisfied by the visual-system cascade across
  every existing screen, not by building those screens early against mock
  data.
- **"Ask AI" button**: still not added; no chat backend exists yet
  (Sprint 19).

## Files touched

- `package.json` — `@fontsource/manrope` removed;
  `@fontsource/ibm-plex-sans` and `@fontsource/ibm-plex-sans-thai` added.
- `ui/src/styles.css` — font imports, 7 `--color-*` values plus new
  `--color-nav`/`--color-info`/`*-bg` tokens, `.shell-header` background,
  `.indicator--*` pill treatment.
- `docs/design/ux-ui-concept.md` — Palette and Typography sections
  updated in place with a supersession note.

## Verification

`npm run typecheck`, `npm test` (236 tests, unchanged — nothing asserts
color or font), `npm run lint`, `npm run format:check` all green. Visual
check via Vite dev server + Browser pane screenshot (dark theme only,
`invoke` rejects outside Tauri as before).
