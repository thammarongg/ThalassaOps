import "@testing-library/jest-dom/vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { I18nProvider } from "./i18n";
import { Shell } from "./shell";

const context = {
  organization_name: "Local Organization",
  team_name: "Local Team",
  workspace_name: "Local Workspace",
  policy_version: 1
};
afterEach(() => {
  cleanup();
  localStorage.clear();
});

it("navigates product areas from the command palette with keyboard and closes it with Escape", async () => {
  const user = userEvent.setup();
  render(
    <I18nProvider>
      <Shell invoke={vi.fn().mockResolvedValue({ ok: true, value: context })} />
    </I18nProvider>
  );
  await user.keyboard("{Control>}k{/Control}");
  const input = await screen.findByRole("textbox", { name: "Command palette" });
  await user.type(input, "inc");
  await user.keyboard("{Enter}");
  expect(screen.getByRole("heading", { name: "Incidents" })).toBeInTheDocument();
  await user.keyboard("{Meta>}k{/Meta}{Escape}");
  expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
});

it("pins a navigation item and opens the honest terminal placeholder", async () => {
  const user = userEvent.setup();
  render(
    <I18nProvider>
      <Shell invoke={vi.fn().mockResolvedValue({ ok: true, value: context })} />
    </I18nProvider>
  );
  await user.click(screen.getByRole("button", { name: "Pin Incidents" }));
  expect(screen.getByRole("navigation", { name: "Favorites" })).toHaveTextContent("Incidents");
  await user.click(screen.getByRole("button", { name: "Open terminal" }));
  expect(screen.getByRole("dialog", { name: "Terminal" })).toHaveTextContent("not yet available");
  await user.click(screen.getByRole("button", { name: "Open external terminal" }));
  expect(screen.getByRole("status")).toHaveTextContent("not yet available");
});
