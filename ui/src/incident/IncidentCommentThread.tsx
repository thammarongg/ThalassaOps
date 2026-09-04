// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useRef, useState, type FormEvent } from "react";
import type { IncidentTimelineEvent, IpcResult } from "../../contracts/ipc";
import { INCIDENT_NOTE_MAXIMUM } from "../../contracts/ipc";
import { EmptyState } from "../design-system/components";
import { useTranslation } from "../i18n";

type CommentEvent = IncidentTimelineEvent & {
  payload: Extract<IncidentTimelineEvent["payload"], { kind: "commented" }>;
};

export type CommentSubmitResult = IpcResult<unknown> | void;

export type IncidentCommentThreadProps = {
  events: IncidentTimelineEvent[];
  onSubmit: (body: string) => CommentSubmitResult | Promise<CommentSubmitResult>;
  submitting: boolean;
};

type CommentErrorKey =
  | "incident.comments.errors.empty"
  | "incident.comments.errors.textTooLong"
  | "incident.comments.errors.unsafeContent"
  | "incident.comments.errors.invalid"
  | "incident.comments.errors.unavailable";

type OptimisticComment = {
  id: string;
  body: string;
  submittedEventId: string | null;
};

const isComment = (event: IncidentTimelineEvent): event is CommentEvent =>
  event.payload.kind === "commented";

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;

const submittedCommentEventId = (result: CommentSubmitResult): string | null => {
  if (result === undefined || !result.ok || !isRecord(result.value)) return null;
  const events = result.value.events;
  if (!Array.isArray(events)) return null;
  const comment = events.find(
    (event) =>
      isRecord(event) &&
      typeof event.id === "string" &&
      isRecord(event.payload) &&
      event.payload.kind === "commented"
  );
  return isRecord(comment) && typeof comment.id === "string" ? comment.id : null;
};

const reasonFrom = (value: unknown): string | undefined => {
  if (!isRecord(value) || !isRecord(value.error) || !isRecord(value.error.details)) {
    return undefined;
  }
  return typeof value.error.details.reason === "string" ? value.error.details.reason : undefined;
};

const errorKeyForReason = (reason: string | undefined): CommentErrorKey => {
  switch (reason) {
    case "incident_unsafe_content":
      return "incident.comments.errors.unsafeContent";
    case "incident_text_too_long":
      return "incident.comments.errors.textTooLong";
    default:
      return "incident.comments.errors.invalid";
  }
};

const errorKeyForSubmission = (result: CommentSubmitResult): CommentErrorKey | null => {
  if (result === undefined || result.ok) return null;
  return errorKeyForReason(reasonFrom(result));
};

/**
 * The immutable comment timeline and its composer. It owns no IPC: the shell
 * supplies the events and the callback, while this component owns only the
 * short-lived optimistic presentation state.
 */
export function IncidentCommentThread({
  events,
  onSubmit,
  submitting
}: IncidentCommentThreadProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState("");
  const [optimistic, setOptimistic] = useState<OptimisticComment[]>([]);
  const [errorKey, setErrorKey] = useState<CommentErrorKey | null>(null);
  const [localSubmitting, setLocalSubmitting] = useState(false);
  const optimisticSequence = useRef(0);
  const comments = useMemo(
    () => events.filter(isComment).sort((left, right) => left.sequence - right.sequence),
    [events]
  );
  const busy = submitting || localSubmitting;

  /*
   * The shell reloads the timeline after a successful write. Once the
   * canonical event arrives, remove only the matching optimistic entry. The
   * event id avoids accidentally reconciling an older comment with the same
   * body.
   */
  useEffect(() => {
    const eventIds = new Set(comments.map((comment) => comment.id));
    setOptimistic((current) =>
      current.filter(
        (comment) => comment.submittedEventId === null || !eventIds.has(comment.submittedEventId)
      )
    );
  }, [comments]);

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (busy) return;

    const body = draft;
    if (body.trim() === "") {
      setErrorKey("incident.comments.errors.empty");
      return;
    }
    if (Array.from(body).length > INCIDENT_NOTE_MAXIMUM) {
      setErrorKey("incident.comments.errors.textTooLong");
      return;
    }

    const optimisticId = `optimistic-comment-${++optimisticSequence.current}`;
    setErrorKey(null);
    setOptimistic((current) => [...current, { id: optimisticId, body, submittedEventId: null }]);
    setDraft("");
    setLocalSubmitting(true);

    try {
      const result = await onSubmit(body);
      const errorKey = errorKeyForSubmission(result);
      if (errorKey !== null) {
        setOptimistic((current) => current.filter((comment) => comment.id !== optimisticId));
        setDraft(body);
        setErrorKey(errorKey);
        return;
      }

      const eventId = submittedCommentEventId(result);
      if (eventId !== null) {
        setOptimistic((current) =>
          current.map((comment) =>
            comment.id === optimisticId ? { ...comment, submittedEventId: eventId } : comment
          )
        );
      }
    } catch (error) {
      setOptimistic((current) => current.filter((comment) => comment.id !== optimisticId));
      setDraft(body);
      const reason = reasonFrom(error);
      setErrorKey(
        reason === undefined ? "incident.comments.errors.unavailable" : errorKeyForReason(reason)
      );
    } finally {
      setLocalSubmitting(false);
    }
  };

  return (
    <section className="incident-comments" aria-labelledby="incident-comments-title">
      <h4 id="incident-comments-title">{t("incident.comments.title")}</h4>
      {comments.length === 0 && optimistic.length === 0 ? (
        <EmptyState titleKey="incident.comments.empty" />
      ) : (
        <ol className="incident-comments__list" aria-label={t("incident.comments.listLabel")}>
          {comments.map((comment) => (
            <li
              key={comment.id}
              className="incident-comments__entry"
              data-sequence={comment.sequence}
            >
              <div className="incident-comments__meta">
                <span className="incident-comments__actor">{comment.actor_id}</span>
                <time dateTime={comment.occurred_at}>
                  {new Date(comment.occurred_at).toLocaleString()}
                </time>
              </div>
              <p>{comment.payload.data.body}</p>
            </li>
          ))}
          {optimistic.map((comment) => (
            <li
              key={comment.id}
              className="incident-comments__entry incident-comments__entry--optimistic"
              data-optimistic="true"
              data-testid="incident-comment-optimistic"
            >
              <div className="incident-comments__meta">
                <span>{t("incident.comments.you")}</span>
                <span>{t("incident.comments.sending")}</span>
              </div>
              <p>{comment.body}</p>
            </li>
          ))}
        </ol>
      )}
      <form className="incident-comments__composer" onSubmit={submit}>
        <label htmlFor="incident-comment-body">{t("incident.comments.bodyLabel")}</label>
        <textarea
          id="incident-comment-body"
          value={draft}
          onChange={(event) => {
            setDraft(event.target.value);
            if (errorKey !== null) setErrorKey(null);
          }}
          placeholder={t("incident.comments.placeholder")}
          aria-describedby="incident-comment-limit"
          disabled={busy}
        />
        <p id="incident-comment-limit" className="incident-comments__limit">
          {t("incident.comments.limit", { maximum: INCIDENT_NOTE_MAXIMUM })}
        </p>
        {errorKey !== null && (
          <p role="alert" className="incident-comments__error">
            {t(errorKey)}
          </p>
        )}
        <button type="submit" disabled={busy}>
          {busy ? t("incident.comments.submitting") : t("incident.comments.submit")}
        </button>
      </form>
    </section>
  );
}
