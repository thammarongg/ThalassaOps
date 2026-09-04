// SPDX-License-Identifier: Apache-2.0

import { useRef, type KeyboardEvent } from "react";
import type { Incident, IncidentSeverity, IncidentStatus } from "../../contracts/ipc";
import { useTranslation } from "../i18n";

export type IncidentQueueFilter = { status: "all" | IncidentStatus };

export const INCIDENT_STATUSES: IncidentStatus[] = [
  "detected",
  "triage",
  "investigating",
  "mitigating",
  "monitoring",
  "resolved",
  "closed",
  "reopened"
];

/**
 * The severity to show. An override replaces the derived value everywhere it
 * is displayed; rendering `derived_severity` alone would leave every override
 * invisible in the queue. Exported because the detail panels must not each
 * re-derive it.
 */
export const effectiveSeverity = (incident: Incident): IncidentSeverity =>
  incident.severity_override?.selected ?? incident.derived_severity;

/**
 * The incident queue. Pure: it renders the incidents it is given and reports
 * selection upward, and it never calls IPC.
 *
 * The status filter is applied here, over the pages already loaded, because
 * `IncidentListRequest` carries no status parameter. A filter that admits
 * nothing on the loaded pages therefore shows the empty state rather than
 * fetching further pages.
 */
export function IncidentList({
  incidents,
  selectedId,
  onSelect,
  filter,
  onFilterChange
}: {
  incidents: Incident[];
  selectedId: string | null;
  onSelect: (incidentId: string) => void;
  filter: IncidentQueueFilter;
  onFilterChange: (filter: IncidentQueueFilter) => void;
}) {
  const { t } = useTranslation();
  const listRef = useRef<HTMLUListElement>(null);
  const visible = incidents.filter(
    (incident) => filter.status === "all" || incident.status === filter.status
  );

  const moveTo = (index: number) => {
    const target = visible[Math.min(Math.max(index, 0), visible.length - 1)];
    if (!target) return;
    onSelect(target.id);
    const option = listRef.current?.querySelector<HTMLLIElement>(
      `[data-incident-id="${target.id}"]`
    );
    option?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    const current = visible.findIndex((incident) => incident.id === selectedId);
    const from = current === -1 ? 0 : current;
    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveTo(from + 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      moveTo(from - 1);
    } else if (event.key === "Home") {
      event.preventDefault();
      moveTo(0);
    } else if (event.key === "End") {
      event.preventDefault();
      moveTo(visible.length - 1);
    }
  };

  return (
    <div className="incident-queue">
      <label className="incident-queue__filter">
        <span>{t("incident.filter.label")}</span>
        <select
          value={filter.status}
          onChange={(event) =>
            onFilterChange({ status: event.target.value as IncidentQueueFilter["status"] })
          }
        >
          <option value="all">{t("incident.filter.all")}</option>
          {INCIDENT_STATUSES.map((status) => (
            <option key={status} value={status}>
              {t("incident.status." + status)}
            </option>
          ))}
        </select>
      </label>
      {visible.length === 0 ? (
        <p className="incident-queue__empty">{t("incident.emptyQueue")}</p>
      ) : (
        <ul
          className="incident-queue__list"
          role="listbox"
          aria-label={t("incident.queueLabel")}
          ref={listRef}
          onKeyDown={onKeyDown}
        >
          {visible.map((incident) => {
            const selected = incident.id === selectedId;
            return (
              <li
                key={incident.id}
                role="option"
                aria-selected={selected}
                data-incident-id={incident.id}
                tabIndex={selected || (selectedId === null && incident === visible[0]) ? 0 : -1}
                className={
                  selected
                    ? "incident-queue__row incident-queue__row--selected"
                    : "incident-queue__row"
                }
                onClick={() => onSelect(incident.id)}
              >
                <span className="incident-queue__summary">{incident.summary}</span>
                <span className="incident-queue__meta">
                  <span data-testid="incident-severity" className="incident-queue__severity">
                    <span className="incident-queue__field-label">
                      {t("incident.severityLabel")}
                    </span>
                    {effectiveSeverity(incident)}
                  </span>
                  <span data-testid="incident-status" className="incident-queue__status">
                    <span className="incident-queue__field-label">{t("incident.statusLabel")}</span>
                    {t("incident.status." + incident.status)}
                  </span>
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
