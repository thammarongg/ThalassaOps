// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useState } from "react";
import type { Invoke, IpcErrorCode } from "../../contracts/ipc";
import { useTranslation } from "../i18n";
import { IncidentList, type IncidentQueueFilter } from "./IncidentList";
import { IncidentNarrative } from "./IncidentNarrative";
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

/**
 * The workspace shell: a filtered queue on the left, the selected incident's
 * detail on the right. It is the only component in the module that receives
 * `invoke` and the only one that owns selection, per the module boundary rule.
 * Tasks 9-13 fill the rest of the detail region; today it carries the
 * incident summary and the deterministic narrative of its lifecycle.
 */
export function IncidentWorkspace({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [filter, setFilter] = useState<IncidentQueueFilter>({ status: "all" });
  const { incidents, loading, error, hasMore, loadMore } = useIncidentList(invoke);
  const timeline = useIncidentTimeline(invoke, selectedId);

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
              <IncidentNarrative events={timeline.events} />
            </>
          ) : (
            <p className="incident-workspace__state">{t("incident.detailEmpty")}</p>
          )}
        </div>
      </div>
    </section>
  );
}
