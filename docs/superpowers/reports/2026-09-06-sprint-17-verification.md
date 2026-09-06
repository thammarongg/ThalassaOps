# Sprint 17 — AI Provider Gateway: verification

Branch `thammarongg/sprint17-ai-provider-gateway`, verified at `37c3bf3`
on 2026-09-06.

## Gates

All seven, run by the coordinator against the branch after each task rather
than taken from the workers' reports:

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --all-targets --all-features -- -D warnings` | clean |
| `cargo test` | 646 passed, 0 failed |
| `npm run format:check` | clean |
| `npm run lint` | clean |
| `npm run typecheck` | clean |
| `npm test` | 234 passed, 0 failed |

Baseline at the start of Sprint 17 was 577 Rust and 216 frontend.

## What shipped

Tasks 1-16 of `docs/superpowers/plans/2026-09-05-sprint-17-ai-provider-gateway.md`.
Tasks 1-13 built the gateway itself: provider-neutral request and response
contracts, the `thalassa-ai` crate with its registry and adapter trait, budget
accounting, the gateway's policy/deadline/cancellation/failover path, migration
0007 and the request store, three adapters (OpenAI-compatible, Anthropic,
local), provider configuration and the fallback order, capability-scoped IPC,
TypeScript contracts and guards, the provider surface, and acceptance.

Tasks 14-16 were added on 2026-09-06, after the user settled the two open
decisions Task 13 left recorded in design section 15:

- **Task 14** (`0b109b0`, `e380b7f`) mounts what was tested and unreachable.
  `AiProviderPanel` renders inside the `integrations` area beside the connector
  list — design section 12 places it in "the existing connector/model status
  area", so no `Area` member was added — and the `incidents` entry, which had
  rendered `EmptyState` since Sprint 16, now routes to `IncidentWorkspace`.
- **Task 16** (`2a07dfe`) adds `ai.provider_order`, a `ConnectorRead` read that
  mirrors `ai.providers`. See "What the review caught" below.
- **Task 15** (`a488cc2`) wires the audit store, with the four contract changes
  the user settled.

## The exit criterion

`src-tauri/tests/ai_acceptance.rs` asserts it directly rather than claiming it:
one `ModelRequest` payload, only `model` replaced, answered by a hosted adapter
and by a local one, with the two `ModelResponse` values equal once
`provider_id`, `model_id`, `usage` and `attempts` are stripped. The restricted
split is asserted alongside it — `ImmutableRestrictedData` for the hosted
destination with the adapter never constructed, and an answer from the local one
under a policy document that permits `Restricted` there.

## What the review caught

Reviewing Task 14's mount against the Rust — not any failing test — found that
the fallback order could be written but never read. `ai.providers` returns
`ProviderSummary`, which carries no ordering; `ProviderConfigStore` keeps the
order in a separate `provider_order: Vec<String>`; only `ai.set_provider_order`
touched it. So `AiProviderPanel` took the order as a prop and the shell had
nothing to seed it with but `[]`.

Two consequences. The panel claimed failover was off whatever the gateway was
configured to do, while `build_registry` used the stored order for real. And
`AiFallbackOrder` derives the next order from the one it is handed: with
`[openai, ollama]` stored and `[]` displayed, one "add ollama" click wrote
`[ollama]`, dropping openai from the control that decides which providers may
receive a request.

Nothing could see it. The panel's tests, the shell test and Task 13's acceptance
test all start from an empty order and mock `ai_set_provider_order` to echo the
payload, so the write direction was asserted and the read direction did not
exist. Task 16's regression test is the one that would have caught it: with
`["openai", "ollama"]` configured, adding a third provider must send
`["openai", "ollama", "vllm"]`.

## What Task 15 changed beyond the four decisions

The gateway's deadline and cancellation early returns were moved to *after* the
attempt is pushed. Previously a provider call that ran past the deadline
returned `DeadlineExceeded` while discarding the attempt that had already burned
tokens; now it is recorded. That is design section 9's "a failed attempt that
still consumed tokens is not lost", which the `Option<ModelUsage>` field alone
would not have delivered.

No zero usage is written anywhere. A failed attempt records `usage: None`,
meaning not observed — the distinction the Sprint 16 Task 12 defect erased.

## Known limitations carried forward

Design section 15 holds thirteen. Items 11-13 were added this sprint:

11. The fallback order had no read counterpart — fixed by Task 16.
12. `WindowBudget` carries no period, and `window_usage` sums every attempt row
    the principal has produced, so the "rolling period" section 7 describes does
    not roll. Adding one changes Task 3's contract.
13. Nothing in production configures a window budget. `ai_window_budget`
    defaults to all-`None` and only the test builder writes it, so the
    accounting Task 15 made correct has no limit to enforce yet. A configuration
    surface belongs with the spend permission in Sprint 20's Policy Center.

## Process note

Every task went to a codex worker; the coordinator ran the gates itself and
reviewed each diff against the source rather than against the task's own tests.
The independent reviewer the routing policy names — codex on a second model —
was not dispatched this sprint; the user waived it on 2026-09-06 given the
coordinator's review. That review read every commit's diff against the source
and verified each settled decision individually; it was not a line-by-line audit
of all 549 changed lines in Task 15.
