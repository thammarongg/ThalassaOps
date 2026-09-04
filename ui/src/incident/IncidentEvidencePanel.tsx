// SPDX-License-Identifier: Apache-2.0

import { open } from "@tauri-apps/plugin-shell";
import type { EvidenceRef } from "../../contracts/ipc";
import { isTrustedNativeUrl } from "../../contracts/guards";
import { EmptyState } from "../design-system/components";
import { useTranslation } from "../i18n";
import type { EvidenceState } from "./incidentEvidence";

const sourceKey = (source: EvidenceRef["source_kind"]) => `incident.evidence.sources.${source}`;

/**
 * One resolved reference. `native_url` arrives from a source record, so it is
 * offered as a link only when the guard admits it — the same rule the topology
 * and correlation panels apply.
 */
function EvidenceEntry({ item }: { item: EvidenceRef }) {
  const { t } = useTranslation();
  const nativeUrl = isTrustedNativeUrl(item.native_url) ? item.native_url : null;
  return (
    <article className="incident-evidence__entry">
      <div className="incident-evidence__entry-header">
        <h4>{t(sourceKey(item.source_kind))}</h4>
        <span className="incident-evidence__entry-id">{item.id}</span>
      </div>
      {nativeUrl !== null && (
        <button
          type="button"
          className="incident-evidence__native-link"
          onClick={() => void Promise.resolve(open(nativeUrl)).catch(() => undefined)}
        >
          {t("incident.evidence.openNative")}
        </button>
      )}
      <dl>
        {item.connector_id !== null && (
          <div>
            <dt>{t("incident.evidence.connector")}</dt>
            <dd>{item.connector_id}</dd>
          </div>
        )}
        <div>
          <dt>{t("incident.evidence.endpoint")}</dt>
          <dd>
            <code>{item.endpoint}</code>
          </dd>
        </div>
        {item.query !== null && (
          <div>
            <dt>{t("incident.evidence.query")}</dt>
            <dd>
              <code>{item.query}</code>
            </dd>
          </div>
        )}
        <div>
          <dt>{t("incident.evidence.observedAt")}</dt>
          <dd>{item.observed_at}</dd>
        </div>
        <div>
          <dt>{t("incident.evidence.excerpt")}</dt>
          <dd>{item.excerpt}</dd>
        </div>
      </dl>
      <p className="incident-evidence__redaction" data-testid="incident-evidence-redaction">
        {item.redaction.masked ? t("incident.evidence.masked") : t("incident.evidence.notMasked")} ·{" "}
        {item.redaction.unparsed ? t("incident.evidence.unparsed") : t("incident.evidence.parsed")}
      </p>
    </article>
  );
}

/**
 * The four evidence states, each rendered as itself. A pure component: the
 * shell resolves the state and this only reports it, per the module boundary
 * rule. "Empty" and "unavailable" are never collapsed — one says the incident
 * has no associations of this kind, the other that the record could not be
 * read.
 */
export function IncidentEvidencePanel({ state }: { state: EvidenceState }) {
  const { t } = useTranslation();
  return (
    <section className="incident-evidence" aria-label={t("incident.evidence.title")}>
      {state.status === "loading" && (
        <p role="status" data-testid="incident-evidence-loading">
          {t("incident.evidence.loading")}
        </p>
      )}
      {state.status === "empty" && (
        <div data-testid="incident-evidence-empty">
          <EmptyState titleKey="incident.evidence.empty" />
        </div>
      )}
      {state.status === "unavailable" && (
        <p
          role="alert"
          className="incident-evidence__unavailable"
          data-testid="incident-evidence-unavailable"
        >
          {t(`incident.evidence.unavailable.${state.cause}`)}
        </p>
      )}
      {state.status === "ready" && (
        <div className="incident-evidence__list">
          {state.evidence.map((item) => (
            <EvidenceEntry key={item.id} item={item} />
          ))}
        </div>
      )}
    </section>
  );
}
