import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { it, expect } from "vitest";
import { I18nProvider, i18n, useTranslation } from "./i18n";

function Probe() {
  const { t } = useTranslation();
  return <p>{t("health.title")}</p>;
}

it("renders English and Thai catalog entries", async () => {
  i18n.changeLanguage("en");
  const { rerender } = render(
    <I18nProvider>
      <Probe />
    </I18nProvider>
  );
  expect(screen.getByText("System health")).toBeInTheDocument();
  await i18n.changeLanguage("th");
  rerender(
    <I18nProvider>
      <Probe />
    </I18nProvider>
  );
  expect(screen.getByText("สถานะระบบ")).toBeInTheDocument();
});
