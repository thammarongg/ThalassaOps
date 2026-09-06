import { useEffect, useState } from "react";
import type {
  AiConfigureProviderRequest,
  CommandEnvelope,
  Invoke,
  ModelDescriptor,
  ProviderKind,
  ProviderSummary
} from "../../contracts/ipc";
import { command } from "../../contracts/ipc";
import { isProviderSummary } from "../../contracts/guards";
import { useTranslation } from "../i18n";

type ModelDraft = {
  id: string;
  context_window_tokens: string;
  max_output_tokens: string;
  supports_system_instruction: boolean;
  input_cost_micros_per_million_tokens: string;
  output_cost_micros_per_million_tokens: string;
};

type AiProviderFormProps = {
  invoke: Invoke;
  provider?: ProviderSummary;
  onSaved?: (provider: ProviderSummary) => void;
  onCancel?: () => void;
};

const providerKinds: ProviderKind[] = ["open_ai_compatible", "anthropic", "ollama", "vllm"];

const toModelDraft = (model: ModelDescriptor): ModelDraft => ({
  id: model.id,
  context_window_tokens: String(model.context_window_tokens),
  max_output_tokens: String(model.max_output_tokens),
  supports_system_instruction: model.supports_system_instruction,
  input_cost_micros_per_million_tokens:
    model.input_cost_micros_per_million_tokens === null
      ? ""
      : String(model.input_cost_micros_per_million_tokens),
  output_cost_micros_per_million_tokens:
    model.output_cost_micros_per_million_tokens === null
      ? ""
      : String(model.output_cost_micros_per_million_tokens)
});

const blankModel = (): ModelDraft => ({
  id: "",
  context_window_tokens: "",
  max_output_tokens: "",
  supports_system_instruction: false,
  input_cost_micros_per_million_tokens: "",
  output_cost_micros_per_million_tokens: ""
});

const initialModels = (provider?: ProviderSummary): ModelDraft[] =>
  provider?.models.map(toModelDraft) ?? [];

const initialKind = (provider?: ProviderSummary): ProviderKind | "" => provider?.kind ?? "";

const unsignedInteger = (value: string): number | undefined => {
  if (value.trim() === "") return undefined;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : undefined;
};

const nullableUnsignedInteger = (value: string): number | null | undefined => {
  if (value.trim() === "") return null;
  return unsignedInteger(value);
};

const envelope = <T,>(verb: string, payload: T): CommandEnvelope<T> => ({
  request_id: crypto.randomUUID(),
  command: command("ai", verb),
  capability: "ConnectorAct",
  scope: { resource_ids: [] },
  payload
});

export function AiProviderForm({ invoke, provider, onSaved, onCancel }: AiProviderFormProps) {
  const { t } = useTranslation();
  const [id, setId] = useState(provider?.id ?? "");
  const [kind, setKind] = useState<ProviderKind | "">(initialKind(provider));
  const [endpoint, setEndpoint] = useState(provider?.endpoint ?? "");
  const [models, setModels] = useState<ModelDraft[]>(initialModels(provider));
  const [credential, setCredential] = useState("");
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    setId(provider?.id ?? "");
    setKind(initialKind(provider));
    setEndpoint(provider?.endpoint ?? "");
    setModels(initialModels(provider));
    setCredential("");
    setError("");
  }, [provider]);

  const updateModel = <K extends keyof ModelDraft>(
    index: number,
    field: K,
    value: ModelDraft[K]
  ) => {
    setModels((current) =>
      current.map((model, modelIndex) =>
        modelIndex === index ? { ...model, [field]: value } : model
      )
    );
  };

  const validateAndBuild = (): AiConfigureProviderRequest | undefined => {
    if (id.trim() === "") {
      setError(t("ai.form.providerIdRequired"));
      return undefined;
    }
    if (kind === "") {
      setError(t("ai.form.kindRequired"));
      return undefined;
    }
    if (endpoint.trim() === "") {
      setError(t("ai.form.endpointRequired"));
      return undefined;
    }
    if (models.length === 0) {
      setError(t("ai.form.modelRequired"));
      return undefined;
    }

    const descriptors: ModelDescriptor[] = [];
    for (const model of models) {
      if (model.id.trim() === "") {
        setError(t("ai.form.modelIdRequired"));
        return undefined;
      }
      const contextWindow = unsignedInteger(model.context_window_tokens);
      const maxOutput = unsignedInteger(model.max_output_tokens);
      const inputPrice = nullableUnsignedInteger(model.input_cost_micros_per_million_tokens);
      const outputPrice = nullableUnsignedInteger(model.output_cost_micros_per_million_tokens);
      if (
        contextWindow === undefined ||
        maxOutput === undefined ||
        inputPrice === undefined ||
        outputPrice === undefined
      ) {
        setError(t("ai.form.numberRequired"));
        return undefined;
      }
      descriptors.push({
        id: model.id.trim(),
        context_window_tokens: contextWindow,
        max_output_tokens: maxOutput,
        supports_system_instruction: model.supports_system_instruction,
        input_cost_micros_per_million_tokens: inputPrice,
        output_cost_micros_per_million_tokens: outputPrice
      });
    }

    const payload: AiConfigureProviderRequest = {
      id: id.trim(),
      kind,
      endpoint: endpoint.trim(),
      models: descriptors
    };
    const nextCredential = credential.trim();
    if (nextCredential !== "") payload.credential = nextCredential;
    return payload;
  };

  const submit = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError("");
    const payload = validateAndBuild();
    if (payload === undefined) return;

    setSaving(true);
    try {
      const result = await invoke<AiConfigureProviderRequest, ProviderSummary>(
        "ai_configure_provider",
        { envelope: envelope("configure_provider", payload) }
      );
      if (!result.ok || !isProviderSummary(result.value)) {
        setError(t("ai.form.saveFailed"));
        return;
      }
      setCredential("");
      onSaved?.(result.value);
    } catch {
      setError(t("ai.form.saveFailed"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <form className="ai-provider-form" onSubmit={(event) => void submit(event)}>
      <header className="ai-provider-form__header">
        <div>
          <p className="eyebrow">{t("ai.form.eyebrow")}</p>
          <h2>{provider === undefined ? t("ai.form.addTitle") : t("ai.form.editTitle")}</h2>
        </div>
        {onCancel && (
          <button type="button" className="ai-button ai-button--quiet" onClick={onCancel}>
            {t("ai.form.cancel")}
          </button>
        )}
      </header>

      {error && (
        <p className="ai-provider-form__error" role="alert">
          {error}
        </p>
      )}

      <div className="ai-provider-form__grid">
        <label>
          <span>{t("ai.form.providerId")}</span>
          <input value={id} onChange={(event) => setId(event.target.value)} />
        </label>
        <label>
          <span>{t("ai.form.kind")}</span>
          <select
            value={kind}
            onChange={(event) => setKind(event.target.value as ProviderKind | "")}
          >
            <option value="">{t("ai.form.chooseKind")}</option>
            {providerKinds.map((providerKind) => (
              <option key={providerKind} value={providerKind}>
                {t(`ai.kinds.${providerKind}`)}
              </option>
            ))}
          </select>
        </label>
        <label className="ai-provider-form__wide-field">
          <span>{t("ai.form.endpoint")}</span>
          <input value={endpoint} onChange={(event) => setEndpoint(event.target.value)} />
        </label>
        <label className="ai-provider-form__wide-field">
          <span>{t("ai.form.credential")}</span>
          <input
            type="password"
            value={credential}
            onChange={(event) => setCredential(event.target.value)}
            autoComplete="new-password"
          />
          <small>
            {provider?.credential_configured
              ? t("ai.form.credentialKeep")
              : t("ai.form.credentialOptional")}
          </small>
        </label>
      </div>

      <div className="ai-provider-form__models">
        <div className="ai-provider-form__section-heading">
          <div>
            <h3>{t("ai.form.models")}</h3>
            <p>{t("ai.form.modelsDescription")}</p>
          </div>
          <button
            type="button"
            className="ai-button ai-button--quiet"
            onClick={() => setModels((current) => [...current, blankModel()])}
          >
            {t("ai.form.addModel")}
          </button>
        </div>
        {models.map((model, index) => (
          <fieldset className="ai-model-card" key={`${model.id}-${index}`}>
            <legend>{t("ai.form.modelNumber", { position: index + 1 })}</legend>
            <label>
              <span>{t("ai.form.modelId")}</span>
              <input
                value={model.id}
                onChange={(event) => updateModel(index, "id", event.target.value)}
              />
            </label>
            <label>
              <span>{t("ai.form.contextWindow")}</span>
              <input
                inputMode="numeric"
                value={model.context_window_tokens}
                onChange={(event) =>
                  updateModel(index, "context_window_tokens", event.target.value)
                }
              />
            </label>
            <label>
              <span>{t("ai.form.maxOutput")}</span>
              <input
                inputMode="numeric"
                value={model.max_output_tokens}
                onChange={(event) => updateModel(index, "max_output_tokens", event.target.value)}
              />
            </label>
            <label>
              <span>{t("ai.form.inputPrice")}</span>
              <input
                inputMode="numeric"
                value={model.input_cost_micros_per_million_tokens}
                onChange={(event) =>
                  updateModel(index, "input_cost_micros_per_million_tokens", event.target.value)
                }
              />
            </label>
            <label>
              <span>{t("ai.form.outputPrice")}</span>
              <input
                inputMode="numeric"
                value={model.output_cost_micros_per_million_tokens}
                onChange={(event) =>
                  updateModel(index, "output_cost_micros_per_million_tokens", event.target.value)
                }
              />
            </label>
            <label className="ai-model-card__checkbox">
              <input
                type="checkbox"
                checked={model.supports_system_instruction}
                onChange={(event) =>
                  updateModel(index, "supports_system_instruction", event.target.checked)
                }
              />
              <span>{t("ai.form.supportsSystem")}</span>
            </label>
            <button
              type="button"
              className="ai-button ai-button--danger ai-model-card__remove"
              aria-label={t("ai.form.removeModel", { position: index + 1 })}
              onClick={() =>
                setModels((current) => current.filter((_, modelIndex) => modelIndex !== index))
              }
            >
              {t("ai.form.removeModelText")}
            </button>
          </fieldset>
        ))}
        {models.length === 0 && (
          <p className="ai-provider-form__empty-models">{t("ai.form.noModels")}</p>
        )}
      </div>

      <footer className="ai-provider-form__footer">
        <button type="submit" className="ai-button ai-button--primary" disabled={saving}>
          {saving ? t("ai.form.saving") : t("ai.form.save")}
        </button>
      </footer>
    </form>
  );
}
