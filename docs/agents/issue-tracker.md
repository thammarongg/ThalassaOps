# Issue tracker: Jira

Issues and specs for this repo live in Jira: project **SCRUM** ("ThalassaOps") at
https://asdevwishes.atlassian.net, board 1 ("SCRUM board"). Use the `jira` MCP
tools (`mcp__jira__*`). GitHub Issues on the repo is **not** used; don't create
issues there.

The board is **team-managed** (`type: simple`). Several dedicated tools fail on
it, so some writes go through the raw `jira_api` tool. The table below says
which.

## Vocabulary

- **Issue types:** `Epic` (level 1), then `Story`, `Task`, `Bug`, `Feature` and
  `Request` (level 0), then `Subtask`. A spec is an `Epic`. A ticket made by
  `/to-tickets` is a `Task` under that epic. An incoming bug report is a `Bug`.
- **Statuses:** `To Do` → `In Progress` → `In Review` → `Done`. Every issue type
  uses the same four.
- **Blocking:** the `Blocks` link type (outward "blocks", inward "is blocked
  by").
- **Sprints:** a sprint name must be under 30 characters, or the API
  returns 400.

## Conventions

| Operation | How |
|---|---|
| Create an issue | `create_issue` (project `SCRUM`, `issuetype`, `summary`, `description`). For an epic, `create_epic`. |
| Put a ticket under an epic | `move_issue_to_epic`, or set `parent: {key: "SCRUM-N"}` at creation. |
| Read an issue | `get_issue` with the key, then `list_comments` for the thread. |
| List / search | `search_issues` with JQL, e.g. `project = SCRUM AND status != Done AND labels = needs-triage ORDER BY created`. |
| Comment | `add_comment`. If it returns 400 "Comment body is not valid!", fall back to `jira_api` `POST /rest/api/3/issue/<KEY>/comment` with an ADF body. |
| Apply / remove labels | `jira_api` `PUT /rest/api/3/issue/<KEY>` with `{"update": {"labels": [{"add": "…"}]}}` (or `"remove"`). |
| Change status | `get_issue_transitions` to find the id, then `transition_issue`. |
| Close | Comment first, then transition to `Done`. For won't-do, add the `wontfix` label and then transition to `Done`. |
| Assign | `jira_api` `PUT /rest/api/3/issue/<KEY>/assignee` with `{"accountId": …}`. `assign_issue` returns 405 on this board. |
| Blocking edge | `jira_api` `POST /rest/api/3/issueLink` with `{"type": {"name": "Blocks"}, "outwardIssue": {"key": "<blocker>"}, "inwardIssue": {"key": "<blocked>"}}`. |
| Sprint create / update / close | `jira_api` only: `POST /rest/agile/1.0/sprint` with `originBoardId: 1` to create, and `POST /rest/agile/1.0/sprint/<id>` with the **full** body (always `name` and `goal`) to change state. `create_sprint`, `update_sprint` and `close_sprint` fail or wipe fields here. |
| Reads for sprint state | `get_backlog`, `get_sprint_view`, `list_sprints`. These dedicated tools work. |

## Pull requests as a triage surface

**PRs as a request surface: no.** Code lives on GitHub, but requests are
tracked only in Jira.

## When a skill says "publish to the issue tracker"

Create a Jira issue in project `SCRUM`: an `Epic` for a spec, a `Task` for a
ticket (parented to its epic), or a `Bug` for a defect report.

## When a skill says "fetch the relevant ticket"

`get_issue` with the `SCRUM-N` key, plus `list_comments`.

## Wayfinding operations

Used by `/wayfinder`.

- **Map:** an `Epic` labelled `wayfinder-map`. Its description holds the Notes,
  Decisions-so-far and Fog sections.
- **Child ticket:** a `Task` whose parent is the map epic, labelled
  `wayfinder-<type>` (`research`, `prototype`, `grilling` or `task`). Once
  someone claims it, it is assigned to the driving dev.
- **Blocking:** `Blocks` issue links, as in the table above. A ticket is
  unblocked when every issue that "blocks" it is `Done`.
- **Frontier query:** `search_issues` with
  `parent = <MAP> AND status != Done AND assignee is EMPTY ORDER BY rank`.
  Drop any result that still has an open blocker (check its `issuelinks`).
  The first one left wins.
- **Claim:** assign it to yourself through the `jira_api` assignee call above.
  This must be the session's first write.
- **Resolve:** comment with the answer, transition the ticket to `Done`, then
  append a pointer (a one-line gist and the key) to the map's
  Decisions-so-far.
