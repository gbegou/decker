import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";

// Tauri expects a fixed port and no clearing of the screen. `$shared` is
// the front-end shared with the viewer (apps/shared); its bare imports
// resolve from this app's node_modules.
const shared = fileURLToPath(new URL("../shared/src", import.meta.url));
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 1430, strictPort: true, fs: { allow: [".", shared] } },
  resolve: { alias: { $shared: shared }, dedupe: ["svelte", "@tauri-apps/api"] },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: { target: ["es2022", "chrome105", "safari13"], minify: !process.env.TAURI_ENV_DEBUG },
});
