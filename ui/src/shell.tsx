import { useEffect, useMemo, useState } from "react";
import type { CommandEnvelope, ConnectorDiagnostics, ConnectorSummary, IpcResult, KubernetesEvent, KubernetesInventory, KubernetesResource, WorkspaceContext } from "../contracts/ipc";
import { command } from "../contracts/ipc";
import { Card, CommandSurface, Drawer, EmptyState, StatusIndicator, Table } from "./design-system/components";
import { useTranslation } from "./i18n";

type Invoke = (command: string, args: Record<string, unknown>) => Promise<IpcResult<unknown>>;
type Area =
  | "commandCenter"
  | "incidents"
  | "environments"
  | "observability"
  | "changes"
  | "vulnerability"
  | "automations"
  | "integrations"
  | "policies"
  | "audit";
type ContextFetchState = "loading" | "ready" | "error";
const areas: Area[] = [
  "commandCenter",
  "incidents",
  "environments",
  "observability",
  "changes",
  "vulnerability",
  "automations",
  "integrations",
  "policies",
  "audit"
];
export type ShellNotification = { id: string; titleKey: string; bodyKey: string };
export const localNotifications: ShellNotification[] = [
  { id: "foundation-demo", titleKey: "shell.notificationTitle", bodyKey: "shell.notificationBody" }
];
const contextEnvelope = (): CommandEnvelope<null> => ({
  request_id: crypto.randomUUID(),
  command: command("system", "context"),
  capability: "WorkspaceRead",
  scope: { resource_ids: [] },
  payload: null
});
export const fuzzyMatches = (query: string, target: string) => {
  let cursor = 0;
  for (const letter of query.toLowerCase()) {
    cursor = target.toLowerCase().indexOf(letter, cursor);
    if (cursor < 0) return false;
    cursor += 1;
  }
  return true;
};

export function Shell({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [context, setContext] = useState<WorkspaceContext>();
  const [contextFetchState, setContextFetchState] = useState<ContextFetchState>("loading");
  const [active, setActive] = useState<Area>("commandCenter");
  const [favorites, setFavorites] = useState<Area[]>(
    () => JSON.parse(localStorage.getItem("thalassaops.favorites") ?? "[]") as Area[]
  );
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [handoffRequested, setHandoffRequested] = useState(false);
  useEffect(() => {
    let active = true;
    setContextFetchState("loading");
    invoke("system_context", { envelope: contextEnvelope() })
      .then((result) => {
        if (!active) return;
        if (result.ok) {
          setContext(result.value as WorkspaceContext);
          setContextFetchState("ready");
        } else {
          setContext(undefined);
          setContextFetchState("error");
        }
      })
      .catch(() => {
        if (!active) return;
        setContext(undefined);
        setContextFetchState("error");
      });
    return () => {
      active = false;
    };
  }, [invoke]);
  useEffect(() => {
    const listener = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen(true);
      }
    };
    document.addEventListener("keydown", listener);
    return () => document.removeEventListener("keydown", listener);
  }, []);
  const matches = useMemo(
    () => areas.filter((area) => fuzzyMatches(query, t(`shell.${area}`))),
    [query, t]
  );
  const select = (area: Area) => {
    setActive(area);
    setPaletteOpen(false);
    setQuery("");
  };
  const contextPlaceholder = contextFetchState === "error" ? t("shell.contextUnavailable") : "…";
  const policyState =
    contextFetchState === "ready"
      ? "healthy"
      : contextFetchState === "error"
        ? "unavailable"
        : "warning";
  const toggleFavorite = (area: Area) =>
    setFavorites((current) => {
      const next = current.includes(area)
        ? current.filter((item) => item !== area)
        : [...current, area];
      localStorage.setItem("thalassaops.favorites", JSON.stringify(next));
      return next;
    });
  return (
    <div className="shell">
      <header className="shell-header">
        <strong>{t("shell.productName")}</strong>
        <div className="switchers">
          <button type="button">
            {t("shell.organization")}: {context?.organization_name ?? contextPlaceholder}
          </button>
          <button type="button">
            {t("shell.team")}: {context?.team_name ?? contextPlaceholder}
          </button>
          <button type="button">
            {t("shell.workspace")}: {context?.workspace_name ?? contextPlaceholder}
          </button>
          <button type="button">
            {t("shell.environment")}: {t("shell.noEnvironments")}
          </button>
        </div>
        <button type="button" onClick={() => setPaletteOpen(true)} aria-label={t("shell.search")}>
          {t("shell.commandShortcut")}
        </button>
        <button
          type="button"
          onClick={() => setNotificationsOpen((value) => !value)}
          aria-label={t("shell.notifications")}
        >
          ●
        </button>
        <button
          type="button"
          onClick={() => setTerminalOpen(true)}
          aria-label={t("shell.openTerminal")}
        >
          ⌘
        </button>
      </header>
      <aside>
        <nav aria-label={t("shell.favorites")}>
          {favorites.map((area) => (
            <button key={area} type="button" onClick={() => select(area)}>
              {t(`shell.${area}`)}
            </button>
          ))}
        </nav>
        <nav aria-label={t("shell.productName")}>
          {areas.map((area) => (
            <div className="nav-row" key={area}>
              <button
                type="button"
                aria-current={active === area ? "page" : undefined}
                onClick={() => select(area)}
              >
                {t(`shell.${area}`)}
              </button>
              <button
                type="button"
                onClick={() => toggleFavorite(area)}
                aria-label={t(favorites.includes(area) ? "shell.unpin" : "shell.pin", {
                  area: t(`shell.${area}`)
                })}
              >
                ☆
              </button>
            </div>
          ))}
        </nav>
      </aside>
      <main className="shell-main">
        <h1>{t(`shell.${active}`)}</h1>
        {active === "integrations" ? <Integrations invoke={invoke} /> : <EmptyState titleKey="shell.routeUnavailable" />}
      </main>
      <aside className="shell-status">
        <StatusIndicator state="unavailable" /> <span>{t("shell.connectorStatus")}</span>
        <StatusIndicator state={policyState} />{" "}
        <span>{t("shell.policyStatus", { version: context?.policy_version ?? "…" })}</span>
        <StatusIndicator state="unavailable" /> <span>{t("shell.modelStatus")}</span>
      </aside>
      {notificationsOpen && (
        <section className="notification-center" aria-label={t("shell.notifications")}>
          <h2>{t("shell.notifications")}</h2>
          {localNotifications.map((notification) => (
            <article key={notification.id}>
              <p>{t(notification.titleKey)}</p>
              <p>{t(notification.bodyKey)}</p>
            </article>
          ))}
        </section>
      )}
      <Drawer
        titleKey="shell.commandPalette"
        isOpen={paletteOpen}
        onClose={() => setPaletteOpen(false)}
      >
        <CommandSurface
          labelKey="shell.commandPalette"
          placeholderKey="shell.commandPlaceholder"
          onChange={setQuery}
          onSubmit={() => {
            const match = matches[0];
            if (match) select(match);
          }}
        />
        {matches.map((area) => (
          <button key={area} type="button" onClick={() => select(area)}>
            {t(`shell.${area}`)}
          </button>
        ))}
      </Drawer>
      <Drawer
        titleKey="shell.terminal"
        isOpen={terminalOpen}
        onClose={() => setTerminalOpen(false)}
      >
        <EmptyState titleKey="shell.terminalUnavailable" />
        <button type="button" onClick={() => setHandoffRequested(true)}>
          {t("shell.externalTerminal")}
        </button>
        {handoffRequested && <p role="status">{t("shell.externalTerminalUnavailable")}</p>}
      </Drawer>
    </div>
  );
}

const connectorEnvelope = <T,>(verb: string, capability: "ConnectorRead" | "ConnectorAct", payload: T): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(), command: command("connector", verb), capability, scope: { resource_ids: [] }, payload
});

function Integrations({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [connectors, setConnectors] = useState<ConnectorSummary[]>([]);
  const [diagnostics, setDiagnostics] = useState<ConnectorDiagnostics>();
  const [loading, setLoading] = useState(true);
  const load = () => {
    setLoading(true);
    invoke("connector_list", { envelope: connectorEnvelope("list", "ConnectorRead", null) })
      .then((result) => { if (result.ok) setConnectors(result.value as ConnectorSummary[]); })
      .finally(() => setLoading(false));
  };
  useEffect(load, [invoke]);
  const act = (verb: "test" | "enable" | "disable" | "remove", id: string) =>
    invoke(`connector_${verb}`, { envelope: connectorEnvelope(verb, "ConnectorAct", { id }) }).then(() => {
      if (verb === "remove") setDiagnostics(undefined);
      load();
    });
  const diagnose = (id: string) => invoke("connector_diagnose", { envelope: connectorEnvelope("diagnose", "ConnectorRead", { id }) })
    .then((result) => { if (result.ok) setDiagnostics(result.value as ConnectorDiagnostics); });
  const add = () => invoke("connector_add", { envelope: connectorEnvelope("add", "ConnectorAct", { kind: "fixture", display_name: t("integrations.fixtureName"), config_metadata: { fixture_health: "healthy" } }) }).then(load);
  if (loading) return <p role="status">{t("integrations.loading")}</p>;
  if (!connectors.length) return <EmptyState titleKey="integrations.empty"><button type="button" onClick={add}>{t("integrations.addFixture")}</button></EmptyState>;
  const kubernetesConnectors = connectors.filter((item) => item.kind === "kubernetes" && item.enabled);
  return <div className="integrations">
    <button type="button" onClick={add}>{t("integrations.addFixture")}</button>
    <Table captionKey="integrations.tableCaption" columns={[
      { key: "name", headerKey: "integrations.name" }, { key: "status", headerKey: "integrations.status" }, { key: "actions", headerKey: "integrations.actions" }
    ]} rows={connectors.map((item) => ({ id: item.id, name: <><strong>{item.display_name}</strong><small>{item.kind}</small></>, status: <StatusIndicator state={item.health_state} />, actions: <div className="connector-actions"><button type="button" onClick={() => act("test", item.id)}>{t("integrations.test")}</button><button type="button" onClick={() => diagnose(item.id)}>{t("integrations.diagnose")}</button><button type="button" onClick={() => act(item.enabled ? "disable" : "enable", item.id)}>{t(item.enabled ? "integrations.disable" : "integrations.enable")}</button><button type="button" onClick={() => act("remove", item.id)}>{t("integrations.remove")}</button></div> }))} />
    {diagnostics && <Card titleKey="integrations.diagnostics"><p>{t("integrations.capabilities")}: {diagnostics.manifest.capabilities.map((capability) => capability.key).join(", ")}</p><p>{t("integrations.lastSync")}: {diagnostics.connector.last_successful_sync_at ?? t("integrations.never")}</p>{diagnostics.logs.length ? <ul>{diagnostics.logs.map((entry) => <li key={entry.id}><StatusIndicator state={entry.outcome} /> {entry.message}</li>)}</ul> : <p>{t("integrations.noLogs")}</p>}</Card>}
    {kubernetesConnectors.length > 0 && <KubernetesInspector invoke={invoke} connectors={kubernetesConnectors} />}
  </div>;
}

const kubernetesEnvelope = <T,>(verb: string, capability: "EnvironmentRead" | "ResourceRead", payload: T): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(), command: command("kubernetes", verb), capability, scope: { resource_ids: [] }, payload
});

function KubernetesInspector({ invoke, connectors }: { invoke: Invoke; connectors: ConnectorSummary[] }) {
  const { t } = useTranslation();
  const [connectorId, setConnectorId] = useState(connectors[0]?.id ?? "");
  const [inventory, setInventory] = useState<KubernetesInventory>();
  const [pod, setPod] = useState<KubernetesResource>();
  const [logs, setLogs] = useState("");
  const [events, setEvents] = useState<KubernetesEvent[]>([]);
  const inspect = () => invoke("kubernetes_inventory", { envelope: kubernetesEnvelope("inventory", "EnvironmentRead", { connector_id: connectorId }) })
    .then((result) => { if (result.ok) { setInventory(result.value as KubernetesInventory); setPod(undefined); setLogs(""); setEvents([]); } });
  const selectPod = (item: KubernetesResource) => {
    const [namespace, name] = item.resource.name.split("/", 2);
    setPod(item); setLogs(""); setEvents([]);
    const payload = { connector_id: connectorId, namespace, pod: name };
    void invoke("kubernetes_pod_logs", { envelope: kubernetesEnvelope("pod_logs", "ResourceRead", payload) }).then((result) => { if (result.ok) setLogs(result.value as string); });
    void invoke("kubernetes_pod_events", { envelope: kubernetesEnvelope("pod_events", "ResourceRead", payload) }).then((result) => { if (result.ok) setEvents(result.value as KubernetesEvent[]); });
  };
  return <Card titleKey="kubernetes.title">
    <label>{t("kubernetes.cluster")} <select value={connectorId} onChange={(event) => setConnectorId(event.target.value)}>{connectors.map((item) => <option key={item.id} value={item.id}>{item.display_name}</option>)}</select></label>
    <button type="button" onClick={inspect}>{t("kubernetes.inspect")}</button>
    {inventory && <><p>{t("kubernetes.availability")}: {inventory.availability.filter((item) => item.available).length}/{inventory.availability.length}</p>
      <Table captionKey="kubernetes.resources" columns={[{ key: "resource", headerKey: "kubernetes.resource" }, { key: "status", headerKey: "kubernetes.status" }, { key: "owner", headerKey: "kubernetes.owner" }]} rows={inventory.resources.map((item) => ({ id: `${item.resource.kind}-${item.resource.name}`, resource: item.resource.kind === "Pod" ? <button type="button" onClick={() => selectPod(item)}>{item.resource.name}</button> : item.resource.name, status: item.status ?? "—", owner: item.owner ? `${item.owner.kind}/${item.owner.name}` : "—" }))} />
      {pod && <section aria-label={t("kubernetes.podDetails")}><h3>{pod.resource.name}</h3><p>{t("kubernetes.owner")}: {pod.owner ? `${pod.owner.kind}/${pod.owner.name}` : "—"}</p><ul>{pod.conditions.map((condition) => <li key={condition.type_}>{condition.type_}: {condition.status} {condition.reason ?? ""} {condition.message ?? ""}</li>)}</ul><h4>{t("kubernetes.events")}</h4><ul>{events.map((event, index) => <li key={`${event.reason}-${index}`}>{event.reason}: {event.message}</li>)}</ul><h4>{t("kubernetes.logs")}</h4><pre>{logs}</pre></section>}
    </>}
  </Card>;
}
