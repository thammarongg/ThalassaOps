// Dev-only preview harness. Renders the real Shell against fixture data so the
// visual layer can be worked on in a browser with HMR, without a Tauri build.
// Never imported by main.tsx, so it cannot reach a shipped bundle.
//
// Correctness must still be reviewed against the backend, never against this
// harness: it answers with fixtures, not with the real IPC surface.
import { createRoot } from "react-dom/client";
import type { Invoke, IpcResult, WorkspaceContext } from "../../contracts/ipc";
import { I18nProvider } from "../i18n";
import { Shell } from "../shell";
import { operationsSnapshotFixture } from "./operations-fixture";
import "../styles.css";

const snapshot = operationsSnapshotFixture();

const context: WorkspaceContext = {
  organization_name: "Local Organization",
  team_name: "Platform",
  workspace_name: "Production",
  policy_version: 1
};

const ok = <U,>(value: U): IpcResult<U> => ({ ok: true, value }) as IpcResult<U>;

const previewInvoke: Invoke = (name, args) => {
  if (name === "system_context") return Promise.resolve(ok(context) as never);
  if (name === "operations_snapshot") return Promise.resolve(ok(snapshot) as never);
  if (name === "operations_evidence") {
    const ids = (args.envelope.payload as { evidence_ids: string[] }).evidence_ids;
    const value = snapshot.evidence.filter((item) => ids.includes(item.id));
    return Promise.resolve(ok(value) as never);
  }
  return Promise.resolve({
    ok: false,
    error: { code: "internal_error", message: `preview has no fixture for ${name}`, details: {} }
  } as never);
};

createRoot(document.getElementById("root")!).render(
  <I18nProvider>
    <Shell invoke={previewInvoke} />
  </I18nProvider>
);
