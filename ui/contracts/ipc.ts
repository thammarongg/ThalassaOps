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

export type IpcResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: IpcError };

/**
 * Tauri command names use lowercase resource.verb components. Commands must
 * be registered with an explicit capability and permission on the Rust side.
 */
export const command = <R extends string, V extends string>(resource: R, verb: V): `${R}.${V}` =>
  `${resource}.${verb}`;
