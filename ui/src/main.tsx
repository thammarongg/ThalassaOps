import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { requestHealth, type Health } from "./health";
import "./styles.css";

function App() {
  const [health, setHealth] = useState<Health | undefined>();
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    requestHealth(invoke)
      .then(setHealth)
      .catch((reason: unknown) =>
        setError(reason instanceof Error ? reason.message : "Health check failed")
      );
  }, []);

  return (
    <main>
      <p className="eyebrow">THALASSAOPS / LOCAL SHELL</p>
      <h1>System health</h1>
      {health && <p className="status">{health.status}</p>}
      {error && <p role="alert">{error}</p>}
      {!health && !error && <p>Checking local core…</p>}
    </main>
  );
}

createRoot(document.getElementById("root")!).render(<App />);
