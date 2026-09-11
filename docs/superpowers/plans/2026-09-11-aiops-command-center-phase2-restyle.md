# AIOps Command Center — Phase 2 (Incident Workspace restyle) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring `IncidentWorkspace` and its six child components onto the new
Claude Design visual token system (colors only — no markup, behavior, or
contract changes), fix a pre-existing undefined-CSS-variable bug found along
the way, and land the three documentation corrections the governance
reconciliation design left outstanding.

**Architecture:** Two sequential CSS-only changes in the app frontend
(`ui/src/styles.css` for new root tokens, `ui/src/incident/incident.css` for
applying them), followed by three doc-only edits. No React component logic
changes. No IPC/Rust changes.

**Tech Stack:** React + TypeScript + Vite frontend (`ui/`), Vitest + Testing
Library for existing component tests, CSS custom properties (no
preprocessor).

**Spec:** `docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md`
(commits `9f46ed5`, `14ed4a3`, `6684777` on `claude/review-requirements-spec-thai-89f240`
— Task 0 below brings these onto this branch). Sections referenced below
(`Phase 2`, `Token gap` table, `Backlog`, `Follow-up documentation
corrections`) are in that file.

## Global Constraints

- This plan executes on `claude/aiops-command-center-4e1759`, **not** the
  branch the spec was written on. Task 0 is a prerequisite for every other
  task.
- Every existing test file touched by a restyle task must keep passing
  **unmodified** — no test file in this plan is edited. This mirrors the
  rule `docs/superpowers/plans/2026-09-10-aiops-command-center-shell-phase1.md`
  used for `shell.test.tsx`.
- No new CSS custom property may be introduced without a value sourced from
  the spec's "Token gap" table — no invented hex values.
- Sections 2, 3, 4 and the Evidence Tide Line (Section 5) from the spec are
  explicitly **not** part of this plan (see spec's "Backlog" section) — do
  not add UI for them.

---

### Task 0: Bring the design doc onto this branch

**Files:**
- None modified — this is a git operation.

**Interfaces:** N/A.

- [ ] **Step 1: Confirm the target worktree and current HEAD**

Run (from `~/Projects/Repo/thalassaops/.claude/worktrees/aiops-command-center-4e1759`):

```bash
git status --short
git log --oneline -1
```

Expected: working tree shows only the pre-existing uncommitted files listed
in the spec's "Execution note" context (`ux-ui-concept.md`,
`package-lock.json`, `package.json`, locale files, `shell.tsx`,
`shell.test.tsx`, `styles.css`, plus the untracked `aiops-command-center.md`
and three plan docs) — if anything else is uncommitted, stop and ask before
proceeding, do not overwrite unrelated work. HEAD is `2b645ca`.

- [ ] **Step 2: Cherry-pick the three spec commits**

```bash
git cherry-pick 9f46ed5 14ed4a3 6684777
```

Expected: three new commits apply cleanly (each only touches
`docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md`,
a path untouched by anything already on this branch). If a conflict occurs,
stop — do not force-resolve by discarding either side; the uncommitted files
already on this branch and the cherry-picked commits should not overlap.

- [ ] **Step 3: Verify the file landed**

```bash
git show HEAD --stat
ls docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md
```

Expected: file exists, `git log --oneline -3` shows the three cherry-picked
commits on top of `2b645ca`.

No commit step here — Step 2 already created the commits.

---

### Task 1: Add the missing design tokens to `styles.css`

**Files:**
- Modify: `ui/src/styles.css:13-35` (the `:root` block)
- Test: none new — verified via Task 2's regression check, since these
  tokens have no consumer until Task 1 lands them and Task 2 uses them.

**Interfaces:**
- Produces: CSS custom properties `--color-fog`, `--color-mist`,
  `--color-deep-water-2`, `--color-deep-water-3`, `--color-border`,
  `--color-border-strong`, `--color-violet`, `--color-violet-bg`,
  `--shadow-card`, `--color-nav-fg`, `--color-nav-sub` — consumed by Task 2.

- [ ] **Step 1: Confirm the baseline — `--color-fog` is currently undefined**

```bash
grep -n -- "--color-fog" ui/src/styles.css
```

Expected: no output (it is used in `incident.css` but never defined in
`:root` — this is the pre-existing bug the spec's "Token gap" table
documents).

- [ ] **Step 2: Add the tokens**

Edit the `:root` block in `ui/src/styles.css` — insert the new declarations
after the existing `--color-info-bg` line and before `--font-ui`:

```css
:root {
  /* AIOps Command Center visual system — see docs/design/ux-ui-concept.md */
  --color-abyss: #0c1116;
  --color-nav: #000716;
  --color-deep-water: #161d26;
  --color-sea-glass: #f2f3f3;
  --color-reef-cyan: #ff9900;
  --color-info: #539fe5;
  --color-kelp: #5dd47a;
  --color-amber: #f0b429;
  --color-coral: #ff7c70;
  --color-kelp-bg: #12241a;
  --color-amber-bg: #2a2113;
  --color-coral-bg: #2a1614;
  --color-info-bg: #12212f;
  /* Added in Phase 2 (docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md,
     "Token gap" table) — fills the gap between the 13 tokens Phase 1 ported
     and the ~25 the Claude Design handoff defines. --color-fog was already
     referenced 12 times in incident.css with no definition; this is that
     fix, not new usage. */
  --color-fog: #9aa7b4;
  --color-mist: #7a8794;
  --color-deep-water-2: #1b232e;
  --color-deep-water-3: #212b38;
  --color-border: #2a3542;
  --color-border-strong: #3a4757;
  --color-violet: #b18cf0;
  --color-violet-bg: #1f1830;
  --shadow-card: 0 1px 4px rgba(0, 0, 0, 0.45);
  --color-nav-fg: #f7f8f8;
  --color-nav-sub: #96a3b0;
  --font-ui: "IBM Plex Sans", "IBM Plex Sans Thai", ui-sans-serif, system-ui, sans-serif;
  --font-mono: "IBM Plex Mono", ui-monospace, monospace;
  --space-1: 0.5rem;
  --space-2: 1rem;
  --space-3: 1.5rem;
  color: var(--color-sea-glass);
  background: var(--color-abyss);
  font-family: var(--font-ui);
}
```

- [ ] **Step 3: Run the frontend checks**

```bash
npm run format:check
npm run lint
npm run typecheck
```

Expected: all pass. These are pure CSS additions — no TypeScript or markup
changed, so this step is a sanity check, not expected to catch anything.

- [ ] **Step 4: Commit**

```bash
git add ui/src/styles.css
git commit -m "$(cat <<'EOF'
fix(ui): define --color-fog and add the remaining Phase 2 design tokens

--color-fog was referenced 12 times in incident.css to de-emphasize
secondary text (queue meta, field labels, evidence ids, comment meta,
action status) but was never defined in :root, so it silently resolved
to nothing and that text rendered at full brightness instead of muted.
Also adds the remaining tokens the Claude Design handoff defines that
Phase 1's rebrand didn't port (surface2/3, border/border2, fg3, ai/aibg,
shadow, nav-fg/nav-sub), per the "Token gap" table in
docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Apply the new tokens in `incident.css`

**Files:**
- Modify: `ui/src/incident/incident.css`
- Test: `ui/src/incident/IncidentWorkspace.test.tsx`,
  `ui/src/incident/IncidentList.test.tsx`,
  `ui/src/incident/IncidentSummaryCard.test.tsx`,
  `ui/src/incident/IncidentTabs.test.tsx`,
  `ui/src/incident/IncidentEvidencePanel.test.tsx`,
  `ui/src/incident/IncidentCommentThread.test.tsx`,
  `ui/src/incident/IncidentActions.test.tsx` — run unmodified, as the
  regression gate.

**Interfaces:**
- Consumes: the ten tokens Task 1 added, plus the existing
  `--color-sea-glass`/`--color-coral` (unchanged).
- Produces: no new selectors or class names — same DOM, same classes, new
  color values only.

- [ ] **Step 1: Run the seven incident test files to confirm the baseline passes**

```bash
npm test -- ui/src/incident/IncidentWorkspace.test.tsx ui/src/incident/IncidentList.test.tsx ui/src/incident/IncidentSummaryCard.test.tsx ui/src/incident/IncidentTabs.test.tsx ui/src/incident/IncidentEvidencePanel.test.tsx ui/src/incident/IncidentCommentThread.test.tsx ui/src/incident/IncidentActions.test.tsx
```

Expected: all PASS (this is the pre-edit baseline — record it so Step 4's
result is a comparison, not a first run).

- [ ] **Step 2: Replace the ad-hoc border color-mix with `--color-border`**

Every occurrence of `color-mix(in srgb, var(--color-sea-glass) 20%, transparent)`
in `ui/src/incident/incident.css` is a border color computed from the
foreground color at 20% opacity — the same value the new `--color-border`
token now names directly. Replace all 11 occurrences
(`.incident-workspace__header`, `.incident-queue__row`,
`.incident-summary-card`, `.incident-evidence__entry`,
`.incident-comments__entry`, `.incident-actions` border-top,
`.incident-actions__principal`, `.incident-actions__form`) — for example:

```css
.incident-workspace__header {
  border-bottom: 1px solid var(--color-border);
  padding-bottom: var(--space-1);
}
```

Do the same substitution (only the color value changes, `border`/
`border-bottom`/`border-top` and width/style stay as they are) for the other
ten selectors listed above. Do not touch
`.incident-actions__conflict`'s `color-mix(in srgb, var(--color-coral) 45%, transparent)`
— that is a semantic error-state border, not a neutral one, and is unrelated
to this token.

- [ ] **Step 3: Give card-like surfaces a distinct background**

The handoff's cards sit on `--surface` while the page sits on `--bg` — two
different values. Today every card in `incident.css` has a border but no
background, so it's visually flush with the page. Add
`background: var(--color-deep-water-2);` to the four card selectors:
`.incident-summary-card`, `.incident-evidence__entry`,
`.incident-comments__entry`, `.incident-actions__form`, and
`.incident-actions__principal`. Example:

```css
.incident-summary-card {
  background: var(--color-deep-water-2);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-1, 4px);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  margin-top: var(--space-2);
  padding: var(--space-1);
}
```

- [ ] **Step 4: Give the selected queue row a token-backed background**

Replace `.incident-queue__row--selected`'s computed background with the
dedicated surface token:

```css
.incident-queue__row--selected {
  border-color: var(--color-sea-glass);
  background: var(--color-deep-water-3);
}
```

- [ ] **Step 5: Re-run the same seven test files**

```bash
npm test -- ui/src/incident/IncidentWorkspace.test.tsx ui/src/incident/IncidentList.test.tsx ui/src/incident/IncidentSummaryCard.test.tsx ui/src/incident/IncidentTabs.test.tsx ui/src/incident/IncidentEvidencePanel.test.tsx ui/src/incident/IncidentCommentThread.test.tsx ui/src/incident/IncidentActions.test.tsx
```

Expected: PASS, identical result to Step 1 — this is a color-only change,
so any new failure means a selector was mistyped, not that a test needs
updating.

- [ ] **Step 6: Run the full frontend gate**

```bash
npm run format:check
npm run lint
npm run typecheck
npm test
```

Expected: all pass (full suite, not just the incident files, to catch any
unrelated regression).

- [ ] **Step 7: Visual verification**

Start the Vite dev server and take a Browser pane screenshot of the Incident
Workspace (list + a selected incident's detail pane), per
`[[screenshotting-thalassaops-ui]]` (outside Tauri, `invoke` rejects — drive
layout/visual state, not live data; use the incident fixtures already wired
into the dev harness). Confirm: card backgrounds are visually distinct from
the page background, borders read as a defined line rather than a faint
tint, and secondary text (queue meta, field labels, evidence ids) is visibly
muted relative to primary text — the `--color-fog` fix should be visible
here for the first time.

- [ ] **Step 8: Commit**

```bash
git add ui/src/incident/incident.css
git commit -m "$(cat <<'EOF'
style(incident): restyle Incident Workspace onto the Phase 2 token set

Color-only change: ad-hoc color-mix() border tints become the dedicated
--color-border token, cards get a --color-deep-water-2 background so
they read as distinct surfaces instead of flush with the page, and the
selected queue row uses --color-deep-water-3. No markup, class name, or
behavior change — all seven incident test files pass unmodified.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: Correct the `ux-ui-concept.md` supersede banner

**Files:**
- Modify: `docs/design/ux-ui-concept.md` (the "Superseded 2026-09-10" banner
  and the "Typography" section's own "Superseded 2026-09-10" note — grep for
  both before editing, there are two occurrences per the spec's earlier
  reading of this file).

**Interfaces:** N/A (documentation only).

- [ ] **Step 1: Find both banner occurrences**

```bash
grep -n "Superseded 2026-09-10" docs/design/ux-ui-concept.md
```

Expected: two matches (a top-level banner near the file's start, and one
inside the "Typography" section, per the design doc's Provenance section
description of this file).

- [ ] **Step 2: Read the file to get exact current wording**

Use the Read tool on `docs/design/ux-ui-concept.md` around both matched line
numbers before editing — the exact banner text was not reproduced verbatim
in the spec, so do not guess it.

- [ ] **Step 3: Edit both banners**

At each location, replace the bare "Superseded 2026-09-10" wording with text
that distinguishes what actually changed. The corrected sense (write it in
this file's existing voice/tense, do not just paste this verbatim if it
clashes stylistically) is:

> Superseded 2026-09-10 for visual language only (palette, typography,
> spacing — see `docs/design/aiops-command-center.md`). This document's
> governance content — the severity/priority split, the Evidence Tide Line,
> and the interaction rules below — remains current; the "drop the
> governance rules" decision recorded in
> `docs/superpowers/plans/2026-09-10-aiops-command-center-spec-deprecation.md`
> was reversed on 2026-09-11, per
> `docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md`.

- [ ] **Step 4: Commit**

```bash
git add docs/design/ux-ui-concept.md
git commit -m "$(cat <<'EOF'
docs(design): scope the ux-ui-concept.md supersede banner to visual-only

The 2026-09-10 banner read as if the whole document were replaced. Only
its visual language was — the severity/priority split, Evidence Tide
Line, and interaction rules it documents are reinstated by the
governance reconciliation design and still apply.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Fold the five governance sections into `aiops-command-center.md`

**Files:**
- Modify: `docs/design/aiops-command-center.md` (its existing
  "Product-model changes from the old spec" and "Divergence from the current
  implementation" sections, per the file structure read while writing the
  design doc this plan implements).

**Interfaces:** N/A (documentation only).

- [ ] **Step 1: Re-read the current file**

```bash
grep -n "^#" docs/design/aiops-command-center.md
```

Expected: the same section list recorded in the design doc's Provenance
research (`Product-model changes from the old spec` at line 267,
`Divergence from the current implementation` at line 305, as of
2026-09-10 — confirm the line numbers still match before editing, since
Task 3 may have shifted nothing here but this file may have changed since).

- [ ] **Step 2: Amend "Product-model changes from the old spec"**

For each of the five bullets in that section (severity/priority split, AI
disclosure, terminal/action risk classification, Evidence Tide Line,
redaction-disclosure UI) that says the mockup "drops" or this spec "adopts
the mockup's lighter model" — append a one-line pointer, e.g. after the
severity/priority bullet:

> **Reversed 2026-09-11:** see
> `docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md`
> Section 1 — both fields are kept; Priority is already shown on the
> Operations Console dashboard row, and is a backend follow-up (not UI) for
> the Incident detail view.

Do the same for the AI-disclosure bullet (point to Section 2), the
terminal/risk-classification bullet (Section 3), the Evidence Tide Line
bullet (Section 5 — note it is deferred to Sprint 19, not dropped), and the
redaction-disclosure bullet (Section 4). Do not delete the original bullets
— per this project's convention (seen in the file's own "kept for history"
treatment of `ux-ui-concept.md`), append the correction rather than rewrite
the historical record.

- [ ] **Step 3: Commit**

```bash
git add docs/design/aiops-command-center.md
git commit -m "$(cat <<'EOF'
docs(design): point aiops-command-center.md at the reversed governance decision

The five "dropped" product-model bullets are now appended with a note
to the section of docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md
that reverses each one, so a reader of this file sees the current
answer without cross-referencing separately.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Append the reversal note to `spec-deprecation.md`

**Files:**
- Modify: `docs/superpowers/plans/2026-09-10-aiops-command-center-spec-deprecation.md`

**Interfaces:** N/A (documentation only).

- [ ] **Step 1: Append a dated note at the end of the file**

```markdown

## 2026-09-11 — decision reversed

The "drop the governance rules" call in this document's "Decision" section
(severity/priority split, AI evidence/confidence/budget disclosure,
terminal/action risk classification) was reversed on 2026-09-11: the user
reviewed the gap against `docs/requirements/requirements-summary.md` and
`docs/requirements/system-requirements.md` and decided to keep all five
rules rather than drop them. See
`docs/superpowers/specs/2026-09-11-aiops-command-center-governance-reconciliation-design.md`
for how each rule is expressed inside the new visual system. This document's
account of the 2026-09-10 decision is kept as-is above — it is what was
decided that day, not what stands today.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/2026-09-10-aiops-command-center-spec-deprecation.md
git commit -m "$(cat <<'EOF'
docs(plans): record that the 2026-09-10 governance-drop decision reversed

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>
EOF
)"
```

---

## Out of scope (tracked in the spec's Backlog section, not here)

- Sections 2, 3, 4 (AI response disclosure, action risk/execution-mode
  pills, redaction-disclosure line) — no component or backend exists yet;
  blocked on Sprint 18/19/22.
- Section 5 / Evidence Tide Line rail — blocked on the Sprint 19
  `IncidentNarrative` rewrite already noted in that file's own comment.
- Priority on the `Incident` domain entity (detail view) — backend/contract
  change, a future sprint's work.
- Kanban/board mode for the incident list — no existing concept, not
  requested.
