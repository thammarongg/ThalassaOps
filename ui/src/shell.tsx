import { useEffect, useMemo, useState, useRef } from "react";
import type {
  CommandEnvelope,
  ConnectorDiagnostics,
  ConnectorSummary,
  KubernetesEvent,
  KubernetesInventory,
  KubernetesManifest,
  KubernetesResource,
  WorkspaceContext,
  Invoke
} from "../contracts/ipc";
import { open } from "@tauri-apps/plugin-shell";
import { command } from "../contracts/ipc";
import {
  Card,
  CommandSurface,
  Drawer,
  EmptyState,
  StatusIndicator,
  Table
} from "./design-system/components";
import { useTranslation } from "./i18n";
import { EnvironmentWorkspace } from "./EnvironmentWorkspace";
import { ObservabilityWorkspace } from "./ObservabilityWorkspace";
import { OperationsConsole } from "./OperationsConsole";
import { TopologyWorkspace } from "./topology/TopologyWorkspace";
type Area =
  | "commandCenter"
  | "incidents"
  | "environments"
  | "observability"
  | "topology"
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
  "topology",
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
  const [topologyIncidentId, setTopologyIncidentId] = useState<string | null>(null);
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
  const openIncidentTopology = (incidentId: string) => {
    setTopologyIncidentId(incidentId);
    setActive("topology");
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
        {active === "commandCenter" ? (
          <OperationsConsole invoke={invoke} onOpenIncidentTopology={openIncidentTopology} />
        ) : active === "environments" ? (
          <>
            <h1>{t(`shell.${active}`)}</h1>
            <EnvironmentWorkspace invoke={invoke} />
          </>
        ) : active === "integrations" ? (
          <>
            <h1>{t(`shell.${active}`)}</h1>
            <Integrations invoke={invoke} />
          </>
        ) : active === "observability" ? (
          <>
            <h1>{t(`shell.${active}`)}</h1>
            <ObservabilityWorkspace invoke={invoke} />
          </>
        ) : active === "topology" ? (
          <TopologyWorkspace invoke={invoke} initialIncidentId={topologyIncidentId} />
        ) : (
          <>
            <h1>{t(`shell.${active}`)}</h1>
            <EmptyState titleKey="shell.routeUnavailable" />
          </>
        )}
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

const connectorEnvelope = <T,>(
  verb: string,
  capability: "ConnectorRead" | "ConnectorAct",
  payload: T
): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("connector", verb),
  capability,
  scope: { resource_ids: [] },
  payload
});

function AddConnectorForm({
  onAdd,
  onCancel
}: {
  onAdd: (payload: Record<string, unknown>) => Promise<void>;
  onCancel: () => void;
}) {
  const { t } = useTranslation();
  const [kind, setKind] = useState("fixture");
  const [displayName, setDisplayName] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [authMode, setAuthMode] = useState("none");
  const [username, setUsername] = useState("");
  const [tenantId, setTenantId] = useState("");
  const [datasourceUid, setDatasourceUid] = useState("");
  const [defaultDashboardUid, setDefaultDashboardUid] = useState("");
  const credentialRef = useRef<HTMLInputElement>(null);
  const [error, setError] = useState("");
  const showHttpWarning = useMemo(() => {
    try {
      return new URL(baseUrl).protocol === "http:";
    } catch {
      return false;
    }
  }, [baseUrl]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    let config_metadata: Record<string, unknown> = {};
    if (kind === "fixture") {
      config_metadata = { fixture_health: "healthy" };
    } else {
      config_metadata = {
        base_url: baseUrl,
        auth_mode: authMode
      };
      if (authMode === "basic") {
        config_metadata.username = username;
      }
      if ((kind === "loki" || kind === "tempo") && tenantId.trim()) {
        config_metadata.tenant_id = tenantId.trim();
      }
      if (kind === "grafana") {
        if (datasourceUid) config_metadata.datasource_uid = datasourceUid;
        if (defaultDashboardUid) config_metadata.default_dashboard_uid = defaultDashboardUid;
      }
    }

    const payload: Record<string, unknown> = {
      kind,
      display_name: displayName,
      config_metadata
    };

    const cred_val = credentialRef.current?.value;
    if (kind !== "fixture" && authMode !== "none" && cred_val) {
      payload.credential_value = cred_val;
    }

    if (credentialRef.current) {
      credentialRef.current.value = ""; // clear password
    }

    try {
      await onAdd(payload);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <form onSubmit={submit} className="add-connector-form">
      <h2>{t("integrations.addConnector")}</h2>
      {error && (
        <p role="status" className="error">
          {error}
        </p>
      )}
      <label>
        {t("integrations.kind")}{" "}
        <select value={kind} onChange={(e) => setKind(e.target.value)}>
          <option value="fixture">{t("integrations.fixtureName")}</option>
          <option value="prometheus">{t("observability.prometheus")}</option>
          <option value="alertmanager">{t("observability.alertmanager")}</option>
          <option value="grafana">{t("observability.grafana")}</option>
          <option value="loki">{t("observability.loki")}</option>
          <option value="tempo">{t("observability.tempo")}</option>
        </select>
      </label>
      <label>
        {t("integrations.name")}{" "}
        <input required value={displayName} onChange={(e) => setDisplayName(e.target.value)} />
      </label>

      {kind !== "fixture" && (
        <>
          <p>{t("integrations.httpsGuidance")}</p>
          {showHttpWarning && <p role="alert">{t("integrations.httpWarning")}</p>}
          <label>
            {t("integrations.baseUrl")}{" "}
            <input
              required
              type="url"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
            />
          </label>
          {(kind === "loki" || kind === "tempo") && (
            <label>
              {t("integrations.tenantId")}{" "}
              <input value={tenantId} onChange={(e) => setTenantId(e.target.value)} />
            </label>
          )}
          <label>
            {t("integrations.authMode")}{" "}
            <select value={authMode} onChange={(e) => setAuthMode(e.target.value)}>
              <option value="none">{t("integrations.authNone")}</option>
              <option value="bearer">{t("integrations.authBearer")}</option>
              <option value="basic">{t("integrations.authBasic")}</option>
            </select>
          </label>
          {authMode === "basic" && (
            <label>
              {t("integrations.username")}{" "}
              <input required value={username} onChange={(e) => setUsername(e.target.value)} />
            </label>
          )}
          {authMode !== "none" && (
            <label>
              {t("integrations.credential")} <input type="password" ref={credentialRef} required />
            </label>
          )}
          {kind === "grafana" && (
            <>
              <label>
                {t("integrations.datasourceUid")}{" "}
                <input value={datasourceUid} onChange={(e) => setDatasourceUid(e.target.value)} />
              </label>
              <label>
                {t("integrations.defaultDashboardUid")}{" "}
                <input
                  value={defaultDashboardUid}
                  onChange={(e) => setDefaultDashboardUid(e.target.value)}
                />
              </label>
            </>
          )}
        </>
      )}

      <div className="actions">
        <button type="submit">{t("integrations.save")}</button>
        <button type="button" onClick={onCancel}>
          {t("integrations.cancel")}
        </button>
      </div>
    </form>
  );
}

function Integrations({ invoke }: { invoke: Invoke }) {
  const { t } = useTranslation();
  const [connectors, setConnectors] = useState<ConnectorSummary[]>([]);
  const [diagnostics, setDiagnostics] = useState<ConnectorDiagnostics>();
  const [loading, setLoading] = useState(true);
  const [showAddForm, setShowAddForm] = useState(false);
  const load = () => {
    setLoading(true);
    invoke("connector_list", { envelope: connectorEnvelope("list", "ConnectorRead", null) })
      .then((result) => {
        if (result.ok) setConnectors(result.value as ConnectorSummary[]);
      })
      .finally(() => setLoading(false));
  };
  useEffect(load, [invoke]);
  const act = (verb: "test" | "enable" | "disable" | "remove", id: string) =>
    invoke(`connector_${verb}`, { envelope: connectorEnvelope(verb, "ConnectorAct", { id }) }).then(
      () => {
        if (verb === "remove") setDiagnostics(undefined);
        load();
      }
    );
  const diagnose = (id: string) =>
    invoke("connector_diagnose", {
      envelope: connectorEnvelope("diagnose", "ConnectorRead", { id })
    }).then((result) => {
      if (result.ok) setDiagnostics(result.value as ConnectorDiagnostics);
    });

  const handleAdd = async (payload: Record<string, unknown>) => {
    const result = await invoke("connector_add", {
      envelope: connectorEnvelope("add", "ConnectorAct", payload)
    });
    if (result.ok) {
      setShowAddForm(false);
      load();
    } else {
      const e = result.error as Record<string, unknown>;
      const code = e?.code;
      if (code === "INVALID_REQUEST") throw new Error(t("integrations.invalidRequest"));
      if (code === "MALFORMED_RESPONSE") throw new Error(t("observability.malformed"));
      if (code === "POLICY_DENIED") throw new Error(t("observability.denied"));
      throw new Error(t("observability.unknownError"));
    }
  };

  if (loading) return <p role="status">{t("integrations.loading")}</p>;
  if (showAddForm)
    return <AddConnectorForm onAdd={handleAdd} onCancel={() => setShowAddForm(false)} />;
  if (!connectors.length)
    return (
      <EmptyState titleKey="integrations.empty">
        <button type="button" onClick={() => setShowAddForm(true)}>
          {t("integrations.addConnector")}
        </button>
      </EmptyState>
    );
  const kubernetesConnectors = connectors.filter(
    (item) => item.kind === "kubernetes" && item.enabled
  );
  return (
    <div className="integrations">
      <button type="button" onClick={() => setShowAddForm(true)}>
        {t("integrations.addConnector")}
      </button>
      <Table
        captionKey="integrations.tableCaption"
        columns={[
          { key: "name", headerKey: "integrations.name" },
          { key: "status", headerKey: "integrations.status" },
          { key: "actions", headerKey: "integrations.actions" }
        ]}
        rows={connectors.map((item) => ({
          id: item.id,
          name: (
            <>
              <strong>{item.display_name}</strong>
              <small>{item.kind}</small>
            </>
          ),
          status: <StatusIndicator state={item.health_state} />,
          actions: (
            <div className="connector-actions">
              <button type="button" onClick={() => act("test", item.id)}>
                {t("integrations.test")}
              </button>
              <button type="button" onClick={() => diagnose(item.id)}>
                {t("integrations.diagnose")}
              </button>
              <button
                type="button"
                onClick={() => act(item.enabled ? "disable" : "enable", item.id)}
              >
                {t(item.enabled ? "integrations.disable" : "integrations.enable")}
              </button>
              <button type="button" onClick={() => act("remove", item.id)}>
                {t("integrations.remove")}
              </button>
            </div>
          )
        }))}
      />
      {diagnostics && (
        <Card titleKey="integrations.diagnostics">
          <p>
            {t("integrations.capabilities")}:{" "}
            {diagnostics.manifest.capabilities.map((capability) => capability.key).join(", ")}
          </p>
          <p>
            {t("integrations.lastSync")}:{" "}
            {diagnostics.connector.last_successful_sync_at ?? t("integrations.never")}
          </p>
          {diagnostics.logs.length ? (
            <ul>
              {diagnostics.logs.map((entry) => (
                <li key={entry.id}>
                  <StatusIndicator state={entry.outcome} /> {entry.message}
                </li>
              ))}
            </ul>
          ) : (
            <p>{t("integrations.noLogs")}</p>
          )}
        </Card>
      )}
      {kubernetesConnectors.length > 0 && (
        <KubernetesInspector invoke={invoke} connectors={kubernetesConnectors} />
      )}
    </div>
  );
}

const kubernetesEnvelope = <T,>(
  verb: string,
  capability: "EnvironmentRead" | "ResourceRead",
  payload: T
): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("kubernetes", verb),
  capability,
  scope: { resource_ids: [] },
  payload
});

function KubernetesInspector({
  invoke,
  connectors
}: {
  invoke: Invoke;
  connectors: ConnectorSummary[];
}) {
  const { t } = useTranslation();
  const [connectorId, setConnectorId] = useState(connectors[0]?.id ?? "");
  const [inventory, setInventory] = useState<KubernetesInventory>();
  const [pod, setPod] = useState<KubernetesResource>();
  const [logs, setLogs] = useState("");
  const [events, setEvents] = useState<KubernetesEvent[]>([]);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState("");
  const [namespace, setNamespace] = useState("");
  const [health, setHealth] = useState("");
  const [manifest, setManifest] = useState<KubernetesManifest>();
  const inspect = () =>
    invoke("kubernetes_inventory", {
      envelope: kubernetesEnvelope("inventory", "EnvironmentRead", { connector_id: connectorId })
    }).then((result) => {
      if (result.ok) {
        setInventory(result.value as KubernetesInventory);
        setPod(undefined);
        setLogs("");
        setEvents([]);
      }
    });
  const selectPod = (item: KubernetesResource) => {
    const [namespace, name] = item.resource.name.split("/", 2);
    setPod(item);
    setLogs("");
    setEvents([]);
    const payload = { connector_id: connectorId, namespace, pod: name };
    void invoke("kubernetes_pod_logs", {
      envelope: kubernetesEnvelope("pod_logs", "ResourceRead", payload)
    }).then((result) => {
      if (result.ok) setLogs(result.value as string);
    });
    void invoke("kubernetes_pod_events", {
      envelope: kubernetesEnvelope("pod_events", "ResourceRead", payload)
    }).then((result) => {
      if (result.ok) setEvents(result.value as KubernetesEvent[]);
    });
  };
  const parts = (item: KubernetesResource) =>
    item.resource.name.includes("/") ? item.resource.name.split("/", 2) : ["", item.resource.name];
  const select = (item: KubernetesResource) => {
    setPod(item);
    setManifest(undefined);
    if (item.resource.kind === "Pod") selectPod(item);
  };
  const viewManifest = (item: KubernetesResource) => {
    const [itemNamespace, name] = parts(item);
    void invoke("kubernetes_resource_manifest", {
      envelope: kubernetesEnvelope("resource_manifest", "ResourceRead", {
        connector_id: connectorId,
        namespace: itemNamespace,
        kind: item.resource.kind,
        name
      })
    }).then((result) => {
      if (result.ok) setManifest(result.value as KubernetesManifest);
    });
  };
  const copyCommand = (item: KubernetesResource) => {
    const [itemNamespace, name] = parts(item);
    const context = String(
      connectors.find((connector) => connector.id === connectorId)?.config_metadata.context_name ??
        ""
    );
    const command =
      item.resource.kind === "Pod"
        ? `kubectl --context ${context} -n ${itemNamespace} logs ${name} --tail=200`
        : `kubectl --context ${context}${itemNamespace ? ` -n ${itemNamespace}` : ""} get ${item.resource.kind.toLowerCase()} ${name} -o yaml`;
    void navigator.clipboard?.writeText(command);
  };
  const consoleUrl = (item: KubernetesResource) => {
    const template = connectors.find((connector) => connector.id === connectorId)?.config_metadata
      .console_url_template;
    const [itemNamespace, name] = parts(item);
    return typeof template === "string"
      ? template.replaceAll("{namespace}", itemNamespace).replaceAll("{name}", name)
      : undefined;
  };
  return (
    <Card titleKey="kubernetes.title">
      <label>
        {t("kubernetes.cluster")}{" "}
        <select value={connectorId} onChange={(event) => setConnectorId(event.target.value)}>
          {connectors.map((item) => (
            <option key={item.id} value={item.id}>
              {item.display_name}
            </option>
          ))}
        </select>
      </label>
      <button type="button" onClick={inspect}>
        {t("kubernetes.inspect")}
      </button>
      {inventory && (
        <>
          {(() => {
            const resources = inventory.resources.filter((item) => {
              const [itemNamespace] = parts(item);
              return (
                item.resource.name.toLowerCase().includes(query.toLowerCase()) &&
                (!kind || item.resource.kind === kind) &&
                (!namespace || itemNamespace === namespace) &&
                (!health || item.health === health)
              );
            });
            const kinds = [...new Set(inventory.resources.map((item) => item.resource.kind))];
            const namespaces = [
              ...new Set(inventory.resources.map((item) => parts(item)[0]).filter(Boolean))
            ];
            return (
              <>
                <p>
                  {t("kubernetes.availability")}:{" "}
                  {inventory.availability.filter((item) => item.available).length}/
                  {inventory.availability.length}
                </p>
                <label>
                  {t("kubernetes.search")}{" "}
                  <input value={query} onChange={(event) => setQuery(event.target.value)} />
                </label>
                <label>
                  {t("kubernetes.kind")}{" "}
                  <select value={kind} onChange={(event) => setKind(event.target.value)}>
                    <option value="">{t("kubernetes.all")}</option>
                    {kinds.map((value) => (
                      <option key={value}>{value}</option>
                    ))}
                  </select>
                </label>
                <label>
                  {t("kubernetes.namespace")}{" "}
                  <select value={namespace} onChange={(event) => setNamespace(event.target.value)}>
                    <option value="">{t("kubernetes.all")}</option>
                    {namespaces.map((value) => (
                      <option key={value}>{value}</option>
                    ))}
                  </select>
                </label>
                <label>
                  {t("kubernetes.health")}{" "}
                  <select value={health} onChange={(event) => setHealth(event.target.value)}>
                    <option value="">{t("kubernetes.all")}</option>
                    {["healthy", "degraded", "crash_loop_back_off", "oom_killed", "pending"].map(
                      (value) => (
                        <option key={value}>{value}</option>
                      )
                    )}
                  </select>
                </label>
                <h3>{t("kubernetes.hierarchy")}</h3>
                <ul>
                  {resources.map((item) => {
                    const children = inventory.topology.filter(
                      (edge) =>
                        edge.from_kind === item.resource.kind &&
                        edge.from_name === item.resource.name
                    );
                    return (
                      <li key={`${item.resource.kind}-${item.resource.name}`}>
                        <button type="button" onClick={() => select(item)}>
                          {item.resource.kind}/{item.resource.name}
                        </button>{" "}
                        —{" "}
                        <StatusIndicator
                          state={
                            item.health === "healthy"
                              ? "healthy"
                              : item.health === "unknown"
                                ? "unavailable"
                                : "degraded"
                          }
                        />{" "}
                        {item.health}{" "}
                        {item.replicas &&
                          `${item.replicas.ready}/${item.replicas.desired} ${t("kubernetes.replicas")}`}
                        {children.length > 0 && (
                          <ul>
                            <li>
                              {item.resource.kind === "Service"
                                ? t("kubernetes.servicePods")
                                : t("kubernetes.owner")}
                              :{" "}
                              {children.map((edge) => `${edge.to_kind}/${edge.to_name}`).join(", ")}
                            </li>
                          </ul>
                        )}
                      </li>
                    );
                  })}
                </ul>
              </>
            );
          })()}
          {pod && (
            <section aria-label={t("kubernetes.podDetails")}>
              <h3>{pod.resource.name}</h3>
              <p>{t("kubernetes.readOnly")}</p>
              <button type="button" onClick={() => viewManifest(pod)}>
                {t("kubernetes.showManifest")}
              </button>
              <button type="button" onClick={() => copyCommand(pod)}>
                {t("kubernetes.copyKubectl")}
              </button>
              {consoleUrl(pod) && (
                <button type="button" onClick={() => void open(consoleUrl(pod)!)}>
                  {t("kubernetes.openConsole")}
                </button>
              )}
              <p>
                {t("kubernetes.owner")}: {pod.owner ? `${pod.owner.kind}/${pod.owner.name}` : "—"}
              </p>
              <ul>
                {pod.conditions.map((condition) => (
                  <li key={condition.type_}>
                    {condition.type_}: {condition.status} {condition.reason ?? ""}{" "}
                    {condition.message ?? ""}
                  </li>
                ))}
              </ul>
              <h4>{t("kubernetes.events")}</h4>
              <ul>
                {events.map((event, index) => (
                  <li key={`${event.reason}-${index}`}>
                    {event.reason}: {event.message}
                  </li>
                ))}
              </ul>
              <h4>{t("kubernetes.logs")}</h4>
              <pre>{logs}</pre>
              {manifest && (
                <section aria-label={t("kubernetes.manifest")}>
                  {manifest.masked && <p role="status">{t("kubernetes.sensitiveRedacted")}</p>}
                  <pre>{manifest.yaml}</pre>
                </section>
              )}
            </section>
          )}
        </>
      )}
    </Card>
  );
}
