// SPDX-License-Identifier: Apache-2.0

import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { isIncidentPage, isIncidentTimelinePage } from "../../contracts/guards";
import type { CommandEnvelope, Invoke, IpcResult } from "../../contracts/ipc";
import { INCIDENT_PAGE_LIMIT } from "./incidentEnvelope";
import {
  incidentFixtureCursor,
  incidentFixturePage,
  incidentFixtureTimeline
} from "./incident-fixtures";
import { useIncidentList } from "./useIncidentList";

type InvokeMock = ReturnType<typeof vi.fn> & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

const ok = <T>(value: T): IpcResult<T> => ({ ok: true, value });

const invokeMock = () => vi.fn() as unknown as InvokeMock;

const listHook = (invoke: InvokeMock) =>
  renderHook(() => useIncidentList(invoke as unknown as Invoke));

describe("incident fixtures", () => {
  it("ships fixtures the Task 4 guards accept", () => {
    expect(incidentFixturePage.items.length).toBeGreaterThan(0);
    expect(isIncidentPage(incidentFixturePage)).toBe(true);
    expect(incidentFixtureTimeline.events.length).toBeGreaterThan(0);
    expect(isIncidentTimelinePage(incidentFixtureTimeline)).toBe(true);
  });
});

describe("useIncidentList", () => {
  it("pages the incident list with the cursor the page returned", async () => {
    const invoke = invokeMock();
    invoke
      .mockResolvedValueOnce(ok(incidentFixturePage))
      .mockResolvedValueOnce(ok({ items: [], next_cursor: null }));

    const { result } = listHook(invoke);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.incidents).toEqual(incidentFixturePage.items);
    expect(result.current.hasMore).toBe(true);

    act(() => result.current.loadMore());
    await waitFor(() => expect(result.current.hasMore).toBe(false));

    expect(invoke.mock.calls[0][0]).toBe("incident_list");
    expect(invoke.mock.calls[0][1].envelope.command).toBe("incident.list");
    expect(invoke.mock.calls[0][1].envelope.capability).toBe("IncidentRead");
    expect(invoke.mock.calls[0][1].envelope.payload).toEqual({
      cursor: null,
      limit: INCIDENT_PAGE_LIMIT
    });
    expect(invoke.mock.calls[1][1].envelope.payload).toEqual({
      cursor: incidentFixtureCursor,
      limit: INCIDENT_PAGE_LIMIT
    });
    expect(result.current.incidents).toEqual(incidentFixturePage.items);
  });

  it("reports a guard failure as MALFORMED_RESPONSE rather than rendering unvalidated data", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue(ok({ items: [{ bogus: true }], next_cursor: null }));

    const { result } = listHook(invoke);
    await waitFor(() => expect(result.current.error).toBe("MALFORMED_RESPONSE"));
    expect(result.current.incidents).toEqual([]);
    expect(result.current.hasMore).toBe(false);
  });

  it("reports the IPC error code a rejected command returns", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue({
      ok: false,
      error: { code: "PERMISSION_DENIED", message: "denied", details: {} }
    });

    const { result } = listHook(invoke);
    await waitFor(() => expect(result.current.error).toBe("PERMISSION_DENIED"));
    expect(result.current.incidents).toEqual([]);
  });

  it("reports a thrown command as INTERNAL_ERROR", async () => {
    const invoke = invokeMock();
    invoke.mockRejectedValue(new Error("transport closed"));

    const { result } = listHook(invoke);
    await waitFor(() => expect(result.current.error).toBe("INTERNAL_ERROR"));
    expect(result.current.incidents).toEqual([]);
  });

  it("ignores loadMore while a page is in flight", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue(ok(incidentFixturePage));

    const { result } = listHook(invoke);
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => {
      result.current.loadMore();
      result.current.loadMore();
    });
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(invoke).toHaveBeenCalledTimes(2);
    expect(result.current.incidents).toHaveLength(incidentFixturePage.items.length * 2);
  });

  it("reload replaces the list rather than appending to it", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue(ok(incidentFixturePage));

    const { result } = listHook(invoke);
    await waitFor(() => expect(result.current.loading).toBe(false));

    act(() => result.current.reload());
    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(invoke).toHaveBeenCalledTimes(2);
    expect(invoke.mock.calls[1][1].envelope.payload).toEqual({
      cursor: null,
      limit: INCIDENT_PAGE_LIMIT
    });
    expect(result.current.incidents).toEqual(incidentFixturePage.items);
  });
});
