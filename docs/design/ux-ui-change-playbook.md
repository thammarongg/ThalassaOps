# UX/UI Change Playbook

**Status:** Adopted 2026-09-15
**Applies to:** any change to UI that already exists — palette, type, layout,
components, navigation, or how a view presents its data.
**Does not apply to:** new views that arrive with new backend capability (AI
insights, Runbooks, Reports). Those follow the normal sprint pattern: design
document, user approval, task plan.

This is the procedure for changing ThalassaOps's UX/UI after the application is
built, so that a redesign lands as one deliberate change rather than drifting in
over several sessions.

## Why this exists

The AIOps Command Center adoption (2026-09-07 to 2026-09-15) produced four
lessons. Each rule below traces back to one of them.

1. **Two directions were built at once.** A chart-palette pilot sat
   uncommitted on `main` while the Claude Design import sat uncommitted in a
   worktree, for five days. Choosing between them meant untangling both. →
   *One direction, one branch, committed daily.*
2. **"Replace the design" meant three different things in two days.** It was
   a reskin on 2026-09-10, then a drop of the governance rules the same
   evening, then a reversal of that on 2026-09-11. → *A visual change and a
   product-model change are different classes with different gates.*
3. **The rebrand cost one file.** Every workspace stylesheet takes colour and
   type from `:root` tokens in `ui/src/styles.css`, so adopting a new palette
   was a value swap that reskinned every screen. → *That property is the
   asset; guard it.*
4. **Green tests said nothing about the palette.** 234 frontend tests passed
   across two unrelated palettes, because `vitest-axe` runs in jsdom, which
   skips the colour-contrast rule. → *A passing suite is not visual
   verification.*

## Sources of truth

| Layer | Where | Owns |
|---|---|---|
| Design | Claude Design project `e51ce2dd-9138-45bb-8bab-70d2c5b6de50`, file `AIOps Command Center.dc.html` | What the product should look like |
| Handoff | [`aiops-command-center-handoff.md`](aiops-command-center-handoff.md) | Exact token values, type scale, spacing, radii, dimensions |
| UX/UI spec | [`aiops-command-center.md`](aiops-command-center.md) | Views, layout, interaction rules, adopted variants |
| Governance | [Reconciliation design](../superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md) | How governance rules render in the visual system |
| Requirements | [`requirements-summary.md`](../requirements/requirements-summary.md) §13, [policy baseline](../policies/operational-policy-baseline.md) | Product rules no design may drop |
| Code | `ui/src/styles.css` `:root`, `ui/src/design-system/` | What actually ships |

**Design baseline.** The spec was written against `AIOps Command Center.dc.html`
with SHA-1 `ac06c6d0be21f0aea8ec7f28b58acbb5e88f3513` (173,526 bytes, exported
2026-09-10). Claude Design reports no last-modified time, so the hash is the
only way to know whether the design has moved since.

## Step 1 — Classify the change

Every change is one of three classes. When one change spans classes, it takes
the strictest gate.

| Class | What changes | Where | Blast radius | Gate |
|---|---|---|---|---|
| **A — Tokens** | Colour, type, spacing, radius or shadow values | `:root` in `ui/src/styles.css` (29 tokens) | Every screen at once | Computed contrast + screenshots |
| **B — Layout and components** | Structure, component markup, class names, variants, navigation layout | `ui/src/design-system/`, `ui/src/shell.tsx`, workspace `.tsx`/`.css` | The screens touched | Class A gate + test review + user sign-off on screenshots |
| **C — Product model** | What a view means: severity or priority display, AI disclosure, action risk labels, redaction line, theme default, removing any requirement | Spec + requirements + `ui/contracts/ipc.ts` + Rust types | Across the stack | Design document → user approval → task plan, as for a sprint |

Precedents: the 2026-09-10 rebrand was Class A; the Phase 1 shell and the
Phase 2 Incident Workspace restyle were Class B; the 2026-09-10 governance drop
and its 2026-09-11 reversal were Class C.

**A Class C change never rides inside an A or B change.** If a redesign in
Claude Design omits a governance element, that omission is a Class C question
to ask the user, not a detail to follow.

## Step 2 — Invariants every change keeps

A change that breaks one of these is Class C by definition.

**Governance (requirements §8.2, §10, §13; reconciliation design Sections 1–5)**

- Severity `S1`–`S5` and priority stay separate, visible fields.
- Evidence, policy and action state stay visible beside AI output (compact
  card plus "Show evidence" disclosure).
- Actions and commands carry a risk-class pill and an execution-mode pill.
- A `sensitive fields redacted: N` line appears whenever `N > 0`.
- Status is never communicated by colour alone.
- Dark mode is the default theme (requirements §13). Offering light as an
  option is Class A/B; making it the default is Class C.

**Localization**

- Every visible string comes from both `ui/src/locales/en.ts` and `th.ts`
  (966 keys each, in parity). The `local/no-user-facing-jsx-text` lint rule
  enforces this — see [`docs/design-system-conventions.md`](../design-system-conventions.md).
- Thai runs about 30% longer than English; label containers must tolerate
  that growth.

**Token discipline**

- No raw colour literal outside `:root`. Today the five workspace stylesheets
  contain none; the known exceptions are listed under Readiness gaps.
- New visual values are added as tokens first, then used.

**Test contracts**

- 24 test files query by accessible role, name or text. Keep roles and
  accessible names stable, or update those tests in the same change.
- 4 test files assert class names: `shell.test.tsx`,
  `design-system/components.test.tsx`, `incident/IncidentNarrative.test.tsx`
  and `operations/operations-console.acceptance.test.tsx`. The
  `.indicator--{tone}` classes are the severity-to-tone contract. A Class B
  change keeps them or updates these four files deliberately.

## Step 3 — Bring the design in

1. **Change the design in Claude Design first**, not in code. Code that leads
   the design turns the spec into an after-the-fact description.
2. **Check drift.** Run `/design-login` if DesignSync is not authorized, then
   `DesignSync get_file` on `AIOps Command Center.dc.html` (173 KB, under the
   256 KiB read cap), and compare its SHA-1 with the baseline above. The
   handoff README can be compared the same way against
   `aiops-command-center-handoff.md`.
3. **Export the new handoff** from Claude Design and replace
   `aiops-command-center-handoff.md` in the same change, so token values in the
   repository always match the design being implemented.
4. **Diff view by view.** Update only the spec sections whose view changed.
   The reconciliation design and several plans link to spec section anchors;
   rewriting the spec wholesale breaks them.

## Step 4 — Write it down before building

- **Class A or B:** a plan in `docs/superpowers/plans/YYYY-MM-DD-<name>.md`
  stating the class, the screens touched, the invariants at risk, and the
  verification to run. The 2026-09-10 visual-rebrand plan is the model.
- **Class C:** a design document in `docs/superpowers/specs/` and the user's
  approval before any code, exactly as for a sprint.
- A rejected direction is **archived, not lost:** tag its last commit under
  `archive/` before deleting its branch. `archive/visual-system-pass-pilot`
  is the example.

## Step 5 — Build on one branch

- One branch and one worktree per direction. Never two competing visual
  directions open at once.
- Commit at least daily. Uncommitted design work in a worktree is invisible
  to every other session and to `git log main..branch`.
- Token changes go in first as their own commit, so a Class A rollback is a
  single revert.

## Step 6 — Verify

Run all seven gates, as in CI:

```bash
npm run format:check && npm run lint && npm run typecheck
NODE_OPTIONS=--no-experimental-webstorage npm test   # the flag is needed on Node 25+
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Then the checks the suite cannot make:

- **Contrast, computed.** Every text token against every surface it sits on
  must reach WCAG AA — 4.5:1 for text, 3:1 for large text and UI boundaries —
  including hover and selected states. Compute the ratios; jsdom does not.
- **Screenshots** of every screen the change touches, in English and Thai,
  reviewed by the user. Use the preview harness (`npm run dev:preview`) or
  `npm run tauri:dev`. With no connectors configured, empty states are
  correct, not a defect.
- **Keyboard and motion.** `⌘K` opens the palette, `Escape` closes overlays,
  focus stays visible, and reduced-motion settings are respected.
- **Governance spot-check.** For each invariant in Step 2 that the change
  touches, confirm it on screen, not only in the spec.

## Step 7 — Close the loop

- Update the spec, the handoff, and the design baseline hash in this document.
- Merge with a merge commit so the whole change reverts with
  `git revert -m 1 <merge>`.
- Push and confirm CI is green.

## Options already designed

The handoff already specifies these. Each can be adopted without a new design
round.

| Option | Handoff source | Class | Notes |
|---|---|---|---|
| Light theme as a user option | Light `:root` token block | A + B | Needs a theme toggle and a contrast recompute. As the default it becomes Class C. |
| `focus` Home layout | `homeLayout` | B | Uses existing incident data; no backend change. |
| `board` incident triage | `triageMode` | B | The reconciliation design's backlog notes there is no current requirement for it. |
| `services` navigation | `navMode` | B | Previously judged redundant with the sidebar groups. |
| Fake window chrome | `platform` | — | Rejected: Tauri draws the real title bar. |

## Readiness gaps

These make future changes safer. None is urgent; each is small.

1. **Preview harness — done 2026-09-15.** `npm run dev:preview` serves
   `ui/dev.html`, which renders the real `Shell` against
   `ui/src/dev/operations-fixture.ts` without Tauri. Use it for screenshots;
   judge correctness against the backend, never against the harness.
2. **Raw colours outside tokens.**
   - `ui/src/observability/MetricsPanel.tsx:177` uses an inline
     `background: "#f5f5f5"` for the selected-alert context box. On the dark
     theme it renders light text on a light box.
   - `ui/src/styles.css` has two literals outside `:root`: a
     `rgb(0 0 0 / 55%)` scrim and a `rgb(0 0 0 / 13%)` shadow.
3. **No contrast tool.** Contrast is computed by hand today. A small script
   that reads `:root` and prints each text/surface ratio would make Step 6
   repeatable.
4. **No lint guard for token discipline.** A stylelint or grep check in CI
   rejecting colour literals outside `:root` would keep invariant 3 true
   without relying on review.
5. **No ADR.** "Visual properties flow only through `:root` tokens" has been
   load-bearing twice and could be recorded as `docs/adr/0007`.

## Out of scope

- Adding views that need backend capability, which follow the sprint gate.
- Governance rules themselves, which change in the reconciliation design and
  the requirements, not here.
