# Plan: close the playbook's readiness gaps

**Date:** 2026-09-17 (same day as, and after,
[the token rename](2026-09-17-design-token-rename.md))
**Applies:** [UX/UI change playbook](../../design/ux-ui-change-playbook.md),
"Readiness gaps" 2–5.

Four items, requested together. They are not one class, so each carries its
own gate.

| # | Item | Class | Pixels move? |
|---|---|---|---|
| 1 | `MetricsPanel.tsx:177` inline `#f5f5f5` box | B — one component's styling | **Yes**, and that is the point |
| 2 | `--radius-1` used but never defined | A — a token value, already the effective one | No |
| 3 | Colour literals `--scrim` / `--shadow-widget` | A — names for existing values | No |
| 4 | Contrast script, token lint guard | Tooling — no UI change | No |

**Design baseline:** unchanged; drift check skipped and recorded, per the
playbook's Step 3 note. None of this follows the design's content: items 2–4
preserve values, and item 1 is a bug fix the design never specified, since
the mockup has no Prometheus panel.

## 1 — The light box on a dark screen

`ui/src/observability/MetricsPanel.tsx:177` renders the selected-alert
context box with `style={{ marginBottom: "1rem", padding: "0.5rem",
background: "#f5f5f5" }}`. The app has been dark-only since 2026-09-10, so
this paints near-white behind `--fg` text: unreadable, and the last raw
colour in any component.

Replaced by a `.metrics-panel__context` class in `ui/src/styles.css`, beside
the other observability panel classes, using `--surface2`, `--border` and
`--radius-1` — the same recipe as `.incident-evidence__entry`, so the box
matches every other context box in the product.

Markup is otherwise untouched: no role, accessible name or text changes, so
no test is affected (`shell.test.tsx` is the only suite that reaches
observability, and it asserts on text, not styling).

This is the one item where a screenshot would be the real evidence, and the
one that cannot be screenshotted: the box renders only when an alert is
selected against a configured Prometheus connector, which the preview
harness has no way to reach. Verified instead by the computed values of the
tokens used, which item 4's script now prints.

## 2 and 3 — The last literals

- `--radius-1: 4px` is added to `:root`. It was already the effective value
  at all four call sites via `var(--radius-1, 4px)`; those fallbacks are
  dropped. Same shape as the `--color-fog` bug the reconciliation design
  found: a variable referenced but never defined, surviving only because CSS
  silently accepts it.
- `--scrim: rgb(0 0 0 / 55%)` (drawer backdrop) and
  `--shadow-widget: 0 1rem 2.5rem rgb(0 0 0 / 13%)` (the raised operations
  widget card) name the two literals left in `styles.css` outside `:root`.

Naming them rather than allowlisting them keeps item 4's rule absolute: no
colour literal outside `:root`, no exceptions to remember.

`--shadow-widget` is deliberately a second shadow, which the handoff does
not have ("One shadow only"). The value is what ships today; adopting the
handoff's single-shadow rule is a visual decision, not a cleanup, and is out
of scope here.

## 4 — The two scripts

`scripts/check-design-tokens.mjs` — the guard. Fails when a colour literal
(hex, `rgb()`, `hsl()`, or a common named colour) appears in `ui/src` CSS
outside the `:root` block, or anywhere in a component. Zero dependencies,
so CI gains a step, not a toolchain. Run by `npm run lint:tokens`, wired
into `.github/workflows/ci.yml` beside `lint`.

`scripts/contrast.mjs` — the measurement the test suite cannot make.
Computes the WCAG relative-luminance ratio for every text-on-surface and
boundary-on-surface pair the product actually renders, and prints them with
their thresholds (4.5:1 text, 3:1 large text and UI boundaries). This is
Step 6's "compute the ratios; jsdom does not", made repeatable.

**Whether it becomes a CI gate is decided after the first run, not before.**
A failing pair means a colour decision, which is not this change's business.

## Verification

- The guard must be **red before item 1 and green after** — run it first
  against the untouched `MetricsPanel.tsx`. A guard that has never failed
  has not been tested.
- The contrast script's full output is reported to the user, pass or fail.
- Seven gates. Rust is untouched, so CI carries the three cargo steps.
- Round-trip reasoning covers items 2 and 3: `4px` for `4px`, and each
  literal for a variable holding exactly that literal.

## Outcome

All four items landed. The guard behaved as a guard should: run against the
untouched tree it failed on `MetricsPanel.tsx:177` and nothing else, and it
passed once that box became a class. It scans 101 files.

The contrast script found more than expected, and the first version of it
was wrong. It reported three failing "boundaries" that were card hairlines
and dividers — decorative edges WCAG sets no threshold for — while missing
the boundaries that do matter, because those are written as `color-mix`
rather than as tokens. Rewritten to resolve `color-mix` against whatever it
sits on, and to judge a control on its edge **or** its fill, it reports:

- 23 text pairs, all passing, the tightest being `--fg3` on `--surface` at
  4.62:1.
- 13 controls, **nine failing**: every secondary button and `select` in the
  app is drawn as text plus a `color-mix(in srgb, var(--fg) 18–30%,
  transparent)` hairline, 1.72–2.55:1, over a fill within 1.1:1 of its
  surroundings. The four that pass use `--accent` (7.92:1) or a solid
  accent fill.
- 6 decorative edges, measured, not gated.

The fix is small — 35% over `--surface`, 36% over `--bg` — and it changes
how every button in the product looks, so it is a Class A/B change with its
own plan and approval, not something to slip into a cleanup. Recorded in
the playbook's readiness gaps; the script stays out of CI until it is
green, so that CI never carries a known failure.
