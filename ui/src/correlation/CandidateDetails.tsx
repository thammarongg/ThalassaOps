import type {
  CorrelationCandidate,
  CorrelationReason,
  Signal,
  SignalTarget
} from "../../contracts/ipc";
import { StatusIndicator } from "../design-system/components";
import { useTranslation } from "../i18n";

const statusIndicator = (status: CorrelationCandidate["status"]) => {
  if (status === "active") return "healthy" as const;
  if (status === "provisional") return "warning" as const;
  return "degraded" as const;
};

const sourceKey = (source: Signal["source"]) => "correlation.sources." + source;
const kindKey = (kind: Signal["kind"]) => "correlation.signalKinds." + kind;
const targetKey = (target: SignalTarget) => "correlation.targetKinds." + target.kind;
const businessSeverityKey = (severity: NonNullable<Signal["business_severity"]>) =>
  "severity." + severity.toLowerCase();

const signalValue = (value: number | null, fallback: string) =>
  value === null ? fallback : String(value);

function SignalDetails({
  signal,
  onOpenEvidence
}: {
  signal: Signal;
  onOpenEvidence: (subject: string, evidenceIds: string[]) => void;
}) {
  const { t } = useTranslation();
  const securityFinding =
    typeof signal.payload === "object" && "security_finding" in signal.payload
      ? signal.payload.security_finding.finding
      : null;
  const anomaly =
    typeof signal.payload === "object" && "anomaly" in signal.payload
      ? signal.payload.anomaly
      : null;
  const healthCheck =
    typeof signal.payload === "object" && "health_check" in signal.payload
      ? signal.payload.health_check
      : null;
  return (
    <li className="correlation-signal">
      <details open>
        <summary className="correlation-signal__summary">
          <div className="correlation-signal__heading">
            <strong>{t(sourceKey(signal.source))}</strong>
            <span>{t(kindKey(signal.kind))}</span>
            <span className="correlation-signal__state">
              <StatusIndicator state={signal.state === "active" ? "healthy" : "unknown"} />{" "}
              {t("correlation.signalStates." + signal.state)}
            </span>
          </div>
        </summary>
        <div className="correlation-signal__body">
          <dl>
            {signal.source_record.native_id && (
              <div>
                <dt>{t("correlation.details.nativeId")}</dt>
                <dd>{signal.source_record.native_id}</dd>
              </div>
            )}
            {signal.source_record.revision && (
              <div>
                <dt>{t("correlation.details.revision")}</dt>
                <dd>{signal.source_record.revision}</dd>
              </div>
            )}
            <div>
              <dt>{t("correlation.details.target")}</dt>
              <dd>
                {signal.targets.length > 0
                  ? signal.targets
                      .map((target) => t(targetKey(target)) + ": " + target.id)
                      .join(" · ")
                  : t("correlation.details.noTarget")}
              </dd>
            </div>
            {signal.business_severity && (
              <div>
                <dt>{t("correlation.details.businessSeverity")}</dt>
                <dd>{t(businessSeverityKey(signal.business_severity))}</dd>
              </div>
            )}
            {securityFinding && (
              <>
                <div>
                  <dt>{t("correlation.details.assetKind")}</dt>
                  <dd>{t("correlation.assetKinds." + securityFinding.asset.kind)}</dd>
                </div>
                {securityFinding.severity !== null && (
                  <div>
                    <dt>{t("correlation.details.findingSeverity")}</dt>
                    <dd>{t("correlation.findingSeverities." + securityFinding.severity)}</dd>
                  </div>
                )}
                {securityFinding.exploitability !== null && (
                  <div>
                    <dt>{t("correlation.details.exploitability")}</dt>
                    <dd>{t("correlation.exploitability." + securityFinding.exploitability)}</dd>
                  </div>
                )}
                {securityFinding.cvss_score !== null && (
                  <div>
                    <dt>{t("correlation.details.cvssScore")}</dt>
                    <dd>
                      {signalValue(
                        securityFinding.cvss_score,
                        t("correlation.details.notProvided")
                      )}
                    </dd>
                  </div>
                )}
                {securityFinding.asset.display_name !== null && (
                  <div>
                    <dt>{t("correlation.details.assetName")}</dt>
                    <dd>{securityFinding.asset.display_name}</dd>
                  </div>
                )}
                {securityFinding.asset.artifact_digest !== null && (
                  <div>
                    <dt>{t("correlation.details.artifactDigest")}</dt>
                    <dd>{securityFinding.asset.artifact_digest}</dd>
                  </div>
                )}
              </>
            )}
            {anomaly && (
              <>
                <div>
                  <dt>{t("correlation.details.observedValue")}</dt>
                  <dd>
                    {signalValue(anomaly.observed_value, t("correlation.details.notProvided"))}
                  </dd>
                </div>
                <div>
                  <dt>{t("correlation.details.comparisonValue")}</dt>
                  <dd>
                    {signalValue(anomaly.comparison_value, t("correlation.details.notProvided"))}
                  </dd>
                </div>
              </>
            )}
            {healthCheck && (
              <div>
                <dt>{t("correlation.details.outcome")}</dt>
                <dd>{t("correlation.healthOutcomes." + healthCheck.outcome)}</dd>
              </div>
            )}
            <div>
              <dt>{t("correlation.details.signalId")}</dt>
              <dd>{signal.id}</dd>
            </div>
          </dl>
          <p className="correlation-signal__suppression" role="status">
            {signal.suppression.kind === "not_suppressed"
              ? t("correlation.suppression.notSuppressed")
              : t("correlation.suppression." + signal.suppression.kind)}{" "}
            {t("correlation.details.suppressionIds", {
              ids:
                signal.suppression.rule_ids
                  .concat(signal.suppression.maintenance_window_ids)
                  .join(", ") || t("correlation.details.notProvided")
            })}{" "}
            {t("correlation.details.policyVersion", {
              version: signal.suppression.policy_version
            })}
          </p>
          <button
            type="button"
            className="correlation-signal__evidence"
            aria-label={t("correlation.details.openSignalEvidence", { id: signal.id })}
            onClick={() => onOpenEvidence(signal.id, signal.evidence_ids)}
          >
            {t("correlation.details.openSignalEvidence", { id: signal.id })}
          </button>
        </div>
      </details>
    </li>
  );
}

function ReasonDetails({ reason }: { reason: CorrelationReason }) {
  const { t } = useTranslation();
  return (
    <li className="correlation-reason">
      <strong>{t("correlation.reasons." + reason.kind)}</strong>
      <span>{t("correlation.qualifications." + reason.qualification)}</span>
      {reason.target && (
        <span>
          {t("correlation.details.targetLabel", {
            target: t(targetKey(reason.target)) + ": " + reason.target.id
          })}
        </span>
      )}
      {reason.topology_path_ids.map((pathId) => (
        <span key={pathId}>{t("correlation.details.topologyPath", { id: pathId })}</span>
      ))}
    </li>
  );
}

export function CandidateDetails({
  candidate,
  signals,
  onOpenEvidence
}: {
  candidate: CorrelationCandidate;
  signals: Signal[];
  onOpenEvidence: (subject: string, evidenceIds: string[]) => void;
}) {
  const { t } = useTranslation();
  const signalById = new Map(signals.map((signal) => [signal.id, signal]));
  const memberSignals = candidate.signal_ids
    .map((signalId) => signalById.get(signalId))
    .filter((signal): signal is Signal => signal !== undefined);
  return (
    <section
      className="correlation-candidate-details"
      role="region"
      aria-label={t("correlation.details.title")}
    >
      <div className="correlation-candidate-details__header">
        <div>
          <p className="eyebrow">{t("correlation.details.eyebrow")}</p>
          <h2>{t("correlation.details.title")}</h2>
        </div>
        <p className="correlation-candidate-details__status">
          <StatusIndicator state={statusIndicator(candidate.status)} />{" "}
          {t("correlation.candidateStatus." + candidate.status)}
        </p>
      </div>
      <div className="correlation-candidate-details__section">
        <h3>{t("correlation.details.reasons")}</h3>
        <ul className="correlation-reason-list">
          {candidate.reasons.map((reason) => (
            <ReasonDetails key={reason.kind + "-" + reason.signal_ids.join("-")} reason={reason} />
          ))}
        </ul>
      </div>
      <div className="correlation-candidate-details__section">
        <h3>{t("correlation.details.members")}</h3>
        {memberSignals.length > 0 ? (
          <ul className="correlation-signal-list">
            {memberSignals.map((signal) => (
              <SignalDetails key={signal.id} signal={signal} onOpenEvidence={onOpenEvidence} />
            ))}
          </ul>
        ) : (
          <p className="correlation-empty">{t("correlation.details.membersUnavailable")}</p>
        )}
      </div>
      <div className="correlation-candidate-details__footer">
        <p role="status">
          {t("correlation.details.windowState", {
            state: t("correlation.windowStates." + candidate.window.state)
          })}
        </p>
        {candidate.late_signal_ids.length > 0 && (
          <p role="status">{t("correlation.details.lateSignals")}</p>
        )}
        <button
          type="button"
          aria-label={t("correlation.details.openEvidence")}
          onClick={() => onOpenEvidence(candidate.id, candidate.evidence_ids)}
        >
          {t("correlation.details.openEvidence")}
        </button>
      </div>
    </section>
  );
}
