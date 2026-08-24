import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
export default defineConfig({
  root: "ui",
  plugins: [react()],
  server: { port: 1420, strictPort: true },
  clearScreen: false,
  test: { environment: "jsdom" }
});
