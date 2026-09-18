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
only way to know whether the design has moved since. The repository has run
one token ahead of the design since 2026-09-18 (`--control-edge`); a re-export
that lacks it must not remove it.

## Step 1 — Classify the change

Every change is one of three classes. When one change spans classes, it takes
the strictest gate.

| Class | What changes | Where | Blast radius | Gate |
|---|---|---|---|---|
| **A — Tokens** | Colour, type, spacing, radius or shadow values | `:root` in `ui/src/styles.css` (33 tokens) | Every screen at once | Computed contrast + screenshots |
| **B — Layout and components** | Structure, component markup, class names, variants, navigation layout | `ui/src/design-system/`, `ui/src/shell.tsx`, workspace `.tsx`/`.css` | The screens touched | Class A gate + test review + user sign-off on screenshots |
| **C — Product model** | What a view means: severity or priority display, AI disclosure, action risk labels, redaction line, theme default, removing any requirement | Spec + requirements + `ui/contracts/ipc.ts` + Rust types | Across the stack | Design document → user approval → task plan, as for a sprint |

Precedents: the 2026-09-10 rebrand was Class A; the Phase 1 shell and the
Phase 2 Incident Workspace restyle were Class B; the 2026-09-10 governance drop
and its 2026-09-11 reversal were Class C.

**A Class C change never rides inside an A or B change.** If a redesign in
Claude Design omits a governance element, that omission is a Class C question
to ask the user, not a detail to follow.

**A rename is its own case.** Renaming a token or a class touches every
screen like Class A but changes no value, so the contrast gate is not
skipped — it is inapplicable. Its gate is a **round-trip diff**: apply the
inverse map to the changed files and diff the result against the previous
commit. An empty diff proves the change is a bijective substitution of
identifiers and therefore cannot move a pixel. Run the collision pre-check
first: every new name must be absent from `ui/` beforehand, as a custom
property *and* as a BEM modifier, or two different things merge silently.
Worked example: [the 2026-09-17 token rename](../superpowers/plans/2026-09-17-design-token-rename.md).

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

- No raw colour literal outside `:root`, anywhere in `ui/src` — stylesheets
  and components alike. `npm run lint:tokens` enforces this in CI, so this
  invariant is the one you cannot break by accident.
- New visual values are added as tokens first, then used.

**Test contracts**

- 24 test files query by accessible role, name or text. Keep roles and
  accessible names stable, or update those tests in the same change.
- 2 test files assert class names: `shell.test.tsx` and
  `design-system/components.test.tsx`. The `.indicator--{tone}` classes are
  the severity-to-tone contract. A Class B change keeps them or updates these
  two files deliberately. `operations/operations-console.acceptance.test.tsx`
  selects by `data-widget-id` and `data-testid`, which are contracts too.

## Step 3 — Bring the design in

1. **Change the design in Claude Design first**, not in code. Code that leads
   the design turns the spec into an after-the-fact description.
2. **Check drift.** Run `/design-login` if DesignSync is not authorized, then
   `DesignSync get_file` on `AIOps Command Center.dc.html` (173 KB, under the
   256 KiB read cap), and compare its SHA-1 with the baseline above. The
   handoff README can be compared the same way against
   `aiops-command-center-handoff.md`.
   `list_files` returns neither a hash nor a size, so there is no cheap
   version of this check — it costs a full 173 KB read. Run it for any change
   that follows the design. Skip it, and record that you did, for a change
   that cannot depend on the design's content: a rename, a lint guard, a
   contrast script.
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

Run all the gates, as in CI:

```bash
npm run format:check && npm run lint && npm run lint:tokens && npm run contrast && npm run typecheck
NODE_OPTIONS=--no-experimental-webstorage npm test   # the flag is needed on Node 25+
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Then the checks the suite cannot make:

- **Contrast, computed.** `npm run contrast` prints every pair the product
  renders: text against its surface (4.5:1), each control against what it
  sits on (3:1, on its edge **or** its fill — WCAG 1.4.11 asks that the
  control be identifiable, not that its border specifically carry it), and
  the decorative edges, measured but not gated. It runs in CI directly after
  `lint:tokens`, so a colour change that drops a required pair below
  threshold fails the build.
  Read the whole table when a change touches colour: a pair moving from
  6:1 to 4.6:1 passes and still matters.
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

These make future changes safer.

1. **Preview harness — done 2026-09-15.** `npm run dev:preview` serves
   `ui/dev.html`, which renders the real `Shell` against
   `ui/src/dev/operations-fixture.ts` without Tauri. Use it for screenshots;
   judge correctness against the backend, never against the harness.
2. **Raw colours outside tokens — done 2026-09-17.** `MetricsPanel`'s inline
   `#f5f5f5` box is now `.metrics-panel__context`; the drawer scrim and the
   raised widget shadow are `--scrim` and `--shadow-widget`; `--radius-1`,
   referenced four times and never defined, is defined. `ui/src` holds no
   colour literal outside `:root`.
3. **Contrast tool — done 2026-09-17; control edges fixed 2026-09-18.**
   `npm run contrast` (`scripts/contrast.mjs`) computes every ratio,
   including `color-mix` edges resolved against what they sit on. All 23
   text pairs passed. **Nine controls failed:** every secondary button and
   `select` drew its boundary with `color-mix(in srgb, var(--fg) 18–30%,
   transparent)`, giving 1.4–2.6:1 where 1.4.11 wants 3:1, and their fills
   were within 1.1:1 of the surface behind them, so the edge was the only
   thing identifying the control. Three more buttons, which the tool had
   not listed, had the same problem. `--control-edge` (`#6e7378`) now draws every control's
   boundary at 3.54:1 on `--surface` and 3.96:1 on `--bg`. The script
   exits 0 and runs in CI after `lint:tokens`. See
   [the control-edge plan](../superpowers/plans/2026-09-18-control-edge-contrast.md).
4. **Lint guard — done 2026-09-17.** `npm run lint:tokens`
   (`scripts/check-design-tokens.mjs`, no dependencies) fails on any colour
   literal in `ui/src` outside the `:root` block, in stylesheets and in
   components alike. It runs in CI directly after `lint`.
5. **ADR — done 2026-09-18.** [ADR 0007](../adr/0007-colour-and-type-live-in-root-tokens.md)
   records that colour and type live only in `:root`. It also records that
   spacing and radius literals are deliberately allowed, so tokenising them
   is a planned Class B change, not a cleanup.

## Out of scope

- Adding views that need backend capability, which follow the sprint gate.
- Governance rules themselves, which change in the reconciliation design and
  the requirements, not here.
