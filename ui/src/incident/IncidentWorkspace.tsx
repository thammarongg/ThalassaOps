// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { Incident, IncidentCommentRequest, Invoke, IpcErrorCode } from "../../contracts/ipc";
import { useTranslation } from "../i18n";
import { IncidentCommentThread, type CommentSubmitResult } from "./IncidentCommentThread";
import { IncidentList, type IncidentQueueFilter } from "./IncidentList";
import { IncidentNarrative } from "./IncidentNarrative";
import { IncidentTabs } from "./IncidentTabs";
import { incidentEnvelope } from "./incidentEnvelope";
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
  const [commentSubmitting, setCommentSubmitting] = useState(false);

  useEffect(() => {
    /*
     * Select the first incident once, when nothing is selected. Selecting on
     * every page would drag the reader back to the top of the queue each time
     * `loadMore` returns.
     */
    if (selectedId !== null || incidents.length === 0) return;
    setSelectedId(incidents[0].id);
  }, [incidents, selectedId]);

  const selected = useMemo(
    () => incidents.find((incident) => incident.id === selectedId) ?? null,
    [incidents, selectedId]
  );
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

  const failure = error ?? timeline.error;

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
              onSelect={setSelectedId}
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
          data-timeline-events={timeline.events.length}
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
              {timeline.loading && timeline.events.length === 0 ? (
                <p className="incident-workspace__state">{t("incident.narrative.loading")}</p>
              ) : timeline.error !== null && timeline.events.length === 0 ? null : (
                <IncidentNarrative events={timeline.events} />
              )}
              <IncidentTabs
                incident={selected}
                states={incidentEvidence.states}
                activeId={activeTabId}
                onSelect={setActiveTabId}
              />
              <IncidentCommentThread
                events={timeline.events}
                onSubmit={submitComment}
                submitting={commentSubmitting}
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
