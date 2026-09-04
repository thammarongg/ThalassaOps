// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { Incident, IncidentSeverity } from "../../contracts/ipc";
import { useTranslation } from "../i18n";

export type IncidentSummaryCardProps = {
  incident: Incident;
  onCopy: (markdown: string) => void | Promise<void>;
};

const effectiveSeverity = (incident: Incident): IncidentSeverity =>
  incident.severity_override?.selected ?? incident.derived_severity;

/**
 * Clipboard egress is deliberately an explicit field list. Do not replace
 * this with serialisation of the aggregate: new Incident fields must stay out
 * of the clipboard until they are reviewed for the summary allowlist.
 */
export const buildSummaryMarkdown = (incident: Incident): string => {
  const fields = [
    `- Incident ID: ${incident.id}`,
    `- Summary: ${incident.summary}`,
    `- Severity: ${effectiveSeverity(incident)}`,
    `- Derived severity: ${incident.derived_severity}`,
    `- Status: ${incident.status}`,
    `- Disposition: ${incident.disposition ?? "none"}`,
    `- Created at: ${incident.created_at}`,
    `- Updated at: ${incident.updated_at}`
  ];
  return `## Incident Summary\n${fields.join("\n")}`;
};

const Field = ({ label, value }: { label: string; value: string }) => (
  <div>
    <dt>{label}</dt>
    <dd>{value}</dd>
  </div>
);

export function IncidentSummaryCard({ incident, onCopy }: IncidentSummaryCardProps) {
  const { t } = useTranslation();
  const [copyState, setCopyState] = useState<"idle" | "copied" | "error">("idle");
  const severity = effectiveSeverity(incident);

  const copy = async () => {
    try {
      await onCopy(buildSummaryMarkdown(incident));
      setCopyState("copied");
    } catch {
      setCopyState("error");
    }
  };

  return (
    <section className="incident-summary-card" aria-labelledby="incident-summary-card-title">
      <div className="incident-summary-card__header">
        <h3 id="incident-summary-card-title">{t("incident.summary.title")}</h3>
        <button type="button" onClick={() => void copy()}>
          {t("incident.summary.copy")}
        </button>
      </div>
      <dl className="incident-summary-card__fields">
        <Field label={t("incident.summary.fields.id")} value={incident.id} />
        <Field label={t("incident.summary.fields.summary")} value={incident.summary} />
        <Field label={t("incident.summary.fields.severity")} value={severity} />
        <Field
          label={t("incident.summary.fields.derivedSeverity")}
          value={incident.derived_severity}
        />
        <Field
          label={t("incident.summary.fields.status")}
          value={t("incident.status." + incident.status)}
        />
        <Field
          label={t("incident.summary.fields.disposition")}
          value={incident.disposition ?? t("incident.summary.none")}
        />
        <Field label={t("incident.summary.fields.createdAt")} value={incident.created_at} />
        <Field label={t("incident.summary.fields.updatedAt")} value={incident.updated_at} />
      </dl>
      {copyState === "copied" && <p role="status">{t("incident.summary.copied")}</p>}
      {copyState === "error" && <p role="alert">{t("incident.summary.copyFailed")}</p>}
    </section>
  );
}
