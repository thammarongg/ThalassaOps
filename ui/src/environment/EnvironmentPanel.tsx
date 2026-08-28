import { useEffect, useState } from "react";
import type {
  CloudEnvironment,
  CloudResource,
  CloudProvider,
  CommandEnvelope,
  ConnectorSummary,
  Invoke,
  IpcResult
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { useTranslation } from "../i18n";
import { AccessBanner } from "./AccessBanner";
import { ResourceTable } from "./ResourceTable";

type CloudRequest = { connector_id: string };
type PreflightState = Pick<CloudEnvironment, "access" | "remedy">;

const cloudEnvelope = <T,>(verb: string, payload: T): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("cloud", verb),
  capability: "EnvironmentRead",
  scope: { resource_ids: [] },
  payload
});

const errorMessage = (t: (key: string) => string) => t("environment.requestUnavailable");

export function EnvironmentPanel({
  connector,
  provider,
  invoke
}: {
  connector: ConnectorSummary;
  provider: CloudProvider;
  invoke: Invoke;
}) {
  const { t } = useTranslation();
  const [environment, setEnvironment] = useState<CloudEnvironment>();
  const [preflight, setPreflight] = useState<PreflightState>();
  const [resources, setResources] = useState<CloudResource[]>([]);
  const [accessLoading, setAccessLoading] = useState(true);
  const [inventoryLoading, setInventoryLoading] = useState(false);
  const [inventoryError, setInventoryError] = useState("");

  useEffect(() => {
    let active = true;
    setEnvironment(undefined);
    setPreflight(undefined);
    setResources([]);
    setInventoryError("");
    setAccessLoading(true);
    setInventoryLoading(false);

    const fetchEnvironment = async () => {
      let accessResult: IpcResult<CloudEnvironment>;
      try {
        accessResult = await invoke<CloudRequest, CloudEnvironment>("cloud_access_check", {
          envelope: cloudEnvelope("access_check", { connector_id: connector.id })
        });
      } catch {
        if (!active) return;
        setPreflight({ access: "unavailable", remedy: "" });
        setAccessLoading(false);
        return;
      }

      if (!active) return;
      setAccessLoading(false);
      if (!accessResult.ok) {
        setPreflight({ access: "unavailable", remedy: "" });
        return;
      }

      const nextEnvironment = accessResult.value;
      setEnvironment(nextEnvironment);
      setPreflight({ access: nextEnvironment.access, remedy: nextEnvironment.remedy });
      if (nextEnvironment.access !== "confirmed") return;

      setInventoryLoading(true);
      try {
        const inventoryResult = await invoke<CloudRequest, CloudResource[]>("cloud_inventory", {
          envelope: cloudEnvelope("inventory", { connector_id: connector.id })
        });
        if (!active) return;
        if (inventoryResult.ok) {
          setResources(inventoryResult.value);
        } else {
          setInventoryError(errorMessage(t));
        }
      } catch {
        if (active) setInventoryError(errorMessage(t));
      } finally {
        if (active) setInventoryLoading(false);
      }
    };

    void fetchEnvironment();
    return () => {
      active = false;
    };
  }, [connector.id, invoke, t]);

  const providerLabel = t(`environment.providers.${provider}`);
  const accountLabel = environment?.account_label || connector.display_name;
  const location = environment?.location || t("environment.notAvailable");
  const isConfirmed = preflight?.access === "confirmed";

  return (
    <section className="environment-panel" aria-labelledby={`environment-panel-${connector.id}`}>
      <header className="environment-panel__header">
        <div>
          <p className="eyebrow">{t("environment.eyebrow")}</p>
          <h2 id={`environment-panel-${connector.id}`}>{connector.display_name}</h2>
        </div>
        <span
          className={`environment-provider environment-provider--${provider}`}
          aria-label={t("environment.providerLabel", { provider: providerLabel })}
        >
          {providerLabel}
        </span>
      </header>
      <dl className="environment-panel__meta">
        <div>
          <dt>{t("environment.account")}</dt>
          <dd>{accountLabel}</dd>
        </div>
        <div>
          <dt>{t("environment.location")}</dt>
          <dd>{location}</dd>
        </div>
      </dl>
      <AccessBanner access={preflight?.access} remedy={preflight?.remedy} loading={accessLoading} />
      {isConfirmed && (
        <div className="environment-panel__resources">
          {inventoryLoading && (
            <p className="environment-inventory-loading" role="status">
              {t("environment.inventoryLoading")}
            </p>
          )}
          {inventoryError && (
            <p className="error" role="alert">
              {inventoryError}
            </p>
          )}
          {!inventoryLoading && !inventoryError && <ResourceTable resources={resources} />}
        </div>
      )}
    </section>
  );
}
