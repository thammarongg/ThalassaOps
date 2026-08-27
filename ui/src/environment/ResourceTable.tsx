import { useState } from "react";
import { open } from "@tauri-apps/plugin-shell";
import type { CloudHealthState, CloudResource, CloudResourceType } from "../../contracts/ipc";
import { Table } from "../design-system/components";
import { useTranslation } from "../i18n";

const resourceTypeKeys: Record<CloudResourceType, string> = {
  kubernetes_cluster: "environment.resourceTypes.kubernetesCluster",
  compute_instance: "environment.resourceTypes.computeInstance"
};

const healthTones: Record<CloudHealthState, "healthy" | "degraded" | "unavailable" | "warning"> = {
  healthy: "healthy",
  degraded: "degraded",
  unavailable: "unavailable",
  unknown: "warning"
};

export function ResourceTable({ resources }: { resources: CloudResource[] }) {
  const { t } = useTranslation();
  const [copiedResourceId, setCopiedResourceId] = useState<string>();

  const copyCommand = async (resource: CloudResource) => {
    if (!navigator.clipboard) return;
    try {
      await navigator.clipboard.writeText(resource.cli_command);
      setCopiedResourceId(resource.id);
    } catch {
      setCopiedResourceId(undefined);
    }
  };

  if (!resources.length) {
    return <p className="environment-resources-empty">{t("environment.noResources")}</p>;
  }

  return (
    <div className="environment-resource-table">
      <Table
        captionKey="environment.resourceTableCaption"
        columns={[
          { key: "name", headerKey: "environment.name" },
          { key: "type", headerKey: "environment.type" },
          { key: "location", headerKey: "environment.location" },
          { key: "health", headerKey: "environment.health" },
          { key: "status", headerKey: "environment.statusDetail" },
          { key: "actions", headerKey: "environment.actions" }
        ]}
        rows={resources.map((resource) => ({
          id: resource.id,
          name: <strong>{resource.name}</strong>,
          type: t(resourceTypeKeys[resource.resource_type]),
          location: resource.location || t("environment.notAvailable"),
          health: (
            <span className={`indicator indicator--${healthTones[resource.health]}`}>
              <span aria-hidden="true">●</span>
              <span>{t(`environment.healthStates.${resource.health}`)}</span>
            </span>
          ),
          status: resource.status_detail || t("environment.notAvailable"),
          actions: (
            <div className="environment-resource-actions">
              <button
                type="button"
                onClick={() =>
                  void Promise.resolve(open(resource.console_url)).catch(() => undefined)
                }
                disabled={!resource.console_url}
              >
                {t("environment.openConsole")}
              </button>
              <button type="button" onClick={() => void copyCommand(resource)}>
                {t("environment.copyCommand")}
              </button>
              {copiedResourceId === resource.id && (
                <span role="status">{t("environment.copiedCommand")}</span>
              )}
            </div>
          )
        }))}
      />
    </div>
  );
}
