import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [sveltekit()],

  // `@xterm/xterm` and `@xterm/addon-fit` are only ever reached via a
  // *dynamic* import buried inside a dynamically-imported component
  // (Panel.svelte lazy-loads TerminalView.svelte, which is what actually
  // imports xterm) — Vite's dependency crawl at dev-server startup walks
  // the *static* import graph from the HTML entry and can miss deps that
  // are only reachable this way. When that happens, Vite discovers the
  // "new" dependency the first time it's actually imported at runtime,
  // re-runs its esbuild pre-bundling step, and forces a full page reload
  // so the browser's module graph matches — which is exactly the "opening
  // a terminal reloads the whole app" bug. Listing them here gets them
  // pre-bundled at server start instead, before anything can trigger that
  // mid-session reload.
  optimizeDeps: {
    include: ["@xterm/xterm", "@xterm/addon-fit"],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
