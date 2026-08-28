import type { IncidentQueueItem, TopologyDirection } from "../../contracts/ipc";
import { useTranslation } from "../i18n";

export type EnvironmentOption = { id: string; name: string };
export type TeamOption = { id: string; name: string };

const ALL = "all";
const NO_INCIDENT = "";

export function TopologyFilters({
  environments,
  teams,
  incidents,
  environment,
  team,
  incident,
  direction,
  maxDepth,
  onEnvironmentChange,
  onTeamChange,
  onIncidentChange,
  onDirectionChange,
  onMaxDepthChange
}: {
  environments: EnvironmentOption[];
  teams: TeamOption[];
  incidents: IncidentQueueItem[];
  environment: string;
  team: string;
  incident: string;
  direction: TopologyDirection;
  maxDepth: number;
  onEnvironmentChange: (environment: string) => void;
  onTeamChange: (team: string) => void;
  onIncidentChange: (incident: string) => void;
  onDirectionChange: (direction: TopologyDirection) => void;
  onMaxDepthChange: (maxDepth: number) => void;
}) {
  const { t } = useTranslation();
  return (
    <section className="topology-filters" aria-label={t("topology.filters.title")}>
      <div className="topology-filters__field">
        <label htmlFor="topology-filter-environment">{t("topology.filters.environment")}</label>
        <select
          id="topology-filter-environment"
          value={environment}
          onChange={(event) => onEnvironmentChange(event.target.value)}
        >
          <option value={ALL}>{t("topology.filters.environmentAll")}</option>
          {environments.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
      </div>
      <div className="topology-filters__field">
        <label htmlFor="topology-filter-team">{t("topology.filters.team")}</label>
        <select
          id="topology-filter-team"
          value={team}
          onChange={(event) => onTeamChange(event.target.value)}
        >
          <option value={ALL}>{t("topology.filters.teamAll")}</option>
          {teams.map((item) => (
            <option key={item.id} value={item.id}>
              {item.name}
            </option>
          ))}
        </select>
      </div>
      <div className="topology-filters__field">
        <label htmlFor="topology-filter-incident">{t("topology.filters.incident")}</label>
        <select
          id="topology-filter-incident"
          value={incident}
          onChange={(event) => onIncidentChange(event.target.value)}
        >
          <option value={NO_INCIDENT}>{t("topology.filters.incidentNone")}</option>
          {incidents.map((item) => (
            <option key={item.id} value={item.id}>
              {item.title}
            </option>
          ))}
        </select>
      </div>
      <div className="topology-filters__field">
        <label htmlFor="topology-filter-direction">{t("topology.filters.direction")}</label>
        <select
          id="topology-filter-direction"
          value={direction}
          onChange={(event) => onDirectionChange(event.target.value as TopologyDirection)}
        >
          <option value="upstream">{t("topology.filters.directionUpstream")}</option>
          <option value="downstream">{t("topology.filters.directionDownstream")}</option>
          <option value="both">{t("topology.filters.directionBoth")}</option>
        </select>
      </div>
      <div className="topology-filters__field">
        <label htmlFor="topology-filter-depth">{t("topology.filters.maxDepth")}</label>
        <select
          id="topology-filter-depth"
          value={maxDepth}
          onChange={(event) => onMaxDepthChange(Number(event.target.value))}
        >
          {[0, 1, 2, 3, 4, 5, 6, 7, 8].map((depth) => (
            <option key={depth} value={depth}>
              {t("topology.filters.depthValue", { depth })}
            </option>
          ))}
        </select>
      </div>
    </section>
  );
}
