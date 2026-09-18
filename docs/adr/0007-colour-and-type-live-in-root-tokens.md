---
status: accepted
---

# Colour and type live only in `:root` tokens

Every colour the UI renders is set in one place: the custom properties in the
`:root` block of `ui/src/styles.css`. That includes shadows and the drawer
scrim. Every font family is set there too. Stylesheets and components use
`var(--…)` and never a literal. The token names are the Claude Design
handoff's own names (`--bg`, `--surface`, `--accent`, …), so a re-export maps
onto the code without translation.

We decided this because the rule has already carried three changes. The
2026-09-10 rebrand reskinned every screen by editing one block. The
2026-09-17 rename (from the ocean names to the handoff names) could be proved
pixel-neutral with a round-trip diff. And the 2026-09-18 control-edge fix
brought nine failing controls to 3:1 by adding one token. Each of those would
have been a hunt through a hundred files if colour lived anywhere else.
`npm run lint:tokens` enforces the rule in CI, and `npm run contrast`, also
in CI, can only compute every ratio because every colour is a token it can
read.

**Spacing and radius are deliberately outside this rule.** `--space-1..3` and
`--radius-1` exist and are preferred, but about 130 spacing literals and 50
radius literals remain, and they are allowed. They carry no theme, so a
rebrand never needs them to move together. Tokenising them would change
nearly every stylesheet, which is a Class B change with its own plan under
`docs/design/ux-ui-change-playbook.md`. It is not a cleanup to make in
passing.

## Consequences

- The repository may define a token the design lacks when the design falls
  short of a requirement: `--control-edge` exists because the handoff's
  strongest edge is 1.79:1. A handoff re-export must keep such tokens. The
  handoff document lists them as repository additions.
- A light theme is a second value block for the same names, not a second set
  of names.
- A colour literal outside `:root` fails the build, so a one-off colour means
  adding a token first.
