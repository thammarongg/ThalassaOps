import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  Card,
  CommandSurface,
  EmptyState,
  StatusIndicator,
  Table,
  Tabs,
  Timeline
} from "./design-system/components";
import { requestHealth, type Health } from "./health";
import { I18nProvider, useTranslation } from "./i18n";
import "./styles.css";

function App() {
  const { t } = useTranslation();
  const [health, setHealth] = useState<Health | undefined>();
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    requestHealth(invoke)
      .then(setHealth)
      .catch((reason: unknown) =>
        setError(reason instanceof Error ? reason.message : t("health.error"))
      );
  }, [t]);

  return (
    <main className="app">
      <p className="eyebrow">{t("health.eyebrow")}</p>
      <h1>{t("health.title")}</h1>
      <div className="grid">
        <Card titleKey="demo.healthCard">
          {health && (
            <StatusIndicator state={health.status === "healthy" ? "healthy" : "degraded"} />
          )}
          {health?.policy_version && (
            <p>
              {t("health.policyVersion")}: <code>{health.policy_version}</code>
            </p>
          )}
        </Card>
        <Card titleKey="demo.secondaryCard">
          <EmptyState titleKey="demo.emptyTitle" />
        </Card>
      </div>
      {error && <p role="alert">{error}</p>}
      {!health && !error && <p>{t("health.checking")}</p>}
      <Tabs
        items={[
          { id: "overview", labelKey: "demo.firstTab" },
          { id: "evidence", labelKey: "demo.secondTab" }
        ]}
      >
        {(active) => <p>{active}</p>}
      </Tabs>
      <Table
        captionKey="demo.tableCaption"
        columns={[{ key: "name", headerKey: "demo.name" }]}
        rows={[{ id: "preview", name: "preview" }]}
      />
      <Timeline items={[{ id: "signal", titleKey: "demo.timelineEvent", state: "warning" }]} />
      <CommandSurface labelKey="demo.commandLabel" placeholderKey="demo.commandPlaceholder" />
    </main>
  );
}

createRoot(document.getElementById("root")!).render(
  <I18nProvider>
    <App />
  </I18nProvider>
);
