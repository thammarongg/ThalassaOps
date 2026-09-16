# Plan: rename the visual tokens to their handoff names

**Date:** 2026-09-17
**Class:** B — token names are a code-wide interface, not a value change.
See [UX/UI change playbook](../../design/ux-ui-change-playbook.md), Step 1.
**Design baseline:** unchanged. This change cannot depend on the design's
content, so the drift check was deliberately skipped — see Feedback below.

## Why

`ui/src/styles.css` names its colours after an ocean palette that no longer
exists. `--color-reef-cyan` is `#ff9900`, an orange. `--color-abyss`,
`--color-sea-glass`, `--color-kelp`, `--color-coral` and `--color-fog` all
carry values from the AIOps Command Center handoff while keeping names from
the palette it replaced on 2026-09-10.

The [governance reconciliation design](../specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md)
already carries a "Token gap" table whose only purpose is to translate
between the handoff's names and the repository's. That table exists because
the names diverged; renaming the tokens is what removes the need for it.

The handoff names are the right target rather than a third invented set:
they are what the design owns, what a re-export from Claude Design will
carry, and what the light-theme block (already specified, not yet adopted)
is written in.

This is the first exercise of the playbook, chosen because it is mechanical
and provably value-preserving — the process is what is being tested.

## The map

Twenty-four renames, no value changes, no additions, no removals.

| Current | Handoff name | Value (dark) |
|---|---|---|
| `--color-abyss` | `--bg` | `#0c1116` |
| `--color-deep-water` | `--surface` | `#161d26` |
| `--color-deep-water-2` | `--surface2` | `#1b232e` |
| `--color-deep-water-3` | `--surface3` | `#212b38` |
| `--color-border` | `--border` | `#2a3542` |
| `--color-border-strong` | `--border2` | `#3a4757` |
| `--color-sea-glass` | `--fg` | `#f2f3f3` |
| `--color-fog` | `--fg2` | `#9aa7b4` |
| `--color-mist` | `--fg3` | `#7a8794` |
| `--color-nav` | `--nav` | `#000716` |
| `--color-nav-fg` | `--navfg` | `#f7f8f8` |
| `--color-nav-sub` | `--navsub` | `#96a3b0` |
| `--color-reef-cyan` | `--accent` | `#ff9900` |
| `--color-coral` / `-bg` | `--crit` / `--critbg` | `#ff7c70` / `#2a1614` |
| `--color-amber` / `-bg` | `--warn` / `--warnbg` | `#f0b429` / `#2a2113` |
| `--color-kelp` / `-bg` | `--ok` / `--okbg` | `#5dd47a` / `#12241a` |
| `--color-info` / `-bg` | `--info` / `--infobg` | `#539fe5` / `#12212f` |
| `--color-violet` / `-bg` | `--ai` / `--aibg` | `#b18cf0` / `#1f1830` |
| `--shadow-card` | `--shadow` | `0 1px 4px rgba(0, 0, 0, 0.45)` |

Not renamed: `--font-ui`, `--font-mono`, `--space-1`–`--space-3`,
`--radius-1`. The handoff names no font, spacing or radius variables — it
gives a scale, not a token set — so there is nothing to align to.

**`--link` is not introduced.** The handoff defines `--link` and `--info`
with the same hex in *both* themes (`#539fe5` dark, `#0972d3` light), so a
future light theme cannot split them. One variable, per the reconciliation
design's own row for `--link`.

## Scope

350 references across six stylesheets — `ui/src/styles.css` (178),
`ui/src/topology/topology.css` (57), `ui/src/ai/ai.css` (39),
`ui/src/correlation/correlation.css` (33), `ui/src/incident/incident.css`
(33), `ui/src/change/change.css` (10). No `.ts` or `.tsx` file references a
token name; no Rust file does.

Every screen is touched, and none changes.

## Invariants at risk

From the playbook's Step 2 list, only one is in play:

- **Token discipline.** The change must not introduce a raw literal or drop
  a variable. Verified by the round-trip diff below.

Not in play: no visible string changes, so localization is untouched; no
role, accessible name or class name changes, so the 24 role-querying and 4
class-asserting test files are untouched; no governance element moves.

**Collision pre-check (run before editing, passed):** none of the 24 new
names already exists in `ui/` as a custom property or a BEM modifier. The
near misses are all safe — `--warning`, `--critical` and `--informational`
are `.indicator--*` class modifiers, and no prefix of a new name ends at a
word boundary inside them.

## Steps

1. Rename in the six stylesheets, longest name first so that
   `--color-deep-water-2` is not eaten by `--color-deep-water`.
2. Rewrite the `:root` comment block. Its Phase 2 note ("`--color-fog` was
   already referenced 12 times with no definition") is a fixed bug's
   history, and false once the name changes; it becomes a pointer to the
   handoff and to this plan.
3. Docs, as a separate commit: the spec's one `--color-nav` reference, and a
   dated note above the reconciliation design's token table.

The 2026-09-10 and 2026-09-11 plans keep their old names. They are dated
records of what was done at the time, not live references.

## Verification

Beyond the seven gates:

- **Round-trip diff — the proof that pixels cannot move.** Apply the inverse
  map to each renamed stylesheet and diff the result against the file at
  `HEAD`. An empty diff for all six files means the change is a bijective
  substitution of identifiers: every selector, property and resolved value
  is unchanged. This is what makes the Class A contrast gate moot here —
  contrast cannot have changed, because no value did.
- **No stale name:** zero occurrences of `--color-` or `--shadow-card` under
  `ui/`.
- **No dangling variable:** every `var(--x)` used in `ui/` resolves to a
  definition in `:root`.
- **One screenshot pair** — the same screen before and after, for the user
  to confirm by eye rather than by diff.

## Feedback on the playbook

Recorded because a first exercise is meant to produce some.

1. **A pure rename does not fit Class A or B cleanly.** Its blast radius is
   Class A (every screen) but its risk is neither: no value changes, so the
   contrast gate is not skipped, it is inapplicable. The playbook should
   name the round-trip diff as the gate that replaces it.
2. **Step 3's drift check has a real cost.** `DesignSync list_files` returns
   no hash and no size, so the only way to compare SHA-1 is `get_file`,
   which pulls the whole 173 KB prototype into the session. That is worth it
   for a change that follows the design and wasteful for one that cannot
   depend on it. The step should say when to run it.
