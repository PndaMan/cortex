// Cortex frontend entry. Load the design-system CSS first (tokens → components →
// shell layout), then Tailwind, then mount the Svelte app.
import "./styles/cortex.css";
import "./styles/app.css";
import "./app.css";
// Touch-adaptation layer — only takes effect under html[data-mobile] (set below).
import "./styles/mobile.css";

import { isMobile } from "./lib/platform";

// Apply the last-used theme BEFORE anything renders. The persisted theme lives
// in the settings table (async — too late for first paint), so applyTheme also
// mirrors it into localStorage; reading that here makes refreshes/relaunches
// paint the right palette instantly instead of flashing the default.
try {
  const cached = localStorage.getItem("cortex-theme");
  if (cached && /^[a-z0-9-]+$/.test(cached)) {
    document.documentElement.setAttribute("data-theme", cached);
  }
} catch { /* storage unavailable — default theme stands */ }

// Tag the root on phones (and when the dev force-mobile flag is on) so the mobile
// shell + mobile.css touch layer apply. Done before mount for a correct first paint.
if (isMobile) document.documentElement.setAttribute("data-mobile", "");

import { mount } from "svelte";
import App from "./App.svelte";

const app = mount(App, { target: document.getElementById("root")! });

export default app;
