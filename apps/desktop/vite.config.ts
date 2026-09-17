import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// The window loads this bundle from disk, never from a URL the user could type.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  build: {
    // Both shipped webviews (Windows and macOS) handle this level.
    target: "es2022",
    sourcemap: false,
  },
});
