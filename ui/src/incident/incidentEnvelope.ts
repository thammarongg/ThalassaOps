// SPDX-License-Identifier: Apache-2.0

import type { CommandEnvelope } from "../../contracts/ipc";
import { command } from "../../contracts/ipc";

/**
 * Page sizes for the two paged incident reads. Both sit inside the `1..=100`
 * the Rust payloads validate, and both are exported so a test asserts the
 * payload the hook sends rather than restating a literal beside it.
 */
export const INCIDENT_PAGE_LIMIT = 25;
export const INCIDENT_TIMELINE_LIMIT = 50;

/** The nine `incident.*` verbs the Tauri core registers. */
export type IncidentVerb =
  | "create"
  | "get"
  | "list"
  | "timeline"
  | "transition"
  | "set_severity"
  | "set_disposition"
  | "assign_role"
  | "add_comment";

export type IncidentCapability = "IncidentRead" | "IncidentWrite";

/**
 * One envelope builder for the whole incident module. Correlation and topology
 * each inline their own, but the incident reads and the incident writes live
 * in different components, and a second copy is how the two capabilities drift
 * apart.
 */
export const incidentEnvelope = <T>(
  verb: IncidentVerb,
  capability: IncidentCapability,
  payload: T
): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("incident", verb),
  capability,
  scope: { resource_ids: [] },
  payload
});
