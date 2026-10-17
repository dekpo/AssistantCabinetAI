import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";

// The document viewer (shrimp-doc-viewer) resolves its default font with
// `new URL("../fonts/NotoSansTC-Regular.ttf", import.meta.url)` when its index module loads, so Vite
// copies that 7 MB file into dist/assets even though every call passes its own `fontUrl`. This
// swaps that one expression for a URL that fetches nothing, in dev and in build, without touching
// node_modules. It throws when the library changes the line, so an upgrade cannot silently bring
// the font back.
const DEFAULT_FONT_EXPRESSION = 'new URL("../fonts/NotoSansTC-Regular.ttf", import.meta.url).href';

function viewerWithoutDefaultFont(): Plugin {
  return {
    name: "viewer-without-default-font",
    enforce: "pre",
    transform(code, id) {
      if (!/shrimp-doc-viewer\/src\/index\.js$/.test(id.replace(/\?.*$/, ""))) return null;
      if (!code.includes(DEFAULT_FONT_EXPRESSION)) {
        throw new Error("shrimp-doc-viewer no longer declares its default font the expected way");
      }
      return { code: code.replace(DEFAULT_FONT_EXPRESSION, '"about:blank"'), map: null };
    },
  };
}

// The window loads this bundle from disk, never from a URL the user could type.
export default defineConfig({
  plugins: [react(), viewerWithoutDefaultFont()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  // The viewer finds its WebAssembly files and workers with `new URL(..., import.meta.url)`, which
  // dev pre-bundling would move away from them. Module workers match the ES build of both.
  optimizeDeps: { exclude: ["shrimp-doc-viewer", "@embedpdf/pdfium"] },
  worker: { format: "es" },
  build: {
    // Both shipped webviews (Windows and macOS) handle this level.
    target: "es2022",
    sourcemap: false,
  },
});
