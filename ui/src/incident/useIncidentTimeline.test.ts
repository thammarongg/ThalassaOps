// SPDX-License-Identifier: Apache-2.0

import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CommandEnvelope, Invoke, IpcResult } from "../../contracts/ipc";
import { INCIDENT_TIMELINE_LIMIT } from "./incident-envelope";
import {
  incidentFixtureCheckoutId,
  incidentFixtureSearchId,
  incidentFixtureTimeline
} from "./incident-fixtures";
import { useIncidentTimeline } from "./useIncidentTimeline";

type InvokeMock = ReturnType<typeof vi.fn> & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

const ok = <T>(value: T): IpcResult<T> => ({ ok: true, value });

const invokeMock = () => vi.fn() as unknown as InvokeMock;

const timelineHook = (invoke: InvokeMock, incidentId: string | null) =>
  renderHook(
    ({ id }: { id: string | null }) => useIncidentTimeline(invoke as unknown as Invoke, id),
    { initialProps: { id: incidentId } }
  );

const exhausted = (incidentId: string) => ({
  incident_id: incidentId,
  events: [],
  next_sequence: null
});

describe("useIncidentTimeline", () => {
  it("resumes from next_sequence unchanged", async () => {
    const invoke = invokeMock();
    invoke
      .mockResolvedValueOnce(ok(incidentFixtureTimeline))
      .mockResolvedValueOnce(ok(exhausted(incidentFixtureCheckoutId)));

    const { result } = timelineHook(invoke, incidentFixtureCheckoutId);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.events).toEqual(incidentFixtureTimeline.events);
    expect(result.current.hasMore).toBe(true);

    act(() => result.current.loadMore());
    await waitFor(() => expect(result.current.hasMore).toBe(false));

    expect(invoke.mock.calls[0][0]).toBe("incident_timeline");
    expect(invoke.mock.calls[0][1].envelope.command).toBe("incident.timeline");
    expect(invoke.mock.calls[0][1].envelope.capability).toBe("IncidentRead");
    expect(invoke.mock.calls[0][1].envelope.payload).toEqual({
      incident_id: incidentFixtureCheckoutId,
      after_sequence: null,
      limit: INCIDENT_TIMELINE_LIMIT
    });
    expect(invoke.mock.calls[1][1].envelope.payload).toEqual({
      incident_id: incidentFixtureCheckoutId,
      after_sequence: incidentFixtureTimeline.next_sequence,
      limit: INCIDENT_TIMELINE_LIMIT
    });
  });

  it("calls no command when no incident is selected", async () => {
    const invoke = invokeMock();
    const { result } = timelineHook(invoke, null);

    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(invoke).not.toHaveBeenCalled();
    expect(result.current.events).toEqual([]);
    expect(result.current.hasMore).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it("refetches from the first page when the incident changes", async () => {
    const invoke = invokeMock();
    invoke
      .mockResolvedValueOnce(ok(incidentFixtureTimeline))
      .mockResolvedValueOnce(ok(exhausted(incidentFixtureSearchId)));

    const { result, rerender } = timelineHook(invoke, incidentFixtureCheckoutId);
    await waitFor(() => expect(result.current.events).toHaveLength(6));

    rerender({ id: incidentFixtureSearchId });
    await waitFor(() => expect(result.current.events).toEqual([]));

    expect(invoke.mock.calls[1][1].envelope.payload).toEqual({
      incident_id: incidentFixtureSearchId,
      after_sequence: null,
      limit: INCIDENT_TIMELINE_LIMIT
    });
  });

  /*
   * The page guard only proves a page is internally consistent, so a valid
   * page for the previous incident can still arrive after the selection moved
   * on. Rendering it would attribute one incident's timeline to another.
   */
  it("drops a page that arrives for a since-deselected incident", async () => {
    const invoke = invokeMock();
    let releaseCheckout: (result: IpcResult<unknown>) => void = () => {};
    invoke
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            releaseCheckout = resolve;
          })
      )
      .mockResolvedValueOnce(ok(exhausted(incidentFixtureSearchId)));

    const { result, rerender } = timelineHook(invoke, incidentFixtureCheckoutId);
    rerender({ id: incidentFixtureSearchId });
    await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));

    await act(async () => {
      releaseCheckout(ok(incidentFixtureTimeline));
    });

    expect(result.current.events).toEqual([]);
    expect(result.current.hasMore).toBe(false);
  });

  it("reports a guard failure as MALFORMED_RESPONSE", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue(
      ok({
        incident_id: incidentFixtureCheckoutId,
        events: [{ bogus: true }],
        next_sequence: null
      })
    );

    const { result } = timelineHook(invoke, incidentFixtureCheckoutId);
    await waitFor(() => expect(result.current.error).toBe("MALFORMED_RESPONSE"));
    expect(result.current.events).toEqual([]);
  });
});
