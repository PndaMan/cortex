// Cortex frontend entry. Load the design-system CSS first (tokens → components →
// shell layout), then Tailwind, then mount the Svelte app.
import "./styles/cortex.css";
import "./styles/app.css";
import "./app.css";

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

import { mount } from "svelte";
import App from "./App.svelte";

const app = mount(App, { target: document.getElementById("root")! });

export default app;
