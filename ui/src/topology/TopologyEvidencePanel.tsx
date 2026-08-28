import type { EvidenceRef } from "../../contracts/ipc";
import { isTrustedNativeUrl } from "../../contracts/guards";
import { open } from "@tauri-apps/plugin-shell";
import { EmptyState } from "../design-system/components";
import { useTranslation } from "../i18n";

export type TopologyEvidenceState = "idle" | "loading" | "ready" | "error";

/**
 * Renders evidence resolved through the `topology.evidence` IPC command.
 * The workspace only requests ids the backend snapshot issued, and the
 * backend admits or rejects the request as a whole, so a ready panel always
 * shows the complete evidence set for the selection.
 */
export function TopologyEvidencePanel({
  subject,
  evidenceState,
  evidence,
  errorMessage
}: {
  subject: string;
  evidenceState: TopologyEvidenceState;
  evidence: EvidenceRef[];
  errorMessage: string;
}) {
  const { t } = useTranslation();
  return (
    <div className="topology-evidence">
      <p className="topology-evidence__context">{t("topology.evidence.context", { subject })}</p>
      {evidenceState === "loading" && <p role="status">{t("topology.evidence.loading")}</p>}
      {evidenceState === "error" && (
        <p role="alert" className="topology-evidence__error">
          {errorMessage}
        </p>
      )}
      {evidenceState === "ready" && evidence.length === 0 && (
        <EmptyState titleKey="topology.evidence.empty" />
      )}
      {evidence.length > 0 && (
        <div className="topology-evidence__list">
          {evidence.map((item) => (
            <article key={item.id} className="topology-evidence__entry">
              <div className="topology-evidence__entry-header">
                <h3>{t(`topology.sources.${item.source_kind}`)}</h3>
                <span className="topology-evidence__entry-id">{item.id}</span>
              </div>
              {item.native_url !== null && isTrustedNativeUrl(item.native_url) && (
                <button
                  type="button"
                  className="topology-evidence__native-link"
                  onClick={() => {
                    if (item.native_url) {
                      void Promise.resolve(open(item.native_url)).catch(() => undefined);
                    }
                  }}
                >
                  {t("topology.evidence.openNative")}
                </button>
              )}
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
