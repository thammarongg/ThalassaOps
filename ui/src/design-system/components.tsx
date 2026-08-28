import { useEffect, useRef, useState, type PropsWithChildren, type ReactNode } from "react";
import { useTranslation } from "../i18n";

export type StatusState =
  | "healthy"
  | "degraded"
  | "unavailable"
  | "warning"
  | "critical"
  | "unknown";
export type Severity = "s1" | "s2" | "s3" | "s4" | "s5";
type TranslationKey = string;
type IndicatorTone = StatusState | "informational";

const statusSymbol: Record<StatusState, string> = {
  healthy: "●",
  degraded: "◐",
  unavailable: "■",
  warning: "▲",
  critical: "!",
  unknown: "?"
};

// Severity communicates business impact, while status communicates operational health; these reuse tones only as visual priority.
const severityTone: Record<Severity, IndicatorTone> = {
  s1: "critical",
  s2: "warning",
  s3: "degraded",
  s4: "healthy",
  s5: "informational"
};

export function StatusIndicator({ state, severity }: { state?: StatusState; severity?: Severity }) {
  const { t } = useTranslation();
  const label = severity ? t(`severity.${severity}`) : t(`status.${state ?? "healthy"}`);
  const tone = severity ? severityTone[severity] : (state ?? "healthy");
  return (
    <span className={`indicator indicator--${tone}`}>
      <span aria-hidden="true">{severity ?? statusSymbol[state ?? "healthy"]}</span>
      <span>{label}</span>
    </span>
  );
}

export function Card({ titleKey, children }: PropsWithChildren<{ titleKey: TranslationKey }>) {
  const { t } = useTranslation();
  return (
    <section className="card">
      <h2>{t(titleKey)}</h2>
      {children}
    </section>
  );
}

export function EmptyState({
  titleKey,
  children
}: PropsWithChildren<{ titleKey: TranslationKey }>) {
  const { t } = useTranslation();
  return (
    <div className="empty-state">
      <span aria-hidden="true">○</span>
      <p>{t(titleKey)}</p>
      {children}
    </div>
  );
}

export function Table({
  captionKey,
  columns,
  rows
}: {
  captionKey: TranslationKey;
  columns: { key: string; headerKey: TranslationKey }[];
  rows: Array<{ id: string; [key: string]: ReactNode }>;
}) {
  const { t } = useTranslation();
  return (
    <div className="table-wrap">
      <table>
        <caption>{t(captionKey)}</caption>
        <thead>
          <tr>
            {columns.map((column) => (
              <th key={column.key} scope="col">
                {t(column.headerKey)}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.id}>
              {columns.map((column) => (
                <td key={column.key}>{row[column.key]}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function Tabs({
  items,
  children
}: {
  items: { id: string; labelKey: TranslationKey }[];
  children: (active: string) => ReactNode;
}) {
  const { t } = useTranslation();
  const [active, setActive] = useState(items[0]?.id ?? "");
  return (
    <section>
      <div className="tabs" role="tablist">
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            role="tab"
            aria-selected={active === item.id}
            className="tab"
            onClick={() => setActive(item.id)}
          >
            {t(item.labelKey)}
          </button>
        ))}
      </div>
      <div role="tabpanel">{children(active)}</div>
    </section>
  );
}

export function Drawer({
  titleKey,
  isOpen = true,
  onClose,
  children
}: PropsWithChildren<{ titleKey: TranslationKey; isOpen?: boolean; onClose?: () => void }>) {
  const { t } = useTranslation();
  const dialog = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!isOpen) return;
    const element = dialog.current;
    element?.focus();
    const trapFocus = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || !element) return;
      const focusable = Array.from(
        element.querySelectorAll<HTMLElement>(
          "button, input, select, textarea, a[href], [tabindex]:not([tabindex='-1'])"
        )
      );
      if (!focusable.length) {
        event.preventDefault();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      }
      if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    document.addEventListener("keydown", trapFocus);
    return () => document.removeEventListener("keydown", trapFocus);
  }, [isOpen]);
  if (!isOpen) return null;
  return (
    <div className="drawer-backdrop" role="presentation">
      <div
        ref={dialog}
        className="drawer"
        role="dialog"
        aria-modal="true"
        aria-label={t(titleKey)}
        tabIndex={-1}
        onKeyDown={(event) => {
          if (event.key === "Escape") onClose?.();
        }}
      >
        <div className="drawer-header">
          <h2>{t(titleKey)}</h2>
          {onClose && (
            <button type="button" onClick={onClose} aria-label={t("demo.close")}>
              ×
            </button>
          )}
        </div>
        {children}
      </div>
    </div>
  );
}

export function Timeline({
  items
}: {
  items: { id: string; titleKey: TranslationKey; state: StatusState }[];
}) {
  const { t } = useTranslation();
  return (
    <ol className="timeline">
      {items.map((item) => (
        <li key={item.id}>
          <StatusIndicator state={item.state} />
          <span>{t(item.titleKey)}</span>
        </li>
      ))}
    </ol>
  );
}

export function CommandSurface({
  labelKey,
  placeholderKey,
  onSubmit,
  onChange
}: {
  labelKey: TranslationKey;
  placeholderKey: TranslationKey;
  onSubmit?: (query: string) => void;
  onChange?: (query: string) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  return (
    <form
      className="command-surface"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit?.(query);
      }}
    >
      <label htmlFor="command-input">{t(labelKey)}</label>
      <input
        id="command-input"
        value={query}
        onChange={(event) => {
          setQuery(event.target.value);
          onChange?.(event.target.value);
        }}
        placeholder={t(placeholderKey)}
      />
    </form>
  );
}
