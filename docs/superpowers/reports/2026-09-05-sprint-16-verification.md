# Sprint 16 — Incident Workspace: verification

Branch `thammarongg/sprint16-incident-workspace`, verified at `ff809e8`
on 2026-09-05.

## Gates

All seven, run against the branch:

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --all-targets --all-features -- -D warnings` | clean |
| `cargo test --workspace` | 577 passed, 0 failed |
| `npm run format:check` | clean |
| `npm run lint` | clean |
| `npm run typecheck` | clean |
| `npm test` | 216 passed, 0 failed |

The Rust surface changed only for the comment event kind, the sequence
allocation and `incident.add_comment`, as design section 2.10 requires.

## What shipped

Tasks 1-14 of `docs/superpowers/plans/2026-09-02-sprint-16-incident-workspace.md`:
transaction-scoped sequence allocation, the `commented` event kind and
`incident.add_comment` through the service and IPC, TypeScript contracts and
guards, a locale parity test, the incident data hooks, the workspace shell and
queue, the deterministic narrative, evidence resolution and panel, the
association tab registry, the comment thread, the action controls with
version-conflict recovery, the summary card with its copy allowlist, and the
end-to-end acceptance test.

## Defects this sprint's reviews caught

Every one of these passed its own tests before it was found. They are recorded
because the shape repeats.

1. **Task 10's registry could not be written against the contract.** The draft
   selected the vulnerability tab from `incident.triggers`, which does not
   exist — `incident_get` returns `Incident`, carrying `trigger_ids` only, and
   no command returns trigger records. Its alerts tab fed signal UUIDs to an
   evidence resolver, which would have been permanently `unavailable` at
   runtime under a green mocked test, and two tabs selected the same ids.
   Settled by partitioning the resolved `EvidenceRef[]` by `source_kind`.
2. **Two association tabs are empty by construction.** The correlation snapshot
   normalizes seven source kinds and the fixture catalog ships no
   `github` / `gitlab` / `argo_cd` fixtures, so no incident evidence id can
   carry a topology or change source kind. Recorded under Task 10 and Task 14
   so no later test asserts content there against a mock.
3. **`incident_version_conflict` is a reason, not a code.** Task 12's draft read
   it from `error.code`; every incident rejection is `INVALID_REQUEST` with the
   reason in `details`. The draft test and a matching implementation would have
   agreed with each other and disagreed with the running app, and conflict
   recovery would never have fired.
4. **The first Task 12 implementation invented the whole transition context.**
   `note`, `action_description`, `resolution_summary`, `closure_notes` and
   `reason` were all set to `incident.summary`; `duplicate_checked` was
   hardcoded `true`; `verification_seconds` was a constant; `impact_ended_at`
   came from `updated_at`; `follow_up_ids` was the incident's own id; and the
   principal fields fell back to `owning_team_id`. All of it satisfied the
   domain validators — which reject `duplicate_checked: false` and an empty
   `follow_up_ids` — while writing false statements into an immutable,
   actor-attributed audit timeline. Fixed in `f0c8a88`: each context is
   collected from the responder and the principal comes from the role
   assignment.
5. **Task 11 counted a comment body in UTF-16 code units** against a domain
   bound expressed in Unicode scalar values, and appended optimistically with no
   rollback for the control-character and sensitive-marker rejections that a
   plausible responder comment ("rotated the API token") triggers.
6. **Tasks 13 and 14 built on things that were not there** — a fixture never
   added, and a command name read off the wrong argument of `invoke`.

The common thread is that a mocked `invoke` cannot see a wrong command name, a
wrong error shape, or a fabricated payload. Every claim in these reviews was
checked against the Rust source rather than against the mock.

## Follow-ups

- `IncidentTabs` re-filters each tab's evidence against the incident's current
  `evidence_ids` on every render, duplicating what `statesForEvidence` already
  computed. It is the guard design 5.3 asks for while the shell re-resolves, and
  is left as is.
- `CorrelationEvidencePanel`, `TopologyEvidencePanel` and
  `IncidentEvidencePanel` are three near-identical components differing only in
  CSS prefix and locale namespace. Extracting a shared entry would touch two
  shipped workspaces and was deliberately left out of this sprint.
- The changes tab may belong on Sprint 14's `change_evidence` command rather
  than the incident's evidence ids. That is a Sprint 17 design question.
- Design section 13.2 records the incident summary as an accepted clipboard
  leak; Sprint 18 must route summaries through redaction, not only evidence.
