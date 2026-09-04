// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { isIncidentTimelinePage } from "../../contracts/guards";
import type {
  IncidentTimelineEvent,
  IncidentTimelinePage,
  IncidentTimelineRequest,
  Invoke,
  IpcErrorCode
} from "../../contracts/ipc";
import { INCIDENT_TIMELINE_LIMIT, incidentEnvelope } from "./incidentEnvelope";

export type IncidentTimelineState = {
  events: IncidentTimelineEvent[];
  loading: boolean;
  error: IpcErrorCode | null;
  hasMore: boolean;
  loadMore: () => void;
  reload: () => void;
};

/*
 * Every field the hook exposes is stored against the incident it was read
 * for, so the whole record is replaced — not patched field by field — when
 * the selection moves.
 */
type KeyedTimelineState = {
  incidentId: string | null;
  events: IncidentTimelineEvent[];
  loading: boolean;
  error: IpcErrorCode | null;
  hasMore: boolean;
  afterSequence: number | null;
};

const blankTimeline = (incidentId: string | null): KeyedTimelineState => ({
  incidentId,
  events: [],
  loading: incidentId !== null,
  error: null,
  hasMore: false,
  afterSequence: null
});

/**
 * One bounded page of an incident's immutable timeline at a time. Paging is by
 * sequence, not cursor: `next_sequence` is the last event on the page and the
 * repository loads `sequence > after_sequence`, so the number goes back
 * unchanged.
 */
export function useIncidentTimeline(
  invoke: Invoke,
  incidentId: string | null
): IncidentTimelineState {
  const [state, setState] = useState<KeyedTimelineState>(() => blankTimeline(incidentId));

  if (state.incidentId !== incidentId) {
    /*
     * React's adjust-state-during-render: the selection is known in the
     * render, and the state keyed to the old incident must not survive into
     * the commit that first shows the new one. Resetting here means no frame
     * can carry the previous incident's events, and a first selection is
     * loading before any effect-driven fetch state — a passive effect would
     * be one commit too late for both.
     */
    setState(blankTimeline(incidentId));
  }

  const ticketRef = useRef(0);
  const inFlightRef = useRef(false);

  const fetchPage = useCallback(
    (id: string, afterSequence: number | null, append: boolean) => {
      if (inFlightRef.current) return;
      inFlightRef.current = true;
      const ticket = ++ticketRef.current;
      setState((current) =>
        current.incidentId === id ? { ...current, loading: true, error: null } : current
      );
      void invoke<IncidentTimelineRequest, IncidentTimelinePage>("incident_timeline", {
        envelope: incidentEnvelope("timeline", "IncidentRead", {
          incident_id: id,
          after_sequence: afterSequence,
          limit: INCIDENT_TIMELINE_LIMIT
        })
      })
        .then((result) => {
          if (ticket !== ticketRef.current) return;
          setState((current) => {
            if (current.incidentId !== id) return current;
            if (!result.ok) {
              return { ...current, error: result.error.code, hasMore: false };
            }
            /*
             * The guard proves a page is internally consistent, not that it
             * is the page that was asked for, so the incident is checked too:
             * a valid page for the previously selected incident must never be
             * rendered under the current one.
             */
            if (!isIncidentTimelinePage(result.value) || result.value.incident_id !== id) {
              return { ...current, error: "MALFORMED_RESPONSE", hasMore: false };
            }
            const page = result.value;
            return {
              ...current,
              events: append ? [...current.events, ...page.events] : page.events,
              afterSequence: page.next_sequence,
              hasMore: page.next_sequence !== null
            };
          });
        })
        .catch(() => {
          if (ticket !== ticketRef.current) return;
          setState((current) =>
            current.incidentId === id
              ? { ...current, error: "INTERNAL_ERROR", hasMore: false }
              : current
          );
        })
        .finally(() => {
          if (ticket !== ticketRef.current) return;
          inFlightRef.current = false;
          setState((current) =>
            current.incidentId === id ? { ...current, loading: false } : current
          );
        });
    },
    [invoke]
  );

  useLayoutEffect(() => {
    /*
     * A selection change abandons whatever is in flight for the old incident
     * in the commit phase itself, before the microtask checkpoint where a
     * settled response would otherwise pass the still-current ticket and
     * apply under the new selection.
     */
    ticketRef.current += 1;
    inFlightRef.current = false;
  }, [incidentId]);

  useEffect(() => {
    if (incidentId === null) return;
    fetchPage(incidentId, null, false);
  }, [fetchPage, incidentId]);

  const loadMore = useCallback(() => {
    if (incidentId === null || !state.hasMore) return;
    fetchPage(incidentId, state.afterSequence, true);
  }, [fetchPage, incidentId, state.afterSequence, state.hasMore]);

  const reload = useCallback(() => {
    ticketRef.current += 1;
    inFlightRef.current = false;
    if (incidentId === null) return;
    fetchPage(incidentId, null, false);
  }, [fetchPage, incidentId]);

  return {
    events: state.events,
    loading: state.loading,
    error: state.error,
    hasMore: state.hasMore,
    loadMore,
    reload
  };
}
