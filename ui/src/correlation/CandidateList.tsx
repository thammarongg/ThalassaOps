import type { CorrelationCandidate } from "../../contracts/ipc";
import { StatusIndicator } from "../design-system/components";
import { useTranslation } from "../i18n";

const statusIndicator = (status: CorrelationCandidate["status"]) => {
  if (status === "active") return "healthy" as const;
  if (status === "provisional") return "warning" as const;
  return "degraded" as const;
};

export function CandidateList({
  candidates,
  selectedId,
  onSelect
}: {
  candidates: CorrelationCandidate[];
  selectedId: string | null;
  onSelect: (candidate: CorrelationCandidate) => void;
}) {
  const { t } = useTranslation();
  if (candidates.length === 0) {
    return <p className="correlation-empty">{t("correlation.candidates.empty")}</p>;
  }
  return (
    <ul className="correlation-candidate-list">
      {candidates.map((candidate) => (
        <li key={candidate.id}>
          <button
            type="button"
            className="correlation-candidate"
            aria-pressed={selectedId === candidate.id}
            aria-label={t("correlation.candidates.select", {
              id: candidate.id,
              reason: candidate.reasons
                .map((reason) => t("correlation.reasons." + reason.kind))
                .join(" · ")
            })}
            onClick={() => onSelect(candidate)}
          >
            <span className="correlation-candidate__id">{candidate.id}</span>
            <span className="correlation-candidate__status">
              <StatusIndicator state={statusIndicator(candidate.status)} />{" "}
              <span>{t("correlation.candidateStatus." + candidate.status)}</span>
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
