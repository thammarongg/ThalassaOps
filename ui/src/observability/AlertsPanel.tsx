import { useEffect, useState } from "react";
import type {
  AlertmanagerAlertsRequest,
  ConnectorSummary,
  Invoke,
  NormalizedAlert,
  ResourceReference
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { Card, Table } from "../design-system/components";
import { useTranslation } from "../i18n";
import type { TimeContext } from "./timeContext";

const mapIpcError = (err: unknown, t: (key: string) => string) => {
  const e = err as Record<string, unknown>;
  const code = e?.code;
  if (code === "CONNECTOR_UNAVAILABLE") return t("observability.unavailable");
  if (code === "POLICY_DENIED") return t("observability.denied");
  if (code === "MALFORMED_RESPONSE") return t("observability.malformed");
  return t("observability.unknownError");
};

export function AlertsPanel({
  connector,
  invoke,
  selectedAlert,
  onSelectAlert
}: {
  connector: ConnectorSummary;
  invoke: Invoke;
  selectedAlert?: NormalizedAlert;
  onSelectAlert: (alert: NormalizedAlert) => void;
  timeContext?: TimeContext;
}) {
  const { t } = useTranslation();
  const [alerts, setAlerts] = useState<NormalizedAlert[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    invoke<AlertmanagerAlertsRequest, NormalizedAlert[]>("alertmanager_alerts", {
      envelope: {
        request_id: crypto.randomUUID(),
        command: command("alertmanager", "alerts"),
        capability: "ResourceRead",
        scope: { resource_ids: [] },
        payload: { connector_id: connector.id }
      }
    })
      .then((res) => {
        if (res.ok) setAlerts(res.value);
        else setError(mapIpcError(res.error, t));
      })
      .catch((err) => setError(mapIpcError(err, t)))
      .finally(() => setLoading(false));
  }, [connector, invoke, t]);

  const renderResource = (ref: ResourceReference) => {
    if ("resolved" in ref) {
      const r = ref.resolved;
      return `${r.kind} ${r.namespace}/${r.name}`;
    }
    return t("observability.unresolved", { reason: ref.unresolved.reason });
  };

  return (
    <Card titleKey="observability.alertmanager">
      <h3>{connector.display_name}</h3>
      {loading && <p role="status">{t("integrations.loading")}</p>}
      {!loading && error && (
        <p role="status" className="error">
          {error}
        </p>
      )}
      {!loading && !error && alerts.length === 0 && <p>{t("observability.empty")}</p>}
      {!loading && !error && alerts.length > 0 && (
        <Table
          captionKey="observability.alerts"
          columns={[
            { key: "select", headerKey: "observability.state" }, // Reuse state header space or something, wait we can just add a blank header or use 'state' for the first col
            { key: "state", headerKey: "observability.state" },
            { key: "timestamp", headerKey: "observability.timestamp" },
            { key: "labels", headerKey: "observability.labels" },
            { key: "resource", headerKey: "observability.resource" }
          ]}
          rows={alerts.map((a) => ({
            id: a.fingerprint,
            select: (
              <input
                type="radio"
                name="selectedAlert"
                aria-label={t("observability.selectAlert", { fingerprint: a.fingerprint })}
                checked={selectedAlert?.fingerprint === a.fingerprint}
                onChange={() => onSelectAlert(a)}
              />
            ),
            state: a.state,
            timestamp: new Date(a.starts_at).toLocaleString(),
            labels: Object.entries(a.labels)
              .map(([k, v]) => `${k}=${v}`)
              .join(", "),
            resource: renderResource(a.resource_reference)
          }))}
        />
      )}
    </Card>
  );
}
