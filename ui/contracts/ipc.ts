// SPDX-License-Identifier: Apache-2.0

/** JSON contract shared by the Tauri Rust core and React UI. */

export type UUID = string;

export type ResourceScope = {
  organization_id?: UUID;
  team_id?: UUID;
  workspace_id?: UUID;
  environment_id?: UUID;
  resource_ids: UUID[];
};

export type Capability =
  | "WorkspaceRead"
  | "EnvironmentRead"
  | "ResourceRead"
  | "IncidentRead"
  | "IncidentWrite"
  | "PolicyEvaluate"
  | "PolicyManage"
  | "ConnectorRead"
  | "ConnectorAct";

export type Permission =
  | "Read"
  | "Investigate"
  | "RecommendAction"
  | "ExecuteAction"
  | "ManagePolicy"
  | "ManageMembership"
  | "AuditRead";

export type CommandName = `${string}.${string}`;

export type CommandEnvelope<T> = {
  request_id: UUID;
  command: CommandName;
  capability: Capability;
  scope: ResourceScope;
  payload: T;
};

export type IpcErrorCode =
  | "INVALID_REQUEST"
  | "NOT_FOUND"
  | "PERMISSION_DENIED"
  | "POLICY_DENIED"
  | "CONNECTOR_UNAVAILABLE"
  | "INTERNAL_ERROR";

export type IpcError = {
  code: IpcErrorCode;
  message: string;
  details: Record<string, unknown>;
};

export type IpcResult<T> = { ok: true; value: T } | { ok: false; error: IpcError };

export type WorkspaceContext = {
  organization_name: string;
  team_name: string;
  workspace_name: string;
  policy_version: number;
};

export type StatusState = "healthy" | "degraded" | "unavailable" | "warning" | "critical";
export type ConnectorCapability = { key: string; operation: "Read" | "Act"; resource_kinds: string[] };
export type ConnectorManifest = { id: string; display_name: string; version: string; capabilities: ConnectorCapability[] };
export type ConnectorSummary = {
  id: string; kind: string; display_name: string; enabled: boolean; config_metadata: Record<string, unknown>;
  credential_configured: boolean; health_state: StatusState; last_checked_at?: string; last_successful_sync_at?: string;
};
export type ConnectorLogEntry = { id: string; checked_at: string; outcome: StatusState; message: string };
export type ConnectorDiagnostics = { connector: ConnectorSummary; manifest: ConnectorManifest; logs: ConnectorLogEntry[] };
export type KubernetesCondition = { type_: string; status: string; reason?: string; message?: string };
export type KubernetesOwner = { kind: string; name: string; uid?: string };
export type KubernetesHealth = "healthy" | "degraded" | "crash_loop_back_off" | "oom_killed" | "pending" | "unknown";
export type KubernetesResource = { resource: { name: string; kind: string; labels: Record<string, string> }; status?: string; conditions: KubernetesCondition[]; owner?: KubernetesOwner; service_selector?: Record<string, string>; replicas?: { desired: number; ready: number; available?: number }; containers: { name: string; restart_count: number; waiting_reason?: string; terminated_reason?: string; last_terminated_reason?: string }[]; health: KubernetesHealth };
export type KubernetesEvent = { type_?: string; reason?: string; message?: string; involved_kind?: string; involved_name?: string };
export type KubernetesInventory = { resources: KubernetesResource[]; availability: { resource_kind: string; available: boolean; reason?: string }[]; topology: { from_kind: string; from_name: string; to_kind: string; to_name: string; relationship: string }[] };
export type KubernetesManifest = { yaml: string; masked: boolean; risk_class: string };

/**
 * Tauri command names use lowercase resource.verb components. Commands must
 * be registered with an explicit capability and permission on the Rust side.
 */
export const command = <R extends string, V extends string>(resource: R, verb: V): `${R}.${V}` =>
  `${resource}.${verb}`;
