import type { ProviderSummary } from "../../contracts/ipc";
import { useTranslation } from "../i18n";

type AiFallbackOrderProps = {
  providers: ProviderSummary[];
  providerOrder: string[];
  onReorder: (nextOrder: string[]) => void;
};

export function AiFallbackOrder({ providers, providerOrder, onReorder }: AiFallbackOrderProps) {
  const { t } = useTranslation();
  const configured = new Set(providerOrder);
  const orderedProviders = providerOrder.flatMap((providerId) => {
    const provider = providers.find((candidate) => candidate.id === providerId);
    return provider === undefined ? [] : [provider];
  });
  const omittedProviders = providers.filter((provider) => !configured.has(provider.id));

  const move = (index: number, offset: -1 | 1) => {
    const nextOrder = [...providerOrder];
    const destination = index + offset;
    if (index < 0 || destination < 0 || destination >= nextOrder.length) return;
    [nextOrder[index], nextOrder[destination]] = [nextOrder[destination], nextOrder[index]];
    onReorder(nextOrder);
  };

  return (
    <section className="ai-fallback-order" aria-labelledby="ai-fallback-order-title">
      <header className="ai-fallback-order__header">
        <div>
          <p className="eyebrow">{t("ai.fallback.eyebrow")}</p>
          <h2 id="ai-fallback-order-title">{t("ai.fallback.title")}</h2>
        </div>
        <span className="ai-fallback-order__count">{orderedProviders.length}</span>
      </header>
      <p className="ai-fallback-order__description">{t("ai.fallback.description")}</p>

      {orderedProviders.length === 0 && (
        <p className="ai-fallback-order__off" role="status">
          {t("ai.fallback.failoverOff")}
        </p>
      )}

      {orderedProviders.length > 0 && (
        <ol className="ai-fallback-order__list">
          {orderedProviders.map((provider, index) => (
            <li className="ai-fallback-order__item" key={provider.id}>
              <div className="ai-fallback-order__provider">
                <span className="ai-fallback-order__position">
                  {t("ai.fallback.position", { position: index + 1 })}
                </span>
                <strong>{provider.id}</strong>
                <code>{provider.endpoint}</code>
              </div>
              <div className="ai-fallback-order__actions">
                <button
                  type="button"
                  aria-label={t("ai.fallback.moveUp", { id: provider.id })}
                  disabled={index === 0}
                  onClick={() => move(index, -1)}
                >
                  ↑
                </button>
                <button
                  type="button"
                  aria-label={t("ai.fallback.moveDown", { id: provider.id })}
                  disabled={index === orderedProviders.length - 1}
                  onClick={() => move(index, 1)}
                >
                  ↓
                </button>
              </div>
            </li>
          ))}
        </ol>
      )}

      {omittedProviders.length > 0 && (
        <div className="ai-fallback-order__omitted">
          <h3>{t("ai.fallback.configuredProviders")}</h3>
          <ul>
            {omittedProviders.map((provider) => (
              <li key={provider.id}>
                <div className="ai-fallback-order__omitted-row">
                  <span>{t("ai.fallback.notFallback", { id: provider.id })}</span>
                  <button
                    type="button"
                    className="ai-button ai-button--quiet"
                    aria-label={t("ai.fallback.addToOrder", { id: provider.id })}
                    onClick={() => onReorder([...providerOrder, provider.id])}
                  >
                    {t("ai.fallback.addToOrderText")}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
