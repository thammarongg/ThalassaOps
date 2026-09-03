// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { isIncidentTimelinePage } from "../../contracts/guards";
import type {
  IncidentTimelineEvent,
  IncidentTimelinePage,
  IncidentTimelineRequest,
  Invoke,
  IpcErrorCode
} from "../../contracts/ipc";
import { INCIDENT_TIMELINE_LIMIT, incidentEnvelope } from "./incident-envelope";

export type IncidentTimelineState = {
  events: IncidentTimelineEvent[];
  loading: boolean;
  error: IpcErrorCode | null;
  hasMore: boolean;
  loadMore: () => void;
  reload: () => void;
};

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
  const [events, setEvents] = useState<IncidentTimelineEvent[]>([]);
  const [loading, setLoading] = useState(incidentId !== null);
  const [error, setError] = useState<IpcErrorCode | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const sequenceRef = useRef<number | null>(null);
  const ticketRef = useRef(0);
  const inFlightRef = useRef(false);

  const fetchPage = useCallback(
    (id: string, afterSequence: number | null, append: boolean) => {
      if (inFlightRef.current) return;
      inFlightRef.current = true;
      const ticket = ++ticketRef.current;
      setLoading(true);
      setError(null);
      void invoke<IncidentTimelineRequest, IncidentTimelinePage>("incident_timeline", {
        envelope: incidentEnvelope("timeline", "IncidentRead", {
          incident_id: id,
          after_sequence: afterSequence,
          limit: INCIDENT_TIMELINE_LIMIT
        })
      })
        .then((result) => {
          if (ticket !== ticketRef.current) return;
          if (!result.ok) {
            setError(result.error.code);
            setHasMore(false);
            return;
          }
          /*
           * The guard proves a page is internally consistent, not that it is
           * the page that was asked for, so the incident is checked too: a
           * valid page for the previously selected incident must never be
           * rendered under the current one.
           */
          if (!isIncidentTimelinePage(result.value) || result.value.incident_id !== id) {
            setError("MALFORMED_RESPONSE");
            setHasMore(false);
            return;
          }
          const page = result.value;
          setEvents((current) => (append ? [...current, ...page.events] : page.events));
          sequenceRef.current = page.next_sequence;
          setHasMore(page.next_sequence !== null);
        })
        .catch(() => {
          if (ticket !== ticketRef.current) return;
          setError("INTERNAL_ERROR");
          setHasMore(false);
        })
        .finally(() => {
          if (ticket !== ticketRef.current) return;
          inFlightRef.current = false;
          setLoading(false);
        });
    },
    [invoke]
  );

  useEffect(() => {
    // A selection change abandons whatever is in flight for the old incident.
    ticketRef.current += 1;
    inFlightRef.current = false;
    sequenceRef.current = null;
    setEvents([]);
    setError(null);
    setHasMore(false);
    if (incidentId === null) {
      setLoading(false);
      return;
    }
    fetchPage(incidentId, null, false);
  }, [fetchPage, incidentId]);

  const loadMore = useCallback(() => {
    if (incidentId === null || !hasMore) return;
    fetchPage(incidentId, sequenceRef.current, true);
  }, [fetchPage, hasMore, incidentId]);

  const reload = useCallback(() => {
    ticketRef.current += 1;
    inFlightRef.current = false;
    sequenceRef.current = null;
    if (incidentId === null) return;
    fetchPage(incidentId, null, false);
  }, [fetchPage, incidentId]);

  return { events, loading, error, hasMore, loadMore, reload };
}
