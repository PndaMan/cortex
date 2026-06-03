// Customizable keybindings. Single source of truth for the global keyboard
// engine (src/App.svelte) AND the Settings editor. Binds persist to the
// settings table under `keybind_<action>` keys, so changes survive restarts and
// take effect live (App.svelte reads `keybinds.map` reactively).

import * as api from "./api";

export type Action =
  | "cmdk"
  | "leader"
  | "toggleChat"
  | "newSubject"
  | "recorder"
  | "websearch"
  | "cycleTheme"
  | "music"
  | "insert"
  | "dashboard" // pressed after the `g` prefix
  | "help"
  | "dismissToast";

export const ACTION_LABELS: Record<Action, string> = {
  cmdk: "Command palette",
  leader: "Leader menu",
  toggleChat: "Toggle chat",
  newSubject: "New subject",
  recorder: "Record lecture",
  websearch: "Web search",
  cycleTheme: "Cycle theme",
  music: "Music panel",
  insert: "Insert / focus compose",
  dashboard: "Go to dashboard (after g)",
  help: "Help overlay",
  dismissToast: "Dismiss toast",
};

export const ACTION_ORDER: Action[] = [
  "cmdk", "leader", "toggleChat", "newSubject", "recorder", "websearch",
  "cycleTheme", "music", "insert", "dashboard", "help", "dismissToast",
];

export const HELIX_BINDS: Record<Action, string> = {
  cmdk: ":",
  leader: " ",
  toggleChat: "c",
  newSubject: "n",
  recorder: "r",
  websearch: "w",
  cycleTheme: "t",
  music: "m",
  insert: "i",
  dashboard: "d",
  help: "?",
  dismissToast: "q",
};

// A vim-flavoured alternative so the preset buttons do something real.
export const VIM_BINDS: Record<Action, string> = {
  ...HELIX_BINDS,
  cmdk: ";",
  insert: "a",
};

export const PRESETS = { helix: HELIX_BINDS, vim: VIM_BINDS } as const;
export type Preset = keyof typeof PRESETS;

const MODIFIER_KEYS = ["Control", "Shift", "Alt", "Meta", "AltGraph", "CapsLock", "ContextMenu"];
const isModifier = (k: string) => MODIFIER_KEYS.includes(k);

class Keybinds {
  map = $state<Record<Action, string>>({ ...HELIX_BINDS });
  preset = $state<Preset | "custom">("helix");

  /** Hydrate from persisted settings (call once on app init). Pass an already
   *  fetched settings map to avoid a second getAllSettings round-trip. */
  async load(preloaded?: Record<string, string>) {
    const all = preloaded ?? (await api.getAllSettings().catch(() => ({}) as Record<string, string>));
    let anyCustom = false;
    for (const a of ACTION_ORDER) {
      const v = all["keybind_" + a];
      // Ignore corrupt binds (empty or a bare modifier — these would hijack
      // Ctrl/Shift/etc.); fall back to the Helix default for that action.
      if (v && !isModifier(v)) {
        this.map[a] = v;
        if (v !== HELIX_BINDS[a]) anyCustom = true;
      }
    }
    // Auto-heal a corrupted command-palette bind: it must be a symbol key (":"),
    // never a letter/digit (a stray "c"/"p" bind would hijack typing & copy).
    if (/^[a-z0-9]$/i.test(this.map.cmdk)) {
      this.map.cmdk = HELIX_BINDS.cmdk;
      api.setSettings({ keybind_cmdk: HELIX_BINDS.cmdk }).catch(() => {});
    }
    // De-dupe: if two actions ended up on the same key (capture-bug artifact),
    // reset the whole map to the Helix preset so nothing double-fires.
    const used = new Set<string>();
    let dupe = false;
    for (const a of ACTION_ORDER) {
      if (used.has(this.map[a])) { dupe = true; break; }
      used.add(this.map[a]);
    }
    if (dupe) {
      this.map = { ...HELIX_BINDS };
      const vals: Record<string, string> = { keybind_preset: "helix" };
      for (const a of ACTION_ORDER) vals["keybind_" + a] = HELIX_BINDS[a];
      api.setSettings(vals).catch(() => {});
      anyCustom = false;
    }
    const savedPreset = all["keybind_preset"] as Preset | undefined;
    this.preset = savedPreset ?? (anyCustom ? "custom" : "helix");
  }

  /** Rebind a single action and persist it. Modifier-only keys are rejected. */
  set(a: Action, key: string) {
    if (!key || isModifier(key)) return;
    this.map[a] = key;
    this.preset = "custom";
    api.setSettings({ ["keybind_" + a]: key, keybind_preset: "custom" }).catch(() => {});
  }

  /** Apply a named preset wholesale and persist every bind. */
  applyPreset(p: Preset) {
    const binds = PRESETS[p];
    this.map = { ...binds };
    this.preset = p;
    const values: Record<string, string> = { keybind_preset: p };
    for (const a of ACTION_ORDER) values["keybind_" + a] = binds[a];
    api.setSettings(values).catch(() => {});
  }

  /** Which single-key action (if any) is bound to this key. */
  actionFor(key: string): Action | null {
    for (const a of ACTION_ORDER) if (this.map[a] === key) return a;
    return null;
  }
}

export const keybinds = new Keybinds();
