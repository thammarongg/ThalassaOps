// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it, vi } from "vitest";
import type { CommandEnvelope, EvidenceRef, Invoke, IpcErrorCode } from "../../contracts/ipc";
import { isEvidenceResponse } from "../../contracts/guards";
import { incidentFixtureEvidence, incidentFixturePage } from "./incident-fixtures";
import { resolveEvidence } from "./incidentEvidence";

type InvokeMock = ReturnType<typeof vi.fn> & {
  mock: { calls: [string, { envelope: CommandEnvelope<unknown> }][] };
};

const invokeMock = () => vi.fn() as unknown as InvokeMock;

const resolve = (invoke: InvokeMock, ids: string[]) =>
  resolveEvidence(invoke as unknown as Invoke, ids);

const failure = (code: IpcErrorCode) => ({
  ok: false as const,
  error: { code, message: "", details: {} }
});

const checkout = incidentFixturePage.items[0];

describe("resolveEvidence", () => {
  it("returns empty without issuing a command when there are no ids", async () => {
    const invoke = invokeMock();
    await expect(resolve(invoke, [])).resolves.toEqual({ status: "empty" });
    expect(invoke).not.toHaveBeenCalled();
  });

  /*
   * `validate_correlation_evidence_ids` rejects a repeated id and an unsorted
   * list alike, and an evidence request is all-or-nothing, so either mistake
   * leaves the tab permanently unavailable rather than merely wrong.
   */
  it("sorts and de-duplicates ids before requesting them", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue({ ok: true, value: [] });

    await resolve(invoke, ["b", "a", "a"]);

    expect(invoke.mock.calls[0][0]).toBe("correlation_evidence");
    expect(invoke.mock.calls[0][1].envelope.command).toBe("correlation.evidence");
    expect(invoke.mock.calls[0][1].envelope.capability).toBe("ResourceRead");
    expect(invoke.mock.calls[0][1].envelope.payload).toEqual({ evidence_ids: ["a", "b"] });
  });

  /*
   * The backend orders identifiers as UTF-8 bytes. A default `sort()` orders
   * UTF-16 code units and would put the astral id first, which the domain
   * validator rejects as unsorted.
   */
  it("orders ids the way the backend compares them", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue({ ok: true, value: [] });

    await resolve(invoke, ["\u{1F600}", "\uFFFD"]);

    expect(invoke.mock.calls[0][1].envelope.payload).toEqual({
      evidence_ids: ["\uFFFD", "\u{1F600}"]
    });
  });

  it.each([
    ["NOT_FOUND", "missing"],
    ["PERMISSION_DENIED", "scope"],
    ["POLICY_DENIED", "unverified"],
    ["INVALID_REQUEST", "unknown"],
    ["INTERNAL_ERROR", "unknown"]
  ] as const)("maps %s to the %s cause", async (code, cause) => {
    const invoke = invokeMock();
    invoke.mockResolvedValue(failure(code));

    await expect(resolve(invoke, ["a"])).resolves.toEqual({ status: "unavailable", cause });
  });

  it("returns the evidence the incident's own identifiers resolve to", async () => {
    expect(isEvidenceResponse(incidentFixtureEvidence, checkout.evidence_ids)).toBe(true);

    const invoke = invokeMock();
    invoke.mockResolvedValue({ ok: true, value: incidentFixtureEvidence });

    await expect(resolve(invoke, checkout.evidence_ids)).resolves.toEqual({
      status: "ready",
      evidence: incidentFixtureEvidence
    });
  });

  /*
   * A response that is not an exact cover of the request is a contract
   * violation, not evidence. Rendering it would show the reader an incomplete
   * record as if it were the whole one.
   */
  it("rejects a response that does not cover the request", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue({ ok: true, value: [incidentFixtureEvidence[0]] });

    await expect(resolve(invoke, checkout.evidence_ids)).resolves.toEqual({
      status: "unavailable",
      cause: "unknown"
    });
  });

  it("treats a rejected command as unavailable rather than letting it escape", async () => {
    const invoke = invokeMock();
    invoke.mockRejectedValue(new Error("transport closed"));

    await expect(resolve(invoke, ["a"])).resolves.toEqual({
      status: "unavailable",
      cause: "unknown"
    });
  });

  it("asks for evidence by identifier alone", async () => {
    const invoke = invokeMock();
    invoke.mockResolvedValue({ ok: true, value: [] as EvidenceRef[] });

    await resolve(invoke, ["a"]);

    expect(Object.keys(invoke.mock.calls[0][1].envelope.payload as object)).toEqual([
      "evidence_ids"
    ]);
  });
});
