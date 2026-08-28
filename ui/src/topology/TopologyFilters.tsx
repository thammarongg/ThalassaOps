import type { IncidentQueueItem } from "../../contracts/ipc";
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
  onEnvironmentChange,
  onTeamChange,
  onIncidentChange
}: {
  environments: EnvironmentOption[];
  teams: TeamOption[];
  incidents: IncidentQueueItem[];
  environment: string;
  team: string;
  incident: string;
  onEnvironmentChange: (environment: string) => void;
  onTeamChange: (team: string) => void;
  onIncidentChange: (incident: string) => void;
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
    </section>
  );
}
