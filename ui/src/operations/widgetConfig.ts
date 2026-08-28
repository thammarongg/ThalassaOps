import type { WidgetDefinition, WidgetId, WidgetPreference, WidgetSize } from "../../contracts/ipc";

export const OPERATIONS_LAYOUT_STORAGE_KEY = "thalassaops.operations-console.layout.v1";
export const OPERATIONS_LAYOUT_VERSION = 1 as const;

export const CURATED_WIDGET_DEFINITIONS: WidgetDefinition[] = [
  {
    id: "health_summary",
    title_key: "operations.health_summary",
    default_order: 0,
    default_size: "wide",
    required: true
  },
  {
    id: "incident_queue",
    title_key: "operations.incident_queue",
    default_order: 1,
    default_size: "wide",
    required: true
  },
  {
    id: "signal_summary",
    title_key: "operations.signal_summary",
    default_order: 2,
    default_size: "standard",
    required: false
  },
  {
    id: "change_stream",
    title_key: "operations.change_stream",
    default_order: 3,
    default_size: "standard",
    required: false
  },
  {
    id: "environment_status",
    title_key: "operations.environment_status",
    default_order: 4,
    default_size: "wide",
    required: false
  }
];

const knownWidgetIds = new Set<WidgetId>([
  "health_summary",
  "incident_queue",
  "signal_summary",
  "change_stream",
  "environment_status"
]);
const requiredWidgetIds = new Set<WidgetId>(["health_summary", "incident_queue"]);
const widgetSizes = new Set<WidgetSize>(["compact", "standard", "wide"]);

type StoredLayout = {
  version: number;
  preferences: unknown;
};

const isWidgetId = (value: unknown): value is WidgetId =>
  typeof value === "string" && knownWidgetIds.has(value as WidgetId);

const isWidgetSize = (value: unknown): value is WidgetSize =>
  typeof value === "string" && widgetSizes.has(value as WidgetSize);

const isPreference = (value: unknown): value is WidgetPreference => {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<WidgetPreference>;
  return (
    isWidgetId(candidate.id) &&
    typeof candidate.visible === "boolean" &&
    typeof candidate.order === "number" &&
    Number.isSafeInteger(candidate.order) &&
    candidate.order >= 0 &&
    isWidgetSize(candidate.size) &&
    typeof candidate.collapsed === "boolean"
  );
};

const definitionsFor = (registry: WidgetDefinition[] = CURATED_WIDGET_DEFINITIONS) => {
  const known = new Map<WidgetId, WidgetDefinition>();
  for (const definition of registry) {
    if (knownWidgetIds.has(definition.id) && !known.has(definition.id)) {
      known.set(definition.id, definition);
    }
  }
  return CURATED_WIDGET_DEFINITIONS.map((fallback) => known.get(fallback.id) ?? fallback).sort(
    (left, right) => left.default_order - right.default_order
  );
};

export const defaultWidgetPreferences = (
  registry: WidgetDefinition[] = CURATED_WIDGET_DEFINITIONS
): WidgetPreference[] =>
  definitionsFor(registry).map((definition, order) => ({
    id: definition.id,
    visible: true,
    order,
    size: definition.default_size,
    collapsed: false
  }));

/**
 * Reconcile untrusted presentation state against the backend-owned registry.
 * Required widgets are forced visible and remain ahead of optional widgets.
 */
export const reconcileWidgetPreferences = (
  registry: WidgetDefinition[],
  candidate: unknown
): WidgetPreference[] => {
  const definitions = definitionsFor(registry);
  const defaults = defaultWidgetPreferences(definitions);
  const byId = new Map<WidgetId, WidgetPreference>();
  if (Array.isArray(candidate)) {
    for (const value of candidate) {
      if (!isPreference(value) || byId.has(value.id)) continue;
      byId.set(value.id, value);
    }
  }

  const merged = defaults.map((fallback) => {
    const definition = definitions.find((item) => item.id === fallback.id);
    const preference = byId.get(fallback.id);
    return {
      id: fallback.id,
      visible: requiredWidgetIds.has(fallback.id) ? true : (preference?.visible ?? true),
      order: preference?.order ?? fallback.order,
      size: preference?.size ?? definition?.default_size ?? fallback.size,
      collapsed: preference?.collapsed ?? false
    } satisfies WidgetPreference;
  });

  return [...merged]
    .sort((left, right) => {
      const leftRequired = requiredWidgetIds.has(left.id);
      const rightRequired = requiredWidgetIds.has(right.id);
      if (leftRequired !== rightRequired) return leftRequired ? -1 : 1;
      if (!leftRequired) return left.order - right.order || left.id.localeCompare(right.id);
      const leftDefinition = definitions.find((item) => item.id === left.id);
      const rightDefinition = definitions.find((item) => item.id === right.id);
      return (
        (leftDefinition?.default_order ?? left.order) -
        (rightDefinition?.default_order ?? right.order)
      );
    })
    .map((preference, order) => ({ ...preference, order }));
};

export const readWidgetPreferences = (
  registry: WidgetDefinition[] = CURATED_WIDGET_DEFINITIONS
): WidgetPreference[] => {
  try {
    const raw = localStorage.getItem(OPERATIONS_LAYOUT_STORAGE_KEY);
    if (!raw) return defaultWidgetPreferences(registry);
    const parsed = JSON.parse(raw) as Partial<StoredLayout>;
    if (parsed.version !== OPERATIONS_LAYOUT_VERSION || !Array.isArray(parsed.preferences)) {
      return defaultWidgetPreferences(registry);
    }
    return reconcileWidgetPreferences(registry, parsed.preferences);
  } catch {
    return defaultWidgetPreferences(registry);
  }
};

export const persistWidgetPreferences = (preferences: WidgetPreference[]) => {
  try {
    localStorage.setItem(
      OPERATIONS_LAYOUT_STORAGE_KEY,
      JSON.stringify({ version: OPERATIONS_LAYOUT_VERSION, preferences })
    );
  } catch {
    // Local storage is an optional presentation cache and must never block the console.
  }
};

export const updateWidgetPreference = (
  preferences: WidgetPreference[],
  id: WidgetId,
  update: (preference: WidgetPreference) => WidgetPreference,
  registry: WidgetDefinition[] = CURATED_WIDGET_DEFINITIONS
) =>
  reconcileWidgetPreferences(
    registry,
    preferences.map((preference) => (preference.id === id ? update(preference) : preference))
  );

export const moveWidget = (
  preferences: WidgetPreference[],
  id: WidgetId,
  direction: "up" | "down",
  registry: WidgetDefinition[] = CURATED_WIDGET_DEFINITIONS
) => {
  const next = [...preferences].sort((left, right) => left.order - right.order);
  const index = next.findIndex((preference) => preference.id === id);
  if (index < 0) return reconcileWidgetPreferences(registry, preferences);
  const adjacentIndex = direction === "up" ? index - 1 : index + 1;
  if (adjacentIndex < 0 || adjacentIndex >= next.length) {
    return reconcileWidgetPreferences(registry, preferences);
  }
  [next[index], next[adjacentIndex]] = [next[adjacentIndex], next[index]];
  return reconcileWidgetPreferences(
    registry,
    next.map((preference, position) => ({ ...preference, order: position }))
  );
};
