<script lang="ts">
  import { app, THEMES, THEME_LABELS } from "../lib/store.svelte";
  import type { Theme } from "../lib/store.svelte";
  import { isMobile } from "../lib/platform";
  import * as api from "../lib/api";
  import type { Memory } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import ModelSearch from "../components/ModelSearch.svelte";
  import { loadOpenRouterModels, type OrModel } from "../lib/openrouter";
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
    { id: "experimental", label: "Experimental", icon: "bolt" },
    { id: "audio",      label: "Audio",         icon: "music" },
    { id: "data",       label: "Data & privacy",icon: "doc" },
    { id: "about",      label: "About",         icon: "diamond" },
  ] as const;

  // Mobile is a homelab-first portable view: drop the desktop-only Keybinds tab
  // and lead with Integrations (homelab). Desktop order/contents are unchanged.
  const MOBILE_TABS = ["homelab", "keys", "models", "appearance", "calendar", "experimental", "audio", "data", "profile", "about"];
  const navTabs = isMobile
    ? MOBILE_TABS.map((id) => TABS.find((t) => t.id === id)).filter((t): t is (typeof TABS)[number] => !!t)
    : TABS;

  let tab = $state<string>("profile");
  // Honour a deep-link from elsewhere (e.g. the subject Overview → Moodle setup).
  $effect(() => {
    if (app.settingsTab) {
      tab = app.settingsTab;
      app.settingsTab = null;
    }
  });

  // This view owns the global keys while open
  $effect(() => {
    (window as any).__cortexViewKeys = true;
    return () => { (window as any).__cortexViewKeys = false; };
  });

  // ---- providers ----
  const PROVIDERS = [
    { id: "gemini",     label: "Gemini",              models: ["gemini-2.5-flash","gemini-2.5-pro","gemini-2.0-flash-001"] },
    { id: "openrouter", label: "OpenRouter",          models: [
      "stepfun/step-3.7-flash",
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
  // Defaults: Step 3.7 Flash (via OpenRouter) for text generation — cheaper than
  // Gemini 2.5 Flash ($0.20/$1.15 vs ~$0.30/$2.50 per Mtok), 256K context, with
  // strong agentic/structured-output quality. Falls back to any configured key
  // if OpenRouter isn't set (see llm::from_spec_or_any). Embeddings stay on Gemini.
  let assign = $state<Record<TaskId, TaskAssign>>({
    chat:       { provider: "openrouter", model: "stepfun/step-3.7-flash", budget: "8000" },
    cheatsheet: { provider: "openrouter", model: "stepfun/step-3.7-flash", budget: "32000" },
    audio:      { provider: "openrouter", model: "stepfun/step-3.7-flash", budget: "16000" },
    quiz:       { provider: "openrouter", model: "stepfun/step-3.7-flash", budget: "8000" },
    flashcard:  { provider: "openrouter", model: "stepfun/step-3.7-flash", budget: "6000" },
    embedding:  { provider: "gemini",     model: "text-embedding-004",     budget: "—" },
  });

  function setTask(id: TaskId, patch: Partial<TaskAssign>) {
    assign = { ...assign, [id]: { ...assign[id], ...patch } };
  }

  // ---- keys state ----
  let keys = $state({ openrouter: "", gemini: "", claude: "", openai: "", custom: "" });
  // Live OpenRouter catalog for the searchable model picker (the curated list is the
  // offline fallback). The /models endpoint is public + CORS-open, fetched once.
  let orModels = $state<OrModel[]>([]);
  let orLoading = $state(false);
  let orLoaded = false;
  async function ensureOrModels() {
    if (orLoaded || orLoading) return;
    orLoading = true;
    try { orModels = await loadOpenRouterModels(); orLoaded = true; }
    catch { /* keep the curated list */ }
    finally { orLoading = false; }
  }
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
  let readFont      = $state("mono");
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
        const action = listening;
        const ok = keybinds.set(action, k); // persists + applies live
        if (ok) {
          app.pushToast({
            kind: "success",
            title: "Keybind updated",
            body: `${ACTION_LABELS[action]} → “${k === " " ? "Space" : k}”. Takes effect outside this screen.`,
          });
        } else {
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

  // ---- unified homelab access ----
  // One base URL fronts every service (search / whisper / ollama / sync) behind
  // a reverse proxy; Cortex appends each service's path. Tailscale/public bases
  // are the auto local→Tailscale→public fallbacks.
  let hlBase      = $state("");
  let hlTailscale = $state("");
  let hlPublic    = $state("");
  let hlState     = $state<"idle" | "testing" | "ok" | "fail">("idle");
  let hlSaveTimer: ReturnType<typeof setTimeout> | undefined;
  function saveHomelabBases() {
    clearTimeout(hlSaveTimer);
    api.setSettings({
      homelab_base: hlBase.trim(),
      homelab_tailscale_base: hlTailscale.trim(),
      homelab_public_base: hlPublic.trim(),
    }).catch(() => {});
  }
  // iOS WKWebView fires change/blur unreliably (and "Sync now" lives in another
  // section), so the base could stay unpersisted → sync reports "target not set".
  // Persist shortly after each keystroke so the URL is always in the DB by sync time.
  function saveHomelabBasesSoon() {
    clearTimeout(hlSaveTimer);
    hlSaveTimer = setTimeout(saveHomelabBases, 400);
  }
  async function testHomelab() {
    if (!hlBase.trim()) return;
    hlState = "testing";
    hlState = await testEndpoint(hlBase);
    if (hlState === "fail") {
      app.pushToast({ kind: "error", title: "Homelab unreachable", body: lastTestError || "Couldn't reach the base URL." });
    }
  }

  // ---- live homelab sync (smart per-record merge + binary file sync) ----
  // Three endpoint tiers tried in order (auto): local LAN → Tailscale → public.
  let syncUrl   = $state(""); // local / LAN
  let syncUrlTs = $state(""); // tailscale
  let syncUrlPub = $state(""); // public
  let syncMode  = $state<"auto" | "local" | "tailscale" | "public">("auto");
  let syncUser  = $state("");
  let syncPass  = $state("");
  let syncOn    = $state(false);
  let syncTestState = $state<null | "testing" | "ok" | "fail">(null);
  // Per-endpoint reachability for the Test-all button.
  let syncReach = $state<Record<string, boolean>>({});

  const anySyncUrl = $derived(!!(syncUrl.trim() || syncUrlTs.trim() || syncUrlPub.trim()));
  // On mobile the per-tier sync URL fields are hidden — live sync rides the single
  // Homelab base URL (→ its /sync WebDAV). So the base alone must be enough to enable
  // the toggle; gating only on anySyncUrl left mobile stuck on "live sync off".
  const canSync = $derived(anySyncUrl || !!hlBase.trim());

  let syncSaveTimer: ReturnType<typeof setTimeout> | undefined;
  function saveSync() {
    clearTimeout(syncSaveTimer);
    api.setSettings({
      sync_url: syncUrl.trim(),
      sync_url_tailscale: syncUrlTs.trim(),
      sync_url_public: syncUrlPub.trim(),
      sync_mode: syncMode,
      sync_user: syncUser.trim(),
      sync_pass: syncPass,
      sync_enabled: syncOn ? "true" : "false",
    }).then(() => app.loadSyncStatus()).catch(() => {});
  }
  // Same iOS blur/change unreliability as the homelab bases — persist creds as typed.
  function saveSyncSoon() {
    clearTimeout(syncSaveTimer);
    syncSaveTimer = setTimeout(saveSync, 400);
  }
  function toggleSync() {
    syncOn = !syncOn;
    if (syncOn && !canSync) { syncOn = false; return; }
    saveSync();
  }
  function setSyncMode(m: "auto" | "local" | "tailscale" | "public") {
    syncMode = m;
    saveSync();
  }
  async function testSync() {
    if (!anySyncUrl) return;
    syncTestState = "testing";
    const reach: Record<string, boolean> = {};
    const tiers: [string, string][] = [["local", syncUrl], ["tailscale", syncUrlTs], ["public", syncUrlPub]];
    for (const [tier, url] of tiers) {
      if (!url.trim()) continue;
      try { reach[tier] = await api.syncTest(url.trim(), syncUser.trim(), syncPass); }
      catch { reach[tier] = false; }
    }
    syncReach = reach;
    try {
      syncTestState = Object.values(reach).some(Boolean) ? "ok" : "fail";
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

  // ---- Experimental features ----
  let expMoodle = $state(false);
  function toggleExpMoodle() {
    expMoodle = !expMoodle;
    api.setSetting("exp_moodle", expMoodle ? "true" : "false").catch(() => {});
  }

  // ---- Moodle (SUNLearn) integration ----
  let mdUrl   = $state("https://learn.sun.ac.za");
  let mdUser  = $state("");
  let mdPass  = $state("");
  let mdToken = $state("");
  let mdAuthMode = $state<"password" | "token">("token"); // SU uses Microsoft SSO → token paste
  let mdBusy  = $state(false);
  let mdStatus = $state<api.MoodleStatus>({ configured: false, user_id: 0, last_sync: 0 });
  let mdData = $state<api.MoodleData>({ courses: [], grades: [], deadlines: [], announcements: [] });
  let mdSummary = $state<api.MoodleSummary | null>(null);
  // Upcoming deadlines (future, soonest first) for the synced-data preview.
  const mdUpcoming = $derived(
    mdData.deadlines
      .filter((d) => d.due_at * 1000 >= Date.now() - 12 * 3600 * 1000)
      .sort((a, b) => a.due_at - b.due_at)
  );

  async function loadMoodle() {
    try {
      mdStatus = await api.moodleStatus();
      if (mdStatus.configured) mdData = await api.moodleData();
    } catch { /* not configured yet */ }
  }
  async function mdConnect() {
    mdBusy = true;
    try {
      const name = mdAuthMode === "password"
        ? await api.moodleConnect(mdUrl, mdUser.trim(), mdPass)
        : await api.moodleSetToken(mdUrl, mdToken);
      mdPass = ""; // never keep the password around
      app.pushToast({ kind: "success", title: "Moodle connected", body: name || undefined });
      await loadMoodle();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Moodle connect failed", body: String(e) });
    } finally { mdBusy = false; }
  }
  async function mdSyncNow() {
    mdBusy = true;
    try {
      mdSummary = await api.moodleSync();
      app.pushToast({ kind: "success", title: "Moodle synced",
        body: `${mdSummary.courses} courses · ${mdSummary.grades} grades · ${mdSummary.deadlines} deadlines · ${mdSummary.announcements} announcements` });
      await loadMoodle();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Moodle sync failed", body: String(e) });
    } finally { mdBusy = false; }
  }
  async function mdAutolink() {
    try {
      const n = await api.moodleAutolink();
      app.pushToast({ kind: n ? "success" : "info", title: `Auto-linked ${n} subject${n === 1 ? "" : "s"}` });
      await app.refresh();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Auto-link failed", body: String(e) });
    }
  }
  async function mdDisconnect() {
    try {
      await api.moodleDisconnect();
      mdToken = "";
      await loadMoodle();
      app.pushToast({ kind: "info", title: "Moodle disconnected" });
    } catch { /* ignore */ }
  }
  function mdCourseName(courseId: string): string {
    return mdData.courses.find((c) => c.id === courseId)?.fullname || courseId;
  }
  async function mdLoginSso() {
    try {
      await api.moodleLoginSso(mdUrl);
    } catch (e) {
      app.pushToast({ kind: "error", title: "Could not open SSO login", body: String(e) });
    }
  }
  // SSO login happens in a separate window; react to its result here.
  $effect(() => {
    let un1: (() => void) | undefined;
    let un2: (() => void) | undefined;
    api.onMoodleSsoDone(async (name) => {
      app.pushToast({ kind: "success", title: "Moodle connected", body: name || undefined });
      await loadMoodle();
    }).then((u) => (un1 = u));
    api.onMoodleSsoError((msg) =>
      app.pushToast({ kind: "error", title: "SSO login failed", body: msg })
    ).then((u) => (un2 = u));
    return () => { un1?.(); un2?.(); };
  });

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

  // Last transport error from a failed reachability test, so the homelab card can
  // explain WHY (e.g. iOS blocking cleartext LAN, DNS, timeout) instead of "fail".
  let lastTestError = $state("");
  async function testEndpoint(url: string): Promise<"ok" | "fail"> {
    try {
      const ok = await api.pingUrl(url);
      lastTestError = ok ? "" : "Reached the server, but it didn't return a success status.";
      return ok ? "ok" : "fail";
    } catch (e) {
      lastTestError = String(e);
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
    if (tab === "homelab" && depReport === null) loadDeps();
    if (tab === "data") loadArchived();
  });

  // ---- external dependency status ----
  let depReport = $state<api.DependencyReport | null>(null);
  let depLoading = $state(false);
  async function loadDeps() {
    depLoading = true;
    try { depReport = await api.dependencyStatus(); }
    catch { depReport = null; }
    finally { depLoading = false; }
  }
  async function copyDepCmd() {
    if (!depReport?.install_command) return;
    try {
      await navigator.clipboard.writeText(depReport.install_command);
      app.pushToast({ kind: "success", title: "Copied", body: "Install command copied to clipboard." });
    } catch {
      app.pushToast({ kind: "warning", title: "Couldn't copy", body: depReport.install_command });
    }
  }
  let gCalendars = $state<api.GoogleCalendar[]>([]);
  let gCalBusy = $state(false);
  async function loadGoogle() {
    try {
      gClientId = (await api.getSetting("google_client_id")) ?? "";
      gClientSecret = (await api.getSetting("google_client_secret")) ?? "";
      gStatus = await api.googleStatus();
      if (gStatus.connected) void loadGoogleCalendars();
    } catch {
      gStatus = { connected: false, email: null, configured: false };
    }
  }
  async function loadGoogleCalendars() {
    if (!gStatus?.connected) return;
    gCalBusy = true;
    try { gCalendars = await api.googleListCalendars(); }
    catch (e) { app.pushToast({ kind: "error", title: "Couldn't list calendars", body: String(e) }); }
    finally { gCalBusy = false; }
  }
  function toggleGoogleCal(id: string) {
    gCalendars = gCalendars.map((c) => (c.id === id ? { ...c, selected: !c.selected } : c));
    const csv = gCalendars.filter((c) => c.selected).map((c) => c.id).join(",");
    api.setSettings({ google_pull_calendars: csv }).catch(() => {});
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
      void loadGoogleCalendars();
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
      await app.refresh();            // pulled events filed to subjects; colours updated
      app.notifyEventsChanged();      // refresh the calendar view
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
  let archivedSubjects = $state<api.Subject[]>([]);

  async function loadArchived() {
    archivedSubjects = await app.listArchivedSubjects();
  }
  async function restoreSubject(id: string) {
    await app.setSubjectArchived(id, false);
    await Promise.all([loadArchived(), loadStats()]);
  }

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
      if (s.homelab_base)           hlBase = s.homelab_base;
      if (s.homelab_tailscale_base) hlTailscale = s.homelab_tailscale_base;
      if (s.homelab_public_base)    hlPublic = s.homelab_public_base;
      if (s.sync_url)   syncUrl  = s.sync_url;
      if (s.sync_url_tailscale) syncUrlTs = s.sync_url_tailscale;
      if (s.sync_url_public)    syncUrlPub = s.sync_url_public;
      if (s.sync_mode === "auto" || s.sync_mode === "local" || s.sync_mode === "tailscale" || s.sync_mode === "public") syncMode = s.sync_mode;
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

      expMoodle = s.exp_moodle === "true";
      if (s.moodle_url) mdUrl = s.moodle_url;
      void loadMoodle();

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

    {#each navTabs as t}
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

    {#if isMobile}
      <div class="set-mnav">
        <Picker
          value={tab}
          onChange={(id) => (tab = id)}
          options={navTabs.map((t) => ({ id: t.id, label: t.label }))}
          icon={navTabs.find((t) => t.id === tab)?.icon}
        />
      </div>
    {/if}

    <!-- ===== PROFILE ===== -->
    {#if tab === "profile"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Profile</div>
          <h1 class="set-title">Who the AI thinks you are</h1>
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
              <div class="set-row-t">In your words</div>
              <textarea class="input set-textarea set-bio" bind:value={about} rows={6}></textarea>
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
          <h1 class="set-title">A model for every task</h1>
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
            {@const isOr = a.provider === "openrouter"}
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
              <ModelSearch
                value={a.model}
                onChange={(m) => onModelChange(t.id, m)}
                options={isOr && orModels.length ? orModels : prov.models.map((m) => ({ id: m, label: m }))}
                loading={isOr && orLoading}
                onOpen={isOr ? ensureOrModels : undefined}
                placeholder={isOr ? "Search OpenRouter…" : undefined}
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
          <h1 class="set-title">Bring your own keys</h1>
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
          <h1 class="set-title">Make it yours</h1>
          <p class="set-sub">Cortex re-skins live from your Omarchy theme, or pick one manually.</p>
        </header>

        <!-- Follow-Omarchy mirrors the desktop's Omarchy palette — meaningless on a phone. -->
        {#if !isMobile}
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
        {/if}

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
                  {#each [{ id: "mono", label: "Mono" }, { id: "serif", label: "Serif" }, { id: "sans", label: "Sans" }] as opt}
                    <button type="button" class={"seg-opt" + (readFont === opt.id ? " on" : "")} onclick={() => (readFont = opt.id)}>{opt.label}</button>
                  {/each}
                </div>
              </div>
            </div>
            <!-- Density is a desktop spacing affordance; touch uses one comfortable scale. -->
            {#if !isMobile}
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
            {/if}
          </div>
        </section>

        <!-- Window / tray is a desktop-only concept — hidden on mobile. -->
        {#if !isMobile}
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
        {/if}
      </div>

    <!-- ===== KEYBINDS ===== -->
    {:else if tab === "keybinds"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Keybinds</div>
          <h1 class="set-title">Helix-style, your way</h1>
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
                    onclick={() => { if (opt.id === "helix" || opt.id === "vim") { keybinds.applyPreset(opt.id); app.pushToast({ kind: "success", title: `${opt.label} keybinds applied`, body: "Shortcuts take effect outside this screen." }); } }}
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
          <h1 class="set-title">Local models, web search & backups</h1>
          <p class="set-sub">Optional, self-hosted services. Cortex stays fully local without any of them — or run them all with the <span class="mono">homelab/</span> docker compose.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">Homelab URL</h3>
              <p class="set-group-d">One address for everything. Run the <span class="mono">homelab/</span> docker compose and point Cortex here — search, transcription, local models and sync are all reached off this single URL (Cortex adds <span class="mono">/searxng</span>, <span class="mono">/whisper</span>, <span class="mono">/ollama</span>, <span class="mono">/sync</span> for you). Add a Tailscale and/or public address and Cortex auto-picks the first reachable: <strong>local → Tailscale → public</strong>.</p>
            </div>
            {#if hlBase.trim()}
              {@const p = connPill(hlState === "idle" ? null : hlState, true)}
              <span class="status-pill status-pill--{p.cls}"><span class="dot"></span>{p.label}</span>
            {/if}
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-t">Local URL</div>
              <div class="row-inline">
                <input class="input mono" bind:value={hlBase} oninput={saveHomelabBasesSoon} onchange={saveHomelabBases} onblur={saveHomelabBases} placeholder="http://192.168.1.10:8080" />
                <button class="btn" onclick={testHomelab} disabled={hlState === 'testing' || !hlBase.trim()}>
                  <Icon name="refresh" size={12} /> Test
                </button>
              </div>
              <div class="set-row-d">Your homelab's LAN address (the Caddy proxy port, default <span class="mono">8080</span>). Leave the per-service overrides below blank to use this.</div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">Tailscale URL <span class="faint">optional</span></div>
              <input class="input mono" bind:value={hlTailscale} oninput={saveHomelabBasesSoon} onchange={saveHomelabBases} onblur={saveHomelabBases} placeholder="https://homelab.tailnet-xxxx.ts.net" />
              <div class="set-row-d">Used when the local URL isn't reachable. Cortex swaps just the host (and port if you give one), keeping the service paths.</div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">Public URL <span class="faint">optional</span></div>
              <input class="input mono" bind:value={hlPublic} oninput={saveHomelabBasesSoon} onchange={saveHomelabBases} onblur={saveHomelabBases} placeholder="https://lab.example.com" />
            </div>
          </div>
        </section>

        <!-- Local CLI dependencies (pdftotext/libreoffice/age/…) never exist on a phone. -->
        {#if !isMobile}
        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">Dependencies</h3>
              <p class="set-group-d">External tools Cortex shells out to for ingest, OCR, transcription and media. Install whatever's missing with the one command below.</p>
            </div>
            <button class="btn btn--sm btn--ghost" onclick={loadDeps} disabled={depLoading}><Icon name="refresh" size={12} /> {depLoading ? "Checking…" : "Re-check"}</button>
          </div>
          <div class="set-card">
            {#if depReport}
              {#each depReport.deps as d (d.name)}
                <div class="dep-row">
                  <span class="dep-ico" class:on={d.present}>{d.present ? "✓" : "✗"}</span>
                  <span class="dep-name">{d.name}</span>
                  <span class="dep-detail mono">{d.detail}</span>
                </div>
              {/each}
              {#if depReport.install_command}
                <div class="set-row stacked" style="margin-top:10px">
                  <div class="set-row-t">Install missing <span class="faint">· {depReport.manager}</span></div>
                  <div class="row-inline">
                    <input class="input mono" readonly value={depReport.install_command} />
                    <button class="btn" onclick={copyDepCmd}><Icon name="doc" size={12} /> Copy</button>
                  </div>
                  <div class="set-row-d">{depReport.note}</div>
                </div>
              {:else}
                <div class="set-row-d" style="color:var(--ok); margin-top:8px">All dependencies present 🎉</div>
              {/if}
            {:else}
              <div class="set-row-d faint">{depLoading ? "Checking installed tools…" : "Couldn't check dependencies."}</div>
            {/if}
          </div>
        </section>
        {/if}

        <!-- Per-service URL overrides are a desktop power-user feature; on mobile the
             single Homelab URL drives every service (search/whisper/ollama/sync/ingest). -->
        {#if !isMobile}
        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">Per-service overrides <span class="faint">optional</span></h3>
            <p class="set-group-d">Only needed if you run a service somewhere other than the Homelab URL above (or aren't using the bundled proxy). A value here takes precedence over the unified URL for that one service.</p>
          </div>
        </section>

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
        {/if}

        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">Live sync</h3>
              <p class="set-group-d">Sync your whole vault to a homelab WebDAV target — database <em>and</em> source files. Smart per-record merge across devices: newest edit wins, rows from either device are kept, and nothing is deleted unless you actually deleted it.</p>
            </div>
            <span class="status-pill status-pill--{syncPill().cls}"><span class="dot"></span>{syncPill().label}</span>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Enable live sync</div>
                <div class="set-row-d">Merges the remote vault in on launch (in the background — never blocks startup), then pushes your changes (debounced).</div>
              </div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (syncOn ? " on" : "")} onclick={toggleSync} disabled={!canSync} role="switch" aria-checked={syncOn} aria-label="live sync"><span class="st-knob"></span></button>
              </div>
            </div>

            <!-- The 3-tier sync URL config is desktop power-user territory; on mobile
                 sync rides the single Homelab URL above (→ its /sync WebDAV). -->
            {#if !isMobile}
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Endpoint</div>
                <div class="set-row-d">Auto tries Local → Tailscale → Public and uses the first reachable — so the same device syncs on LAN, over Tailscale, or from anywhere.</div>
              </div>
              <div class="set-row-r">
                <div class="seg">
                  {#each [{ id: "auto", label: "Auto" }, { id: "local", label: "Local" }, { id: "tailscale", label: "Tailscale" }, { id: "public", label: "Public" }] as opt}
                    <button type="button" class={"seg-opt" + (syncMode === opt.id ? " on" : "")} onclick={() => setSyncMode(opt.id as typeof syncMode)}>{opt.label}</button>
                  {/each}
                </div>
              </div>
            </div>

            <div class="set-row stacked">
              <div class="set-row-t">Local / LAN URL</div>
              <input class="input mono" bind:value={syncUrl} onchange={saveSync} onblur={saveSync} placeholder="http://192.168.1.50:9010" />
              {#if syncReach.local !== undefined}<div class="set-row-d" style="color:{syncReach.local ? 'var(--ok)' : 'var(--err,#e5484d)'}">{syncReach.local ? "✓ reachable" : "✗ unreachable"}</div>{/if}
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">Tailscale URL <span class="faint">optional</span></div>
              <input class="input mono" bind:value={syncUrlTs} onchange={saveSync} onblur={saveSync} placeholder="http://homelab.tailnet-xxxx.ts.net:9010" />
              {#if syncReach.tailscale !== undefined}<div class="set-row-d" style="color:{syncReach.tailscale ? 'var(--ok)' : 'var(--err,#e5484d)'}">{syncReach.tailscale ? "✓ reachable" : "✗ unreachable"}</div>{/if}
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">Public URL <span class="faint">optional</span></div>
              <div class="row-inline">
                <input class="input mono" bind:value={syncUrlPub} onchange={saveSync} onblur={saveSync} placeholder="https://cortex.example.com" />
                <button class="btn" onclick={testSync} disabled={syncTestState === "testing" || !anySyncUrl}>
                  <Icon name="refresh" size={12} /> Test all
                </button>
              </div>
              {#if syncReach.public !== undefined}<div class="set-row-d" style="color:{syncReach.public ? 'var(--ok)' : 'var(--err,#e5484d)'}">{syncReach.public ? "✓ reachable" : "✗ unreachable"}</div>{/if}
              {#if syncTestState === "fail"}
                <div class="set-row-d" style="color:var(--err,#e5484d)">No endpoint reachable — check the URLs and credentials.</div>
              {:else if syncTestState === "ok"}
                <div class="set-row-d" style="color:var(--ok)">At least one endpoint reachable.</div>
              {/if}
            </div>
            {/if}
            {#if isMobile}
              <div class="set-row stacked"><div class="set-row-d">Syncs via the <strong>Homelab URL</strong> above (its <span class="mono">/sync</span> WebDAV). Enter the username/password your homelab expects, then tap <strong>Sync now</strong>.</div></div>
            {/if}
            <div class="set-row stacked">
              <div class="set-row-t">Username <span class="faint">optional</span></div>
              <input class="input mono" bind:value={syncUser} oninput={saveSyncSoon} onchange={saveSync} onblur={saveSync} placeholder="cortex" />
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">Password <span class="faint">optional</span></div>
              <div class="row-inline">
                <input class="input mono" type="password" bind:value={syncPass} oninput={saveSyncSoon} onchange={saveSync} onblur={saveSync} placeholder="••••••••" />
                <button class="btn btn--primary" disabled={app.syncState === "syncing" || !(anySyncUrl || hlBase.trim())} onclick={() => app.syncManual()}>
                  <Icon name="upload" size={12} /> {app.syncState === "syncing" ? "Syncing…" : "Sync now"}
                </button>
              </div>
              <div class="set-row-d">Last synced: <span class="mono">{fmtSyncTime(app.syncLastAt)}</span>. Files live under <span class="mono">files/</span> on the target; the DB merges by record. Bring up a WebDAV target with the <span class="mono">homelab/</span> compose.</div>
            </div>
          </div>
        </section>

        <!-- Encrypted backups use the age + rclone sidecars — desktop-only. -->
        {#if !isMobile}
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
        {/if}

      </div>

    <!-- ===== AUDIO ===== -->
    {:else if tab === "audio"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Audio</div>
          <h1 class="set-title">Study sound & voices</h1>
          <p class="set-sub">Defaults for the music player and generated audio overviews.</p>
        </header>

        <!-- Music is cut on mobile (no mpv/yt-dlp sidecars) — hide its settings. -->
        {#if !isMobile}
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
        {/if}

        <!-- YouTube streaming needs the mpv + yt-dlp sidecars, which don't ship on
             mobile — hide the whole section there. -->
        {#if !isMobile}
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
        {/if}

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
          <h1 class="set-title">Sync your calendar</h1>
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

        {#if gStatus?.connected}
          <section class="set-group">
            <div class="set-group-h">
              <h3 class="set-group-t">Calendars to sync</h3>
              <p class="set-group-d">Pick which Google calendars to pull events from — tick your <strong>university / timetable</strong> calendar here so its classes, deadlines and exams land on the Cortex calendar.</p>
            </div>
            <div class="set-card">
              {#if gCalBusy && gCalendars.length === 0}
                <div class="set-row"><div class="set-row-d faint">Loading calendars…</div></div>
              {:else if gCalendars.length === 0}
                <div class="set-row">
                  <div class="set-row-l"><div class="set-row-d faint">No calendars found.</div></div>
                  <div class="set-row-r"><button class="btn btn--sm" onclick={loadGoogleCalendars}>Reload</button></div>
                </div>
              {:else}
                {#each gCalendars as cal (cal.id)}
                  <label class="gcal-row" class:on={cal.selected}>
                    <input type="checkbox" checked={cal.selected} onchange={() => toggleGoogleCal(cal.id)} />
                    <span class="gcal-swatch" style:background={cal.color || "var(--border-strong)"}></span>
                    <span class="gcal-name">{cal.summary}</span>
                    {#if cal.primary}<span class="gcal-tag mono">primary</span>{/if}
                    {#if cal.selected}<span class="gcal-on mono">syncing</span>{/if}
                  </label>
                {/each}
              {/if}
            </div>
          </section>
        {/if}

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

    <!-- ===== EXPERIMENTAL ===== -->
    {:else if tab === "experimental"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Settings</div>
          <h2 class="set-title">Experimental</h2>
          <p class="set-sub">Early features that may be rough or change. Toggle one on to try it.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">University portal (Moodle / SUNLearn)</h3>
              <p class="set-group-d">Pull grades, assignments, deadlines and announcements from your Moodle portal (Stellenbosch SUNLearn = <span class="mono">learn.sun.ac.za</span>) into Cortex.</p>
            </div>
            <button class={"st-toggle" + (expMoodle ? " on" : "")} type="button" onclick={toggleExpMoodle} role="switch" aria-checked={expMoodle} aria-label="enable moodle"><span class="st-knob"></span></button>
          </div>

          {#if expMoodle}
            <div class="set-card">
              <div class="set-row">
                <div class="set-row-l">
                  <div class="set-row-t">Connection</div>
                  <div class="set-row-d">{mdStatus.configured ? `Connected · last sync ${fmtSyncTime(mdStatus.last_sync)}` : "Not connected"}</div>
                </div>
                <span class="status-pill status-pill--{mdStatus.configured ? 'ready' : 'pending'}"><span class="dot"></span>{mdStatus.configured ? "Connected" : "Off"}</span>
              </div>

              <div class="set-row stacked">
                <div class="set-row-t">Moodle site URL</div>
                <input class="input mono" bind:value={mdUrl} placeholder="https://learn.sun.ac.za" />
              </div>

              <div class="set-row">
                <div class="set-row-l">
                  <div class="set-row-t">Sign-in method</div>
                  <div class="set-row-d">Stellenbosch uses Microsoft SSO, so username/password usually won't work — paste a web-services token instead.</div>
                </div>
                <div class="set-row-r" style="gap:6px">
                  <button class={"btn btn--sm" + (mdAuthMode === 'token' ? ' btn--primary' : ' btn--ghost')} type="button" onclick={() => (mdAuthMode = 'token')}>Token</button>
                  <button class={"btn btn--sm" + (mdAuthMode === 'password' ? ' btn--primary' : ' btn--ghost')} type="button" onclick={() => (mdAuthMode = 'password')}>Password</button>
                </div>
              </div>

              {#if mdAuthMode === 'password'}
                <div class="set-row stacked">
                  <div class="set-row-t">Username</div>
                  <input class="input mono" bind:value={mdUser} placeholder="student number" />
                </div>
                <div class="set-row stacked">
                  <div class="set-row-t">Password</div>
                  <input class="input mono" type="password" bind:value={mdPass} placeholder="••••••••" />
                </div>
              {:else}
                <div class="set-row stacked">
                  <div class="set-row-t">Web-services token</div>
                  <input class="input mono" bind:value={mdToken} placeholder="paste your Moodle token" />
                  <div class="set-row-d">Obtain it from the official SUNLearn / Moodle app or a browser login. Stored locally; your password is never sent to Cortex.</div>
                </div>
              {/if}

              <div class="set-row">
                <div class="set-row-l"><div class="set-row-d">Connect, then sync to pull your data.</div></div>
                <div class="set-row-r" style="gap:8px">
                  {#if mdStatus.configured}
                    <button class="btn btn--ghost btn--sm" type="button" onclick={mdDisconnect} disabled={mdBusy}>Disconnect</button>
                    <button class="btn btn--primary btn--sm" type="button" onclick={mdSyncNow} disabled={mdBusy}><Icon name="refresh" size={12} /> {mdBusy ? "Syncing…" : "Sync now"}</button>
                  {:else}
                    <button class="btn btn--sm" type="button" onclick={mdLoginSso}>Sign in via browser (SSO)</button>
                    <button class="btn btn--primary btn--sm" type="button" onclick={mdConnect} disabled={mdBusy || (mdAuthMode==='token' ? !mdToken.trim() : !mdUser.trim())}>{mdBusy ? "Connecting…" : "Connect"}</button>
                  {/if}
                </div>
              </div>

              {#if mdStatus.configured}
                <div class="set-row">
                  <div class="set-row-l">
                    <div class="set-row-t">Link subjects to courses</div>
                    <div class="set-row-d">Auto-match your Cortex subjects to Moodle courses by code/name.</div>
                  </div>
                  <div class="set-row-r"><button class="btn btn--sm" type="button" onclick={mdAutolink}>Auto-link</button></div>
                </div>
              {/if}

              {#if mdData.courses.length}
                <div class="set-row stacked">
                  <div class="set-row-t">Synced data</div>
                  <div class="md-stats">
                    <span class="md-stat"><b>{mdData.courses.length}</b> courses</span>
                    <span class="md-stat"><b>{mdData.grades.length}</b> grades</span>
                    <span class="md-stat"><b>{mdData.deadlines.length}</b> deadlines</span>
                    <span class="md-stat"><b>{mdData.announcements.length}</b> announcements</span>
                  </div>
                  {#if mdUpcoming.length}
                    <div class="md-up-h mono">Upcoming deadlines</div>
                    <ul class="md-up">
                      {#each mdUpcoming.slice(0, 6) as d (d.id)}
                        <li>
                          <span class="md-up-date mono">{new Date(d.due_at * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short" })}</span>
                          {#if d.url}
                            <button class="md-up-name md-up-link" onclick={() => d.url && api.openExternal(d.url)} title={d.name}>{d.name}</button>
                          {:else}
                            <span class="md-up-name" title={d.name}>{d.name}</span>
                          {/if}
                          {#if d.course_id}<span class="md-up-course" title={mdCourseName(d.course_id)}>{mdCourseName(d.course_id)}</span>{/if}
                        </li>
                      {/each}
                    </ul>
                  {/if}
                </div>
              {/if}
            </div>
          {/if}
        </section>
      </div>

    <!-- ===== DATA & PRIVACY ===== -->
    {:else if tab === "data"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Data & privacy</div>
          <h1 class="set-title">Local-first by default</h1>
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

        {#if archivedSubjects.length}
          <section class="set-group">
            <div class="set-group-h">
              <h3 class="set-group-t">Archived subjects</h3>
              <p class="set-group-d">Hidden from the app, kept for storage. Restore any time.</p>
            </div>
            <div class="set-card">
              {#each archivedSubjects as s (s.id)}
                <div class="set-row">
                  <div class="set-row-l">
                    <div class="set-row-t">
                      <span style="color:{app.subjectColor(s)}">{s.glyph}</span> {s.name}
                    </div>
                    <div class="set-row-d">
                      {s.code ? s.code + " · " : ""}{s.sourceCount} source{s.sourceCount === 1 ? "" : "s"}
                    </div>
                  </div>
                  <div class="set-row-r">
                    <button class="btn" onclick={() => restoreSubject(s.id)}>Restore</button>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}

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
          <h1 class="set-title">Cortex</h1>
          <p class="set-sub">A desktop study OS for serious students.</p>
        </header>

        <div class="set-card">
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">Version</div></div>
            <div class="set-row-r"><span class="mono faint">1.0.7</span></div>
          </div>
          <div class="set-row">
            <div class="set-row-l">
              <div class="set-row-t">Updates</div>
              <div class="set-row-d">Check GitHub for a newer release and install it.</div>
            </div>
            <div class="set-row-r">
              <button class="btn" onclick={() => app.checkForUpdates()} disabled={app.updateChecking}>
                <Icon name="refresh" size={12} /> {app.updateChecking ? "Checking…" : "Check for updates"}
              </button>
            </div>
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
