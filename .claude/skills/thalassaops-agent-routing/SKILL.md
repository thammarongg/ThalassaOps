---
name: thalassaops-agent-routing
description: Use ONLY in the thalassaops repo when dispatching Sprint work through Orca orchestration (worker-start, dispatch, task assignment). Picks which agent implements, reviews, and approves, and gives the exact launch commands. Triggers on "dispatch a task", "start a worker", "who implements", sprint execution, or any worker-start decision for thalassaops. Projects other than thammarongg's thalassaops repo must NOT use this routing.
---

# ThalassaOps Agent Routing

Scope guard: this policy applies **only to the thalassaops repository** (all of its Orca worktrees). If the current project is anything else — e.g. blueocean-vector, jira-mcp — ignore this skill entirely and route per that project's own instructions.

Fixed policy for Orca orchestration dispatch in thalassaops. Do not re-decide per task.

## Roles

| Role | Agent | Launch |
|---|---|---|
| Implementer A (default) | codex `gpt-5.6-luna` effort `max` (already the default in ~/.codex/config.toml) | `orca orchestration worker-start --task <id> --worktree <wt> --agent codex --model gpt-5.6-luna --effort max --json` |
| Implementer B (parallel stream) | opencode on the Z.AI Coding Plan (GLM-5.3 max — its configured default) | `orca orchestration worker-start --task <id> --worktree <wt> --agent opencode --json` |
| Reviewer / QA | codex on a DIFFERENT model at `max` for independence | `... --agent codex --model <review-model> --effort max --json` |
| Final approver | claude | never implements, never reviews code; approves/rejects at the gate and coordinates |

Rules:
- `--model`/`--effort` work only for claude/codex/cursor agents. For opencode, omit them — its default model is already the Z.AI GLM-5.3 plan; passing model ids for opencode is not supported.
- Review model: verify once per sprint with `codex models` in a real terminal before first review dispatch (was `gpt-5.6-terra`; `gpt-5.6-sol` was seen 2026-09-02). Implementer and reviewer must not share a model.
- Two implementers must not share one worktree (shared git index). Give the second stream a child worktree off the sprint branch; state the file boundary in each task spec.
- OMP is not a default implementer. If the user explicitly routes work to OMP: the coordinator runs all gates itself and sends the worker exact failing items (Sprint 12: $12.68/task, no convergence, boundary violations without this), and require a status diffstat + blocking ask before its final commit.

## Launch health (this machine)

- 2026-09-03: `worker-start` for opencode/claude/omp all launch clean (~3s, `input_accepted`). Kiro CLI was uninstalled the same day (root cause of the earlier `agent_prompt_stalled`/black-hole terminals); rc files are clean and a post-uninstall opencode probe passed. If that error ever returns, suspect a new shell integration re-exec'ing zsh inside Orca shells (`ps` check: agent terminals must show a plain `zsh --login` -> agent child, no process rename).
- Manual fallback if launch is broken: `orca terminal create --worktree <wt> --command "opencode" --json` → `terminal wait --for tui-idle` → `terminal switch` → `dispatch --task <id> --to <handle> --inject` → if pasted-but-unsubmitted, `terminal send --enter`.

## Token/usage limits

- Detect: worker `escalation`/`worker_done failed` citing quota, or provider auth errors. opencode footer shows context % + $ spent; Orca tracks per-agent usage (`orca-*-usage.json` in its userData).
- Fail over at dispatch level: the Task stays valid — `worker-start --task <same> --agent <other-implementer>` (codex <-> opencode). Keep implementer and reviewer on different models/accounts so one limit never blocks both roles.
- Extra quota: Orca manages multiple accounts — `orca account add --agent codex|claude`, `orca account list`, switch when one seat is exhausted (codex also has a reset-credit action). For opencode/Z.AI, add a fallback provider in its config or wait out the plan's reset window; queue tasks as `pending` meanwhile.
- Burn reduction: drop `--effort` below max for routine tasks; coordinator runs gates itself and sends exact failing items (Sprint 12 lesson); watch worker context % and tell a low-context worker to stop investigating and write its report; prefer fresh worker sessions per task over long continuations.

## Per-dispatch checklist

1. Task spec names: plan path, task number, file boundary, gates to run, commit message shape.
2. After `worker_done`: verify the commit/gates, then `orca orchestration worker-release --dispatch <id>`, then `check --ack <deliveryId>`. Full lifecycle rules live in the `orchestration` skill — follow it.
3. Review checkpoint: workers send a diffstat + blocking ask before final commit on multi-file tasks; review findings go back as a follow-up task to the same implementer.
