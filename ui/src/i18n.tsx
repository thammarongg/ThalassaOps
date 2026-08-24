import i18n from "i18next";
import { I18nextProvider, initReactI18next, useTranslation } from "react-i18next";
import type { PropsWithChildren } from "react";
import en from "./locales/en";
import th from "./locales/th";

void i18n.use(initReactI18next).init({
  resources: { en: { translation: en }, th: { translation: th } },
  lng: "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false }
});

export function I18nProvider({ children }: PropsWithChildren) {
  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>;
}

export { i18n, useTranslation };
