# AIOps Command Center — Phase 1 (shell + Home)

## Source

Claude Design project "AIOps Command Center design"
(`e51ce2dd-9138-45bb-8bab-70d2c5b6de50`), file `AIOps Command Center.dc.html`.
A full 11-view product mockup (Home, Alerts, Incidents, Topology, Metrics,
Logs, AI insights, Runbooks, Agents, Reports, Settings) with 100% mock data,
a fake macOS/Windows window chrome, and a light/dark AWS-console-style
palette. Decoded copy and extracted state script kept in the session
scratchpad for reference; not committed (source lives in the Design project).

## Scope decision (user-approved)

Phase 1 only: new shell chrome (top bar + sidebar) and the Home dashboard,
wired to real IPC data. Existing workspaces (Incidents, Topology,
Observability, Correlation, Environment, Integrations) keep rendering
inside the new shell unchanged. Views with no backend today (Runbooks,
Agents fleet, Reports, a standalone AI-insights feed, free-form Metrics
explorer, Logs search) are explicitly **out of scope** — building them
against the mock arrays (`INCIDENTS`, `ALERTS`, `NODES`, `LOGS`, …) would
repeat the Sprint 16 defect shape (UI reviewed against its own mock, not
the backend — see `[[review-ui-tasks-against-the-backend-not-the-mock]]`
in project memory).

## What ships in Phase 1

1. **Sidebar** regrouped from a flat 12-item list into four labelled groups
   mirroring the design's IA (`Operate` / `Investigate` / `Automate` /
   `Govern`), each `Area` given an icon. Existing `Area` ids, routing, and
   Favorites nav are unchanged — this is a presentational regrouping, not
   an IA rewrite. Sidebar gets a collapse toggle (new, default expanded).
2. **Top bar** restyled to the design's visual language: icons added to
   search / notifications / terminal buttons (all `aria-hidden`, so
   accessible names are untouched). A language toggle (en ⇄ th) is added —
   real, since `i18next` already carries both locales and nothing
   currently exposes a switch.
3. **Command palette** (existing `Drawer` + `CommandSurface`) restyled
   only — same components, same keybinding, same behavior.
4. **Home dashboard** (`OperationsConsole`): left as-is. Its CSS
   (`.operations-*` in `styles.css`) already has a deliberate, coherent
   visual identity (a documented "command-bridge / tide line" design with
   severity-accented headlines, card shadows, a 12-col widget grid) — it
   is not the bare, unstyled surface the shell chrome is. Reskinning it to
   chase the mockup's look would fight an existing intentional design for
   no functional gain. It inherits the shell's new chrome for free. No KPI
   strip either: the design's MTTA/MTTR cards have no backing field on
   `OperationsSnapshot`/`HealthSummary` today, so they're not built as
   decoration.

## Explicitly deferred / dropped

- **Fake window chrome** (macOS traffic lights / Windows menu bar): the
  app has `decorations` unset in `tauri.conf.json` (OS draws the title
  bar), so this part of the mockup is a prototype artifact, not a real
  requirement.
- **Google Fonts `<link>`**: blocked by the app's CSP
  (`default-src 'self'`) and the app should work offline. Existing
  self-hosted Manrope / IBM Plex Mono (`@fontsource/*`) stay as-is.
- **Light theme / theme toggle**: the product is intentionally a single
  dark "ocean" palette today (`--color-abyss` etc.); the mockup's
  light+dark token system is a separate design initiative, not requested
  here. No non-functional toggle is added.
- **Scope switcher (site/cluster)**: no such concept exists in
  `WorkspaceContext`. The existing Organization/Team/Workspace/Environment
  switcher buttons are kept, restyled only.
- **Services mega-menu**: redundant with the new sidebar groups; skipped.
- **"Ask AI" button / chat drawer**: no chat backend exists yet (Sprint 17
  built provider config, not a chat surface). Not added.
- Remaining 10 views: unchanged, deferred to later phases per the
  scope decision above.

## Files touched

- `ui/src/shell.tsx` — sidebar grouping/icons/collapse, top bar icons,
  language toggle.
- `ui/src/shell.test.tsx` — coverage for sidebar collapse and language
  toggle; all existing assertions (button names, landmarks, aria-labels)
  must keep passing unmodified.
- `ui/src/styles.css` — visual restyle of shell chrome + operations cards.
- `ui/src/locales/en.ts`, `ui/src/locales/th.ts` — new keys for group
  labels, sidebar collapse/expand, language toggle.

## Verification

- `npm run format:check`, `npm run lint`, `npm run typecheck`, `npm test`
  (frontend-only change; Rust gates untouched).
- Vite dev server + Browser pane screenshot of the new shell (per
  `[[screenshotting-thalassaops-ui]]`: outside Tauri, `invoke` calls
  reject, so drive/verify layout and interaction, not live data).
