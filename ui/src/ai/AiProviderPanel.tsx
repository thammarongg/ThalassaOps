import { useCallback, useEffect, useState } from "react";
import type {
  AiProvidersRequest,
  AiSetProviderOrderRequest,
  CommandEnvelope,
  Invoke,
  ProviderHealth,
  ProviderKind,
  ProviderSummary
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { isProviderSummary, isStringArray } from "../../contracts/guards";
import { useTranslation } from "../i18n";
import { AiFallbackOrder } from "./AiFallbackOrder";
import { AiProviderForm } from "./AiProviderForm";
import "./ai.css";

type AiProviderPanelProps = {
  invoke: Invoke;
  providerOrder: string[];
  onProviderOrderChange?: (nextOrder: string[]) => void;
};

const kindKeys = {
  open_ai_compatible: "ai.kinds.open_ai_compatible",
  anthropic: "ai.kinds.anthropic",
  ollama: "ai.kinds.ollama",
  vllm: "ai.kinds.vllm"
} satisfies Record<ProviderKind, string>;

const healthKeys = {
  healthy: "ai.healthStates.healthy",
  unreachable: "ai.healthStates.unreachable",
  unauthorized: "ai.healthStates.unauthorized",
  model_unavailable: "ai.healthStates.model_unavailable",
  rate_limited: "ai.healthStates.rate_limited",
  budget_exhausted: "ai.healthStates.budget_exhausted"
} satisfies Record<ProviderHealth, string>;

const aiEnvelope = <T,>(
  verb: string,
  capability: "ConnectorRead" | "ConnectorAct",
  payload: T
): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("ai", verb),
  capability,
  scope: { resource_ids: [] },
  payload
});

const healthClass = (health: ProviderHealth | null) => health ?? "unknown";

export function AiProviderPanel({
  invoke,
  providerOrder,
  onProviderOrderChange
}: AiProviderPanelProps) {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<ProviderSummary[]>([]);
  const [currentOrder, setCurrentOrder] = useState(providerOrder);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [formOpen, setFormOpen] = useState(false);
  const [editingProvider, setEditingProvider] = useState<ProviderSummary>();

  useEffect(() => setCurrentOrder(providerOrder), [providerOrder]);

  const loadProviders = useCallback(async () => {
    setLoading(true);
    setError("");
    try {
      const result = await invoke<AiProvidersRequest, ProviderSummary[]>("ai_providers", {
        envelope: aiEnvelope("providers", "ConnectorRead", {})
      });
      if (!result.ok || !Array.isArray(result.value) || !result.value.every(isProviderSummary)) {
        setError(t("ai.loadError"));
        return;
      }
      setProviders(result.value);
    } catch {
      setError(t("ai.loadError"));
    } finally {
      setLoading(false);
    }
  }, [invoke, t]);

  useEffect(() => {
    void loadProviders();
  }, [loadProviders]);

  const reorder = async (nextOrder: string[]) => {
    setError("");
    try {
      const result = await invoke<AiSetProviderOrderRequest, string[]>("ai_set_provider_order", {
        envelope: aiEnvelope("set_provider_order", "ConnectorAct", {
          provider_order: nextOrder
        })
      });
      if (!result.ok || !isStringArray(result.value)) {
        setError(t("ai.orderError"));
        return;
      }
      setCurrentOrder(result.value);
      onProviderOrderChange?.(result.value);
    } catch {
      setError(t("ai.orderError"));
    }
  };

  const openNewForm = () => {
    setEditingProvider(undefined);
    setFormOpen(true);
  };

  return (
    <section className="ai-provider-panel" aria-labelledby="ai-provider-panel-title">
      <header className="ai-provider-panel__header">
        <div>
          <p className="eyebrow">{t("ai.eyebrow")}</p>
          <h1 id="ai-provider-panel-title">{t("ai.title")}</h1>
          <p>{t("ai.description")}</p>
        </div>
        <button type="button" className="ai-button ai-button--primary" onClick={openNewForm}>
          {t("ai.addProvider")}
        </button>
      </header>

      {error && (
        <p className="ai-provider-panel__error" role="alert">
          {error}
        </p>
      )}
      {loading && (
        <p className="ai-provider-panel__loading" role="status">
          {t("ai.loading")}
        </p>
      )}
      {!loading && !error && providers.length === 0 && (
        <p className="ai-provider-panel__empty">{t("ai.empty")}</p>
      )}

      {!loading && providers.length > 0 && (
        <div className="ai-provider-panel__roster" aria-label={t("ai.providers")}>
          {providers.map((provider) => (
            <article className="ai-provider-row" key={provider.id}>
              <header className="ai-provider-row__header">
                <div>
                  <p className="ai-provider-row__kind">{t(kindKeys[provider.kind])}</p>
                  <h2>{provider.id}</h2>
                </div>
                <span className={`ai-health ai-health--${healthClass(provider.health)}`}>
                  <span aria-hidden="true">{provider.health === null ? "?" : "●"}</span>
                  {provider.health === null
                    ? t("ai.healthStates.unknown")
                    : t(healthKeys[provider.health])}
                </span>
              </header>
              <dl className="ai-provider-row__details">
                <div>
                  <dt>{t("ai.endpoint")}</dt>
                  <dd>
                    <code>{provider.endpoint}</code>
                  </dd>
                </div>
                <div>
                  <dt>{t("ai.credential")}</dt>
                  <dd>
                    {provider.credential_configured
                      ? t("ai.credentialConfigured")
                      : t("ai.credentialNotConfigured")}
                  </dd>
                </div>
                <div>
                  <dt>{t("ai.models")}</dt>
                  <dd>{provider.models.length}</dd>
                </div>
              </dl>
              <button
                type="button"
                className="ai-button ai-button--quiet"
                onClick={() => {
                  setEditingProvider(provider);
                  setFormOpen(true);
                }}
              >
                {t("ai.editProvider")}
              </button>
            </article>
          ))}
        </div>
      )}

      {!loading && (
        <AiFallbackOrder
          providers={providers}
          providerOrder={currentOrder}
          onReorder={(nextOrder) => void reorder(nextOrder)}
        />
      )}

      {formOpen && (
        <div className="ai-provider-panel__form-shell">
          <AiProviderForm
            invoke={invoke}
            provider={editingProvider}
            onCancel={() => setFormOpen(false)}
            onSaved={() => {
              setFormOpen(false);
              void loadProviders();
            }}
          />
        </div>
      )}
    </section>
  );
}
