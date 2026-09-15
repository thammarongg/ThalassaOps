# Direction review — 2026-09-10

**Status:** Decisions recorded, not yet reflected in `sprint-plan.md`
**Updated:** 2026-09-15 — the visual direction below was superseded; see that section
**Context:** External review of the project against the owner's stated constraint —
something interview-ready in roughly two months, full-time, with a revenue path left
open, and no warm audience to validate against.

This document exists so the reasoning survives the move to Claude Code. It records
decisions and their grounds; it does not replace `sprint-plan.md`, which still describes
the original 28-sprint plan.

## Verdict

Continue the project. Change the plan.

Measured on `main` at 2b645ca: 64,790 lines of Rust, 26,194 of TypeScript, 646 Rust test
functions, 227 frontend test cases across 35 files, 46 design documents, real SDKs wired
in (`kube-rs`, `k8s-openapi`, `aws-sigv4`, `azure_identity`, `gcp_auth`), no `todo!()` or
`unimplemented!()` anywhere in the crates. This is not a prototype and should not be
restarted.

The problem is the plan, not the code.

## The three findings

### 1. The AI is the product, and the AI is not built

Sprint 17 delivered the provider gateway — routing, fallback order, budgets, audit store,
three adapters. That is plumbing. Sprint 19 holds the actual claim: Kubernetes analyzers,
evidence retrieval, hypotheses with confidence, citations, missing-context warnings. It is
unwritten.

Today the repository is a well-built read-only infrastructure console with an unused model
gateway, published as an "AI-Powered DevOps Operations Platform."

Note against the plan's own rule 10 — *prefer small vertical slices over completing an
entire technical layer before testing it*. The gateway was built before anything called
it. Sprint 19 is the slice that makes Sprint 17 mean something.

### 2. The project is invisible

231 commits, 0 stars. The README has no screenshot, no demo and no download — only
build-from-source instructions. For a desktop application that is close to invisible.

There is no distribution step anywhere in the 28 sprints. Launch appears once, at Sprint
28. A project that only becomes visible at the end cannot learn whether anyone wants it.

### 3. The calendar does not fit

The plan's own staffing note says 12–16 months solo. Eleven sprints remain at two weeks
each — 22 weeks. The window is 8.

The failure mode to avoid is not a bad idea. It is that "production-ready macOS release"
was set as the bar for first contact with the world, and that bar does not survive a job
search.

## The change: v0.1 is Sprint 19

The plan already names the milestone — Sprint 19, "Read-only AI Beta." Treat that as v0.1
and ship it publicly.

| Sprint | Decision | Reasoning |
| --- | --- | --- |
| 18 · Context optimization and redaction | **Keep** | The README promises secrets never reach hosted providers. That promise is the differentiator and must be true before anyone else runs this. |
| 19 · Read-only AI investigation | **Keep** | This is v0.1. |
| 20 · Policy Center | Defer | A read-only beta needs a policy file, not an administration surface. |
| 21 · Approval and action framework | Defer | Nothing mutates in v0.1. |
| 22 · Terminal and runbook workflows | Cut | Large surface, no bearing on whether the investigation is any good. |
| 23 · Jira, Slack, Discord, PagerDuty | Cut | Four integrations, weeks of work, demos nothing the evidence graph does not already show. |
| 24 · Multi-organization team access | Cut | Contradicts the local-first single-operator positioning that makes the product distinctive. |
| 25 · Security posture and hardening | Partial | Take the secret-leak tests only. They defend the one claim that cannot be wrong. |
| 26 · Performance, resilience, scale | Defer | Optimising for scale that does not exist. One operator, one cluster. |
| 27 · macOS packaging and UX polish | Pull forward | Thin slice: a DMG on GitHub Releases plus screenshots. Skip notarization — it needs a paid Apple account, and right-click-open works for early users. |
| 28 · Release candidate and launch | Pull forward | Take the launch, leave the release-candidate ceremony. |
| **New · Make it visible** | **Add** | Screenshots, a 90-second demo, a rewritten README opening, a launch post. Absent from the plan and the most needed item in it. |

### Indicative sequence

- **Weeks 1–2** — Sprint 18, with the Sprint 25 secret-leak tests pulled in alongside.
  Prove a hosted provider request cannot carry a token, private key or regulated data.
- **Weeks 3–5** — Sprint 19. Five Kubernetes analyzers, evidence retrieval, structured
  findings with citations. Fixtures first, then one real failure on the local k3s cluster.
- **Week 6** — Screenshots, demo recording, README rewrite, DMG.
- **Week 7** — Launch: r/kubernetes, r/devops, Hacker News, CNCF Slack, and the Rust and
  Tauri communities. The Rust angle draws attention the AIOps angle will not. Lead with
  local-first and no egress, not with "AI-powered."
- **Week 8** — Answer everyone. Whoever files an issue that week decides which of sprints
  20–28 actually matters.

## Visual system pass — reviewed, continue

> **Superseded 2026-09-15.** The owner chose the AIOps Command Center design from Claude
> Design instead of this pass; see [`aiops-command-center.md`](../design/aiops-command-center.md).
> The pilot is archived as the tag `archive/visual-system-pass-pilot`, and its preview
> harness was kept on `main`. Two points below still hold: the Sprint 19 investigation
> view needs a design, and the stale-source case belongs in it. The pilot's reading of the
> Evidence Tide Line — per-source freshness marks under the console header — is not
> carried forward; the adopted design places the Tide Line on the incident timeline.

`docs/design/2026-09-07-visual-system-pass.md` was reviewed against the working tree.

It is justified rather than displacement activity: the problem was measured (15 font
sizes, 3 spacing steps, 21 duplicated button rules, 0 elevation levels, 1 surface
treatment), the defect list was captured from the running application, contrast was
computed rather than eyeballed, and the observation that `vitest-axe` under jsdom skips
the colour-contrast rule entirely is correct and unusually honest.

One check worth recording: `operations-console.acceptance.test.tsx` lost 294 lines in the
uncommitted pass, which looks like tests removed to force a green run. It is not. Test
case counts are unchanged (1 and 19 respectively); the fixture was extracted to
`ui/src/dev/operations-fixture.ts` and shared with a new `preview.tsx` harness. The
harness is also the fastest route to screenshots.

Two changes to the rollout:

1. **Cut stage 2 from seven workspaces to three.** Do Operations Console (done), Topology
   (defect 6 is fatal — a topology view that renders no topology cannot be screenshotted)
   and the Incident Workspace, because that is where the Sprint 19 investigation lands.
   Correlation, environments, observability, changes and AI providers are secondary
   screens that no screenshot includes. After launch.

2. **Design the Sprint 19 investigation view now**, during this pass, not after it. It has
   no design and no markup. It is also the same design problem as the Evidence Tide Line —
   both show how much the console trusts what it is telling you, one per source, one per
   finding. Solving them together produces the hero screenshot and the launch image.

Tooling split: keep the token layer in code — it is already ~1,186 lines into
`styles.css` and moving it to a canvas loses work. Use a design canvas for the two screens
that do not exist yet and for README and launch imagery.

## Open items

- `ux-ui-concept.md` (2026-08-24) is partially superseded by the visual system pass. The
  supersession is handled inside that document, but `sprint-plan.md` and the requirements
  documents still describe the old visual direction. One reconciliation pass.
  **Resolved 2026-09-15:** `ux-ui-concept.md` was removed and the AIOps Command Center
  spec is the design of record.
- **No document defines v0.1.** "Done" is currently Sprint 28. One page naming what ships,
  what is explicitly excluded and what the demo shows would settle most scope questions.
  This is the missing artifact. **Resolved:** [`v0.1-definition.md`](./v0.1-definition.md).
- The visual system pass is a workstream that appears nowhere in the 28 sprints.
  **Superseded 2026-09-15.**
- No `CLAUDE.md` at the repository root.
- As of 2026-09-10 the pilot is uncommitted: ten modified files on `main` including a
  ~1,186-line stylesheet change, plus untracked `ui/src/dev/`, `ui/dev.html`,
  `.agents/`, `skills-lock.json` and the visual system pass document.
  **Resolved 2026-09-15:** the pilot is archived as a tag and its harness is on `main`.

## Positioning note

Every competitor found in review — HolmesGPT, K8sGPT, Robusta, Keep, and the commercial
AI SRE products — is a CLI, an agent or a SaaS backend. A local-first desktop console
where operational data never leaves the machine is a position none of them occupy. State
it as a design choice rather than a market claim; absence of a competitor in a review is
not proof that none exists.
