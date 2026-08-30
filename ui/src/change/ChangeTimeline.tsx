import type { ChangeEvent, ChangeSnapshot } from "../../contracts/ipc";
import { EmptyState, StatusIndicator } from "../design-system/components";
import { useTranslation } from "../i18n";
import "./change.css";

const sourceKey = (source: ChangeEvent["source"]) => "change.sources." + source;
const kindKey = (kind: ChangeEvent["kind"]) => "change.kinds." + kind;
const outcomeKey = (outcome: ChangeEvent["outcome"]) => "change.outcomes." + outcome;
const actorKindKey = (kind: ChangeEvent["actor"]["kind"]) => "change.actorKinds." + kind;

const outcomeIndicator = (outcome: ChangeEvent["outcome"]) => {
  if (outcome === "succeeded") return "healthy" as const;
  if (outcome === "failed" || outcome === "reverted") return "degraded" as const;
  return "unknown" as const;
};

/**
 * Ordered lane of source-backed changes. Only events the snapshot placed in
 * the timeline are rendered, so a change without a retained record cannot
 * appear here, and truncation is stated rather than implied.
 */
export function ChangeTimeline({
  snapshot,
  selectedChangeId,
  onSelect
}: {
  snapshot: ChangeSnapshot;
  selectedChangeId: string | null;
  onSelect: (event: ChangeEvent) => void;
}) {
  const { t } = useTranslation();
  const eventById = new Map(snapshot.events.map((event) => [event.id, event]));
  const entries = snapshot.timeline.entry_ids
    .map((entryId) => eventById.get(entryId))
    .filter((event): event is ChangeEvent => event !== undefined);

  return (
    <section className="change-timeline" aria-labelledby="change-timeline-title">
      <div className="change-section-heading">
        <div>
          <p className="eyebrow">{t("change.eyebrow")}</p>
          <h2 id="change-timeline-title">{t("change.timelineTitle")}</h2>
          <p className="change-timeline__description">{t("change.timelineDescription")}</p>
        </div>
        <p className="change-timeline__window">
          {t("change.window", {
            start: snapshot.timeline.window.start,
            end: snapshot.timeline.window.end
          })}
        </p>
      </div>

      {snapshot.timeline.truncated && (
        <p className="change-timeline__truncated" role="status">
          {t("change.truncated", { count: entries.length })}
        </p>
      )}

      {entries.length === 0 ? (
        <EmptyState titleKey="change.empty">
          <p>{t("change.emptyDetail")}</p>
        </EmptyState>
      ) : (
        <ol className="change-timeline__list">
          {entries.map((event) => (
            <li key={event.id}>
              <button
                type="button"
                className={
                  event.id === selectedChangeId
                    ? "change-entry change-entry--selected"
                    : "change-entry"
                }
                aria-pressed={event.id === selectedChangeId}
                onClick={() => onSelect(event)}
              >
                <span className="change-entry__heading">
                  <strong>{t(sourceKey(event.source))}</strong>
                  <span>{t(kindKey(event.kind))}</span>
                  <span className="change-entry__outcome">
                    <StatusIndicator state={outcomeIndicator(event.outcome)} />{" "}
                    {t(outcomeKey(event.outcome))}
                  </span>
                </span>
                <span className="change-entry__meta">
                  <span>{event.occurred_at}</span>
                  <span>{event.actor.handle ?? t(actorKindKey(event.actor.kind))}</span>
                  <span>
                    {event.targets.length > 0
                      ? event.targets.map((target) => target.id).join(" · ")
                      : t("change.fields.none")}
                  </span>
                </span>
              </button>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
