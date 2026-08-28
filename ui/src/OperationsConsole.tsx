import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type {
  CommandEnvelope,
  ConsoleHealthState,
  ConsoleSeverity,
  CriticalNumber,
  DrillDownReference,
  DrillDownTarget,
  EvidenceRef,
  ImpactLevel,
  IncidentQueueItem,
  OperationsSnapshot,
  SourceStatus,
  StatusReason,
  WidgetDefinition,
  WidgetId,
  WidgetPreference,
  WidgetSize,
  NumberUnit,
  Invoke
} from "../contracts/ipc";
import { command } from "../contracts/ipc";
import { Card, Drawer, EmptyState, StatusIndicator } from "./design-system/components";
import { useTranslation } from "./i18n";
import {
  CURATED_WIDGET_DEFINITIONS,
  defaultWidgetPreferences,
  moveWidget,
  readWidgetPreferences,
  reconcileWidgetPreferences,
  persistWidgetPreferences,
  updateWidgetPreference
} from "./operations/widgetConfig";
import {
  isEvidenceResponse,
  isOperationsSnapshot,
  isTrustedNativeUrl
} from "./operations/contractValidation";
import { open } from "@tauri-apps/plugin-shell";

type SnapshotState = "loading" | "ready" | "error";
type EvidenceState = "idle" | "loading" | "ready" | "error";

type DrillDownSelection = {
  target: DrillDownTarget;
  reference: DrillDownReference;
  evidenceIds: string[];
};

const widgetTitleKey = (id: WidgetId) => `operations.${id}`;
const operationsEnvelope = <T,>(
  verb: "snapshot" | "evidence",
  capability: "WorkspaceRead" | "ResourceRead",
  payload: T
): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("operations", verb),
  capability,
  scope: { resource_ids: [] },
  payload
});

const healthIndicatorState = (state: ConsoleHealthState) => {
  if (state === "healthy") return "healthy" as const;
  if (state === "critical") return "critical" as const;
  if (state === "degraded") return "degraded" as const;
  return "unavailable" as const;
};

const impactIndicatorState = (impact: ImpactLevel) => {
  if (impact === "critical") return "critical" as const;
  if (impact === "unknown") return "unavailable" as const;
  if (impact === "high" || impact === "medium") return "degraded" as const;
  return "healthy" as const;
};

const severityIndicator = (severity: ConsoleSeverity) =>
  `s${severity.slice(1)}` as "s1" | "s2" | "s3" | "s4" | "s5";

const statusReasonKey = (reason: StatusReason | null) =>
  reason ? `operations.reasons.${reason}` : "operations.reasons.unknown";

const criticalNumberLabelKey = (number: CriticalNumber, category: "severity" | "environment") => {
  const suffix = number.key.split(".").at(-1)?.toLowerCase();
  const severityKey = suffix && /^s[1-5]$/.test(suffix) ? `active_${suffix}` : suffix;
  if (category === "severity" && severityKey && /^active_s[1-5]$/.test(severityKey)) {
    return `operations.severityTotals.${severityKey}`;
  }
  const knownSuffixes = ["critical", "degraded", "healthy", "unknown"];
  if (suffix && knownSuffixes.includes(suffix)) {
    return `operations.${category}Totals.${suffix}`;
  }
  return category === "severity" ? "operations.severityTotal" : "operations.environmentTotal";
};

const criticalNumberUnitKey = (unit: NumberUnit) => `operations.units.${unit}`;

const requiredWidget = (id: WidgetId) => id === "health_summary" || id === "incident_queue";

const uniqueIssuedEvidenceIds = (ids: string[], issuedIds: Set<string>): string[] => [
  ...new Set(ids.filter((id) => issuedIds.has(id)))
];

const sourceKeysByWidget: Record<WidgetId, string[]> = {
  health_summary: [],
  incident_queue: [
    "alertmanager",
    "prometheus",
    "health_checks",
    "environment_status",
    "environment:",
    "cloud:",
    "aws:",
    "azure:",
    "gcp:"
  ],
  signal_summary: ["alertmanager", "prometheus", "health_checks"],
  change_stream: ["changes"],
  environment_status: ["environment_status", "environment:", "cloud:", "aws:", "azure:", "gcp:"]
};

const sourceNoticesFor = (snapshot: OperationsSnapshot, widgetId: WidgetId) => {
  const sourceKeys = sourceKeysByWidget[widgetId];
  return snapshot.source_status.filter(
    (source) =>
      source.state !== "fresh" &&
      (!sourceKeys.length || sourceKeys.some((key) => source.source_key.startsWith(key)))
  );
};

function SourceNotice({ source }: { source: SourceStatus }) {
  const { t } = useTranslation();
  const state = source.state === "stale" ? "degraded" : "unavailable";
  const role = source.state === "unavailable" || source.state === "unverified" ? "alert" : "status";
  return (
    <p className="operations-source-notice" role={role}>
      <StatusIndicator state={state} />{" "}
      <span>
        {t("operations.sourceUnavailable", {
          source: source.source_key,
          state: t(`operations.sourceStates.${source.state}`),
          reason: t(statusReasonKey(source.reason))
        })}
      </span>
      {source.detail && <span className="operations-source-notice__detail">{source.detail}</span>}
    </p>
  );
}

function SourceNotices({
  snapshot,
  widgetId
}: {
  snapshot: OperationsSnapshot;
  widgetId: WidgetId;
}) {
  const notices = sourceNoticesFor(snapshot, widgetId);
  if (!notices.length) return null;
  return (
    <div className="operations-source-notices">
      {notices.map((source) => (
        <SourceNotice key={`${source.source_key}-${source.state}`} source={source} />
      ))}
    </div>
  );
}

function WidgetFrame({
  definition,
  preference,
  snapshotState,
  errorMessage,
  children,
  onExpand
}: {
  definition: WidgetDefinition;
  preference: WidgetPreference;
  snapshotState: SnapshotState;
  errorMessage: string;
  children: ReactNode;
  onExpand: () => void;
}) {
  const { t } = useTranslation();
  const titleKey = widgetTitleKey(definition.id);
  return (
    <div
      className={`operations-widget operations-widget--${preference.size}`}
      data-widget-id={definition.id}
    >
      <Card titleKey={titleKey}>
        {preference.collapsed ? (
          <button type="button" className="operations-widget__expand" onClick={onExpand}>
            {t("operations.expandWidget")}
          </button>
        ) : snapshotState === "loading" ? (
          <p className="operations-widget-state" role="status">
            {t("operations.widgetLoading")}
          </p>
        ) : snapshotState === "error" ? (
          <p className="operations-widget-state operations-widget-state--error" role="alert">
            {errorMessage}
          </p>
        ) : (
          children
        )}
      </Card>
    </div>
  );
}

function CriticalNumberLink({
  number,
  labelKey,
  issuedEvidenceIds,
  onOpen
}: {
  number: CriticalNumber;
  labelKey: string;
  issuedEvidenceIds: Set<string>;
  onOpen: (target: DrillDownTarget, reference: DrillDownReference, ids: string[]) => void;
}) {
  const { t } = useTranslation();
  const label = t(labelKey);
  const unit = t(criticalNumberUnitKey(number.unit));
  const evidenceIds = uniqueIssuedEvidenceIds(number.evidence_ids, issuedEvidenceIds);
  return (
    <div
      className="operations-critical-number"
      data-testid="operations-critical-number"
      data-number-key={number.key}
    >
      {evidenceIds.length ? (
        <button
          type="button"
          className="operations-critical-number__button"
          data-evidence-ids={JSON.stringify(evidenceIds)}
          aria-label={t("operations.openDrillDown", { label, value: number.value })}
          onClick={() => onOpen(number.drill_down, number.drill_down_reference, evidenceIds)}
        >
          <span className="operations-critical-number__value">
            {number.value}
            {number.unit !== "count" && unit}
          </span>{" "}
          <span className="operations-critical-number__label">{label}</span>
          <span className="operations-critical-number__affordance" aria-hidden="true">
            ↗
          </span>
        </button>
      ) : (
        <span className="operations-critical-number__unavailable" role="status">
          <StatusIndicator state="unavailable" /> {t("operations.numberUnavailable")}
        </span>
      )}
    </div>
  );
}

function ItemDrillDownButton({
  label,
  target,
  reference,
  issuedEvidenceIds,
  onOpen
}: {
  label: string;
  target: DrillDownTarget;
  reference: DrillDownReference;
  issuedEvidenceIds: Set<string>;
  onOpen: (target: DrillDownTarget, reference: DrillDownReference, ids: string[]) => void;
}) {
  const { t } = useTranslation();
  const evidenceIds = uniqueIssuedEvidenceIds(
    [...target.evidence_ids, ...reference.evidence_ids],
    issuedEvidenceIds
  );
  if (!evidenceIds.length) {
    return (
      <span className="operations-item-drilldown__unavailable" role="status">
        {t("operations.numberUnavailable")}
      </span>
    );
  }
  return (
    <button
      type="button"
      className="operations-item-drilldown"
      aria-label={t("operations.openItemEvidence", { label })}
      onClick={() => onOpen(target, reference, evidenceIds)}
    >
      {t("operations.openEvidence")}
    </button>
  );
}

function HealthSummaryWidget({
  snapshot,
  issuedEvidenceIds,
  onOpen
}: {
  snapshot: OperationsSnapshot;
  issuedEvidenceIds: Set<string>;
  onOpen: (target: DrillDownTarget, reference: DrillDownReference, ids: string[]) => void;
}) {
  const { t } = useTranslation();
  const { health_summary: summary } = snapshot;
  return (
    <div className="operations-health-summary">
      <SourceNotices snapshot={snapshot} widgetId="health_summary" />
      <div className={`operations-health-headline operations-health-headline--${summary.state}`}>
        <StatusIndicator state={healthIndicatorState(summary.state)} />
        <div>
          <h3>{summary.headline.summary}</h3>
          <p>{summary.headline.customer_scope}</p>
          <p className="operations-health-headline__meta">
            {t("operations.serviceCriticality")}: {summary.headline.service_criticality} ·{" "}
            {t("operations.trajectory")}:{" "}
            {t(`operations.trajectories.${summary.headline.trajectory}`)}
          </p>
        </div>
      </div>
      <div className="operations-number-grid operations-number-grid--primary">
        <CriticalNumberLink
          number={summary.attention}
          labelKey="operations.attention"
          issuedEvidenceIds={issuedEvidenceIds}
          onOpen={onOpen}
        />
        <CriticalNumberLink
          number={summary.impacted_services}
          labelKey="operations.impactedServices"
          issuedEvidenceIds={issuedEvidenceIds}
          onOpen={onOpen}
        />
      </div>
      <div className="operations-health-breakdown">
        <div>
          <h3>{t("operations.activeBySeverity")}</h3>
          <div className="operations-number-list">
            {summary.active_by_severity.map((number) => (
              <CriticalNumberLink
                key={number.key}
                number={number}
                labelKey={criticalNumberLabelKey(number, "severity")}
                issuedEvidenceIds={issuedEvidenceIds}
                onOpen={onOpen}
              />
            ))}
          </div>
        </div>
        <div>
          <h3>{t("operations.environmentsByState")}</h3>
          <div className="operations-number-list">
            {summary.environments_by_state.map((number) => (
              <CriticalNumberLink
                key={number.key}
                number={number}
                labelKey={criticalNumberLabelKey(number, "environment")}
                issuedEvidenceIds={issuedEvidenceIds}
                onOpen={onOpen}
              />
            ))}
          </div>
        </div>
      </div>
      {summary.contributing_scopes.length > 0 && (
        <div className="operations-contributing-scopes">
          <h3>{t("operations.contributingScopes")}</h3>
          <ul>
            {summary.contributing_scopes.map((scope) => (
              <li key={`${scope.impact}-${scope.summary}`}>
                <StatusIndicator state={impactIndicatorState(scope.impact)} />{" "}
                <span>{scope.summary}</span>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function IncidentQueueWidget({
  snapshot,
  issuedEvidenceIds,
  onOpen
}: {
  snapshot: OperationsSnapshot;
  issuedEvidenceIds: Set<string>;
  onOpen: (target: DrillDownTarget, reference: DrillDownReference, ids: string[]) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="operations-incident-queue">
      <SourceNotices snapshot={snapshot} widgetId="incident_queue" />
      {snapshot.incident_queue.length === 0 ? (
        <EmptyState titleKey="operations.noActiveIncidents" />
      ) : (
        <div className="operations-incident-list">
          {snapshot.incident_queue.map((item) => (
            <IncidentQueueEntry
              key={item.id}
              item={item}
              issuedEvidenceIds={issuedEvidenceIds}
              onOpen={onOpen}
            />
          ))}
        </div>
      )}
      <p className="operations-widget-caption">{t("operations.queueImpactHint")}</p>
    </div>
  );
}

function IncidentQueueEntry({
  item,
  issuedEvidenceIds,
  onOpen
}: {
  item: IncidentQueueItem;
  issuedEvidenceIds: Set<string>;
  onOpen: (target: DrillDownTarget, reference: DrillDownReference, ids: string[]) => void;
}) {
  const { t } = useTranslation();
  return (
    <article className="operations-incident-entry">
      <div className="operations-incident-entry__topline">
        <StatusIndicator severity={severityIndicator(item.severity)} />
        {item.priority && <span className="operations-priority">{item.priority}</span>}
        <span className="operations-incident-status">
          {t(`operations.queueStatuses.${item.status}`)}
        </span>
      </div>
      <h3>{item.title}</h3>
      <p>{item.business_impact.summary}</p>
      <p className="operations-incident-entry__meta">
        {t("operations.customerImpact")}: {item.business_impact.customer_scope}
      </p>
      <ItemDrillDownButton
        label={item.title}
        target={item.drill_down}
        reference={item.drill_down_reference}
        issuedEvidenceIds={issuedEvidenceIds}
        onOpen={onOpen}
      />
    </article>
  );
}

function SignalSummaryWidget({
  snapshot,
  issuedEvidenceIds,
  onOpen
}: {
  snapshot: OperationsSnapshot;
  issuedEvidenceIds: Set<string>;
  onOpen: (target: DrillDownTarget, reference: DrillDownReference, ids: string[]) => void;
}) {
  const { t } = useTranslation();
  const signal = snapshot.signal_summary;
  const numbers = [
    { number: signal.active_alerts, labelKey: "operations.activeAlerts" },
    { number: signal.active_anomalies, labelKey: "operations.activeAnomalies" },
    { number: signal.checks_due, labelKey: "operations.checksDue" },
    { number: signal.checks_timed_out, labelKey: "operations.checksTimedOut" }
  ];
  return (
    <div className="operations-signal-summary">
      <SourceNotices snapshot={snapshot} widgetId="signal_summary" />
      <div className="operations-number-grid">
        {numbers.map(({ number, labelKey }) => (
          <CriticalNumberLink
            key={number.key}
            number={number}
            labelKey={labelKey}
            issuedEvidenceIds={issuedEvidenceIds}
            onOpen={onOpen}
          />
        ))}
      </div>
      {signal.by_source.length > 0 && (
        <div className="operations-signal-sources">
          <h3>{t("operations.signalsBySource")}</h3>
          <div className="operations-number-list">
            {signal.by_source.map((source) => (
              <CriticalNumberLink
                key={source.count.key}
                number={source.count}
                labelKey={`operations.sourceKinds.${source.source_kind}`}
                issuedEvidenceIds={issuedEvidenceIds}
                onOpen={onOpen}
              />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function ChangeStreamWidget({
  snapshot,
  issuedEvidenceIds,
  onOpen
}: {
  snapshot: OperationsSnapshot;
  issuedEvidenceIds: Set<string>;
  onOpen: (target: DrillDownTarget, reference: DrillDownReference, ids: string[]) => void;
}) {
  const { t } = useTranslation();
  const status = snapshot.change_stream_status;
  return (
    <div className="operations-change-stream">
      <SourceNotices snapshot={snapshot} widgetId="change_stream" />
      {status.state === "unavailable" ? (
        <p className="operations-widget-state operations-widget-state--error" role="alert">
          <StatusIndicator state="unavailable" />{" "}
          {t("operations.changeStreamUnavailable", { reason: t(statusReasonKey(status.reason)) })}
          {status.detail && (
            <span className="operations-source-notice__detail">{status.detail}</span>
          )}
        </p>
      ) : status.state === "empty" || snapshot.changes.length === 0 ? (
        <EmptyState titleKey="operations.noRecentChanges" />
      ) : (
        <ol className="operations-change-list">
          {snapshot.changes.map((change) => (
            <li key={change.id} className="operations-change-entry">
              <div>
                <p className="operations-change-entry__time">{change.occurred_at}</p>
                <h3>{change.summary}</h3>
                <p>
                  {change.target_resource ?? t("operations.unknownTarget")} ·{" "}
                  {change.actor ?? t("operations.unknownActor")}
                </p>
              </div>
              <ItemDrillDownButton
                label={change.summary}
                target={change.drill_down}
                reference={{
                  source_query: "operations:change",
                  scope: change.scope,
                  time_window: null,
                  evidence_ids: change.evidence_ids
                }}
                issuedEvidenceIds={issuedEvidenceIds}
                onOpen={onOpen}
              />
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}

function EnvironmentStatusWidget({
  snapshot,
  issuedEvidenceIds,
  onOpen
}: {
  snapshot: OperationsSnapshot;
  issuedEvidenceIds: Set<string>;
  onOpen: (target: DrillDownTarget, reference: DrillDownReference, ids: string[]) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="operations-environment-status">
      <SourceNotices snapshot={snapshot} widgetId="environment_status" />
      {snapshot.environments.length === 0 ? (
        <EmptyState titleKey="operations.noEnvironments" />
      ) : (
        <div className="operations-environment-list">
          {snapshot.environments.map((environment) => (
            <article key={environment.environment_id} className="operations-environment-entry">
              <div className="operations-environment-entry__header">
                <div>
                  <h3>{environment.name}</h3>
                  <p className="operations-environment-entry__provider">
                    {environment.provider ?? t("operations.unknownProvider")}
                  </p>
                </div>
                <StatusIndicator state={healthIndicatorState(environment.health)} />
              </div>
              <p>{environment.status_detail}</p>
              <CriticalNumberLink
                number={environment.resource_count}
                labelKey="operations.resources"
                issuedEvidenceIds={issuedEvidenceIds}
                onOpen={onOpen}
              />
              <p className="operations-environment-entry__observed">
                {t("operations.lastObserved")}: {environment.last_observed_at}
              </p>
              <ItemDrillDownButton
                label={environment.name}
                target={environment.drill_down}
                reference={{
                  source_query: "operations:environment",
                  scope: snapshot.scope,
                  time_window: null,
                  evidence_ids: environment.evidence_ids
                }}
                issuedEvidenceIds={issuedEvidenceIds}
                onOpen={onOpen}
              />
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

function WidgetSettings({
  definitions,
  preferences,
  onToggle,
  onMove,
  onSize,
  onCollapse,
  onReset
}: {
  definitions: WidgetDefinition[];
  preferences: WidgetPreference[];
  onToggle: (id: WidgetId) => void;
  onMove: (id: WidgetId, direction: "up" | "down") => void;
  onSize: (id: WidgetId, size: WidgetSize) => void;
  onCollapse: (id: WidgetId) => void;
  onReset: () => void;
}) {
  const { t } = useTranslation();
  const ordered = [...preferences].sort((left, right) => left.order - right.order);
  const titleFor = (id: WidgetId) => t(widgetTitleKey(id));
  return (
    <div className="operations-widget-settings">
      <p>{t("operations.customizeHint")}</p>
      <ol className="operations-widget-settings__list">
        {ordered.map((preference, index) => {
          const definition = definitions.find((item) => item.id === preference.id);
          if (!definition) return null;
          const title = titleFor(preference.id);
          const isRequired = requiredWidget(preference.id);
          return (
            <li key={preference.id} className="operations-widget-settings__item">
              <div className="operations-widget-settings__main">
                <label>
                  <input
                    type="checkbox"
                    checked={preference.visible}
                    disabled={isRequired}
                    onChange={() => onToggle(preference.id)}
                  />{" "}
                  {t("operations.showWidget", { widget: title })}
                </label>
                <label>
                  {t("operations.widgetSize", { widget: title })}{" "}
                  <select
                    value={preference.size}
                    aria-label={t("operations.widgetSize", { widget: title })}
                    onChange={(event) => onSize(preference.id, event.target.value as WidgetSize)}
                  >
                    <option value="compact">{t("operations.sizes.compact")}</option>
                    <option value="standard">{t("operations.sizes.standard")}</option>
                    <option value="wide">{t("operations.sizes.wide")}</option>
                  </select>
                </label>
                <label>
                  <input
                    type="checkbox"
                    checked={preference.collapsed}
                    onChange={() => onCollapse(preference.id)}
                  />{" "}
                  {t("operations.collapseWidget", { widget: title })}
                </label>
              </div>
              <div className="operations-widget-settings__order">
                <button
                  type="button"
                  aria-label={t("operations.moveWidgetUp", { widget: title })}
                  disabled={
                    index === 0 ||
                    isRequired ||
                    (ordered[index - 1] ? requiredWidget(ordered[index - 1].id) : false)
                  }
                  onClick={() => onMove(preference.id, "up")}
                >
                  ↑
                </button>
                <button
                  type="button"
                  aria-label={t("operations.moveWidgetDown", { widget: title })}
                  disabled={index === ordered.length - 1 || isRequired}
                  onClick={() => onMove(preference.id, "down")}
                >
                  ↓
                </button>
              </div>
            </li>
          );
        })}
      </ol>
      <button type="button" onClick={onReset}>
        {t("operations.resetLayout")}
      </button>
    </div>
  );
}

function EvidencePanel({
  selection,
  evidenceState,
  evidence,
  errorMessage,
  onOpenNative
}: {
  selection?: DrillDownSelection;
  evidenceState: EvidenceState;
  evidence: EvidenceRef[];
  errorMessage: string;
  onOpenNative: (url: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="operations-evidence-panel">
      {selection && (
        <div className="operations-evidence-panel__context">
          <p>
            {t("operations.destination")}:{" "}
            {t(`operations.destinations.${selection.target.destination}`)}
          </p>
          <p>
            {t("operations.sourceQuery")}: <code>{selection.reference.source_query}</code>
          </p>
          {selection.reference.time_window && (
            <p>
              {t("operations.timeWindow")}: {selection.reference.time_window.start} →{" "}
              {selection.reference.time_window.end}
            </p>
          )}
        </div>
      )}
      {evidenceState === "loading" && <p role="status">{t("operations.evidenceLoading")}</p>}
      {evidenceState === "error" && (
        <p role="alert" className="error">
          {errorMessage}
        </p>
      )}
      {evidenceState === "ready" && evidence.length === 0 && (
        <EmptyState titleKey="operations.noEvidence" />
      )}
      {evidence.length > 0 && (
        <div className="operations-evidence-list">
          {evidence.map((item) => (
            <article key={item.id} className="operations-evidence-entry">
              <div className="operations-evidence-entry__header">
                <h3>{t(`operations.sources.${item.source_kind}`)}</h3>
                <span className="operations-evidence-entry__id">{item.id}</span>
              </div>
              <dl>
                {item.connector_id && (
                  <div>
                    <dt>{t("operations.connector")}</dt>
                    <dd>{item.connector_id}</dd>
                  </div>
                )}
                <div>
                  <dt>{t("operations.endpoint")}</dt>
                  <dd>
                    <code>{item.endpoint}</code>
                  </dd>
                </div>
                {item.query && (
                  <div>
                    <dt>{t("operations.query")}</dt>
                    <dd>
                      <code>{item.query}</code>
                    </dd>
                  </div>
                )}
                <div>
                  <dt>{t("operations.observedAt")}</dt>
                  <dd>{item.observed_at}</dd>
                </div>
                <div>
                  <dt>{t("operations.excerpt")}</dt>
                  <dd>{item.excerpt}</dd>
                </div>
              </dl>
              {item.native_url && isTrustedNativeUrl(item.native_url) && (
                <button type="button" onClick={() => onOpenNative(item.native_url as string)}>
                  {t("operations.openTrustedSource")}
                </button>
              )}
              <p className="operations-evidence-entry__redaction" role="status">
                {item.redaction.masked ? t("operations.masked") : t("operations.notMasked")} ·{" "}
                {item.redaction.unparsed ? t("operations.unparsed") : t("operations.parsed")}
              </p>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

export function OperationsConsole({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<OperationsSnapshot>();
  const [snapshotState, setSnapshotState] = useState<SnapshotState>("loading");
  const [snapshotError, setSnapshotError] = useState("");
  const [preferences, setPreferences] = useState<WidgetPreference[]>(() =>
    readWidgetPreferences(CURATED_WIDGET_DEFINITIONS)
  );
  const [layoutOpen, setLayoutOpen] = useState(false);
  const [evidenceOpen, setEvidenceOpen] = useState(false);
  const [evidenceState, setEvidenceState] = useState<EvidenceState>("idle");
  const [evidence, setEvidence] = useState<EvidenceRef[]>([]);
  const [evidenceError, setEvidenceError] = useState("");
  const [selection, setSelection] = useState<DrillDownSelection>();
  const drillDownRequestRef = useRef(0);

  const definitions = snapshot?.widget_registry?.length
    ? snapshot.widget_registry
    : CURATED_WIDGET_DEFINITIONS;
  const layout = useMemo(
    () => reconcileWidgetPreferences(definitions, preferences),
    [definitions, preferences]
  );
  const issuedEvidenceIds = useMemo(
    () => new Set(snapshot?.evidence.map((item) => item.id) ?? []),
    [snapshot]
  );

  useEffect(() => {
    let active = true;
    setSnapshotState("loading");
    setSnapshotError("");
    void invoke<null, OperationsSnapshot>("operations_snapshot", {
      envelope: operationsEnvelope("snapshot", "WorkspaceRead", null)
    })
      .then((result) => {
        if (!active) return;
        if (result.ok && isOperationsSnapshot(result.value)) {
          setSnapshot(result.value);
          setSnapshotState("ready");
          setPreferences((current) =>
            reconcileWidgetPreferences(result.value.widget_registry, current)
          );
        } else {
          setSnapshot(undefined);
          setSnapshotState("error");
          setSnapshotError(t("operations.snapshotError"));
        }
      })
      .catch(() => {
        if (!active) return;
        setSnapshot(undefined);
        setSnapshotState("error");
        setSnapshotError(t("operations.snapshotError"));
      });
    return () => {
      active = false;
    };
  }, [invoke, t]);

  const persistLayout = useCallback((next: WidgetPreference[]) => {
    setPreferences(next);
    persistWidgetPreferences(next);
  }, []);

  const updateLayout = useCallback(
    (update: (current: WidgetPreference[]) => WidgetPreference[]) => {
      setPreferences((current) => {
        const updated = reconcileWidgetPreferences(definitions, update(current));
        persistWidgetPreferences(updated);
        return updated;
      });
    },
    [definitions]
  );

  const toggleWidget = (id: WidgetId) =>
    updateLayout((current) =>
      updateWidgetPreference(
        current,
        id,
        (preference) => ({ ...preference, visible: !preference.visible }),
        definitions
      )
    );
  const move = (id: WidgetId, direction: "up" | "down") => {
    setPreferences((current) => {
      const next = moveWidget(current, id, direction, definitions);
      persistWidgetPreferences(next);
      return next;
    });
  };
  const setSize = (id: WidgetId, size: WidgetSize) =>
    updateLayout((current) =>
      updateWidgetPreference(current, id, (preference) => ({ ...preference, size }), definitions)
    );
  const toggleCollapse = (id: WidgetId) =>
    updateLayout((current) =>
      updateWidgetPreference(
        current,
        id,
        (preference) => ({ ...preference, collapsed: !preference.collapsed }),
        definitions
      )
    );
  const resetLayout = () => persistLayout(defaultWidgetPreferences(definitions));

  const openDrillDown = useCallback(
    (target: DrillDownTarget, reference: DrillDownReference, evidenceIds: string[]) => {
      const requestId = ++drillDownRequestRef.current;
      const ids = uniqueIssuedEvidenceIds(evidenceIds, issuedEvidenceIds);
      setSelection({ target, reference, evidenceIds: ids });
      setEvidence([]);
      setEvidenceError("");
      setEvidenceOpen(true);
      if (!ids.length) {
        setEvidenceState("error");
        setEvidenceError(t("operations.numberUnavailable"));
        return;
      }
      setEvidenceState("loading");
      void invoke<{ evidence_ids: string[] }, EvidenceRef[]>("operations_evidence", {
        envelope: operationsEnvelope("evidence", "ResourceRead", { evidence_ids: ids })
      })
        .then((result) => {
          if (requestId !== drillDownRequestRef.current) return;
          if (result.ok && isEvidenceResponse(result.value, ids)) {
            setEvidence(result.value);
            setEvidenceState("ready");
          } else {
            setEvidenceState("error");
            setEvidenceError(t("operations.evidenceError"));
          }
        })
        .catch(() => {
          if (requestId !== drillDownRequestRef.current) return;
          setEvidenceState("error");
          setEvidenceError(t("operations.evidenceError"));
        });
    },
    [invoke, issuedEvidenceIds, t]
  );

  const openNativeSource = useCallback((url: string) => {
    if (!isTrustedNativeUrl(url)) return;
    void Promise.resolve(open(url)).catch(() => undefined);
  }, []);

  const renderWidget = (id: WidgetId) => {
    if (!snapshot) return null;
    const props = { snapshot, issuedEvidenceIds, onOpen: openDrillDown };
    if (id === "health_summary") return <HealthSummaryWidget {...props} />;
    if (id === "incident_queue") return <IncidentQueueWidget {...props} />;
    if (id === "signal_summary") return <SignalSummaryWidget {...props} />;
    if (id === "change_stream") return <ChangeStreamWidget {...props} />;
    return <EnvironmentStatusWidget {...props} />;
  };

  const visibleLayout = layout.filter((preference) => preference.visible);
  const showSnapshotContext = snapshotState === "ready" && snapshot;

  return (
    <div className="operations-console">
      <header className="operations-console__header">
        <div>
          <p className="eyebrow">{t("operations.eyebrow")}</p>
          <h1>{t("operations.title")}</h1>
          <p className="operations-console__subtitle">{t("operations.subtitle")}</p>
        </div>
        <div className="operations-console__header-actions">
          {showSnapshotContext && (
            <p className="operations-console__sync">
              {t("operations.lastSync", { timestamp: snapshot.generated_at })}
            </p>
          )}
          <button type="button" onClick={() => setLayoutOpen(true)}>
            {t("operations.customizeConsole")}
          </button>
        </div>
      </header>
      {snapshotState === "error" && (
        <p className="operations-console__error" role="alert">
          {snapshotError}
        </p>
      )}
      <div className="operations-widget-grid">
        {layout.map((preference) => {
          const definition = definitions.find((item) => item.id === preference.id);
          if (!definition || !preference.visible) return null;
          return (
            <WidgetFrame
              key={preference.id}
              definition={definition}
              preference={preference}
              snapshotState={snapshotState}
              errorMessage={snapshotError}
              onExpand={() => toggleCollapse(preference.id)}
            >
              {renderWidget(preference.id)}
            </WidgetFrame>
          );
        })}
        {!visibleLayout.length && <EmptyState titleKey="operations.noVisibleWidgets" />}
      </div>
      <Drawer
        titleKey="operations.customizeConsole"
        isOpen={layoutOpen}
        onClose={() => setLayoutOpen(false)}
      >
        <WidgetSettings
          definitions={definitions}
          preferences={layout}
          onToggle={toggleWidget}
          onMove={move}
          onSize={setSize}
          onCollapse={toggleCollapse}
          onReset={resetLayout}
        />
      </Drawer>
      <Drawer
        titleKey="operations.evidenceTitle"
        isOpen={evidenceOpen}
        onClose={() => setEvidenceOpen(false)}
      >
        <EvidencePanel
          selection={selection}
          evidenceState={evidenceState}
          evidence={evidence}
          errorMessage={evidenceError}
          onOpenNative={openNativeSource}
        />
      </Drawer>
    </div>
  );
}
