// SPDX-License-Identifier: Apache-2.0

import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { I18nProvider, i18n } from "../i18n";
import { IncidentList, type IncidentQueueFilter } from "./IncidentList";
import { incidentFixturePage } from "./incident-fixtures";

afterEach(() => {
  cleanup();
  void i18n.changeLanguage("en");
});

const [checkout, search] = incidentFixturePage.items;

const renderList = (
  overrides: {
    selectedId?: string | null;
    onSelect?: (id: string) => void;
    filter?: IncidentQueueFilter;
  } = {}
) =>
  render(
    <I18nProvider>
      <IncidentList
        incidents={incidentFixturePage.items}
        selectedId={overrides.selectedId ?? null}
        onSelect={overrides.onSelect ?? (() => {})}
        filter={overrides.filter ?? { status: "all" }}
        onFilterChange={() => {}}
      />
    </I18nProvider>
  );

/*
 * The status filter is a native `<select>`, whose `<option>` children carry
 * the same ARIA role as the queue rows. Every row query is scoped to the
 * listbox so the two never blur together.
 */
const queue = () => within(screen.getByRole("listbox"));

it("renders the effective severity, not the derived one, when an override is present", () => {
  renderList();

  const checkoutRow = queue().getByRole("option", { name: /checkout/i });
  expect(within(checkoutRow).getByTestId("incident-severity")).toHaveTextContent("S1");

  /*
   * The search incident derives S2 and overrides it to S1. A row that renders
   * `derived_severity` alone passes the checkout assertion above and hides
   * every override in the queue, so both rows are asserted.
   */
  expect(search.derived_severity).toBe("S2");
  const searchRow = queue().getByRole("option", { name: /search/i });
  expect(within(searchRow).getByTestId("incident-severity")).toHaveTextContent("S1");
});

it("renders no priority, because an incident carries none", () => {
  renderList();
  expect(screen.queryByTestId("incident-priority")).not.toBeInTheDocument();
});

it("calls onSelect with the incident id when a row is chosen", async () => {
  const onSelect = vi.fn();
  renderList({ onSelect });
  await userEvent.click(queue().getByRole("option", { name: /checkout/i }));
  expect(onSelect).toHaveBeenCalledWith(checkout.id);
});

it("shows only the incidents the status filter admits", () => {
  renderList({ filter: { status: "triage" } });
  expect(queue().getAllByRole("option")).toHaveLength(1);
  expect(queue().getByRole("option", { name: /search/i })).toBeInTheDocument();
});

it("shows the empty state when the filter admits nothing", () => {
  renderList({ filter: { status: "closed" } });
  expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  expect(screen.getByText("No incidents match this filter")).toBeInTheDocument();
});

it("marks exactly the selected row as the selected option", () => {
  renderList({ selectedId: search.id });
  const selected = queue().getAllByRole("option", { selected: true });
  expect(selected).toHaveLength(1);
  expect(selected[0]).toHaveAccessibleName(/search/i);
});

it("moves the selection with the arrow keys", async () => {
  const onSelect = vi.fn();
  renderList({ selectedId: checkout.id, onSelect });
  queue().getByRole("option", { selected: true }).focus();

  await userEvent.keyboard("{ArrowDown}");
  expect(onSelect).toHaveBeenLastCalledWith(search.id);

  await userEvent.keyboard("{End}");
  expect(onSelect).toHaveBeenLastCalledWith(incidentFixturePage.items[2].id);
});
