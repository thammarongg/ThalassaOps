// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import type { IpcResult } from "../../contracts/ipc";
import { INCIDENT_NOTE_MAXIMUM } from "../../contracts/ipc";
import { I18nProvider, i18n } from "../i18n";
import en from "../locales/en";
import { incidentFixtureTimeline } from "./incident-fixtures";
import { IncidentCommentThread } from "./IncidentCommentThread";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

type SubmitResult = IpcResult<unknown> | void;

const renderThread = (
  onSubmit: (body: string) => SubmitResult | Promise<SubmitResult> = () => undefined,
  events = incidentFixtureTimeline.events,
  submitting = false
) =>
  render(
    <I18nProvider>
      <IncidentCommentThread events={events} onSubmit={onSubmit} submitting={submitting} />
    </I18nProvider>
  );

it("shows the one commented event in sequence order", () => {
  renderThread();

  const items = screen.getAllByRole("listitem");
  expect(items).toHaveLength(1);
  expect(items[0]).toHaveAttribute("data-sequence", "6");
  expect(items[0]).toHaveTextContent("Payment provider confirms a regional outage on their side");
  expect(screen.queryByText(/investigating/i)).not.toBeInTheDocument();
});

it("blocks an empty or oversized body before calling onSubmit", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn<(...args: [string]) => SubmitResult>();
  renderThread(onSubmit);
  const send = screen.getByRole("button", { name: /comment/i });

  await user.click(send);
  expect(onSubmit).not.toHaveBeenCalled();

  const oversized = "x".repeat(INCIDENT_NOTE_MAXIMUM + 1);
  fireEvent.change(screen.getByRole("textbox"), { target: { value: oversized } });
  await user.click(send);
  expect(onSubmit).not.toHaveBeenCalled();
  expect(screen.getByRole("alert")).toHaveTextContent(en.incident.comments.errors.textTooLong);
});

it("counts Unicode scalar values for the body bound", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn<(...args: [string]) => SubmitResult>();
  renderThread(onSubmit);
  const body = "🙂".repeat(INCIDENT_NOTE_MAXIMUM);

  fireEvent.change(screen.getByRole("textbox"), { target: { value: body } });
  await user.click(screen.getByRole("button", { name: /comment/i }));

  expect(onSubmit).toHaveBeenCalledWith(body);
});

it("renders a submitted comment optimistically", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn<(...args: [string]) => SubmitResult>().mockResolvedValue(undefined);
  renderThread(onSubmit);

  await user.type(screen.getByRole("textbox"), "paged the on-call");
  await user.click(screen.getByRole("button", { name: /comment/i }));

  expect(screen.getByText("paged the on-call")).toBeInTheDocument();
  expect(screen.getByRole("textbox")).toHaveValue("");
});

it.each([
  ["incident_unsafe_content", "rotated the API token", en.incident.comments.errors.unsafeContent],
  ["incident_text_too_long", "draft remains editable", en.incident.comments.errors.textTooLong]
] as const)(
  "rolls back a rejected optimistic comment for %s and keeps the draft",
  async (reason, body, copy) => {
    const user = userEvent.setup();
    const onSubmit = vi.fn<(...args: [string]) => SubmitResult>().mockResolvedValue({
      ok: false,
      error: {
        code: "INVALID_REQUEST",
        message: "incident request was rejected",
        details: { reason }
      }
    });
    renderThread(onSubmit);

    fireEvent.change(screen.getByRole("textbox"), { target: { value: body } });
    await user.click(screen.getByRole("button", { name: /comment/i }));

    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(body));
    expect(screen.queryByTestId("incident-comment-optimistic")).not.toBeInTheDocument();
    expect(screen.getByRole("textbox")).toHaveValue(body);
    expect(screen.getByRole("alert")).toHaveTextContent(copy);
  }
);
