import { useEffect, useMemo, useState } from "react";
import type { CommandEnvelope, IpcResult, WorkspaceContext } from "../contracts/ipc";
import { command } from "../contracts/ipc";
import { CommandSurface, Drawer, EmptyState, StatusIndicator } from "./design-system/components";
import { useTranslation } from "./i18n";

type Invoke = (
  command: string,
  args: Record<string, unknown>
) => Promise<IpcResult<WorkspaceContext>>;
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
          setContext(result.value);
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
        <EmptyState titleKey="shell.routeUnavailable" />
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
