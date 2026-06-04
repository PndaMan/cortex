import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// @tauri-apps/cli sets TAURI_DEV_HOST when running on a device
const host = process.env.TAURI_DEV_HOST;

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [svelte()],

  // Tauri expects a fixed port, fail if that port is not available
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Don't eagerly pre-transform the module graph. Vite's proactive crawl
    // requests Svelte `?type=style&lang.css` sub-modules out of order (before the
    // parent component fills the CSS cache), so PostCSS receives the raw .svelte
    // source and logs spurious "[postcss] Unknown word <script" errors at startup.
    // Real in-order browser imports resolve fine; disabling pre-transform keeps the
    // dev console clean with no effect on correctness or the production build.
    preTransformRequests: false,
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
