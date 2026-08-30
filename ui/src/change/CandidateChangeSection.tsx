import type { ChangeAssociation, ChangeEvent } from "../../contracts/ipc";
import { useTranslation } from "../i18n";
import "./change.css";

const qualificationKey = (qualification: ChangeAssociation["qualification"]) =>
  "change.candidate.qualification." + qualification;

const leadTimeMinutes = (seconds: number) => Math.round((seconds / 60) * 10) / 10;

/**
 * Changes that precede one correlation candidate.
 *
 * Every entry is a change that both preceded the candidate's first signal and
 * shares a target or topology path with it. The copy states precedence only;
 * the panel never claims a change caused the candidate.
 */
export function CandidateChangeSection({
  associations,
  events,
  onOpenEvidence
}: {
  associations: ChangeAssociation[];
  events: ChangeEvent[];
  onOpenEvidence: (subject: string, evidenceIds: string[]) => void;
}) {
  const { t } = useTranslation();
  const eventById = new Map(events.map((event) => [event.id, event]));
  const entries = associations
    .map((association) => ({ association, event: eventById.get(association.change_id) }))
    .filter(
      (entry): entry is { association: ChangeAssociation; event: ChangeEvent } =>
        entry.event !== undefined
    );

  return (
    <section
      className="change-candidate-section"
      role="region"
      aria-label={t("change.candidate.title")}
    >
      <div className="change-section-heading">
        <div>
          <h3>{t("change.candidate.title")}</h3>
          <p className="change-candidate-section__description">
            {t("change.candidate.description")}
          </p>
        </div>
      </div>

      {entries.length === 0 ? (
        <p className="change-candidate-section__empty">{t("change.candidate.empty")}</p>
      ) : (
        <ul className="change-candidate-section__list">
          {entries.map(({ association, event }) => (
            <li key={association.change_id} className="change-association">
              <p className="change-association__heading">
                <strong>{t("change.candidate.reason")}</strong>
                <span>{t("change.sources." + event.source)}</span>
                <span>{t("change.kinds." + event.kind)}</span>
              </p>
              <p className="change-association__meta">
                <span>{event.occurred_at}</span>
                <span>{t(qualificationKey(association.qualification))}</span>
                <span>
                  {t("change.candidate.leadTime", {
                    minutes: leadTimeMinutes(association.lead_time_seconds)
                  })}
                </span>
              </p>
              <p className="change-association__structure">
                {association.target
                  ? t("change.candidate.matchedTarget", { target: association.target.id })
                  : t("change.candidate.matchedPath", {
                      path: association.topology_path_ids.join(" · ")
                    })}
              </p>
              {event.source_link ? (
                <a
                  className="change-association__link"
                  href={event.source_link.url}
                  target="_blank"
                  rel="noreferrer noopener"
                >
                  {t("change.openSource")}
                </a>
              ) : (
                <p className="change-association__link change-association__link--missing">
                  {t("change.noLink")}
                </p>
              )}
              <button
                type="button"
                className="change-association__evidence"
                onClick={() =>
                  onOpenEvidence(t("change.candidate.reason"), association.evidence_ids)
                }
              >
                {t("change.openEvidence")}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
