import { invoke } from "@tauri-apps/api/core";
import { createRoot } from "react-dom/client";
import { I18nProvider } from "./i18n";
import { Shell } from "./shell";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <I18nProvider>
    <Shell invoke={invoke} />
  </I18nProvider>
);
