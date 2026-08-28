import type { EvidenceRef } from "../../contracts/ipc";
import { isTrustedNativeUrl } from "../../contracts/guards";
import { open } from "@tauri-apps/plugin-shell";
import { EmptyState } from "../design-system/components";
import { useTranslation } from "../i18n";

export type CorrelationEvidenceState = "idle" | "loading" | "ready" | "error";

const sourceKey = (source: EvidenceRef["source_kind"]) => "correlation.sources." + source;

/** Evidence returned by the capability-scoped correlation.evidence command. */
export function CorrelationEvidencePanel({
  subject,
  evidenceState,
  evidence,
  errorMessage
}: {
  subject: string;
  evidenceState: CorrelationEvidenceState;
  evidence: EvidenceRef[];
  errorMessage: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="correlation-evidence">
      <p className="correlation-evidence__context">
        {t("correlation.evidence.context", { subject })}
      </p>
      {evidenceState === "loading" && <p role="status">{t("correlation.evidence.loading")}</p>}
      {evidenceState === "error" && (
        <p role="alert" className="correlation-evidence__error">
          {errorMessage}
        </p>
      )}
      {evidenceState === "ready" && evidence.length === 0 && (
        <EmptyState titleKey="correlation.evidence.empty" />
      )}
      {evidence.length > 0 && (
        <div className="correlation-evidence__list">
          {evidence.map((item) => (
            <article key={item.id} className="correlation-evidence__entry">
              <div className="correlation-evidence__entry-header">
                <h3>{t(sourceKey(item.source_kind))}</h3>
                <span className="correlation-evidence__entry-id">{item.id}</span>
              </div>
              {item.native_url !== null && isTrustedNativeUrl(item.native_url) && (
                <button
                  type="button"
                  className="correlation-evidence__native-link"
                  onClick={() => {
                    if (item.native_url) {
                      void Promise.resolve(open(item.native_url)).catch(() => undefined);
                    }
                  }}
                >
                  {t("correlation.evidence.openNative")}
                </button>
              )}
              <dl>
                {item.connector_id && (
                  <div>
                    <dt>{t("correlation.evidence.connector")}</dt>
                    <dd>{item.connector_id}</dd>
                  </div>
                )}
                <div>
                  <dt>{t("correlation.evidence.endpoint")}</dt>
                  <dd>
                    <code>{item.endpoint}</code>
                  </dd>
                </div>
                {item.query && (
                  <div>
                    <dt>{t("correlation.evidence.query")}</dt>
                    <dd>
                      <code>{item.query}</code>
                    </dd>
                  </div>
                )}
                <div>
                  <dt>{t("correlation.evidence.observedAt")}</dt>
                  <dd>{item.observed_at}</dd>
                </div>
                <div>
                  <dt>{t("correlation.evidence.excerpt")}</dt>
                  <dd>{item.excerpt}</dd>
                </div>
              </dl>
              <p className="correlation-evidence__redaction" role="status">
                {item.redaction.masked
                  ? t("correlation.evidence.masked")
                  : t("correlation.evidence.notMasked")}{" "}
                ·{" "}
                {item.redaction.unparsed
                  ? t("correlation.evidence.unparsed")
                  : t("correlation.evidence.parsed")}
              </p>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}
