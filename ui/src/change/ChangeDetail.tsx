import type { ChangeEvent } from "../../contracts/ipc";
import { useTranslation } from "../i18n";
import "./change.css";

const actorKindKey = (kind: ChangeEvent["actor"]["kind"]) => "change.actorKinds." + kind;
const targetKindKey = (kind: ChangeEvent["targets"][number]["kind"]) =>
  "change.targetKinds." + kind;

/**
 * Source record behind one change. Diff bodies never reach the frontend, so
 * the panel states where the diff is read instead of presenting an empty
 * in-app diff viewer.
 */
export function ChangeDetail({
  event,
  onOpenEvidence
}: {
  event: ChangeEvent;
  onOpenEvidence: (subject: string, evidenceIds: string[]) => void;
}) {
  const { t } = useTranslation();
  const none = t("change.fields.none");
  return (
    <section className="change-detail" role="region" aria-label={t("change.detailTitle")}>
      <div className="change-section-heading">
        <div>
          <p className="eyebrow">{t("change.detailTitle")}</p>
          <h2>
            {t("change.sources." + event.source)} · {t("change.kinds." + event.kind)}
          </h2>
        </div>
        <button
          type="button"
          className="change-detail__evidence"
          onClick={() => onOpenEvidence(t("change.sources." + event.source), event.evidence_ids)}
        >
          {t("change.openEvidence")}
        </button>
      </div>

      <dl className="change-detail__fields">
        <div>
          <dt>{t("change.fields.occurredAt")}</dt>
          <dd>{event.occurred_at}</dd>
        </div>
        <div>
          <dt>{t("change.fields.actor")}</dt>
          <dd>
            {event.actor.handle ?? none} ({t(actorKindKey(event.actor.kind))})
          </dd>
        </div>
        <div>
          <dt>{t("change.fields.target")}</dt>
          <dd>
            {event.targets.length > 0
              ? event.targets
                  .map((target) => t(targetKindKey(target.kind)) + ": " + target.id)
                  .join(" · ")
              : none}
          </dd>
        </div>
        <div>
          <dt>{t("change.fields.environment")}</dt>
          <dd>{event.environment ?? none}</dd>
        </div>
        <div>
          <dt>{t("change.fields.revision")}</dt>
          <dd>{event.revision ? (event.revision.short_id ?? event.revision.id) : none}</dd>
        </div>
        {event.revision && event.revision.parent_ids.length > 0 && (
          <div>
            <dt>{t("change.fields.parents")}</dt>
            <dd>{event.revision.parent_ids.join(" · ")}</dd>
          </div>
        )}
        <div>
          <dt>{t("change.fields.repository")}</dt>
          <dd>
            {event.repository
              ? [event.repository.host, event.repository.namespace, event.repository.name]
                  .filter((part): part is string => Boolean(part))
                  .join("/")
              : none}
          </dd>
        </div>
        {event.repository?.reference && (
          <div>
            <dt>{t("change.fields.reference")}</dt>
            <dd>{event.repository.reference}</dd>
          </div>
        )}
        <div>
          <dt>{t("change.fields.diffStat")}</dt>
          <dd>
            {event.diff_stat
              ? t("change.diffStat", {
                  files: event.diff_stat.files_changed,
                  insertions: event.diff_stat.insertions,
                  deletions: event.diff_stat.deletions
                })
              : none}
          </dd>
        </div>
        <div>
          <dt>{t("change.fields.changedPaths")}</dt>
          <dd>
            {event.changed_paths.length > 0 ? (
              <ul className="change-detail__paths">
                {event.changed_paths.map((path) => (
                  <li key={path}>{path}</li>
                ))}
              </ul>
            ) : (
              none
            )}
          </dd>
        </div>
        <div>
          <dt>{t("change.fields.nativeId")}</dt>
          <dd>{event.source_record.native_id ?? none}</dd>
        </div>
      </dl>

      <p className="change-detail__diff-notice">{t("change.diffNotice")}</p>

      {event.source_link ? (
        <a
          className="change-detail__link"
          href={event.source_link.url}
          target="_blank"
          rel="noreferrer noopener"
        >
          {t("change.openSource")}
        </a>
      ) : (
        <p className="change-detail__link change-detail__link--missing">{t("change.noLink")}</p>
      )}
    </section>
  );
}
