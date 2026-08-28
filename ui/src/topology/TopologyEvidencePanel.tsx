import type { ConsoleEvidenceId, EvidenceRef } from "../../contracts/ipc";
import { useTranslation } from "../i18n";

/**
 * Resolves requested evidence IDs against the snapshot's admitted evidence
 * set. Resolution is all-or-nothing, mirroring the backend contract: an
 * unknown ID yields the unavailable state instead of a partial result.
 */
export function TopologyEvidencePanel({
  subject,
  requestedIds,
  evidence
}: {
  subject: string;
  requestedIds: ConsoleEvidenceId[];
  evidence: EvidenceRef[];
}) {
  const { t } = useTranslation();
  const uniqueIds = [...new Set(requestedIds)];
  const evidenceById = new Map(evidence.map((item) => [item.id, item]));
  const resolved = uniqueIds.length > 0 ? uniqueIds.map((id) => evidenceById.get(id)) : [];
  const complete = resolved.length > 0 && resolved.every((item) => item !== undefined);

  return (
    <div className="topology-evidence">
      <p className="topology-evidence__context">{t("topology.evidence.context", { subject })}</p>
      {!complete ? (
        <p role="alert" className="topology-evidence__error">
          {t("topology.evidence.unavailable")}
        </p>
      ) : (
        <div className="topology-evidence__list">
          {(resolved as EvidenceRef[]).map((item) => (
            <article key={item.id} className="topology-evidence__entry">
              <div className="topology-evidence__entry-header">
                <h3>{t(`topology.sources.${item.source_kind}`)}</h3>
                <span className="topology-evidence__entry-id">{item.id}</span>
              </div>
              <dl>
                {item.connector_id && (
                  <div>
                    <dt>{t("topology.evidence.connector")}</dt>
                    <dd>{item.connector_id}</dd>
                  </div>
                )}
                <div>
                  <dt>{t("topology.evidence.endpoint")}</dt>
                  <dd>
                    <code>{item.endpoint}</code>
                  </dd>
                </div>
                {item.query && (
                  <div>
                    <dt>{t("topology.evidence.query")}</dt>
                    <dd>
                      <code>{item.query}</code>
                    </dd>
                  </div>
                )}
                <div>
                  <dt>{t("topology.evidence.observedAt")}</dt>
                  <dd>{item.observed_at}</dd>
                </div>
                <div>
                  <dt>{t("topology.evidence.excerpt")}</dt>
                  <dd>{item.excerpt}</dd>
                </div>
              </dl>
              <p className="topology-evidence__redaction" role="status">
                {item.redaction.masked
                  ? t("topology.evidence.masked")
                  : t("topology.evidence.notMasked")}{" "}
                ·{" "}
                {item.redaction.unparsed
                  ? t("topology.evidence.unparsed")
                  : t("topology.evidence.parsed")}
              </p>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
