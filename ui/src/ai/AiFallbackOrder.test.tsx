import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { I18nProvider } from "../i18n";
import { AiFallbackOrder } from "./AiFallbackOrder";
import { localProvider, unauthorizedHostedProvider } from "./ai-fixtures";

afterEach(cleanup);

const renderOrder = (providerOrder: string[], onReorder: (nextOrder: string[]) => void = vi.fn()) =>
  render(
    <I18nProvider>
      <AiFallbackOrder
        providers={[unauthorizedHostedProvider, localProvider]}
        providerOrder={providerOrder}
        onReorder={onReorder}
      />
    </I18nProvider>
  );

describe("AI fallback order", () => {
  it("says failover is off and marks every configured provider as not a fallback when empty", () => {
    renderOrder([]);

    expect(screen.getByText(/failover is off/i)).toBeInTheDocument();
    expect(screen.getAllByText(/not a fallback/i)).toHaveLength(2);
  });

  it("emits the displayed order when a provider moves", () => {
    const onReorder = vi.fn();
    renderOrder(["openai", "ollama"], onReorder);

    screen.getByRole("button", { name: "Move ollama up" }).click();

    expect(onReorder).toHaveBeenCalledWith(["ollama", "openai"]);
  });

  it("visibly marks a configured provider omitted from the order as not a fallback", () => {
    renderOrder(["openai"]);

    expect(screen.getByText(/ollama.*not a fallback/i)).toBeInTheDocument();
  });

  it("lets the operator add an omitted provider to the fallback order", () => {
    const onReorder = vi.fn();
    renderOrder([], onReorder);

    screen.getByRole("button", { name: "Add ollama to fallback order" }).click();

    expect(onReorder).toHaveBeenCalledWith(["ollama"]);
  });
});
