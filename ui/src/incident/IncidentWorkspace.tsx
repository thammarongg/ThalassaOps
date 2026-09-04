// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { isIncident, isIncidentTimelinePage } from "../../contracts/guards";
import type { Incident, IncidentCommentRequest, Invoke, IpcErrorCode } from "../../contracts/ipc";
import type {
  IncidentGetRequest,
  IncidentRoleCommand,
  IncidentRoleRequest,
  IncidentSeverityCommand,
  IncidentSeverityRequest,
  IncidentTimelineEvent,
  IncidentTimelinePage,
  IncidentTimelineRequest,
  IncidentTransition,
  IncidentTransitionRequest
} from "../../contracts/ipc";
import { useTranslation } from "../i18n";
import { IncidentActions, type ActionResult } from "./IncidentActions";
import { IncidentCommentThread, type CommentSubmitResult } from "./IncidentCommentThread";
import { IncidentList, type IncidentQueueFilter } from "./IncidentList";
import { IncidentNarrative } from "./IncidentNarrative";
import { IncidentTabs } from "./IncidentTabs";
import { INCIDENT_TIMELINE_LIMIT, incidentEnvelope } from "./incidentEnvelope";
import { statesForEvidence, type IncidentTabId, type IncidentTabStates } from "./incidentTabConfig";
import { resolveEvidence, type EvidenceState } from "./incidentEvidence";
import { useIncidentList } from "./useIncidentList";
import { useIncidentTimeline } from "./useIncidentTimeline";
import "./incident.css";

/**
 * Every `IpcErrorCode` gets a message. The switch is total on purpose: a code
 * that reached the surface untranslated would show the reader wire text.
 */
const localizedErrorKey = (code: IpcErrorCode): string => {
  switch (code) {
    case "INVALID_REQUEST":
      return "incident.errors.invalidRequest";
    case "NOT_FOUND":
      return "incident.errors.notFound";
    case "PERMISSION_DENIED":
      return "incident.errors.permissionDenied";
    case "POLICY_DENIED":
      return "incident.errors.policyDenied";
    case "CONNECTOR_UNAVAILABLE":
      return "incident.errors.connectorUnavailable";
    case "MALFORMED_RESPONSE":
      return "incident.errors.malformedResponse";
    case "INVALID_EVENT_SEQUENCE":
      return "incident.errors.invalidEventSequence";
    case "INVALID_SEVERITY_OVERRIDE":
      return "incident.errors.invalidSeverityOverride";
    case "WRITE_CONTENTION":
      return "incident.errors.writeContention";
    case "INTERNAL_ERROR":
      return "incident.errors.internalError";
    default:
      return "incident.errors.internalError";
  }
};

type KeyedIncidentEvidence = {
  incidentId: string | null;
  evidenceKey: string;
  states: IncidentTabStates;
};

type IncidentConflict = { actor: string; at: string };
type TimelineOverride = { incidentId: string; events: IncidentTimelineEvent[] };
type VersionedIncidentVerb = "transition" | "set_severity" | "assign_role";
type VersionedIncidentPayload =
  IncidentTransitionRequest | IncidentSeverityRequest | IncidentRoleRequest;

const isVersionConflict = (result: ActionResult) =>
  result !== undefined &&
  !result.ok &&
  result.error.code === "INVALID_REQUEST" &&
  result.error.details.reason === "incident_version_conflict";

const mutationIncident = (value: unknown): Incident | null => {
  if (typeof value !== "object" || value === null || !("incident" in value)) return null;
  const incident = value.incident;
  return isIncident(incident) ? incident : null;
};

const newestTimelineEvent = (events: IncidentTimelineEvent[]): IncidentTimelineEvent | null =>
  events.reduce<IncidentTimelineEvent | null>(
    (newest, event) => (newest === null || event.sequence > newest.sequence ? event : newest),
    null
  );

const emptyIncidentEvidence = (): KeyedIncidentEvidence => ({
  incidentId: null,
  evidenceKey: "",
  states: statesForEvidence({ status: "empty" })
});

const incidentEvidenceKey = (incident: Incident | null) =>
  incident?.evidence_ids.join("\u0000") ?? "";

/**
 * The workspace shell: a filtered queue on the left, the selected incident's
 * detail on the right. It is the only component in the module that receives
 * `invoke` and the only one that owns selection, per the module boundary rule.
 * Tasks 11-13 fill the remaining detail region; today it carries the incident
 * summary, deterministic narrative and association tabs.
 */
export function IncidentWorkspace({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filter, setFilter] = useState<IncidentQueueFilter>({ status: "all" });
  const [activeTabId, setActiveTabId] = useState<IncidentTabId>("alerts");
  const { incidents, loading, error, hasMore, loadMore } = useIncidentList(invoke);
  const timeline = useIncidentTimeline(invoke, selectedId);
  const reloadTimeline = timeline.reload;
  const evidenceRequestRef = useRef(0);
  const actionRequestRef = useRef(0);
  const [commentSubmitting, setCommentSubmitting] = useState(false);
  const [actionPending, setActionPending] = useState(false);
  const [actionError, setActionError] = useState<IpcErrorCode | null>(null);
  const [conflict, setConflict] = useState<IncidentConflict | null>(null);
  const [incidentOverride, setIncidentOverride] = useState<Incident | null>(null);
  const [timelineOverride, setTimelineOverride] = useState<TimelineOverride | null>(null);

  const selectIncident = useCallback((incidentId: string) => {
    actionRequestRef.current += 1;
    setSelectedId(incidentId);
    setIncidentOverride(null);
    setTimelineOverride(null);
    setConflict(null);
    setActionError(null);
    setActionPending(false);
  }, []);

  useEffect(() => {
    /*
     * Select the first incident once, when nothing is selected. Selecting on
     * every page would drag the reader back to the top of the queue each time
     * `loadMore` returns.
     */
    if (selectedId !== null || incidents.length === 0) return;
    selectIncident(incidents[0].id);
  }, [incidents, selectIncident, selectedId]);

  const listedSelected = useMemo(
    () => incidents.find((incident) => incident.id === selectedId) ?? null,
    [incidents, selectedId]
  );
  const selected = incidentOverride?.id === selectedId ? incidentOverride : listedSelected;
  const selectedEvidenceKey = incidentEvidenceKey(selected);
  const [incidentEvidence, setIncidentEvidence] =
    useState<KeyedIncidentEvidence>(emptyIncidentEvidence);

  if (
    incidentEvidence.incidentId !== (selected?.id ?? null) ||
    incidentEvidence.evidenceKey !== selectedEvidenceKey
  ) {
    setIncidentEvidence({
      incidentId: selected?.id ?? null,
      evidenceKey: selectedEvidenceKey,
      states: statesForEvidence(selected ? { status: "loading" } : { status: "empty" })
    });
  }

  useLayoutEffect(() => {
    // Invalidate the old request in the commit before a settled promise can apply.
    evidenceRequestRef.current += 1;
  }, [selected?.id, selectedEvidenceKey]);

  useEffect(() => {
    if (selected === null) return;
    const requestId = ++evidenceRequestRef.current;
    const incidentId = selected.id;
    const requestEvidenceKey = selectedEvidenceKey;
    void resolveEvidence(invoke, selected.evidence_ids).then((resolved: EvidenceState) => {
      if (requestId !== evidenceRequestRef.current) return;
      setIncidentEvidence((current) => {
        if (current.incidentId !== incidentId || current.evidenceKey !== requestEvidenceKey) {
          return current;
        }
        return {
          ...current,
          states: statesForEvidence(resolved)
        };
      });
    });
    return () => {
      evidenceRequestRef.current += 1;
    };
  }, [invoke, selected, selectedEvidenceKey]);

  const reloadAfterConflict = useCallback(
    async (incidentId: string, requestId: number) => {
      const [incidentResult, timelineResult] = await Promise.all([
        invoke<IncidentGetRequest, Incident>("incident_get", {
          envelope: incidentEnvelope("get", "IncidentRead", { incident_id: incidentId })
        }),
        invoke<IncidentTimelineRequest, IncidentTimelinePage>("incident_timeline", {
          envelope: incidentEnvelope("timeline", "IncidentRead", {
            incident_id: incidentId,
            after_sequence: null,
            limit: INCIDENT_TIMELINE_LIMIT
          })
        })
      ]);
      if (requestId !== actionRequestRef.current) return;

      if (incidentResult.ok && isIncident(incidentResult.value)) {
        setIncidentOverride(incidentResult.value);
      } else {
        setActionError(incidentResult.ok ? "MALFORMED_RESPONSE" : incidentResult.error.code);
      }

      if (
        timelineResult.ok &&
        isIncidentTimelinePage(timelineResult.value) &&
        timelineResult.value.incident_id === incidentId
      ) {
        setTimelineOverride({ incidentId, events: timelineResult.value.events });
        const newest = newestTimelineEvent(timelineResult.value.events);
        if (newest === null) {
          setActionError("MALFORMED_RESPONSE");
        } else {
          setConflict({ actor: newest.actor_id, at: newest.occurred_at });
        }
      } else {
        setActionError(timelineResult.ok ? "MALFORMED_RESPONSE" : timelineResult.error.code);
      }
    },
    [invoke]
  );

  const runVersionedMutation = useCallback(
    async (
      verb: VersionedIncidentVerb,
      payload: VersionedIncidentPayload
    ): Promise<ActionResult> => {
      if (selected === null) return undefined;
      const requestId = ++actionRequestRef.current;
      setActionPending(true);
      setConflict(null);
      setActionError(null);
      try {
        const result = await invoke<VersionedIncidentPayload, unknown>(`incident_${verb}`, {
          envelope: incidentEnvelope(verb, "IncidentWrite", payload)
        });
        if (requestId !== actionRequestRef.current) return result;
        if (result.ok) {
          const updated = mutationIncident(result.value);
          if (updated !== null && updated.id === selected.id) setIncidentOverride(updated);
        } else if (isVersionConflict(result)) {
          await reloadAfterConflict(selected.id, requestId);
        }
        return result;
      } catch (error) {
        if (requestId === actionRequestRef.current) setActionError("INTERNAL_ERROR");
        throw error;
      } finally {
        if (requestId === actionRequestRef.current) setActionPending(false);
      }
    },
    [invoke, reloadAfterConflict, selected]
  );

  const onTransition = useCallback(
    (transition: IncidentTransition) =>
      selected === null
        ? undefined
        : runVersionedMutation("transition", {
            incident_id: selected.id,
            expected_version: selected.version,
            transition
          }),
    [runVersionedMutation, selected]
  );

  const onSeverity = useCallback(
    (command: IncidentSeverityCommand) =>
      selected === null
        ? undefined
        : runVersionedMutation("set_severity", {
            incident_id: selected.id,
            expected_version: selected.version,
            command
          }),
    [runVersionedMutation, selected]
  );

  const onAssign = useCallback(
    (command: IncidentRoleCommand) =>
      selected === null
        ? undefined
        : runVersionedMutation("assign_role", {
            incident_id: selected.id,
            expected_version: selected.version,
            command
          }),
    [runVersionedMutation, selected]
  );

  const submitComment = useCallback(
    async (body: string): Promise<CommentSubmitResult> => {
      if (selected === null) return undefined;
      setCommentSubmitting(true);
      try {
        const result = await invoke<IncidentCommentRequest, unknown>("incident_add_comment", {
          envelope: incidentEnvelope("add_comment", "IncidentWrite", {
            incident_id: selected.id,
            body
          })
        });
        if (result.ok) reloadTimeline();
        return result;
      } finally {
        setCommentSubmitting(false);
      }
    },
    [invoke, reloadTimeline, selected]
  );

  const detailEvents =
    selected !== null && timelineOverride?.incidentId === selected.id
      ? timelineOverride.events
      : timeline.events;
  const failure = error ?? timeline.error ?? actionError;

  return (
    <section className="incident-workspace" aria-labelledby="incident-workspace-title">
      <header className="incident-workspace__header">
        <h2 id="incident-workspace-title">{t("incident.queueTitle")}</h2>
      </header>
      {failure && (
        <p className="incident-workspace__state incident-workspace__state--error" role="alert">
          {t(localizedErrorKey(failure))}
        </p>
      )}
      <div className="incident-workspace__split">
        <div className="incident-workspace__queue">
          {loading && incidents.length === 0 ? (
            <p className="incident-workspace__state">{t("incident.loading")}</p>
          ) : (
            <IncidentList
              incidents={incidents}
              selectedId={selectedId}
              onSelect={selectIncident}
              filter={filter}
              onFilterChange={setFilter}
            />
          )}
          {hasMore && (
            <button type="button" className="incident-workspace__more" onClick={loadMore}>
              {t("incident.loadMore")}
            </button>
          )}
        </div>
        <div
          className="incident-workspace__detail"
          data-incident-id={selected?.id ?? ""}
          data-timeline-events={detailEvents.length}
        >
          <h3>{t("incident.detailTitle")}</h3>
          {selected ? (
            <>
              <p className="incident-workspace__detail-summary">{selected.summary}</p>
              {/*
               * Every incident carries at least its creation event, so the
               * narrative's empty state is never true of a real one. It is
               * withheld while the first page loads rather than shown as a
               * false statement for the length of the read, and a failed
               * first read shows only the translated error above — an error
               * is not evidence of an empty record. A `loadMore` has events
               * already and keeps rendering them, failure or not.
               */}
              {timeline.loading && detailEvents.length === 0 ? (
                <p className="incident-workspace__state">{t("incident.narrative.loading")}</p>
              ) : timeline.error !== null && detailEvents.length === 0 ? null : (
                <IncidentNarrative events={detailEvents} />
              )}
              <IncidentTabs
                incident={selected}
                states={incidentEvidence.states}
                activeId={activeTabId}
                onSelect={setActiveTabId}
              />
              <IncidentCommentThread
                events={detailEvents}
                onSubmit={submitComment}
                submitting={commentSubmitting}
              />
              <IncidentActions
                incident={selected}
                onTransition={onTransition}
                onSeverity={onSeverity}
                onAssign={onAssign}
                pending={actionPending}
                conflict={conflict}
              />
            </>
          ) : (
            <p className="incident-workspace__state">{t("incident.detailEmpty")}</p>
          )}
        </div>
      </div>
    </section>
  );
}
