// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, type FormEvent } from "react";
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
import { INCIDENT_NOTE_MAXIMUM } from "../../contracts/ipc";
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

type ActionErrorKey =
  | "incident.actions.errors.rejected"
  | "incident.actions.errors.unavailable"
  | "incident.actions.errors.required"
  | "incident.actions.errors.textTooLong"
  | "incident.actions.errors.invalidDuration"
  | "incident.actions.errors.invalidTime"
  | "incident.actions.errors.invalidFollowUp";

type TransitionDraft = {
  businessImpactConfirmed: boolean;
  duplicateChecked: boolean;
  note: string;
  actionDescription: string;
  expectedImpact: string;
  verificationSeconds: string;
  successCriteria: string;
  resolutionSummary: string;
  impactEndedAt: string;
  closureNotes: string;
  followUpIds: string;
  reason: string;
};

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

const emptyTransitionDraft = (): TransitionDraft => ({
  businessImpactConfirmed: false,
  duplicateChecked: false,
  note: "",
  actionDescription: "",
  expectedImpact: "",
  verificationSeconds: "",
  successCriteria: "",
  resolutionSummary: "",
  impactEndedAt: "",
  closureNotes: "",
  followUpIds: "",
  reason: ""
});

const effectiveSeverity = (incident: Incident): IncidentSeverity =>
  incident.severity_override?.selected ?? incident.derived_severity;

const primaryRole = (incident: Incident): IncidentRole =>
  incident.roles.find((assignment) => assignment.role === "owner")?.role ??
  incident.roles[0]?.role ??
  roles[0];

const principalForRole = (incident: Incident, role: IncidentRole): string =>
  incident.roles.find((assignment) => assignment.role === role)?.principal_id ?? "";

const transitionTextError = (value: string): ActionErrorKey | null => {
  if (value.trim() === "") return "incident.actions.errors.required";
  if (Array.from(value).length > INCIDENT_NOTE_MAXIMUM) {
    return "incident.actions.errors.textTooLong";
  }
  return null;
};

const followUpIds = (value: string): string[] =>
  value
    .split(/[\n,]+/)
    .map((id) => id.trim())
    .filter((id) => id !== "");

const parseVerificationSeconds = (value: string): number | null => {
  const seconds = Number(value);
  return Number.isInteger(seconds) && seconds >= 1 && seconds <= 86_400 ? seconds : null;
};

const dateTimeLocalValue = (date: Date): string => {
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(
    date.getHours()
  )}:${pad(date.getMinutes())}`;
};

const parseImpactEndedAt = (value: string, incident: Incident): string | null => {
  const date = new Date(value);
  const createdAt = new Date(incident.created_at);
  if (
    value.trim() === "" ||
    Number.isNaN(date.getTime()) ||
    Number.isNaN(createdAt.getTime()) ||
    date.getTime() < createdAt.getTime() ||
    date.getTime() > Date.now()
  ) {
    return null;
  }
  return date.toISOString();
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

const isVersionConflict = (result: ActionResult): boolean =>
  result !== undefined &&
  !result.ok &&
  result.error.code === "INVALID_REQUEST" &&
  result.error.details.reason === "incident_version_conflict";

const accepted = (result: ActionResult): boolean => result === undefined || result.ok;

const buildTransition = (
  incident: Incident,
  target: TransitionTarget,
  draft: TransitionDraft,
  principalId: string
): { value: IncidentTransition | null; error: ActionErrorKey | null } => {
  const principal = principalId.trim();
  const evidenceIds = incident.evidence_ids;
  switch (target) {
    case "triage": {
      if (principal === "" || !draft.businessImpactConfirmed || !draft.duplicateChecked) {
        return { value: null, error: "incident.actions.errors.required" };
      }
      return {
        value: {
          target,
          context: {
            business_impact: incident.business_impact,
            owner: principal,
            duplicate_checked: draft.duplicateChecked
          }
        },
        error: null
      };
    }
    case "investigating": {
      const error = transitionTextError(draft.note);
      if (error !== null || evidenceIds.length === 0) {
        return { value: null, error: error ?? "incident.actions.errors.required" };
      }
      return {
        value: { target, context: { note: draft.note.trim(), evidence_ids: evidenceIds } },
        error: null
      };
    }
    case "mitigating": {
      const actionError = transitionTextError(draft.actionDescription);
      const impactError = transitionTextError(draft.expectedImpact);
      if (principal === "" || actionError !== null || impactError !== null) {
        return {
          value: null,
          error: actionError ?? impactError ?? "incident.actions.errors.required"
        };
      }
      return {
        value: {
          target,
          context: {
            action_description: draft.actionDescription.trim(),
            executor: principal,
            expected_impact: draft.expectedImpact.trim()
          }
        },
        error: null
      };
    }
    case "monitoring": {
      const criteriaError = transitionTextError(draft.successCriteria);
      if (principal === "" || criteriaError !== null) {
        return { value: null, error: criteriaError ?? "incident.actions.errors.required" };
      }
      if (draft.verificationSeconds.trim() === "") {
        return { value: null, error: "incident.actions.errors.required" };
      }
      const seconds = parseVerificationSeconds(draft.verificationSeconds);
      if (seconds === null) {
        return { value: null, error: "incident.actions.errors.invalidDuration" };
      }
      return {
        value: {
          target,
          context: {
            verification_seconds: seconds,
            success_criteria: draft.successCriteria.trim(),
            watch_owner: principal
          }
        },
        error: null
      };
    }
    case "resolved": {
      const summaryError = transitionTextError(draft.resolutionSummary);
      if (summaryError !== null) return { value: null, error: summaryError };
      if (evidenceIds.length === 0) {
        return { value: null, error: "incident.actions.errors.required" };
      }
      if (draft.impactEndedAt.trim() === "") {
        return { value: null, error: "incident.actions.errors.required" };
      }
      const impactEndedAt = parseImpactEndedAt(draft.impactEndedAt, incident);
      if (impactEndedAt === null) {
        return { value: null, error: "incident.actions.errors.invalidTime" };
      }
      return {
        value: {
          target,
          context: {
            resolution_summary: draft.resolutionSummary.trim(),
            evidence_ids: evidenceIds,
            impact_ended_at: impactEndedAt
          }
        },
        error: null
      };
    }
    case "closed": {
      const notesError = transitionTextError(draft.closureNotes);
      if (notesError !== null) return { value: null, error: notesError };
      const ids = followUpIds(draft.followUpIds);
      if (ids.length === 0) {
        return { value: null, error: "incident.actions.errors.invalidFollowUp" };
      }
      return {
        value: {
          target,
          context: { closure_notes: draft.closureNotes.trim(), follow_up_ids: ids }
        },
        error: null
      };
    }
    case "reopened": {
      const reasonError = transitionTextError(draft.reason);
      if (reasonError !== null || evidenceIds.length === 0) {
        return { value: null, error: reasonError ?? "incident.actions.errors.required" };
      }
      return {
        value: {
          target,
          context: {
            reason: draft.reason.trim(),
            evidence_ids: evidenceIds,
            recurrence_signal_id: null
          }
        },
        error: null
      };
    }
    default: {
      const exhaustive: never = target;
      return exhaustive;
    }
  }
};

const hasRequiredTransitionFields = (
  incident: Incident,
  target: TransitionTarget,
  draft: TransitionDraft,
  principalId: string
): boolean => {
  const principalRequired = ["triage", "mitigating", "monitoring"].includes(target);
  if (principalRequired && principalId.trim() === "") return false;
  switch (target) {
    case "triage":
      return draft.businessImpactConfirmed && draft.duplicateChecked;
    case "investigating":
      return draft.note.trim() !== "" && incident.evidence_ids.length > 0;
    case "mitigating":
      return draft.actionDescription.trim() !== "" && draft.expectedImpact.trim() !== "";
    case "monitoring":
      return draft.verificationSeconds.trim() !== "" && draft.successCriteria.trim() !== "";
    case "resolved":
      return (
        draft.resolutionSummary.trim() !== "" &&
        draft.impactEndedAt.trim() !== "" &&
        incident.evidence_ids.length > 0
      );
    case "closed":
      return draft.closureNotes.trim() !== "" && followUpIds(draft.followUpIds).length > 0;
    case "reopened":
      return draft.reason.trim() !== "" && incident.evidence_ids.length > 0;
    default: {
      const exhaustive: never = target;
      return exhaustive;
    }
  }
};

/**
 * Versioned incident controls. Status and severity changes open forms first;
 * the shell owns IPC, reloads conflicts, and supplies the attributed event
 * that explains what changed.
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
  const [selectedRole, setSelectedRole] = useState<IncidentRole>(() => primaryRole(incident));
  const [principalId, setPrincipalId] = useState(() =>
    principalForRole(incident, primaryRole(incident))
  );
  const [transitionTarget, setTransitionTarget] = useState<TransitionTarget | null>(null);
  const [transitionDraft, setTransitionDraft] = useState<TransitionDraft>(emptyTransitionDraft);
  const [severityTarget, setSeverityTarget] = useState<IncidentSeverity | null>(null);
  const [severityReason, setSeverityReason] = useState("");
  const [localPending, setLocalPending] = useState(false);
  const [lastAction, setLastAction] = useState<ActionIntent | null>(null);
  const [errorKey, setErrorKey] = useState<ActionErrorKey | null>(null);
  const busy = pending || localPending;

  useEffect(() => {
    setDisplayStatus(incident.status);
    setDisplaySeverity(effectiveSeverity(incident));
    const nextRole = primaryRole(incident);
    setSelectedRole(nextRole);
    setPrincipalId(principalForRole(incident, nextRole));
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
      if (intent.kind === "transition") {
        setDisplayStatus(intent.target);
        setTransitionTarget(null);
      }
      if (intent.kind === "severity") {
        setDisplaySeverity(intent.target);
        setSeverityTarget(null);
      }
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
  const roleAction = roleActionFor(incident, selectedRole, principalId.trim());

  const updateTransitionField = <K extends keyof TransitionDraft>(
    field: K,
    value: TransitionDraft[K]
  ) => {
    setTransitionDraft((current) => ({ ...current, [field]: value }));
    setErrorKey(null);
  };

  const openTransition = (target: TransitionTarget) => {
    setTransitionTarget(target);
    setSeverityTarget(null);
    setTransitionDraft(emptyTransitionDraft());
    setErrorKey(null);
  };

  const openSeverity = (target: IncidentSeverity) => {
    setSeverityTarget(target);
    setTransitionTarget(null);
    setSeverityReason("");
    setErrorKey(null);
  };

  const handleTransitionSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (busy || transitionTarget === null) return;
    const result = buildTransition(incident, transitionTarget, transitionDraft, principalId);
    if (result.value === null) {
      setErrorKey(result.error ?? "incident.actions.errors.required");
      return;
    }
    void execute({ kind: "transition", target: transitionTarget, value: result.value });
  };

  const handleSeveritySubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (busy || severityTarget === null) return;
    const error = transitionTextError(severityReason);
    if (error !== null) {
      setErrorKey(error);
      return;
    }
    const command: IncidentSeverityCommand =
      severityTarget === incident.derived_severity
        ? {
            action: "reassess",
            details: {
              business_impact: incident.business_impact,
              reason: severityReason.trim()
            }
          }
        : {
            action: "override",
            details: {
              selected: severityTarget,
              reason: severityReason.trim(),
              // Evidence is the one automatic context value: these ids are
              // displayed in the form and are carried from this incident.
              evidence_ids: incident.evidence_ids
            }
          };
    if (command.action === "override" && incident.evidence_ids.length === 0) {
      setErrorKey("incident.actions.errors.required");
      return;
    }
    void execute({ kind: "severity", target: severityTarget, value: command });
  };

  const submitRole = () => {
    if (busy || roleAction === null || principalId.trim() === "") return;
    const command: IncidentRoleCommand = {
      action: roleAction,
      details: { role: selectedRole, principal_id: principalId.trim() }
    };
    void execute({ kind: "assign", target: selectedRole, value: command });
  };

  const evidenceContext = (
    <div className="incident-actions__evidence">
      <p>{t("incident.actions.evidenceContext")}</p>
      {incident.evidence_ids.length === 0 ? (
        <p>{t("incident.actions.noEvidence")}</p>
      ) : (
        <ul>
          {incident.evidence_ids.map((evidenceId) => (
            <li key={evidenceId}>{evidenceId}</li>
          ))}
        </ul>
      )}
    </div>
  );

  const transitionFields =
    transitionTarget === null
      ? null
      : (() => {
          switch (transitionTarget) {
            case "triage":
              return (
                <>
                  <p className="incident-actions__business-impact">
                    {t("incident.actions.businessImpact")}: {incident.business_impact.summary}
                  </p>
                  <label>
                    <input
                      type="checkbox"
                      checked={transitionDraft.businessImpactConfirmed}
                      onChange={(event) =>
                        updateTransitionField("businessImpactConfirmed", event.target.checked)
                      }
                    />
                    {t("incident.actions.confirmBusinessImpact")}
                  </label>
                  <label>
                    <input
                      type="checkbox"
                      checked={transitionDraft.duplicateChecked}
                      onChange={(event) =>
                        updateTransitionField("duplicateChecked", event.target.checked)
                      }
                    />
                    {t("incident.actions.duplicateChecked")}
                  </label>
                </>
              );
            case "investigating":
              return (
                <>
                  <label htmlFor="incident-transition-note">{t("incident.actions.note")}</label>
                  <textarea
                    id="incident-transition-note"
                    value={transitionDraft.note}
                    required
                    onChange={(event) => updateTransitionField("note", event.target.value)}
                  />
                  {evidenceContext}
                </>
              );
            case "mitigating":
              return (
                <>
                  <label htmlFor="incident-transition-action-description">
                    {t("incident.actions.actionDescription")}
                  </label>
                  <textarea
                    id="incident-transition-action-description"
                    value={transitionDraft.actionDescription}
                    required
                    onChange={(event) =>
                      updateTransitionField("actionDescription", event.target.value)
                    }
                  />
                  <label htmlFor="incident-transition-expected-impact">
                    {t("incident.actions.expectedImpact")}
                  </label>
                  <textarea
                    id="incident-transition-expected-impact"
                    value={transitionDraft.expectedImpact}
                    required
                    onChange={(event) =>
                      updateTransitionField("expectedImpact", event.target.value)
                    }
                  />
                </>
              );
            case "monitoring":
              return (
                <>
                  <label htmlFor="incident-transition-verification-seconds">
                    {t("incident.actions.verificationSeconds")}
                  </label>
                  <input
                    id="incident-transition-verification-seconds"
                    type="number"
                    min={1}
                    max={86_400}
                    step={1}
                    inputMode="numeric"
                    value={transitionDraft.verificationSeconds}
                    required
                    onChange={(event) =>
                      updateTransitionField("verificationSeconds", event.target.value)
                    }
                  />
                  <label htmlFor="incident-transition-success-criteria">
                    {t("incident.actions.successCriteria")}
                  </label>
                  <textarea
                    id="incident-transition-success-criteria"
                    value={transitionDraft.successCriteria}
                    required
                    onChange={(event) =>
                      updateTransitionField("successCriteria", event.target.value)
                    }
                  />
                </>
              );
            case "resolved":
              return (
                <>
                  <label htmlFor="incident-transition-resolution-summary">
                    {t("incident.actions.resolutionSummary")}
                  </label>
                  <textarea
                    id="incident-transition-resolution-summary"
                    value={transitionDraft.resolutionSummary}
                    required
                    onChange={(event) =>
                      updateTransitionField("resolutionSummary", event.target.value)
                    }
                  />
                  <label htmlFor="incident-transition-impact-ended-at">
                    {t("incident.actions.impactEndedAt")}
                  </label>
                  <input
                    id="incident-transition-impact-ended-at"
                    type="datetime-local"
                    value={transitionDraft.impactEndedAt}
                    min={dateTimeLocalValue(new Date(incident.created_at))}
                    max={dateTimeLocalValue(new Date())}
                    required
                    onChange={(event) => updateTransitionField("impactEndedAt", event.target.value)}
                  />
                  {evidenceContext}
                </>
              );
            case "closed":
              return (
                <>
                  <label htmlFor="incident-transition-closure-notes">
                    {t("incident.actions.closureNotes")}
                  </label>
                  <textarea
                    id="incident-transition-closure-notes"
                    value={transitionDraft.closureNotes}
                    required
                    onChange={(event) => updateTransitionField("closureNotes", event.target.value)}
                  />
                  <label htmlFor="incident-transition-follow-up-ids">
                    {t("incident.actions.followUpIds")}
                  </label>
                  <textarea
                    id="incident-transition-follow-up-ids"
                    value={transitionDraft.followUpIds}
                    required
                    onChange={(event) => updateTransitionField("followUpIds", event.target.value)}
                  />
                </>
              );
            case "reopened":
              return (
                <>
                  <label htmlFor="incident-transition-reason">{t("incident.actions.reason")}</label>
                  <textarea
                    id="incident-transition-reason"
                    value={transitionDraft.reason}
                    required
                    onChange={(event) => updateTransitionField("reason", event.target.value)}
                  />
                  {evidenceContext}
                </>
              );
            default: {
              const exhaustive: never = transitionTarget;
              return exhaustive;
            }
          }
        })();

  return (
    <section className="incident-actions" aria-labelledby="incident-actions-title">
      <h4 id="incident-actions-title">{t("incident.actions.title")}</h4>
      <p className="incident-actions__status">
        <span>{t("incident.actions.currentStatus")}</span>{" "}
        <strong data-testid="incident-status">{t("incident.status." + displayStatus)}</strong>
      </p>

      <fieldset className="incident-actions__principal">
        <legend>{t("incident.actions.principalContext")}</legend>
        <label htmlFor="incident-action-role">{t("incident.actions.roleLabel")}</label>
        <select
          id="incident-action-role"
          value={selectedRole}
          disabled={busy}
          onChange={(event) => {
            const role = event.target.value as IncidentRole;
            setSelectedRole(role);
            const assignment = incident.roles.find((item) => item.role === role);
            setPrincipalId(assignment?.principal_id ?? "");
            setErrorKey(null);
          }}
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
          onChange={(event) => {
            setPrincipalId(event.target.value);
            setErrorKey(null);
          }}
        />
      </fieldset>

      <div className="incident-actions__group" aria-label={t("incident.actions.statusLabel")}>
        {transitionOptions.map((target) => (
          <button key={target} type="button" disabled={busy} onClick={() => openTransition(target)}>
            {t("incident.actions.moveTo", { status: t("incident.status." + target) })}
          </button>
        ))}
      </div>

      {transitionTarget !== null && (
        <form
          className="incident-actions__form"
          aria-label={t("incident.actions.transitionForm", {
            status: t("incident.status." + transitionTarget)
          })}
          onSubmit={handleTransitionSubmit}
        >
          <h5>
            {t("incident.actions.transitionForm", {
              status: t("incident.status." + transitionTarget)
            })}
          </h5>
          {transitionFields}
          <div className="incident-actions__form-buttons">
            <button
              type="submit"
              disabled={
                busy ||
                !hasRequiredTransitionFields(
                  incident,
                  transitionTarget,
                  transitionDraft,
                  principalId
                )
              }
            >
              {t("incident.actions.submitTransition")}
            </button>
            <button type="button" disabled={busy} onClick={() => setTransitionTarget(null)}>
              {t("incident.actions.cancel")}
            </button>
          </div>
        </form>
      )}

      <div className="incident-actions__group" aria-label={t("incident.actions.severityLabel")}>
        {severityChoices.map((severity) => {
          if (severity === currentSeverity) return null;
          return (
            <button
              key={severity}
              type="button"
              disabled={busy}
              onClick={() => openSeverity(severity)}
            >
              {t("incident.actions.setSeverity", { severity })}
            </button>
          );
        })}
      </div>

      {severityTarget !== null && (
        <form
          className="incident-actions__form"
          aria-label={t("incident.actions.severityForm", { severity: severityTarget })}
          onSubmit={handleSeveritySubmit}
        >
          <h5>{t("incident.actions.severityForm", { severity: severityTarget })}</h5>
          {severityTarget === incident.derived_severity ? (
            <p className="incident-actions__business-impact">
              {t("incident.actions.businessImpact")}: {incident.business_impact.summary}
            </p>
          ) : (
            evidenceContext
          )}
          <label htmlFor="incident-severity-reason">{t("incident.actions.reason")}</label>
          <textarea
            id="incident-severity-reason"
            value={severityReason}
            required
            onChange={(event) => {
              setSeverityReason(event.target.value);
              setErrorKey(null);
            }}
          />
          <div className="incident-actions__form-buttons">
            <button type="submit" disabled={busy || severityReason.trim() === ""}>
              {t("incident.actions.submitSeverity")}
            </button>
            <button type="button" disabled={busy} onClick={() => setSeverityTarget(null)}>
              {t("incident.actions.cancel")}
            </button>
          </div>
        </form>
      )}

      <div className="incident-actions__assignment">
        <button type="button" disabled={busy || roleAction === null} onClick={submitRole}>
          {t("incident.actions.assign")}
        </button>
      </div>

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
