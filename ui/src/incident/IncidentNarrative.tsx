// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";
import type { IncidentTimelineEvent, IncidentTimelinePayload } from "../../contracts/ipc";
import { EmptyState, Table } from "../design-system/components";
import { useTranslation } from "../i18n";

/** Everything the narrative renders. Comments are the responder's voice and
 * belong to the comment thread, not to the record of what the system did. */
type LifecyclePayload = Exclude<IncidentTimelinePayload, { kind: "commented" }>;
type LifecycleEvent = IncidentTimelineEvent & { payload: LifecyclePayload };

const isLifecycle = (event: IncidentTimelineEvent): event is LifecycleEvent =>
  event.payload.kind !== "commented";

const columns = [
  { key: "time", headerKey: "incident.narrative.columns.time" },
  { key: "actor", headerKey: "incident.narrative.columns.actor" },
  { key: "change", headerKey: "incident.narrative.columns.change" },
  { key: "reason", headerKey: "incident.narrative.columns.reason" }
];

type Translate = ReturnType<typeof useTranslation>["t"];

/**
 * What one lifecycle event changed, as a label and a value — never a composed
 * sentence. Two languages whose word order differs would each need their own
 * grammar, and Sprint 19 rewrites the narrative anyway (design 15).
 *
 * The switch is total over `LifecyclePayload`: a kind added to the contract
 * fails the `never` assignment here rather than rendering a blank cell.
 */
const describe = (payload: LifecyclePayload, t: Translate): { label: string; value: string } => {
  const none = t("incident.narrative.none");
  const arrow = (from: string, to: string) => `${from} → ${to}`;
  switch (payload.kind) {
    case "created":
      return {
        label: t("incident.narrative.kind.created"),
        value: payload.data.derived_severity
      };
    case "triggers_attached":
      return {
        label: t("incident.narrative.kind.triggersAttached"),
        // A bare count: the trigger identifiers mean nothing to a reader here,
        // and the association tabs resolve them.
        value: String(payload.data.trigger_ids.length)
      };
    case "status_transitioned":
      return {
        label: t("incident.narrative.kind.statusTransitioned"),
        value: arrow(
          t("incident.status." + payload.data.from),
          t("incident.status." + payload.data.to)
        )
      };
    case "severity_changed":
      // Severity codes are the same in both catalogs, so they are not keys.
      return {
        label: t("incident.narrative.kind.severityChanged"),
        value: arrow(payload.data.previous_severity, payload.data.current_severity)
      };
    case "disposition_changed": {
      const label = (disposition: string | null) =>
        disposition === null ? none : t("incident.disposition." + disposition);
      return {
        label: t("incident.narrative.kind.dispositionChanged"),
        value: arrow(label(payload.data.previous), label(payload.data.current))
      };
    }
    case "role_changed": {
      // Who holds the role is the change, so both sides of the arrow are
      // principals. Showing only the new holder would read as an assignment
      // out of nowhere and hide a release entirely.
      const previous = payload.data.previous_principal_ids;
      return {
        label: `${t("incident.narrative.kind.roleChanged")} · ${t("incident.role." + payload.data.role)}`,
        value: arrow(
          previous.length === 0 ? none : previous.join(", "),
          payload.data.current_principal_id ?? none
        )
      };
    }
    default: {
      const exhaustive: never = payload;
      return exhaustive;
    }
  }
};

const changeCell = (payload: LifecyclePayload, t: Translate): ReactNode => {
  const { label, value } = describe(payload, t);
  return (
    <span className="incident-narrative__change">
      <span className="incident-narrative__kind">{label}</span>
      <span className="incident-narrative__value">{value}</span>
    </span>
  );
};

/**
 * The deterministic record of one incident's lifecycle. Pure: it renders the
 * events it is given, calls no IPC, and holds no selection.
 *
 * `actor_id` is rendered as the identifier it is. No principal directory
 * reaches the UI in this sprint, and a truncated or invented display name
 * would misattribute a change on an audit surface.
 */
export function IncidentNarrative({ events }: { events: IncidentTimelineEvent[] }) {
  const { t } = useTranslation();
  const lifecycle = events
    .filter(isLifecycle)
    // Ordered here rather than trusted from the page: a resumed read appends,
    // and an earlier event must not read as if it happened last.
    .sort((left, right) => left.sequence - right.sequence);

  return (
    <section className="incident-narrative" aria-labelledby="incident-narrative-title">
      <h4 id="incident-narrative-title">{t("incident.narrative.title")}</h4>
      {lifecycle.length === 0 ? (
        <EmptyState titleKey="incident.narrative.empty" />
      ) : (
        <Table
          captionKey="incident.narrative.caption"
          columns={columns}
          rows={lifecycle.map((event) => ({
            id: event.id,
            time: (
              <time dateTime={event.occurred_at}>
                {new Date(event.occurred_at).toLocaleString()}
              </time>
            ),
            actor: <span className="incident-narrative__actor">{event.actor_id}</span>,
            change: changeCell(event.payload, t),
            reason: event.reason
          }))}
        />
      )}
    </section>
  );
}
