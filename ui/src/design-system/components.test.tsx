import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "vitest-axe";
import { expect, it } from "vitest";
import { I18nProvider } from "../i18n";
import {
  Card,
  CommandSurface,
  Drawer,
  EmptyState,
  StatusIndicator,
  Table,
  Tabs,
  Timeline
} from "./components";

it("renders shared components in distinct usages without accessibility violations", async () => {
  const user = userEvent.setup();
  render(
    <I18nProvider>
      <Card titleKey="demo.primaryCard">
        <StatusIndicator state="healthy" />
      </Card>
      <Card titleKey="demo.secondaryCard">
        <EmptyState titleKey="demo.emptyTitle" />
      </Card>
      <EmptyState titleKey="demo.emptyTitle" />
      <Table
        captionKey="demo.tableCaption"
        columns={[{ key: "name", headerKey: "demo.name" }]}
        rows={[
          { id: "one", name: "first" },
          { id: "two", name: "second" }
        ]}
      />
      <Table
        captionKey="demo.tableCaption"
        columns={[{ key: "name", headerKey: "demo.name" }]}
        rows={[{ id: "three", name: "third" }]}
      />
      <Tabs
        items={[
          { id: "one", labelKey: "demo.firstTab" },
          { id: "two", labelKey: "demo.secondTab" }
        ]}
      >
        {(active) => <div>{active}</div>}
      </Tabs>
      <Tabs
        items={[
          { id: "first", labelKey: "demo.firstTab" },
          { id: "second", labelKey: "demo.secondTab" }
        ]}
      >
        {(active) => <div>{active}</div>}
      </Tabs>
      <Timeline items={[{ id: "first", titleKey: "demo.timelineEvent", state: "warning" }]} />
      <Timeline items={[{ id: "second", titleKey: "demo.timelineEvent", state: "healthy" }]} />
      <CommandSurface labelKey="demo.commandLabel" placeholderKey="demo.commandPlaceholder" />
      <CommandSurface labelKey="demo.commandLabel" placeholderKey="demo.commandPlaceholder" />
      <Drawer titleKey="demo.drawerTitle">
        <button type="button">Close</button>
      </Drawer>
      <Drawer titleKey="demo.drawerTitle">
        <button type="button">Close</button>
      </Drawer>
    </I18nProvider>
  );

  await user.click(screen.getAllByRole("tab", { name: "Evidence" })[0]);
  expect(screen.getByText("two")).toBeInTheDocument();
  expect(
    (
      await axe(document.body, {
        rules: { "color-contrast": { enabled: false }, region: { enabled: false } }
      })
    ).violations
  ).toEqual([]);
});
