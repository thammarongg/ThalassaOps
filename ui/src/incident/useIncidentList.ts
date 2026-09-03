// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { isIncidentPage } from "../../contracts/guards";
import type {
  Incident,
  IncidentListRequest,
  IncidentPage,
  Invoke,
  IpcErrorCode
} from "../../contracts/ipc";
import { INCIDENT_PAGE_LIMIT, incidentEnvelope } from "./incident-envelope";

export type IncidentListState = {
  incidents: Incident[];
  loading: boolean;
  error: IpcErrorCode | null;
  hasMore: boolean;
  loadMore: () => void;
  reload: () => void;
};

/**
 * One bounded page of workspace incidents at a time, resumed with the cursor
 * the previous page returned. The hook renders nothing and translates nothing:
 * it reports an `IpcErrorCode` and the workspace turns that into a message.
 */
export function useIncidentList(invoke: Invoke): IncidentListState {
  const [incidents, setIncidents] = useState<Incident[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<IpcErrorCode | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const cursorRef = useRef<string | null>(null);
  // Discards a response whose request has since been superseded by a reload.
  const ticketRef = useRef(0);
  // Held in a ref, not in `loading`: two `loadMore` calls in one React batch
  // both read the same rendered state, and only a ref stops the second.
  const inFlightRef = useRef(false);

  const fetchPage = useCallback(
    (cursor: string | null, append: boolean) => {
      if (inFlightRef.current) return;
      inFlightRef.current = true;
      const ticket = ++ticketRef.current;
      setLoading(true);
      setError(null);
      void invoke<IncidentListRequest, IncidentPage>("incident_list", {
        envelope: incidentEnvelope("list", "IncidentRead", {
          cursor,
          limit: INCIDENT_PAGE_LIMIT
        })
      })
        .then((result) => {
          if (ticket !== ticketRef.current) return;
          if (!result.ok) {
            setError(result.error.code);
            setHasMore(false);
            return;
          }
          if (!isIncidentPage(result.value)) {
            // An unvalidated page is not rendered at all: the incidents
            // already on screen stay, and the workspace says the read failed.
            setError("MALFORMED_RESPONSE");
            setHasMore(false);
            return;
          }
          const page = result.value;
          setIncidents((current) => (append ? [...current, ...page.items] : page.items));
          cursorRef.current = page.next_cursor;
          setHasMore(page.next_cursor !== null);
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
    cursorRef.current = null;
    fetchPage(null, false);
  }, [fetchPage]);

  const loadMore = useCallback(() => {
    if (!hasMore) return;
    fetchPage(cursorRef.current, true);
  }, [fetchPage, hasMore]);

  const reload = useCallback(() => {
    ticketRef.current += 1;
    inFlightRef.current = false;
    cursorRef.current = null;
    fetchPage(null, false);
  }, [fetchPage]);

  return { incidents, loading, error, hasMore, loadMore, reload };
}
