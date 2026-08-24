# ThalassaOps desktop shell

Sprint 2 establishes the macOS-first Tauri 2 shell and a minimal React status screen. It uses the Sprint 1 contracts in `crates/` and `ui/contracts/ipc.ts`; the frontend does not redefine IPC contract types.

## Commands

Install JavaScript dependencies once:

```bash
npm install
```

Run the desktop app in development mode (Tauri performs normal macOS development/ad-hoc signing):

```bash
npm run tauri:dev
```

Run checks and produce an ad-hoc-signed macOS application bundle:

```bash
cargo test --workspace
npm run typecheck
npm test
npm run tauri:build
```

`npm run lint`, `npm run format:check`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings` are the corresponding lint/format checks.

## Health-check verification

1. Run `npm run tauri:dev` on macOS and wait for the ThalassaOps window. To verify the packaged app, run `npm run tauri:build` then open `target/release/bundle/macos/ThalassaOps.app`.
2. The only screen calls the registered Rust command `system_health` on load using `CommandEnvelope` from `ui/contracts/ipc.ts` with `system.health`, `WorkspaceRead`, and an unbounded system scope.
3. Confirm the screen displays `healthy`. The Rust test suite covers the same command path and verifies that an altered capability or a claimed resource scope produces an IPC error instead.

## Local state

Startup applies embedded SQL files from `src-tauri/migrations/` using a `schema_migrations` ledger, then creates a local administrator principal, Organization → Team → Workspace hierarchy, workspace-owner membership, and `PolicyDocument::baseline(1)` if they do not exist. The SQLite database is placed in Tauri's macOS application-data directory as `thalassaops.sqlite`.

This sprint intentionally contains no connectors, Operations Console functionality, AI, or design-system components.
