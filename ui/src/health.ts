import type { CommandEnvelope, IpcResult } from "../contracts/ipc";
import { command } from "../contracts/ipc";

export type Health = { status: string; policy_version?: number };
type Invoke = (command: string, args: Record<string, unknown>) => Promise<IpcResult<Health>>;

export async function requestHealth(invoke: Invoke): Promise<Health> {
  const envelope: CommandEnvelope<null> = {
    request_id: crypto.randomUUID(),
    command: command("system", "health"),
    capability: "WorkspaceRead",
    scope: { resource_ids: [] },
    payload: null
  };
  const result = await invoke("system_health", { envelope });
  if (!result.ok) throw new Error(result.error.message);
  return result.value;
}
