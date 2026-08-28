# Sprint 10 verification report

Date: 2026-08-28
Task: Sprint 10, Task 12 — regression, security and acceptance verification
Branch: `thammarongg/sprint-10-cloud-inventory`

## Executive result

All required Rust and frontend commands were run from the repository root, with
`npm ci` run before every frontend gate. The commands completed successfully,
but verification found three defects that remain for coordinator disposition:
the cloud connector test is still a stub, the UI acceptance fixture does not
exercise compute instances, and an Azure fixture contains an unredacted opaque
pagination token. No code or tests were changed by this verification task.

## Environment and baselines

- Node: `v24.18.0`
- npm: `11.16.0`
- Task 2 Rust baseline recorded by the preceding verification report: 79
  package tests passed.
- `git status --short --untracked-files=all` was clean before this report was
  created.

## Rust gates

All commands ran at the repository root.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS — exit 0, no output. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS — exit 0; `Finished dev profile [unoptimized + debuginfo]`. |
| `cargo test --workspace` | PASS — exit 0; 116 tests passed, 0 failed across workspace package/integration targets. The `thalassaops` package ran 92 unit tests plus 4 fixture-capture integration tests; workspace integration targets added 2 (`thalassa-connectors`), 4 (`thalassa-domain`), 3 (`thalassa-ipc`) and 11 (`thalassa-policy`). All five doctest targets ran 0 tests with 0 failures. The 92 package tests exceed the Task 2 baseline of 79. |

The concise rerun `cargo test --workspace 2>&1 | grep '^test result'` printed 16
successful `test result` lines: the nonzero counts were 2, 4, 3, 11, 92 and 4;
the remaining ten targets reported 0 passed and 0 failed.

Additional focused checks also ran successfully:

- `cargo test -p thalassaops cloud::` — 30 passed, 0 failed, 62 filtered.
- `cargo test -p thalassaops --example cloud_fixture_capture` — 4 passed,
  0 failed.

## Frontend gates

All commands ran at the repository root under Node 24.18.0, after `npm ci`.

| Command | Actual result |
| --- | --- |
| `npm ci` | PASS — added 298 packages and audited 299; npm reported 5 vulnerabilities (3 moderate, 1 high, 1 critical), two deprecation warnings, and pending install-script approval for `esbuild` and `fsevents`. |
| `npm run format:check` | PASS — `All matched files use Prettier code style!`; exit 0. |
| `npm run lint` | PASS — ESLint exit 0 with no diagnostics. |
| `npm run typecheck` | PASS — `tsc -b --force` exit 0. |
| `npm run build` | PASS — TypeScript and Vite completed; Vite 5.4.21 transformed 80 modules and emitted the production bundle. |
| `npm test` | PASS — 4 test files, 20 tests passed, 0 failed, 0 skipped. |

## Fixture acceptance journey

The targeted journey was run with:

```text
npx vitest run ui/src/shell.test.tsx -t "shows three cloud environments with provider boundaries and keeps healthy ones visible when one session expires"
```

Actual result: 1 test passed and 15 tests skipped in the selected file. Its
assertions confirm AWS, Azure and GCP provider badges, `prod-eks` and
`prod-gke` rows for the two confirmed environments, the Azure `az login`
remedy, and suppression of the failed environment's `prod-aks` row while the
other panels remain visible.

The journey is incomplete against Task 12 Step 4: `ui/src/shell.test.tsx`
only places `kubernetes_cluster` entries in the AWS and GCP inventory fixtures
and has no `compute_instance` entries or assertions. The Rust mapper tests do
map compute instances for all three providers, but the required UI acceptance
journey does not verify that those rows render.

## Diff and security audit

- `git diff --check` — PASS, exit 0, no output.
- `git diff main...HEAD` — exit 0. The reviewed diff contains 59 files,
  9,975 insertions and 1,218 deletions; the source, fixture, infrastructure,
  contract and UI changes were read rather than checked only by pattern.
- Cloud provider failures carry only a status or generic sanitized error:
  `CloudClient` maps non-success responses to `ProviderError(status)` without
  reading the body, and `app/cloud.rs` maps provider/auth/request errors to an
  empty-details generic `IpcError`. No provider response body, credential or
  authorization header is sent to a log, connector diagnostic, React fixture,
  or serialized cloud `IpcResult` in the reviewed paths. The capture utility
  logs only operation, status, content type and byte count; its test confirms
  bearer/token/XML-sensitive values are redacted before writing fixtures.
- No keychain call or credential-store write was added for cloud connectors;
  cloud connector validation rejects any `credential_value` and accepts only
  non-secret selectors. The keychain implementation remains the existing
  observability/connector path.
- `src-tauri/capabilities/default.json` still lists exactly
  `core:default` and `shell:allow-open`.
- `CloudResourceType` and its TypeScript union contain exactly
  `kubernetes_cluster` and `compute_instance`; no third cloud model resource
  type was added.
- `grep -c "name = .aws-lc-rs." Cargo.lock` printed `0`. Its exit code was 1,
  which is the normal grep status for zero matches.

One committed fixture exception was found: line 2 of
`docs/superpowers/fixtures/2026-08-27-capture/azure/azure_aks_managed_clusters.json`
contains a raw, opaque `$skiptoken` value in `nextLink`. It is a pagination
cursor rather than an authorization credential, but it is still an
unredacted token in a committed provider fixture and the capture redactor does
not classify `skiptoken`/`nextLink` as sensitive.

## Defects escalated

1. `src-tauri/src/connectors.rs:523-551` routes every AWS, Azure and GCP
   `connector_test` through a hard-coded `Err(CloudClientError::RequestFailed)`.
   Consequently connector tests never run a provider preflight and always
   record `unavailable`, contradicting Task 6's requirement that connector test
   and access check share the same preflight. The Environment UI calls
   `cloud_access_check` directly, so the targeted journey does not expose this
   defect.
2. `ui/src/shell.test.tsx:1190-1234` has only cluster records in the healthy
   inventory fixtures. The test passes, but it cannot prove the Step 4 compute
   instance rendering requirement.
3. `docs/superpowers/fixtures/2026-08-27-capture/azure/azure_aks_managed_clusters.json:2`
   contains the unredacted `$skiptoken` described above.

These findings were escalated to the coordinator during the run. No fixes were
applied, no merge was performed, and nothing was pushed.

## Defect-fix rerun

The three defects above were fixed in separate commits: `72e6b60` routes cloud
connector tests through the shared provider access checks and adds the missing
credential regression; `5501318` adds compute instances to both healthy UI
fixtures and asserts their rows; and `f433146` redacts pagination cursors in
query parameters and response fields. The Azure fixture's opaque cursor was
removed and replaced with its pagination placeholder; this removes captured
data rather than inventing fixture content. AWS and GCP fixtures were checked:
AWS `nextToken` remains `null`, and GCP has no cursor, so neither changed.

All gates were rerun from the repository root on 2026-08-28:

| Command | Actual result |
| --- | --- |
| `cargo build --workspace` | PASS — exit 0. |
| `cargo test --workspace` | PASS — 119 tests passed, 0 failed. The `thalassaops` package ran 93 unit tests plus 6 fixture-capture integration tests; workspace integration targets added 2 (`thalassa-connectors`), 4 (`thalassa-domain`), 3 (`thalassa-ipc`) and 11 (`thalassa-policy`). Five doctest targets ran 0 tests with 0 failures. |
| `cargo fmt --all -- --check` | PASS — exit 0, no output. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS — exit 0. |
| `npm ci` | PASS — 298 packages installed and 299 audited; npm reported 5 existing vulnerabilities. |
| `npm test` | PASS — 4 test files, 20 tests passed, 0 failed. |
| `npm run typecheck` | PASS — exit 0. |
| `npm run lint` | PASS — exit 0 with no diagnostics. |
| `npm run build` | PASS — TypeScript and Vite production build completed. |
| `npm run format:check` | PASS — all files matched Prettier style. |
| `grep -c "name = .aws-lc-rs." Cargo.lock` | Prints `0` (exit 1, as expected for zero matches). |

The targeted acceptance journey was rerun after the fixture change: 1 test
passed and 15 were skipped; AWS and GCP each rendered both a cluster and a
compute-instance row, while Azure retained its copyable `az login` remedy and
did not hide the healthy panels. The new connector and capture regressions also
passed in the workspace run.
