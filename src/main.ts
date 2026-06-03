// Cortex frontend entry. Load the design-system CSS first (tokens → components →
// shell layout), then Tailwind, then mount the Svelte app.
import "./styles/cortex.css";
import "./styles/app.css";
import "./app.css";

import { mount } from "svelte";
import App from "./App.svelte";

const app = mount(App, { target: document.getElementById("root")! });

export default app;
