import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// @tauri-apps/cli sets TAURI_DEV_HOST when running on a device
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async ({ command }) => ({
  // In DEV, inject each component's <style> via JS instead of emitting a separate
  // `?svelte&type=style&lang.css` virtual module. Those virtual modules are what
  // vite's CSS/PostCSS pipeline chokes on at startup — on a cold request it gets
  // the raw .svelte source and logs "[postcss] Unknown word <script". Injecting in
  // dev removes the virtual modules entirely (no error), while PRODUCTION builds
  // still emit a proper external CSS bundle (emitCss: true).
  plugins: [svelte({ emitCss: command === "build" })],

  // Tauri expects a fixed port, fail if that port is not available
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
