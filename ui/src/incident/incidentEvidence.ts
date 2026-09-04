// SPDX-License-Identifier: Apache-2.0

import type {
  CommandEnvelope,
  ConsoleEvidenceId,
  EvidenceRef,
  Invoke,
  IpcErrorCode
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { isEvidenceResponse } from "../../contracts/guards";

/**
 * Why the evidence behind an association could not be shown. "Empty" is not a
 * cause: an incident with no associations of a kind and one whose evidence
 * could not be resolved mean different things during a retrospective, and
 * collapsing them loses the distinction the responder needs.
 */
export type EvidenceUnavailableCause = "missing" | "scope" | "unverified" | "unknown";

export type EvidenceState =
  | { status: "loading" }
  | { status: "empty" }
  | { status: "unavailable"; cause: EvidenceUnavailableCause }
  | { status: "ready"; evidence: EvidenceRef[] };

/**
 * An incident's evidence identifiers are the ones the correlation source-record
 * store admitted while normalizing the signal that raised it, so the
 * correlation snapshot is the only store that can resolve all of them. The
 * operations snapshot carries no security evidence, and a security finding is
 * one of the six source kinds an incident can be raised from.
 */
const EVIDENCE_TAURI_COMMAND = "correlation_evidence";

const evidenceEnvelope = (
  evidenceIds: ConsoleEvidenceId[]
): CommandEnvelope<{ evidence_ids: ConsoleEvidenceId[] }> => ({
  request_id: crypto.randomUUID(),
  command: command("correlation", "evidence"),
  capability: "ResourceRead",
  scope: { resource_ids: [] },
  payload: { evidence_ids: evidenceIds }
});

/**
 * Every failure code the evidence command can return, mapped to the reason the
 * panel states. `INVALID_REQUEST` means an empty, repeated or unsorted list
 * reached the backend, which this helper prevents — reaching it is a defect
 * here, not a fact about the evidence, so it reports no more than "unknown".
 */
const causeFor = (code: IpcErrorCode): EvidenceUnavailableCause => {
  switch (code) {
    case "NOT_FOUND":
      return "missing";
    case "PERMISSION_DENIED":
      return "scope";
    case "POLICY_DENIED":
      return "unverified";
    default:
      return "unknown";
  }
};

const unavailable = (cause: EvidenceUnavailableCause): EvidenceState => ({
  status: "unavailable",
  cause
});

/**
 * Resolves one association set to its evidence.
 *
 * The request is sorted and de-duplicated because `validate_correlation_evidence_ids`
 * rejects a repeated or unsorted list outright, and resolution is
 * all-or-nothing: one bad identifier fails the whole request, which would make
 * the tab permanently unavailable rather than merely incomplete. An empty list
 * is answered without issuing a command at all, since an empty request is a
 * hard error rather than an empty result.
 */
export const resolveEvidence = async (
  invoke: Invoke,
  ids: ConsoleEvidenceId[]
): Promise<EvidenceState> => {
  const requested = [...new Set(ids)].sort();
  if (requested.length === 0) return { status: "empty" };

  try {
    const result = await invoke<{ evidence_ids: ConsoleEvidenceId[] }, EvidenceRef[]>(
      EVIDENCE_TAURI_COMMAND,
      { envelope: evidenceEnvelope(requested) }
    );
    if (!result.ok) return unavailable(causeFor(result.error.code));
    /*
     * A response that is not an exact cover of the request is a contract
     * violation. Rendering it would present a partial record as the whole one.
     */
    if (!isEvidenceResponse(result.value, requested)) return unavailable("unknown");
    return { status: "ready", evidence: result.value };
  } catch {
    return unavailable("unknown");
  }
};
