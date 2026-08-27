import { useEffect, useState } from "react";
import type { CloudProvider, CommandEnvelope, ConnectorSummary, Invoke } from "../contracts/ipc";
import { command } from "../contracts/ipc";
import { EmptyState } from "./design-system/components";
import { useTranslation } from "./i18n";
import { EnvironmentPanel } from "./environment/EnvironmentPanel";

const providerFor = (kind: string): CloudProvider | undefined => {
  if (kind === "aws" || kind === "azure" || kind === "gcp") return kind;
  return undefined;
};

const connectorListEnvelope: CommandEnvelope<null> = {
  request_id: crypto.randomUUID(),
  command: command("connector", "list"),
  capability: "ConnectorRead",
  scope: { resource_ids: [] },
  payload: null
};

export function EnvironmentWorkspace({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [connectors, setConnectors] = useState<ConnectorSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(false);
    void invoke<null, ConnectorSummary[]>("connector_list", {
      envelope: { ...connectorListEnvelope, request_id: crypto.randomUUID() }
    })
      .then((result) => {
        if (!active) return;
        if (result.ok) {
          setConnectors(result.value);
        } else {
          setConnectors([]);
          setError(true);
        }
      })
      .catch(() => {
        if (!active) return;
        setConnectors([]);
        setError(true);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [invoke]);

  if (loading) return <p role="status">{t("environment.loading")}</p>;
  if (error)
    return (
      <p role="alert" className="error">
        {t("environment.connectorUnavailable")}
      </p>
    );

  const cloudConnectors = connectors.filter(
    (connector) => connector.enabled && providerFor(connector.kind)
  );
  if (!cloudConnectors.length) return <EmptyState titleKey="environment.empty" />;

  return (
    <div className="environment-workspace">
      <div className="environment-workspace__intro">
        <p className="eyebrow">{t("environment.eyebrow")}</p>
        <p>{t("environment.description")}</p>
      </div>
      <div className="environment-panels">
        {cloudConnectors.map((connector) => {
          const provider = providerFor(connector.kind);
          if (!provider) return null;
          return (
            <EnvironmentPanel
              key={connector.id}
              connector={connector}
              provider={provider}
              invoke={invoke}
            />
          );
        })}
      </div>
    </div>
  );
}
