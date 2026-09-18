# Plan: give every control an edge that reaches 3:1

**Date:** 2026-09-18
**Status:** Built on `claude/control-edge-contrast`, awaiting the user's
decision on the screenshots. Not merged.
**Class:** B. One new `:root` token (Class A) that twelve control rules are
rewired to use (Class B). No class name, role or accessible name changes,
so none of the four class-asserting test files are touched.
See [UX/UI change playbook](../../design/ux-ui-change-playbook.md), Step 1.

## Why

`npm run contrast` reports nine controls failing WCAG 1.4.11 (readiness gap
3 in the playbook). Every secondary button and `select` draws its boundary
with `color-mix(in srgb, var(--fg) 18–30%, transparent)`, giving
1.72–2.55:1 against the surface, and their fills are within 1.12:1 of that
surface. So the edge is the only thing that marks them as controls, and it
is too faint.

WCAG's Understanding document exempts a border that is not needed to
identify the control, such as a button with a text label. All nine have text
labels, so this is a design decision rather than a compliance defect. We are
fixing it anyway, because on a dark console the edge is what tells you
something is a button.

## Findings from the audit

1. **The script's list was short by three.** The buttons
   `.topology-graph__relationship-select`, `.correlation-candidate` and
   `.change-entry` also draw a 25% `--fg` edge, and none of them was
   measured. The list below comes from a scan of every rule with a
   `--fg`-mix or `--border` edge, checked against its element in the `.tsx`
   files, so the script's control list is now exhaustive.
2. **`.incident-queue__row` fails for a different reason.** Its edge is
   `var(--border)` (1.36:1), not an `--fg` mix. The script also claimed the
   row has a `--surface2` fill, which it does not. The fill is now `null`.
3. **The design has no compliant value to follow.** The handoff's strongest
   edge, `--border2`, is 1.79:1 on `--surface`. Step 3.1 says to change
   Claude Design first, but this change corrects the design rather than
   following it. **Decision for the user:** we skipped the drift check and
   recorded the skip (the change cannot depend on the design's content). On
   approval, the token goes into `aiops-command-center-handoff.md` in Step 7
   and into Claude Design by hand.

## The token

`--control-edge: #6e7378`, which is `--fg` at 40% flattened over
`--surface`. We chose a solid hex rather than a `color-mix` because
`contrast.mjs` parses only hex tokens, and because a solid value measures
the same on every surface.

| On | Ratio |
|---|---|
| `--bg` | 3.96:1 |
| `--surface` | 3.54:1 |
| `--surface2` | 3.31:1 |
| `--surface3` | 2.99:1 (used only for the selected incident row, whose edge is `--fg`) |

Why 40% and not the 35–36% proposed on 2026-09-17: 36% clears `--surface`
at 3.11:1, but only just, and it fails `--surface2`. 38% clears both, with
3.09:1 on `--surface2`. 40% leaves margin on every surface a control sits
on today.

## Scope, in two tiers

**Tier 1: standalone controls** (commit 2). These are unambiguously
controls, and their only boundary is the edge.

| Rule | File | Was |
|---|---|---|
| `.shell-header button, .shell aside button, .notification-center button` | `styles.css` | 30% |
| `.operations-widget-settings__main select` | `styles.css` | 30% |
| `.ai-button` | `ai/ai.css` | 30% |
| `.ai-fallback-order__actions button` | `ai/ai.css` | 30% |
| `.ai-provider-form input, .ai-provider-form select` | `ai/ai.css` | 30% |
| `.topology-filters__field select` | `topology/topology.css` | 30% |

**Tier 2: selectable items** (commit 3, which can be dropped). These are
buttons or listbox options that look like cards. You could argue each is
identified by its content and position rather than its edge, which is the
Understanding document's exemption. Raising their edges makes lists visibly
heavier.

| Rule | File | Was |
|---|---|---|
| `.operations-critical-number__button` | `styles.css` | 18% |
| `.topology-graph__node-select` | `topology/topology.css` | 25% |
| `.topology-graph__relationship-select` | `topology/topology.css` | 25% |
| `.correlation-candidate` | `correlation/correlation.css` | 25% |
| `.change-entry` | `change/change.css` | 25% |
| `.incident-queue__row` | `incident/incident.css` | `--border` |

If Tier 2 is dropped, those six move into a script section called
"selectable items: identified by content, measured not gated". The script
then passes, and it can still join CI.

Selected, hover and pressed states keep their own edges (`--accent` or
`--fg`), which are already stronger.

## Invariants at risk

- **Focus visibility.** The 3px `--accent` focus ring has to stay distinct
  from a brighter grey edge. Check with the keyboard.
- **Status not by colour alone.** Unaffected, because no status pill uses
  these rules.
- **Localization.** No strings change.
- **Token discipline.** The new value lives in `:root`, and
  `npm run lint:tokens` stays green.

## Commits

1. `feat(ui): add a control-edge token that reaches 3:1`: the `:root` token
   only.
2. `fix(ui): draw standalone controls with the control edge`: Tier 1.
3. `fix(ui): draw selectable items with the control edge`: Tier 2.
4. `chore(ui): measure every control, and gate contrast in CI`: the script's
   list completed, the `.incident-queue__row` fill corrected, and
   `npm run contrast` added to CI after `lint:tokens`.

## Verification

- The seven gates from playbook Step 6, plus `npm run contrast`, which
  should exit 0.
- Before and after screenshots, English and Thai, from the preview harness
  (`npm run dev:preview`) of every screen with a touched control. Controls
  that the fixtures cannot reach are named as unverified.
- Keyboard: a Tab pass over a screen with Tier 1 controls to confirm the
  focus ring.

## On approval (Step 7)

- Add `--control-edge` to `aiops-command-center-handoff.md` and the spec.
- Update playbook readiness gap 3 to done, and the `:root` token count from
  32 to 33.
- Merge `--no-ff`, push, and confirm CI.

## Verification (2026-09-18, on the branch)

- `npm run contrast`: **39 required pairs, 0 below threshold**, exit 0.
  Controls now measure 3.54:1 on `--surface`, 3.73:1 on the topology
  graph's mixed surface, and 3.96:1 on `--bg`.
- Gates: `format:check`, `lint`, `lint:tokens`, `contrast` and
  `typecheck` pass; 236/236 frontend tests pass; `cargo fmt` is clean.
  Rust is untouched.
- Before and after screenshots in English and Thai: Command Center,
  Customize console, Incidents, Signal correlation, Resource topology,
  Changes and Integrations. Visible in the harness: the header and sidebar
  buttons, the Customize console buttons and selects, the topology filter
  selects, and the critical-number tiles.
- **Not verifiable in the harness:** the AI provider form and the
  fallback-order buttons (providers fail to load), the topology graph
  buttons, correlation candidates, change entries and incident queue rows.
  With no connectors, each of these renders an empty or error state. Their
  ratios are computed, but nobody has looked at them on screen.
- Keyboard: the 3px `--accent` focus ring stays distinct from the new edge,
  on a header button and on a critical-number tile.
