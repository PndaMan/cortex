<script lang="ts">
  import { app, THEMES, THEME_LABELS } from "../lib/store.svelte";
  import type { Theme } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { Memory } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import { stations } from "../lib/mock";
  import { keybinds, ACTION_LABELS, ACTION_ORDER, LEADER_ACTIONS } from "../lib/keybinds.svelte";
  import type { Action } from "../lib/keybinds.svelte";
  import type { Snippet } from "svelte";

  // Shape for the reusable homelab endpoint-service snippet (Integrations tab).
  type EndpointOpts = {
    title: string;
    desc: string;
    value: string;
    oninput: (v: string) => void;
    onsave: () => void;
    onTest: () => void;
    state: null | "testing" | "ok" | "fail";
    placeholder: string;
    failHint?: string;
    hint?: string;
    extra?: Snippet;
  };

  // Fixed system bindings (not rebindable) shown for reference in the Keybinds tab
  // so the page reflects every shortcut, not just the customizable single-key set.
  const SYSTEM_BINDS = [
    { keys: "Ctrl F", label: "Find on page" },
    { keys: "Ctrl P", label: "Command palette" },
    { keys: "Esc",    label: "Close overlay / go back" },
    { keys: "g d",    label: "Go to dashboard" },
    { keys: "Alt 1–9", label: "Jump to subject N" },
  ];

  // ---- tab navigation ----
  const TABS = [
    { id: "profile",    label: "Profile",       icon: "diamond" },
    { id: "models",     label: "Models",        icon: "bolt" },
    { id: "keys",       label: "API keys",      icon: "lock" },
    { id: "appearance", label: "Appearance",    icon: "grid" },
    { id: "keybinds",   label: "Keybinds",      icon: "cmd" },
    { id: "homelab",    label: "Integrations",  icon: "globe" },
    { id: "calendar",   label: "Google Calendar", icon: "globe" },
    { id: "audio",      label: "Audio",         icon: "music" },
    { id: "data",       label: "Data & privacy",icon: "doc" },
    { id: "about",      label: "About",         icon: "diamond" },
  ] as const;

  let tab = $state<string>("profile");

  // This view owns the global keys while open
  $effect(() => {
    (window as any).__cortexViewKeys = true;
    return () => { (window as any).__cortexViewKeys = false; };
  });

  // ---- providers ----
  const PROVIDERS = [
    { id: "gemini",     label: "Gemini",              models: ["gemini-2.5-flash","gemini-2.5-pro","gemini-2.0-flash-001"] },
    { id: "openrouter", label: "OpenRouter",          models: [
      "google/gemini-2.5-flash",
      "anthropic/claude-sonnet-4.5",
      "openai/gpt-5-mini",
      "openai/gpt-4o",
      "openai/gpt-4o-mini",
      "openai/o3-mini",
      "anthropic/claude-3.7-sonnet",
      "anthropic/claude-3.5-sonnet",
      "anthropic/claude-3.5-haiku",
      "google/gemini-2.0-flash-001",
      "google/gemini-2.5-pro",
      "deepseek/deepseek-chat",
      "deepseek/deepseek-r1",
      "meta-llama/llama-3.3-70b-instruct",
      "qwen/qwen-2.5-72b-instruct",
      "mistralai/mistral-large",
      "x-ai/grok-2-1212",
    ] },
    { id: "openai",     label: "OpenAI",              models: ["gpt-4o","gpt-4o-mini","o3-mini"] },
    { id: "claude",     label: "Claude",              models: ["claude-opus-4-8","claude-sonnet-4-6","claude-haiku-4-5-20251001","claude-3-7-sonnet-20250219","claude-3-5-sonnet-20241022","claude-3-5-haiku-20241022"] },
    { id: "ollama",     label: "Ollama (local)",      models: ["llama3.3:70b","qwen2.5:32b","mistral-small"] },
    { id: "custom",     label: "Custom endpoint",     models: ["custom-model"] },
  ];
  const EMBED_PROVIDERS = [
    { id: "gemini",  label: "Gemini",       models: ["text-embedding-004"] },
    { id: "openai",  label: "OpenAI",       models: ["text-embedding-3-large","text-embedding-3-small"] },
    { id: "ollama",  label: "Ollama (local)",models: ["nomic-embed-text","mxbai-embed-large"] },
  ];
  const MODEL_TASKS = [
    { id: "chat",       label: "Chat",                  desc: "Scoped Q&A across sources" },
    { id: "cheatsheet", label: "Cheatsheet synthesis",  desc: "Completeness-checked merges" },
    { id: "audio",      label: "Audio overview script", desc: "Two-host podcast dialogue" },
    { id: "quiz",       label: "Quiz generation",       desc: "MCQ · short answer · cloze" },
    { id: "flashcard",  label: "Flashcard generation",  desc: "Q/A pairs + SRS scheduling" },
    { id: "embedding",  label: "Embedding",             desc: "Vector index for retrieval" },
  ] as const;

  type TaskId = typeof MODEL_TASKS[number]["id"];

  // ---- profile state ----
  let name       = $state("Sam Okonkwo");
  let pronouns   = $state("they/them");
  let level      = $state("postgrad");
  let field      = $state("Computer Science — MSc");
  let about      = $state("Final-year MSc student. I think in code and analogies, already comfortable with Big-O. I revise late at night and learn fastest from worked examples, then a terse summary.");
  let style      = $state("balanced");
  let explain    = $state<string[]>(["worked-examples","analogies"]);

  // ---- long-term memory state ----
  let memories     = $state<Memory[]>([]);
  let newMemory    = $state("");
  let memoryBusy   = $state(false);

  async function loadMemory() {
    memories = await api.listMemory().catch(() => [] as Memory[]);
  }
  async function addMemoryFact() {
    const text = newMemory.trim();
    if (!text || memoryBusy) return;
    memoryBusy = true;
    try {
      await api.addMemory(text);
      newMemory = "";
      await loadMemory();
    } catch {
      app.pushToast({ kind: "error", title: "Could not save memory" });
    } finally {
      memoryBusy = false;
    }
  }
  async function removeMemory(id: string) {
    try {
      await api.deleteMemory(id);
      await loadMemory();
    } catch {
      app.pushToast({ kind: "error", title: "Could not delete memory" });
    }
  }

  // ---- models state ----
  type TaskAssign = { provider: string; model: string; budget: string };
  let assign = $state<Record<TaskId, TaskAssign>>({
    chat:       { provider: "claude",  model: "claude-3-5-sonnet-20241022", budget: "8000" },
    cheatsheet: { provider: "gemini",  model: "gemini-2.5-pro",             budget: "32000" },
    audio:      { provider: "gemini",  model: "gemini-2.5-flash",           budget: "16000" },
    quiz:       { provider: "openai",  model: "gpt-4o-mini",                budget: "8000" },
    flashcard:  { provider: "claude",  model: "claude-3-5-haiku-20241022",  budget: "6000" },
    embedding:  { provider: "gemini",  model: "text-embedding-004",         budget: "—" },
  });

  function setTask(id: TaskId, patch: Partial<TaskAssign>) {
    assign = { ...assign, [id]: { ...assign[id], ...patch } };
  }

  // ---- keys state ----
  let keys = $state({ openrouter: "", gemini: "", claude: "", openai: "", custom: "" });
  const keyMeta = [
    { id: "openrouter", label: "OpenRouter",              note: "openrouter.ai/keys",    placeholder: "sk-or-…" },
    { id: "gemini",     label: "Gemini",                  note: "Google AI Studio",       placeholder: "AIza…" },
    { id: "claude",     label: "Claude",                  note: "console.anthropic.com",  placeholder: "sk-ant-…" },
    { id: "openai",     label: "OpenAI",                  note: "platform.openai.com",    placeholder: "sk-…" },
    { id: "custom",     label: "Custom / OpenAI-compatible", note: "self-hosted gateway", placeholder: "https://… + token" },
  ] as const;
  // show/hide per key
  let showKey = $state<Record<string, boolean>>({ openrouter: false, gemini: false, claude: false, openai: false, custom: false });

  // ---- appearance state ----
  const THEME_OPTS: { id: Theme; n: string; c: string; b: string }[] = [
    { id: "osaka-jade",  n: "Osaka Jade",       c: "#2dd5b7", b: "#111c18" },
    { id: "tokyo-night", n: "Tokyo Night",      c: "#7aa2f7", b: "#1a1b26" },
    { id: "catppuccin",  n: "Catppuccin Mocha", c: "#94e2d5", b: "#1e1e2e" },
    { id: "gruvbox",     n: "Gruvbox",          c: "#fabd2f", b: "#282828" },
    { id: "nord",        n: "Nord",             c: "#88c0d0", b: "#2e3440" },
    { id: "dracula",     n: "Dracula",          c: "#bd93f9", b: "#282a36" },
    { id: "rose-pine",   n: "Rosé Pine",        c: "#ebbcba", b: "#191724" },
    { id: "everforest",  n: "Everforest",       c: "#a7c080", b: "#2d353b" },
    { id: "solarized",   n: "Solarized",        c: "#268bd2", b: "#002b36" },
    { id: "kanagawa",    n: "Kanagawa",         c: "#7e9cd8", b: "#1f1f28" },
  ];
  let readFont      = $state("serif");
  let density       = $state("regular");

  $effect(() => { document.documentElement.setAttribute("data-read", readFont); });
  $effect(() => { document.documentElement.setAttribute("data-density", density === "compact" ? "compact" : "regular"); });

  // persist appearance on change
  $effect(() => {
    // track both values
    const rf = readFont;
    const d  = density;
    if (!loaded) return;
    api.setSettings({ reading_font: rf, density: d }).catch(() => {});
  });

  // ---- keybinds state ----
  // Binds live in the shared keybinds module (persisted + applied live).
  let listening = $state<Action | null>(null);

  function displayKey(k: string): string {
    return k === " " ? "Space" : k;
  }

  $effect(() => {
    if (!listening) return;
    (window as any).__cortexModalOpen = true;
    function onKey(e: KeyboardEvent) {
      // Ignore standalone modifier presses so the user can type a shifted key
      // (e.g. press Shift then ":") — keep listening until a real key arrives.
      if (["Shift", "Control", "Alt", "Meta", "AltGraph", "CapsLock"].includes(e.key)) {
        return;
      }
      // Swallow the event entirely so the global keyboard engine doesn't also
      // act on it (otherwise ":" would open the command palette).
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();
      const k = e.key === "Escape" ? null : e.key;
      if (k && listening) {
        const ok = keybinds.set(listening, k); // persists + applies live
        if (!ok) {
          app.pushToast({
            kind: "warning",
            title: "Key already in use",
            body: `“${k === " " ? "Space" : k}” is bound to another action — pick a different key.`,
          });
        }
      }
      listening = null;
      (window as any).__cortexModalOpen = false;
    }
    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      (window as any).__cortexModalOpen = false;
    };
  });

  // ---- local models (ollama) + web search + remote whisper ----
  let endpoint   = $state("http://localhost:11434");
  let searxng    = $state("");
  let whisperUrl = $state("");
  let webImages  = $state(false);
  let testState  = $state<null | "testing" | "ok" | "fail">(null);
  let searxState = $state<null | "testing" | "ok" | "fail">(null);
  let whisperState = $state<null | "testing" | "ok" | "fail">(null);

  function saveWhisper() {
    api.setSetting("whisper_url", whisperUrl.trim()).catch(() => {});
  }
  async function testWhisper() {
    if (!whisperUrl.trim()) return;
    whisperState = "testing";
    whisperState = await testEndpoint(whisperUrl);
  }

  // ---- live homelab sync (last-write-wins DB snapshot) ----
  let syncUrl   = $state("");
  let syncUser  = $state("");
  let syncPass  = $state("");
  let syncOn    = $state(false);
  let syncTestState = $state<null | "testing" | "ok" | "fail">(null);

  function saveSync() {
    api.setSettings({
      sync_url: syncUrl.trim(),
      sync_user: syncUser.trim(),
      sync_pass: syncPass,
      sync_enabled: syncOn ? "true" : "false",
    }).then(() => app.loadSyncStatus()).catch(() => {});
  }
  function toggleSync() {
    syncOn = !syncOn;
    if (syncOn && !syncUrl.trim()) { syncOn = false; return; }
    saveSync();
  }
  async function testSync() {
    if (!syncUrl.trim()) return;
    syncTestState = "testing";
    try {
      syncTestState = (await api.syncTest(syncUrl.trim(), syncUser.trim(), syncPass)) ? "ok" : "fail";
    } catch {
      syncTestState = "fail";
    }
  }
  function fmtSyncTime(ms: number): string {
    return ms ? new Date(ms).toLocaleString() : "never";
  }
  function syncPill() {
    switch (app.syncState) {
      case "syncing": return { cls: "draft", label: "Syncing…" };
      case "synced":  return { cls: "ready", label: "Synced" };
      case "error":   return { cls: "error", label: "Sync error" };
      case "idle":    return { cls: "ready", label: "On" };
      default:        return { cls: "pending", label: "Off" };
    }
  }

  // Diagrams/images from the homelab SearXNG. Mirrors app.webImagesEnabled and
  // persists an explicit choice so it survives the "default-on-when-connected".
  function setWebImages(on: boolean) {
    webImages = on;
    app.webImagesEnabled = on;
    api.setSetting("web_images_enabled", on ? "true" : "false").catch(() => {});
  }

  // ---- encrypted backups (age + rclone) ----
  let ageRecipient = $state("");
  let rcloneRemote = $state("");
  let backupInfo   = $state<api.BackupStatus | null>(null);
  let backingUp    = $state(false);
  async function refreshBackupStatus() {
    try { backupInfo = await api.backupStatus(); } catch { /* non-fatal */ }
  }
  function saveBackupConfig() {
    api.setSettings({
      backup_age_recipient: ageRecipient.trim(),
      backup_rclone_remote: rcloneRemote.trim(),
    }).then(refreshBackupStatus).catch(() => {});
  }
  async function runBackup() {
    if (backingUp) return;
    backingUp = true;
    try {
      const dest = await api.backupNow();
      app.pushToast({ kind: "success", title: "Backup uploaded", body: dest });
      await refreshBackupStatus();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Backup failed", body: String(e) });
    } finally {
      backingUp = false;
    }
  }
  function fmtBackupTime(ms: number | null): string {
    if (!ms) return "never";
    return new Date(ms).toLocaleString();
  }

  async function testEndpoint(url: string): Promise<"ok" | "fail"> {
    try {
      return (await api.pingUrl(url)) ? "ok" : "fail";
    } catch {
      return "fail";
    }
  }

  async function testConnection() {
    testState = "testing";
    testState = await testEndpoint(endpoint);
  }

  function saveSearxng() {
    api.setSetting("searxng_url", searxng).catch(() => {});
  }

  // ---- Google Calendar ----
  let gClientId = $state("");
  let gClientSecret = $state("");
  let gStatus = $state<api.GoogleStatus | null>(null);
  let gBusy = $state(false);
  $effect(() => {
    if (tab === "calendar" && gStatus === null) loadGoogle();
  });
  async function loadGoogle() {
    try {
      gClientId = (await api.getSetting("google_client_id")) ?? "";
      gClientSecret = (await api.getSetting("google_client_secret")) ?? "";
      gStatus = await api.googleStatus();
    } catch {
      gStatus = { connected: false, email: null, configured: false };
    }
  }
  function saveGoogleCreds() {
    api.setSettings({
      google_client_id: gClientId.trim(),
      google_client_secret: gClientSecret.trim(),
    }).catch(() => {});
  }
  async function connectGoogle() {
    gBusy = true;
    try {
      saveGoogleCreds();
      gStatus = await api.googleConnect();
      app.pushToast({ kind: "success", title: "Google Calendar connected", body: gStatus.email ?? undefined });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Connect failed", body: String(e) });
    } finally {
      gBusy = false;
    }
  }
  async function syncGoogle() {
    gBusy = true;
    try {
      const r = await api.googleSync();
      app.pushToast({ kind: "success", title: "Calendar synced", body: `${r.pulled} pulled · ${r.pushed} pushed` });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Sync failed", body: String(e) });
    } finally {
      gBusy = false;
    }
  }
  async function disconnectGoogle() {
    try {
      gStatus = await api.googleDisconnect();
      app.pushToast({ kind: "info", title: "Google disconnected" });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Disconnect failed", body: String(e) });
    }
  }

  async function testSearxng() {
    if (!searxng.trim()) return;
    searxState = "testing";
    searxState = await testEndpoint(searxng);
  }

  // Map a connection test state + whether the endpoint is set to a design-system
  // status pill (class modifier + label), so every integration shows state the
  // same way the rest of the app shows source/cheatsheet state.
  function connPill(state: null | "testing" | "ok" | "fail", configured: boolean) {
    if (state === "testing") return { cls: "draft", label: "Checking…" };
    if (state === "ok") return { cls: "ready", label: "Connected" };
    if (state === "fail") return { cls: "error", label: "Unreachable" };
    return configured
      ? { cls: "pending", label: "Untested" }
      : { cls: "pending", label: "Not set" };
  }

  // persist the Ollama endpoint on change
  $effect(() => {
    const ep = endpoint;
    if (!loaded) return;
    api.setSettings({ ollama_url: ep }).catch(() => {});
  });

  // ---- focus timer (pomodoro) durations — bound to the app-wide timer ----
  const pomoFields = [
    { key: "workMin",            label: "Focus length",  unit: " min", step: 5, min: 5, max: 90 },
    { key: "breakMin",           label: "Short break",   unit: " min", step: 1, min: 1, max: 30 },
    { key: "longBreakMin",       label: "Long break",    unit: " min", step: 5, min: 5, max: 60 },
    { key: "sessionsBeforeLong", label: "Sessions / set", unit: "",    step: 1, min: 2, max: 8  },
  ] as const;
  function pomoVal(key: string): number {
    return (app.pomo as unknown as Record<string, number>)[key];
  }
  function setPomo(key: string, delta: number) {
    const f = pomoFields.find((x) => x.key === key)!;
    const next = Math.max(f.min, Math.min(f.max, pomoVal(key) + delta));
    (app.pomo as unknown as Record<string, number>)[key] = next;
    api.setSettings({ ["pomo_" + key]: String(next) }).catch(() => {});
  }

  // ---- audio state ----
  let autoplay = $state(false);
  let station  = $state("lofi");
  let voiceA   = $state("maya");
  let voiceB   = $state("theo");

  // Set true only once the mount hydration below has loaded persisted values.
  // The persist $effects fire immediately on mount with their *default* values;
  // without this guard, opening Settings would clobber saved settings (e.g. it
  // silently reset default_station to "lofi" on every visit).
  let loaded = $state(false);

  // Built-in + user-added stations, so the Default-station picker shows them all.
  const allStations = $derived([
    ...stations.map((s) => ({ id: s.id, label: s.name })),
    ...app.customStations.map((s) => ({ id: s.id, label: s.name })),
  ]);

  // Stream-tool detection for the YouTube-audio engine (mpv sidecar).
  let mediaTools = $state<api.MediaTools | null>(null);
  $effect(() => {
    if (tab === "audio" && mediaTools === null) {
      api.mediaToolsStatus().then((t) => (mediaTools = t)).catch(() => {});
    }
  });

  // persist audio on change. NOTE: never read app.music here — reading + writing
  // it in one effect creates a self-triggering reactive loop (crashes the view).
  // The live-player sync happens in the Picker's onChange handler instead.
  $effect(() => {
    const st = station;
    const ap = autoplay;
    if (!loaded) return;
    api.setSettings({ default_station: st, autoplay: String(ap) }).catch(() => {});
  });

  // persist host voices on change
  $effect(() => {
    const a = voiceA;
    const b = voiceB;
    if (!loaded) return;
    api.setSettings({ voice_a: a, voice_b: b }).catch(() => {});
  });

  // ---- window behaviour ----
  // Default ON: closing the window hides to the tray so ingest/generation/music
  // keep running; the backend treats anything but "false" as enabled.
  let closeToTray = $state(true);
  function toggleCloseToTray() {
    closeToTray = !closeToTray;
    api.setSetting("close_to_tray", closeToTray ? "true" : "false").catch(() => {});
  }

  // ---- data & privacy state ----
  let offlineMode = $state(false);
  let stats       = $state<api.DbStats | null>(null);

  function fmtBytes(n: number): string {
    if (n >= 1024 * 1024 * 1024) return (n / (1024 * 1024 * 1024)).toFixed(1) + " GB";
    if (n >= 1024 * 1024)        return (n / (1024 * 1024)).toFixed(1) + " MB";
    if (n >= 1024)               return (n / 1024).toFixed(0) + " KB";
    return n + " B";
  }

  async function loadStats() {
    stats = await api.dbStats().catch(() => null);
  }

  function toggleOffline() {
    offlineMode = !offlineMode;
    api.setSetting("offline_mode", offlineMode ? "true" : "false").catch(() => {});
  }

  async function clearCaches() {
    testState = null;
    searxState = null;
    try {
      await api.optimizeDb();
      await loadStats();
      app.pushToast({ kind: "success", title: "Storage optimized", body: "Reclaimed unused space (VACUUM)." });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Optimize failed", body: String(e) });
    }
  }

  async function exportData() {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const dest = await save({
        defaultPath: "cortex-export.db",
        filters: [{ name: "SQLite database", extensions: ["db"] }],
      });
      if (!dest) return;
      await api.exportDatabase(dest);
      app.pushToast({ kind: "success", title: "Exported", body: dest });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Export failed", body: String(e) });
    }
  }

  async function deleteEverything() {
    const ok = window.confirm(
      "Delete ALL data?\n\nThis wipes the local database — every subject, source, cheatsheet, and embedding. This cannot be undone. Your settings and API keys are kept.",
    );
    if (!ok) return;
    try {
      await api.deleteAllData();
      await loadStats();
      app.pushToast({ kind: "success", title: "All data deleted", body: "Reload the app to start fresh." });
    } catch {
      app.pushToast({ kind: "error", title: "Delete failed" });
    }
  }

  // ---- mount: hydrate from backend ----
  $effect(() => {
    // All three are independent; issue them concurrently.
    (async () => {
      const [s] = await Promise.all([
        api.getAllSettings().catch(() => ({}) as Record<string, string>),
        loadMemory(),
        loadStats(),
      ]);
      // API keys
      if (s.openrouter_api_key) keys = { ...keys, openrouter: s.openrouter_api_key };
      if (s.gemini_api_key)     keys = { ...keys, gemini: s.gemini_api_key };
      if (s.claude_api_key)     keys = { ...keys, claude: s.claude_api_key };
      if (s.openai_api_key)     keys = { ...keys, openai: s.openai_api_key };
      if (s.custom_endpoint)    keys = { ...keys, custom: s.custom_endpoint };

      // Models
      for (const taskId of ["chat","cheatsheet","audio","quiz","flashcard","embedding"] as TaskId[]) {
        const raw = s[`model_${taskId}`];
        if (raw) {
          const sep = raw.indexOf(":");
          if (sep !== -1) {
            const prov  = raw.slice(0, sep);
            const model = raw.slice(sep + 1);
            assign = { ...assign, [taskId]: { ...assign[taskId], provider: prov, model } };
          }
        }
        const budget = s[`budget_${taskId}`];
        if (budget) {
          assign = { ...assign, [taskId]: { ...assign[taskId], budget } };
        }
      }
      if (s.embed_provider) {
        assign = { ...assign, embedding: { ...assign.embedding, provider: s.embed_provider } };
      }

      // Local models + web search + remote whisper
      if (s.ollama_url)                    endpoint = s.ollama_url;
      if (s.searxng_url)                   searxng  = s.searxng_url;
      if (s.whisper_url)                   whisperUrl = s.whisper_url;
      // Live sync
      if (s.sync_url)   syncUrl  = s.sync_url;
      if (s.sync_user)  syncUser = s.sync_user;
      if (s.sync_pass)  syncPass = s.sync_pass;
      syncOn = s.sync_enabled === "true";
      // Encrypted backups
      if (s.backup_age_recipient) ageRecipient = s.backup_age_recipient;
      if (s.backup_rclone_remote) rcloneRemote = s.backup_rclone_remote;
      refreshBackupStatus();

      // Appearance
      if (s.reading_font)   readFont      = s.reading_font;
      if (s.density)        density       = s.density;
      // Window behaviour (default ON: closing hides to the tray)
      if (s.close_to_tray !== undefined) closeToTray = s.close_to_tray !== "false";

      // Audio: default station, autoplay, host voices
      if (s.default_station) station = s.default_station;
      if (s.autoplay !== undefined) autoplay = s.autoplay === "true";
      if (s.voice_a) voiceA = s.voice_a;
      if (s.voice_b) voiceB = s.voice_b;

      // Homelab: web images (diagrams) toggle mirrors the store's resolved value.
      webImages = app.webImagesEnabled;

      // Data & privacy
      if (s.offline_mode !== undefined) offlineMode = s.offline_mode === "true";

      // Profile
      if (s.profile_name)      name     = s.profile_name;
      if (s.profile_pronouns)  pronouns = s.profile_pronouns;
      if (s.profile_level)     level    = s.profile_level;
      if (s.profile_field)     field    = s.profile_field;
      if (s.profile_about)     about    = s.profile_about;
      if (s.profile_style)     style    = s.profile_style;
      if (s.profile_explain)   explain  = s.profile_explain.split(",").filter(Boolean);

      // Hydration complete — persist effects may now write without clobbering.
      loaded = true;
    })();
  });

  // ---- helpers ----
  function saveProfile() {
    api.setSettings({
      profile_name: name,
      profile_pronouns: pronouns,
      profile_level: level,
      profile_field: field,
      profile_about: about,
      profile_style: style,
      profile_explain: explain.join(","),
    }).then(() => app.pushToast({ kind: "success", title: "Profile saved", body: "The AI will use your updated context." }))
      .catch(() => app.pushToast({ kind: "error", title: "Save failed" }));
  }

  function saveKeys() {
    api.setSettings({
      openrouter_api_key: keys.openrouter,
      gemini_api_key:     keys.gemini,
      claude_api_key:     keys.claude,
      openai_api_key:     keys.openai,
      custom_endpoint:    keys.custom,
    }).then(() => app.pushToast({ kind: "success", title: "Keys saved", body: "Stored in the system keychain." }))
      .catch(() => app.pushToast({ kind: "error", title: "Save failed" }));
  }

  function onModelProviderChange(taskId: TaskId, p: string) {
    const provList = taskId === "embedding" ? EMBED_PROVIDERS : PROVIDERS;
    const np = provList.find((x) => x.id === p) ?? provList[0];
    setTask(taskId, { provider: p, model: np.models[0] });
    const kv: Record<string, string> = { [`model_${taskId}`]: p + ":" + np.models[0] };
    if (taskId === "embedding") kv.embed_provider = p;
    api.setSettings(kv).catch(() => {});
  }

  function onModelChange(taskId: TaskId, m: string) {
    setTask(taskId, { model: m });
    api.setSettings({ [`model_${taskId}`]: assign[taskId].provider + ":" + m }).catch(() => {});
  }

  const levelLabels: Record<string, string> = {
    undergrad: "Undergraduate",
    postgrad:  "Postgraduate",
    phd:       "PhD / research",
    self:      "Self-study",
  };

  function toggleExplain(id: string) {
    explain = explain.includes(id)
      ? explain.filter((x) => x !== id)
      : [...explain, id];
  }
</script>

<div class="settings">
  <!-- NAV SIDEBAR -->
  <aside class="set-nav">
    <div class="set-nav-h">
      <button
        class="btn btn--icon btn--sm btn--ghost"
        onclick={() => app.setView("subject")}
        title="Back"
      >
        <Icon name="chevron" size={14} style="transform:rotate(180deg)" />
      </button>
      <span class="mono" style="color:var(--fg-bright);font-weight:600">Settings</span>
    </div>

    {#each TABS as t}
      <button
        class={"set-nav-item" + (tab === t.id ? " on" : "")}
        onclick={() => (tab = t.id)}
      >
        <Icon name={t.icon} size={13} /> {t.label}
      </button>
    {/each}

    <div class="set-nav-foot mono faint">Cortex v0.1 · BYOK · local-first</div>
  </aside>

  <!-- MAIN BODY -->
  <div class="set-body">

    <!-- ===== PROFILE ===== -->
    {#if tab === "profile"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Profile</div>
          <h1 class="read set-title">Who the AI thinks you are</h1>
          <p class="set-sub">Shared with every chat and generation so answers fit your level and style. Stays on this machine.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Identity</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Display name</div></div>
              <div class="set-row-r"><input class="input" bind:value={name} /></div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Pronouns</div></div>
              <div class="set-row-r"><input class="input" bind:value={pronouns} /></div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Level</div></div>
              <div class="set-row-r">
                <Picker
                  value={level}
                  onChange={(v) => (level = v)}
                  options={[
                    { id: "undergrad", label: "Undergraduate" },
                    { id: "postgrad",  label: "Postgraduate" },
                    { id: "phd",       label: "PhD / research" },
                    { id: "self",      label: "Self-study" },
                  ]}
                />
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Field of study</div></div>
              <div class="set-row-r"><input class="input" bind:value={field} /></div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">About you</h3>
            <p class="set-group-d">Context the AI uses to personalize explanations.</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-l"><div class="set-row-t">In your words</div></div>
              <div class="set-row-r">
                <textarea class="input set-textarea" bind:value={about} rows={4}></textarea>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Response style</div>
                <div class="set-row-d">How much detail by default.</div>
              </div>
              <div class="set-row-r">
                <div class="seg">
                  {#each [{ id: "concise", label: "Concise" }, { id: "balanced", label: "Balanced" }, { id: "detailed", label: "Detailed" }] as opt}
                    <button type="button" class={"seg-opt" + (style === opt.id ? " on" : "")} onclick={() => (style = opt.id)}>{opt.label}</button>
                  {/each}
                </div>
              </div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-l">
                <div class="set-row-t">Explain with</div>
                <div class="set-row-d">Pick what helps you learn fastest.</div>
              </div>
              <div class="set-row-r">
                <div class="tag-suggest" style="margin-top:0">
                  {#each [{ id: "worked-examples", label: "worked examples" }, { id: "analogies", label: "analogies" }, { id: "formal-proofs", label: "formal proofs" }, { id: "diagrams", label: "diagrams" }, { id: "code", label: "code snippets" }] as opt}
                    <button
                      type="button"
                      class={"tag-chip-add" + (explain.includes(opt.id) ? " on" : "")}
                      onclick={() => toggleExplain(opt.id)}
                    >
                      {#if explain.includes(opt.id)}<Icon name="check" size={9} />{/if}
                      {opt.label}
                    </button>
                  {/each}
                </div>
              </div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">Memory</h3>
            <p class="set-group-d">Long-term facts the AI is given in every chat — like remembering your exam date, the textbook you use, or how you like answers framed.</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-r">
                <div class="row-inline">
                  <input
                    class="input"
                    bind:value={newMemory}
                    placeholder="e.g. My final exam is on June 20th"
                    onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); addMemoryFact(); } }}
                  />
                  <button class="btn btn--primary" onclick={addMemoryFact} disabled={!newMemory.trim() || memoryBusy}>
                    <Icon name="check" size={13} /> Remember
                  </button>
                </div>
              </div>
            </div>
            {#if memories.length === 0}
              <div class="set-row">
                <div class="set-row-l"><div class="set-row-d">No memories yet. Add a fact above and the AI will keep it in mind.</div></div>
              </div>
            {:else}
              {#each memories as m (m.id)}
                <div class="set-row">
                  <div class="set-row-l"><div class="set-row-t">{m.content}</div></div>
                  <div class="set-row-r">
                    <button class="btn btn--icon btn--sm btn--ghost" onclick={() => removeMemory(m.id)} title="Forget this">
                      <Icon name="x" size={13} />
                    </button>
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        </section>

        <div class="set-preview">
          <div class="label" style="margin-bottom:8px">What the AI receives</div>
          <pre class="set-sysprompt mono">User: {name} ({pronouns}) · {levelLabels[level] ?? level}
Studying: {field}
Style: {style}, prefers {explain.join(", ") || "no special format"}
Notes: {about}</pre>
        </div>

        <div class="set-foot-actions">
          <button class="btn btn--primary" onclick={saveProfile}>
            <Icon name="check" size={13} /> Save profile
          </button>
        </div>
      </div>

    <!-- ===== MODELS ===== -->
    {:else if tab === "models"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Models</div>
          <h1 class="read set-title">A model for every task</h1>
          <p class="set-sub">Route each job to the provider that does it best. Token budgets cap spend per call.</p>
        </header>

        <div class="set-card set-table">
          <div class="mt-head mono">
            <span>Task</span><span>Provider</span><span>Model</span><span>Token budget</span>
          </div>
          {#each MODEL_TASKS as t}
            {@const a = assign[t.id]}
            {@const provList = t.id === "embedding" ? EMBED_PROVIDERS : PROVIDERS}
            {@const prov = provList.find((p) => p.id === a.provider) ?? provList[0]}
            <div class="mt-row">
              <div class="mt-task">
                <div class="mt-task-t">{t.label}</div>
                <div class="mt-task-d mono">{t.desc}</div>
              </div>
              <Picker
                value={a.provider}
                onChange={(p) => onModelProviderChange(t.id, p)}
                options={provList.map((p) => ({ id: p.id, label: p.label }))}
              />
              <Picker
                value={a.model}
                onChange={(m) => onModelChange(t.id, m)}
                options={prov.models.map((m) => ({ id: m, label: m }))}
              />
              {#if t.id === "embedding"}
                <span class="mono faint mt-budget-na">n/a</span>
              {:else}
                <input
                  class="input mono mt-budget"
                  value={a.budget}
                  oninput={(e) => { const v = (e.target as HTMLInputElement).value; setTask(t.id, { budget: v }); api.setSettings({ ["budget_" + t.id]: v }).catch(() => {}); }}
                />
              {/if}
            </div>
          {/each}
        </div>

        <div class="set-note mono">
          <Icon name="diamond" size={11} color="var(--accent)" />
          Ollama tasks run fully offline on this machine or your homelab — no key required.
        </div>
      </div>

    <!-- ===== API KEYS ===== -->
    {:else if tab === "keys"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">API keys</div>
          <h1 class="read set-title">Bring your own keys</h1>
          <p class="set-sub">Stored in the OS keychain, never synced. Nothing routes through Cortex servers.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Providers</h3></div>
          <div class="set-card">
            {#each keyMeta as k}
              {@const isSet = !!keys[k.id as keyof typeof keys]}
              <div class="set-row stacked">
                <div class="set-row-l">
                  <div class="set-row-t">
                    <span class="row-keytitle">
                      {k.label}
                      <span class={"key-status " + (isSet ? "ok" : "off")}>
                        {isSet ? `connected · ${keys[k.id as keyof typeof keys].trim().length} chars` : "not set"}
                      </span>
                    </span>
                  </div>
                  <div class="set-row-d">{k.note}</div>
                </div>
                <div class="set-row-r">
                  <div class="masked">
                    <input
                      class="input mono"
                      type={showKey[k.id] ? "text" : "password"}
                      value={keys[k.id as keyof typeof keys]}
                      oninput={(e) => { keys = { ...keys, [k.id]: (e.target as HTMLInputElement).value }; }}
                      placeholder={k.placeholder}
                      spellcheck={false}
                    />
                    <button
                      type="button"
                      class="masked-eye"
                      onclick={() => { showKey = { ...showKey, [k.id]: !showKey[k.id] }; }}
                      title={showKey[k.id] ? "Hide" : "Show"}
                    >
                      <Icon name={showKey[k.id] ? "x" : "search"} size={13} />
                    </button>
                  </div>
                  {#if isSet}
                    <button
                      type="button"
                      class="btn btn--ghost btn--sm"
                      style="margin-top:6px"
                      onclick={() => { keys = { ...keys, [k.id]: "" }; }}
                    >Clear</button>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </section>

        <div class="set-foot-actions">
          <button class="btn btn--primary" onclick={saveKeys}>
            <Icon name="check" size={13} /> Save keys
          </button>
        </div>
      </div>

    <!-- ===== APPEARANCE ===== -->
    {:else if tab === "appearance"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Appearance</div>
          <h1 class="read set-title">Make it yours</h1>
          <p class="set-sub">Cortex re-skins live from your Omarchy theme, or pick one manually.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Theme</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Follow Omarchy theme</div>
                <div class="set-row-d">Mirror your desktop's current Omarchy palette on every launch. Picking a theme below turns this off.</div>
              </div>
              <div class="set-row-r">
                <button
                  type="button"
                  class={"st-toggle" + (app.followOmarchy ? " on" : "")}
                  role="switch"
                  aria-checked={app.followOmarchy}
                  aria-label="follow omarchy theme"
                  onclick={async () => {
                    const matched = await app.setFollowOmarchy(!app.followOmarchy);
                    if (app.followOmarchy && !matched) {
                      app.pushToast({ kind: "warning", title: "Omarchy theme not found", body: "Couldn't read your Omarchy theme, or it has no Cortex match." });
                    } else if (matched) {
                      app.pushToast({ kind: "success", title: "Following Omarchy", body: `Matched → ${THEME_LABELS[matched]}.` });
                    }
                  }}
                ><span class="st-knob"></span></button>
              </div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Manual theme</h3></div>
          <div class="set-card">
            <div class="set-themes">
              {#each THEME_OPTS as t}
                <button
                  class={"set-theme" + (app.theme === t.id ? " on" : "")}
                  onclick={() => { if (app.followOmarchy) app.setFollowOmarchy(false); app.setTheme(t.id); }}
                  style="background:{t.b}"
                >
                  <div class="set-theme-sws">
                    <span style="background:{t.c}"></span>
                    <span style="background:{t.b};border:1px solid {t.c}"></span>
                  </div>
                  <span class="set-theme-n" style="color:{app.theme === t.id ? '#fff' : '#cbd5d0'}">{t.n}</span>
                  {#if app.theme === t.id}
                    <span class="set-theme-check" style="color:{t.c}">
                      <Icon name="check" size={13} />
                    </span>
                  {/if}
                </button>
              {/each}
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Reading</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Cheatsheet typeface</div>
                <div class="set-row-d">The voice of everything you read to learn.</div>
              </div>
              <div class="set-row-r">
                <div class="seg">
                  {#each [{ id: "serif", label: "Serif" }, { id: "sans", label: "Sans" }] as opt}
                    <button type="button" class={"seg-opt" + (readFont === opt.id ? " on" : "")} onclick={() => (readFont = opt.id)}>{opt.label}</button>
                  {/each}
                </div>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Density</div>
                <div class="set-row-d">Spacing throughout the app.</div>
              </div>
              <div class="set-row-r">
                <div class="seg">
                  {#each [{ id: "regular", label: "Regular" }, { id: "compact", label: "Compact" }] as opt}
                    <button type="button" class={"seg-opt" + (density === opt.id ? " on" : "")} onclick={() => (density = opt.id)}>{opt.label}</button>
                  {/each}
                </div>
              </div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Window</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Close to tray</div>
                <div class="set-row-d">Closing the window keeps Cortex running in the tray — ingest, generation and music continue. Quit from the tray menu.</div>
              </div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (closeToTray ? " on" : "")} onclick={toggleCloseToTray} role="switch" aria-checked={closeToTray} aria-label="close to tray"><span class="st-knob"></span></button>
              </div>
            </div>
          </div>
        </section>
      </div>

    <!-- ===== KEYBINDS ===== -->
    {:else if tab === "keybinds"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Keybinds</div>
          <h1 class="read set-title">Helix-style, your way</h1>
          <p class="set-sub">Click any binding to rebind it. Press Esc while listening to cancel.</p>
        </header>

        <div class="set-card">
          <div class="set-row">
            <div class="set-row-l">
              <div class="set-row-t">Preset</div>
              <div class="set-row-d">Starting point for bindings.</div>
            </div>
            <div class="set-row-r">
              <div class="seg">
                {#each [{ id: "helix", label: "Helix" }, { id: "vim", label: "Vim" }, { id: "custom", label: "Custom" }] as opt}
                  <button
                    type="button"
                    class={"seg-opt" + (keybinds.preset === opt.id ? " on" : "")}
                    disabled={opt.id === "custom"}
                    onclick={() => { if (opt.id === "helix" || opt.id === "vim") keybinds.applyPreset(opt.id); }}
                  >{opt.label}</button>
                {/each}
              </div>
            </div>
          </div>
        </div>

        <div class="set-card set-binds">
          {#each ACTION_ORDER as action}
            <div class="bind-row">
              <span class="bind-label">{ACTION_LABELS[action]}</span>
              <button
                class={"bind-keys" + (listening === action ? " listening" : "")}
                onclick={() => (listening = action)}
              >
                {#if listening === action}
                  <span class="mono faint">press a key…</span>
                {:else}
                  <span class="kbd">{displayKey(keybinds.map[action])}</span>
                {/if}
              </button>
            </div>
          {/each}
        </div>

        <div class="set-foot-actions">
          <button
            class="btn btn--ghost"
            onclick={() => {
              const p = keybinds.preset === "vim" ? "vim" : "helix";
              keybinds.applyPreset(p);
              app.pushToast({ kind: "info", title: "Reset", body: `Bindings restored to ${p === "vim" ? "Vim" : "Helix"} preset.` });
            }}
          >
            Reset to preset
          </button>
        </div>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">Leader menu (Space then…)</h3>
            <p class="set-group-d">Press <span class="kbd">Space</span> to open the leader menu, then a key. Fixed, mnemonic — they only fire while the menu is open.</p>
          </div>
          <div class="set-card set-binds">
            {#each LEADER_ACTIONS as a}
              <div class="bind-row">
                <span class="bind-label">{a.label} <span class="faint">· {a.detail}</span></span>
                <span class="bind-keys"><span class="kbd">Space</span> <span class="kbd">{a.key}</span></span>
              </div>
            {/each}
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">System</h3>
            <p class="set-group-d">Built-in shortcuts that follow OS conventions and can't be rebound.</p>
          </div>
          <div class="set-card set-binds">
            {#each SYSTEM_BINDS as b}
              <div class="bind-row">
                <span class="bind-label">{b.label}</span>
                <span class="bind-keys">
                  {#each b.keys.split(" ") as part}<span class="kbd">{part}</span> {/each}
                </span>
              </div>
            {/each}
          </div>
        </section>
      </div>

    <!-- ===== INTEGRATIONS ===== -->
    {:else if tab === "homelab"}
      {#snippet endpointService(o: EndpointOpts)}
        {@const p = connPill(o.state, !!o.value.trim())}
        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">{o.title}</h3>
              <p class="set-group-d">{@html o.desc}</p>
            </div>
            <span class="status-pill status-pill--{p.cls}"><span class="dot"></span>{p.label}</span>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="row-inline">
                <input class="input mono" value={o.value} oninput={(e) => o.oninput(e.currentTarget.value)} onchange={o.onsave} onblur={o.onsave} placeholder={o.placeholder} />
                <button class="btn" onclick={o.onTest} disabled={o.state === "testing" || !o.value.trim()}>
                  <Icon name="refresh" size={12} /> Test
                </button>
              </div>
              {#if o.state === "fail" && o.failHint}
                <div class="set-row-d" style="color:var(--err,#e5484d)">{o.failHint}</div>
              {/if}
              {#if o.hint}<div class="set-row-d">{@html o.hint}</div>{/if}
            </div>
            {#if o.extra}{@render o.extra()}{/if}
          </div>
        </section>
      {/snippet}

      {#snippet diagramsToggle()}
        <div class="set-row">
          <div class="set-row-l">
            <div class="set-row-t">Illustrate with diagrams</div>
            <div class="set-row-d">A relevant diagram per cheatsheet section + images in chat. On by default once SearXNG is connected.</div>
          </div>
          <div class="set-row-r">
            <button type="button" class={"st-toggle" + (webImages ? " on" : "")} onclick={() => setWebImages(!webImages)} disabled={!searxng.trim()} role="switch" aria-checked={webImages} aria-label="diagrams"><span class="st-knob"></span></button>
          </div>
        </div>
      {/snippet}

      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Integrations</div>
          <h1 class="read set-title">Local models, web search & backups</h1>
          <p class="set-sub">Optional, self-hosted services. Cortex stays fully local without any of them — or run them all with the <span class="mono">homelab/</span> docker compose.</p>
        </header>

        {@render endpointService({
          title: "Local models (Ollama)",
          desc: "Run chat/embeddings on a local or self-hosted <span class='mono'>ollama</span> server — keyless and private. Select <span class='mono'>ollama:&lt;model&gt;</span> per task in Models.",
          value: endpoint,
          oninput: (v: string) => (endpoint = v),
          onsave: () => {},
          onTest: testConnection,
          state: testState,
          placeholder: "http://localhost:11434",
          failHint: "Unreachable — check the URL and that ollama is running.",
        })}

        {@render endpointService({
          title: "Web search (SearXNG)",
          desc: "Pulls diagrams/images into cheatsheets and enriches chat answers. There is no separate web-search page.",
          value: searxng,
          oninput: (v: string) => (searxng = v),
          onsave: saveSearxng,
          onTest: testSearxng,
          state: searxState,
          placeholder: "http://192.168.1.50:8080",
          failHint: "Unreachable — check the URL and that SearXNG is running with JSON enabled.",
          hint: "Base URL only. Enable JSON in settings.yml (<span class='mono'>formats: [html, json]</span>).",
          extra: diagramsToggle,
        })}

        {@render endpointService({
          title: "Remote transcription (Whisper)",
          desc: "Offload lecture transcription to a homelab Whisper server instead of installing Python locally. Leave blank to transcribe on this machine.",
          value: whisperUrl,
          oninput: (v: string) => (whisperUrl = v),
          onsave: saveWhisper,
          onTest: testWhisper,
          state: whisperState,
          placeholder: "http://192.168.1.50:9009",
          failHint: "Unreachable — check the URL and that the Whisper service is running.",
          hint: "OpenAI-compatible endpoint, base URL only (Cortex calls <span class='mono'>/v1/audio/transcriptions</span>).",
        })}

        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">Live sync</h3>
              <p class="set-group-d">Auto-store your library to a homelab WebDAV target and fetch the newest copy on launch. Last-write-wins across devices (whole-database, not per-record).</p>
            </div>
            <span class="status-pill status-pill--{syncPill().cls}"><span class="dot"></span>{syncPill().label}</span>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Enable live sync</div>
                <div class="set-row-d">Push after changes (debounced) + pull a newer copy on launch.</div>
              </div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (syncOn ? " on" : "")} onclick={toggleSync} disabled={!syncUrl.trim()} role="switch" aria-checked={syncOn} aria-label="live sync"><span class="st-knob"></span></button>
              </div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">WebDAV URL</div>
              <div class="row-inline">
                <input class="input mono" bind:value={syncUrl} onchange={saveSync} onblur={saveSync} placeholder="http://192.168.1.50:9010" />
                <button class="btn" onclick={testSync} disabled={syncTestState === "testing" || !syncUrl.trim()}>
                  <Icon name="refresh" size={12} /> Test
                </button>
              </div>
              {#if syncTestState === "fail"}
                <div class="set-row-d" style="color:var(--err,#e5484d)">Unreachable or auth failed — check the URL and credentials.</div>
              {:else if syncTestState === "ok"}
                <div class="set-row-d" style="color:var(--ok)">Reachable.</div>
              {/if}
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">Username <span class="faint">optional</span></div>
              <input class="input mono" bind:value={syncUser} onchange={saveSync} onblur={saveSync} placeholder="cortex" />
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">Password <span class="faint">optional</span></div>
              <div class="row-inline">
                <input class="input mono" type="password" bind:value={syncPass} onchange={saveSync} onblur={saveSync} placeholder="••••••••" />
                <button class="btn btn--primary" disabled={!syncOn || app.syncState === "syncing"} onclick={() => app.syncNow()}>
                  <Icon name="upload" size={12} /> {app.syncState === "syncing" ? "Syncing…" : "Sync now"}
                </button>
              </div>
              <div class="set-row-d">Last synced: <span class="mono">{fmtSyncTime(app.syncLastAt)}</span>. Bring up a WebDAV target with the <span class="mono">homelab/</span> compose.</div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">Encrypted backups</h3>
              <p class="set-group-d">Snapshot the database, encrypt it with <span class="mono">age</span>, and upload with <span class="mono">rclone</span>. Nothing leaves the machine unencrypted.</p>
            </div>
            {#if backupInfo}
              <div class="svc-tools">
                <span class="badge {backupInfo.age_found ? 'badge--ok' : 'badge--err'}"><span class="dot"></span>age</span>
                <span class="badge {backupInfo.rclone_found ? 'badge--ok' : 'badge--err'}"><span class="dot"></span>rclone</span>
              </div>
            {/if}
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-t">age recipient (public key)</div>
              <input class="input mono" bind:value={ageRecipient} onchange={saveBackupConfig} onblur={saveBackupConfig} placeholder="age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p" />
              <div class="set-row-d">From <span class="mono">age-keygen</span>. Only this key's holder can decrypt the backups.</div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">rclone remote</div>
              <div class="row-inline">
                <input class="input mono" bind:value={rcloneRemote} onchange={saveBackupConfig} onblur={saveBackupConfig} placeholder="homelab:cortex-backups" />
                <button class="btn btn--primary" disabled={backingUp} onclick={runBackup}>
                  <Icon name={backingUp ? "refresh" : "upload"} size={12} /> {backingUp ? "Backing up…" : "Back up now"}
                </button>
              </div>
              <div class="set-row-d">
                An rclone remote + path, e.g. <span class="mono">homelab:cortex-backups</span> (configure with <span class="mono">rclone config</span>).
                Last backup: <span class="mono">{fmtBackupTime(backupInfo?.last_at ?? null)}</span>{#if backupInfo?.last_dest} → <span class="mono">{backupInfo.last_dest}</span>{/if}
              </div>
            </div>
          </div>
        </section>

      </div>

    <!-- ===== AUDIO ===== -->
    {:else if tab === "audio"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Audio</div>
          <h1 class="read set-title">Study sound & voices</h1>
          <p class="set-sub">Defaults for the music player and generated audio overviews.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Study music</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Default station</div></div>
              <div class="set-row-r">
                <Picker
                  value={station}
                  onChange={(v) => { station = v; app.music = { ...app.music, current: v }; }}
                  options={allStations}
                />
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Autoplay on launch</div></div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (autoplay ? " on" : "")} onclick={() => (autoplay = !autoplay)} role="switch" aria-checked={autoplay} aria-label="autoplay"><span class="st-knob"></span></button>
              </div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">YouTube streaming</h3>
            <p class="set-group-d">Paste a YouTube video or livestream URL in the music panel to stream it ad-free. Uses a headless <span class="mono">mpv</span> + <span class="mono">yt-dlp</span> (auto-downloaded on first use). Nothing is bundled — only the URL is saved.</p>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Tools</div></div>
              <div class="set-row-r">
                {#if mediaTools}
                  <span class="mono" style="color:{mediaTools.mpv ? 'var(--ok)' : 'var(--danger,#e06c75)'}">
                    <Icon name={mediaTools.mpv ? "check" : "x"} size={12} /> mpv
                  </span>
                  <span class="mono" style="margin-left:14px;color:{mediaTools.ffmpeg ? 'var(--ok)' : 'var(--danger,#e06c75)'}">
                    <Icon name={mediaTools.ffmpeg ? "check" : "x"} size={12} /> ffmpeg
                  </span>
                  <span class="mono" style="margin-left:14px;color:{mediaTools.ytdlp ? 'var(--ok)' : 'var(--warn)'}">
                    <Icon name={mediaTools.ytdlp ? "check" : "refresh"} size={12} /> yt-dlp
                  </span>
                {:else}
                  <span class="mono faint">…</span>
                {/if}
              </div>
            </div>
            {#if mediaTools && !mediaTools.mpv}
              <div class="set-row">
                <div class="set-row-l"><div class="set-row-d">Install mpv to enable YouTube streaming: <span class="mono">sudo pacman -S mpv</span></div></div>
              </div>
            {:else if mediaTools && !mediaTools.ytdlp}
              <div class="set-row">
                <div class="set-row-l"><div class="set-row-d">yt-dlp will be downloaded automatically the first time you play a YouTube station.</div></div>
              </div>
            {/if}
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">Focus timer</h3>
            <p class="set-group-d">Pomodoro session lengths (applies app-wide).</p>
          </div>
          <div class="set-card">
            {#each pomoFields as f (f.key)}
              <div class="set-row">
                <div class="set-row-l"><div class="set-row-t">{f.label}</div></div>
                <div class="set-row-r">
                  <div style="display:flex;align-items:center;gap:8px">
                    <button class="btn btn--icon btn--sm" onclick={() => setPomo(f.key, -f.step)} aria-label="decrease {f.label}">−</button>
                    <span class="mono" style="min-width:62px;text-align:center;color:var(--fg-bright)">{pomoVal(f.key)}{f.unit}</span>
                    <button class="btn btn--icon btn--sm" onclick={() => setPomo(f.key, f.step)} aria-label="increase {f.label}">+</button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">Audio overview voices</h3>
            <p class="set-group-d">The two hosts of generated podcasts.</p>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Host A</div></div>
              <div class="set-row-r">
                <Picker
                  value={voiceA}
                  onChange={(v) => (voiceA = v)}
                  options={[{ id: "maya", label: "Maya · warm" }, { id: "nova", label: "Nova · bright" }, { id: "io", label: "Io · neutral" }]}
                />
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Host B</div></div>
              <div class="set-row-r">
                <Picker
                  value={voiceB}
                  onChange={(v) => (voiceB = v)}
                  options={[{ id: "theo", label: "Theo · calm" }, { id: "rex", label: "Rex · energetic" }, { id: "sol", label: "Sol · deep" }]}
                />
              </div>
            </div>
          </div>
        </section>
      </div>

    <!-- ===== GOOGLE CALENDAR ===== -->
    {:else if tab === "calendar"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Google Calendar</div>
          <h1 class="read set-title">Sync your calendar</h1>
          <p class="set-sub">Two-way sync with Google Calendar. The native Cortex calendar works fully without this — connecting just mirrors events both ways.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">Status</h3>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Connection</div>
                <div class="set-row-d mono faint">
                  {#if gStatus?.connected}
                    Connected{gStatus.email ? " · " + gStatus.email : ""}
                  {:else if gStatus?.configured}
                    Credentials saved — not connected yet
                  {:else}
                    Not configured
                  {/if}
                </div>
              </div>
              <div class="set-row-r">
                {#if gStatus?.connected}
                  <div class="row-inline">
                    <button class="btn" onclick={syncGoogle} disabled={gBusy}>
                      <Icon name="refresh" size={12} /> Sync now
                    </button>
                    <button class="btn btn--danger" onclick={disconnectGoogle} disabled={gBusy}>Disconnect</button>
                  </div>
                {:else}
                  <button class="btn btn--primary" onclick={connectGoogle} disabled={gBusy}>
                    <Icon name="globe" size={12} /> {gBusy ? "Connecting…" : "Connect Google"}
                  </button>
                {/if}
              </div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">Credentials</h3>
            <p class="set-group-d">Create an OAuth client of type “Desktop app” in Google Cloud → APIs &amp; Services → Credentials, enable the Calendar API, then paste the ID and secret here.</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-l"><div class="set-row-t">Client ID</div></div>
              <div class="set-row-r">
                <input class="input mono" bind:value={gClientId} onblur={saveGoogleCreds} placeholder="…apps.googleusercontent.com" />
              </div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-l"><div class="set-row-t">Client secret</div></div>
              <div class="set-row-r">
                <input class="input mono" type="password" bind:value={gClientSecret} onblur={saveGoogleCreds} placeholder="GOCSPX-…" />
              </div>
            </div>
          </div>
        </section>
      </div>

    <!-- ===== DATA & PRIVACY ===== -->
    {:else if tab === "data"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Data & privacy</div>
          <h1 class="read set-title">Local-first by default</h1>
          <p class="set-sub">Everything lives in a SQLite database on this machine. You own it.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Storage</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Database</div>
                <div class="set-row-d">~/.cortex/cortex.db</div>
              </div>
              <div class="set-row-r">
                <span class="mono faint">
                  {#if stats}
                    {fmtBytes(stats.db_bytes)} · {stats.subjects} subject{stats.subjects === 1 ? "" : "s"} · {stats.sources} source{stats.sources === 1 ? "" : "s"}
                  {:else}
                    …
                  {/if}
                </span>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Vector index</div>
                <div class="set-row-d">Local embeddings for retrieval</div>
              </div>
              <div class="set-row-r">
                <span class="mono faint">{stats ? `${stats.chunks} chunk${stats.chunks === 1 ? "" : "s"}` : "…"}</span>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Offline mode</div>
                <div class="set-row-d">Block all network calls; Ollama only.</div>
              </div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (offlineMode ? " on" : "")} onclick={toggleOffline} role="switch" aria-checked={offlineMode} aria-label="offline"><span class="st-knob"></span></button>
              </div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Manage</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Export everything</div>
                <div class="set-row-d">Subjects, sources, cheatsheets → a portable archive.</div>
              </div>
              <div class="set-row-r">
                <button class="btn" onclick={exportData}>
                  <Icon name="external" size={12} /> Export
                </button>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Optimize storage</div>
                <div class="set-row-d">Reclaim unused disk space (VACUUM). Safe.</div>
              </div>
              <div class="set-row-r">
                <button class="btn" onclick={clearCaches}>Optimize</button>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Delete all data</div>
                <div class="set-row-d">Irreversible. Wipes the local database.</div>
              </div>
              <div class="set-row-r">
                <button class="btn btn--danger" onclick={deleteEverything}>Delete…</button>
              </div>
            </div>
          </div>
        </section>
      </div>

    <!-- ===== ABOUT ===== -->
    {:else if tab === "about"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">About</div>
          <h1 class="read set-title">Cortex</h1>
          <p class="set-sub">A desktop study OS for serious students.</p>
        </header>

        <div class="set-card">
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">Version</div></div>
            <div class="set-row-r"><span class="mono faint">0.1.0 · build 2026.06.02</span></div>
          </div>
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">Engine</div></div>
            <div class="set-row-r"><span class="mono faint">Rust · Tauri · Svelte</span></div>
          </div>
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">Theme source</div></div>
            <div class="set-row-r">
              <span class="mono faint">
                Omarchy · {THEME_LABELS[app.theme]}
              </span>
            </div>
          </div>
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">License</div></div>
            <div class="set-row-r"><span class="mono faint">Source-available · BYOK</span></div>
          </div>
        </div>

        <div class="set-note mono">
          <Icon name="diamond" size={11} color="var(--accent)" />
          Offline-first. Your notes never leave this machine unless you choose a cloud model.
        </div>
      </div>
    {/if}

  </div>
</div>
