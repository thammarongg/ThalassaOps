// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type {
  Incident,
  IncidentRole,
  IncidentRoleCommand,
  IncidentSeverity,
  IncidentSeverityCommand,
  IncidentStatus,
  IncidentTransition,
  IpcResult
} from "../../contracts/ipc";
import { useTranslation } from "../i18n";

export type ActionResult = IpcResult<unknown> | void;

type ActionCallback<T> = (value: T) => ActionResult | Promise<ActionResult>;

export type IncidentActionsProps = {
  incident: Incident;
  onTransition: ActionCallback<IncidentTransition>;
  onSeverity: ActionCallback<IncidentSeverityCommand>;
  onAssign: ActionCallback<IncidentRoleCommand>;
  pending: boolean;
  conflict: { actor: string; at: string } | null;
};

type TransitionTarget = Exclude<IncidentStatus, "detected">;

type ActionIntent =
  | { kind: "transition"; target: TransitionTarget; value: IncidentTransition }
  | { kind: "severity"; target: IncidentSeverity; value: IncidentSeverityCommand }
  | { kind: "assign"; target: IncidentRole; value: IncidentRoleCommand };

type ActionErrorKey = "incident.actions.errors.rejected" | "incident.actions.errors.unavailable";

const transitionTargets: Record<IncidentStatus, readonly IncidentStatus[]> = {
  detected: ["triage"],
  triage: ["investigating"],
  investigating: ["mitigating"],
  mitigating: ["monitoring"],
  monitoring: ["resolved", "reopened"],
  resolved: ["closed", "reopened"],
  closed: ["reopened"],
  reopened: ["investigating"]
};

const severityChoices: readonly IncidentSeverity[] = ["S1", "S2", "S3", "S4", "S5"];
const roles: readonly IncidentRole[] = [
  "owner",
  "incident_commander",
  "technical_lead",
  "communications_lead",
  "approver",
  "change_owner",
  "stakeholder"
];

const effectiveSeverity = (incident: Incident): IncidentSeverity =>
  incident.severity_override?.selected ?? incident.derived_severity;

const primaryPrincipal = (incident: Incident): string =>
  incident.roles[0]?.principal_id ?? incident.owning_team_id;

const severityCommandFor = (
  incident: Incident,
  selected: IncidentSeverity
): IncidentSeverityCommand =>
  selected === incident.derived_severity
    ? {
        action: "reassess",
        details: { business_impact: incident.business_impact, reason: incident.summary }
      }
    : {
        action: "override",
        details: {
          selected,
          reason: incident.summary,
          evidence_ids: incident.evidence_ids
        }
      };

const roleActionFor = (
  incident: Incident,
  role: IncidentRole,
  principalId: string
): IncidentRoleCommand["action"] | null => {
  const assignments = incident.roles.filter((assignment) => assignment.role === role);
  if (role === "stakeholder") {
    return assignments.some((assignment) => assignment.principal_id === principalId)
      ? null
      : "assign";
  }
  if (assignments.length === 0) return "assign";
  return assignments[0].principal_id === principalId ? null : "replace";
};

const transitionFor = (incident: Incident, target: TransitionTarget): IncidentTransition => {
  const principal = primaryPrincipal(incident);
  switch (target) {
    case "triage":
      return {
        target,
        context: {
          business_impact: incident.business_impact,
          owner: principal,
          duplicate_checked: true
        }
      };
    case "investigating":
      return {
        target,
        context: { note: incident.summary, evidence_ids: incident.evidence_ids }
      };
    case "mitigating":
      return {
        target,
        context: {
          action_description: incident.summary,
          executor: principal,
          expected_impact: incident.business_impact.summary
        }
      };
    case "monitoring":
      return {
        target,
        context: {
          verification_seconds: 300,
          success_criteria: incident.business_impact.summary,
          watch_owner: principal
        }
      };
    case "resolved":
      return {
        target,
        context: {
          resolution_summary: incident.summary,
          evidence_ids: incident.evidence_ids,
          impact_ended_at: incident.updated_at
        }
      };
    case "closed":
      return {
        target,
        context: { closure_notes: incident.summary, follow_up_ids: [incident.id] }
      };
    case "reopened":
      return {
        target,
        context: {
          reason: incident.summary,
          evidence_ids: incident.evidence_ids,
          recurrence_signal_id: null
        }
      };
    default: {
      const exhaustive: never = target;
      return exhaustive;
    }
  }
};

const isVersionConflict = (result: ActionResult): boolean =>
  result !== undefined &&
  !result.ok &&
  result.error.code === "INVALID_REQUEST" &&
  result.error.details.reason === "incident_version_conflict";

const accepted = (result: ActionResult): boolean => result === undefined || result.ok;

/**
 * Versioned incident controls. This component waits for each callback before
 * changing its displayed state; the shell owns IPC, reloads conflicts, and
 * supplies the attributed event that explains what changed.
 */
export function IncidentActions({
  incident,
  onTransition,
  onSeverity,
  onAssign,
  pending,
  conflict
}: IncidentActionsProps) {
  const { t } = useTranslation();
  const [displayStatus, setDisplayStatus] = useState<IncidentStatus>(incident.status);
  const [displaySeverity, setDisplaySeverity] = useState<IncidentSeverity>(
    effectiveSeverity(incident)
  );
  const [selectedRole, setSelectedRole] = useState<IncidentRole>(roles[0]);
  const [principalId, setPrincipalId] = useState(primaryPrincipal(incident));
  const [localPending, setLocalPending] = useState(false);
  const [lastAction, setLastAction] = useState<ActionIntent | null>(null);
  const [errorKey, setErrorKey] = useState<ActionErrorKey | null>(null);
  const busy = pending || localPending;

  useEffect(() => {
    setDisplayStatus(incident.status);
    setDisplaySeverity(effectiveSeverity(incident));
    setPrincipalId(primaryPrincipal(incident));
  }, [incident]);

  const execute = async (intent: ActionIntent) => {
    if (busy) return;
    setLastAction(intent);
    setErrorKey(null);
    setLocalPending(true);
    try {
      const result =
        intent.kind === "transition"
          ? await onTransition(intent.value)
          : intent.kind === "severity"
            ? await onSeverity(intent.value)
            : await onAssign(intent.value);
      if (!accepted(result)) {
        if (!isVersionConflict(result)) setErrorKey("incident.actions.errors.rejected");
        return;
      }
      if (intent.kind === "transition") setDisplayStatus(intent.target);
      if (intent.kind === "severity") setDisplaySeverity(intent.target);
    } catch {
      setErrorKey("incident.actions.errors.unavailable");
    } finally {
      setLocalPending(false);
    }
  };

  const transitionOptions = transitionTargets[displayStatus].filter(
    (target): target is TransitionTarget => target !== "detected"
  );
  const currentSeverity = displaySeverity;
  const roleAction = roleActionFor(incident, selectedRole, principalId);

  return (
    <section className="incident-actions" aria-labelledby="incident-actions-title">
      <h4 id="incident-actions-title">{t("incident.actions.title")}</h4>
      <p className="incident-actions__status">
        <span>{t("incident.actions.currentStatus")}</span>{" "}
        <strong data-testid="incident-status">{t("incident.status." + displayStatus)}</strong>
      </p>
      <div className="incident-actions__group" aria-label={t("incident.actions.statusLabel")}>
        {transitionOptions.map((target) => {
          const intent: ActionIntent = {
            kind: "transition",
            target,
            value: transitionFor(incident, target)
          };
          return (
            <button key={target} type="button" disabled={busy} onClick={() => void execute(intent)}>
              {t("incident.actions.moveTo", { status: t("incident.status." + target) })}
            </button>
          );
        })}
      </div>
      <div className="incident-actions__group" aria-label={t("incident.actions.severityLabel")}>
        {severityChoices.map((severity) => {
          if (severity === currentSeverity) return null;
          const command = severityCommandFor(incident, severity);
          const intent: ActionIntent = { kind: "severity", target: severity, value: command };
          return (
            <button
              key={severity}
              type="button"
              disabled={busy}
              onClick={() => void execute(intent)}
            >
              {t("incident.actions.setSeverity", { severity })}
            </button>
          );
        })}
      </div>
      <form
        className="incident-actions__assignment"
        onSubmit={(event) => {
          event.preventDefault();
          if (roleAction === null) return;
          const command: IncidentRoleCommand = {
            action: roleAction,
            details: { role: selectedRole, principal_id: principalId }
          };
          void execute({ kind: "assign", target: selectedRole, value: command });
        }}
      >
        <label htmlFor="incident-action-role">{t("incident.actions.roleLabel")}</label>
        <select
          id="incident-action-role"
          value={selectedRole}
          disabled={busy}
          onChange={(event) => setSelectedRole(event.target.value as IncidentRole)}
        >
          {roles.map((role) => (
            <option key={role} value={role}>
              {t("incident.role." + role)}
            </option>
          ))}
        </select>
        <label htmlFor="incident-action-principal">{t("incident.actions.principalLabel")}</label>
        <input
          id="incident-action-principal"
          value={principalId}
          disabled={busy}
          onChange={(event) => setPrincipalId(event.target.value)}
        />
        <button type="submit" disabled={busy || principalId.trim() === "" || roleAction === null}>
          {t("incident.actions.assign")}
        </button>
      </form>
      {errorKey !== null && (
        <p className="incident-actions__error" role="alert">
          {t(errorKey)}
        </p>
      )}
      {conflict !== null && (
        <div className="incident-actions__conflict" role="alert">
          <p>{t("incident.actions.conflict", { actor: conflict.actor, at: conflict.at })}</p>
          {lastAction !== null && (
            <button type="button" disabled={busy} onClick={() => void execute(lastAction)}>
              {t("incident.actions.retry")}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
