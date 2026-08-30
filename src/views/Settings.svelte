<script lang="ts">
  import { app, THEMES, THEME_LABELS } from "../lib/store.svelte";
  import { t, i18n, LOCALES, setLocale } from "../lib/i18n.svelte";
  import type { Theme } from "../lib/store.svelte";
  import { isMobile } from "../lib/platform";
  import * as api from "../lib/api";
  import { getVersion } from "@tauri-apps/api/app";
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
    { keys: "Ctrl F", label: t("Find on page") },
    { keys: "Ctrl P", label: t("Command palette") },
    { keys: "Esc",    label: t("Close overlay / go back") },
    { keys: "g d",    label: t("Go to dashboard") },
    { keys: "Alt 1–9", label: t("Jump to subject N") },
  ];

  // ---- tab navigation ----
  const TABS = [
    { id: "profile",    label: t("Profile"),       icon: "diamond" },
    { id: "models",     label: t("Models"),        icon: "bolt" },
    { id: "keys",       label: t("API keys"),      icon: "lock" },
    { id: "appearance", label: t("Appearance"),    icon: "grid" },
    { id: "keybinds",   label: t("Keybinds"),      icon: "cmd" },
    { id: "homelab",    label: t("Integrations"),  icon: "globe" },
    { id: "calendar",   label: t("Google Calendar"), icon: "globe" },
    { id: "experimental", label: t("Experimental"), icon: "bolt" },
    { id: "audio",      label: t("Audio"),         icon: "music" },
    { id: "data",       label: t("Data & privacy"),icon: "doc" },
    { id: "about",      label: t("About"),         icon: "diamond" },
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
  // Model catalog. Each provider's models are ordered by MY cost-to-quality read —
  // best value (cheap + capable) first, then premium/frontier, then reasoning — and
  // labelled with a human name + a one-word tier hint so the dropdown is scannable.
  // `custom` (OpenRouter / OpenAI-compatible) lets you type any slug not listed.
  type Model = { id: string; label: string };
  const PROVIDERS: { id: string; label: string; models: Model[] }[] = [
    { id: "gemini", label: "Gemini", models: [
      { id: "gemini-2.5-flash",      label: "Gemini 2.5 Flash — ⚡ best value" },
      { id: "gemini-2.5-flash-lite", label: "Gemini 2.5 Flash-Lite — cheapest" },
      { id: "gemini-2.0-flash-001",  label: "Gemini 2.0 Flash — legacy, cheap" },
      { id: "gemini-2.5-pro",        label: "Gemini 2.5 Pro — ★ premium" },
    ] },
    { id: "openrouter", label: "OpenRouter", models: [
      // Curated shortlist + ordering (the live catalog with per-row pricing replaces
      // these labels once it loads). Order ≈ best value → all-rounders → premium →
      // reasoning; the editorial "cheap/fast" tags are dropped since the searchable
      // list shows price-per-token + context per row.
      { id: "deepseek/deepseek-v4-flash",         label: "DeepSeek V4 Flash" },
      { id: "google/gemini-2.5-flash",            label: "Gemini 2.5 Flash" },
      { id: "google/gemini-2.5-flash-lite",       label: "Gemini 2.5 Flash-Lite" },
      { id: "openai/gpt-5-mini",                  label: "GPT-5 mini" },
      { id: "openai/gpt-4o-mini",                 label: "GPT-4o mini" },
      { id: "anthropic/claude-3.5-haiku",         label: "Claude 3.5 Haiku" },
      { id: "stepfun/step-3.7-flash",             label: "Step 3.7 Flash" },
      { id: "deepseek/deepseek-chat",             label: "DeepSeek V3" },
      { id: "meta-llama/llama-3.3-70b-instruct",  label: "Llama 3.3 70B" },
      { id: "qwen/qwen-2.5-72b-instruct",         label: "Qwen 2.5 72B" },
      { id: "mistralai/mistral-large",            label: "Mistral Large" },
      { id: "x-ai/grok-2-1212",                   label: "Grok 2" },
      { id: "google/gemini-2.0-flash-001",        label: "Gemini 2.0 Flash" },
      { id: "openai/gpt-4o",                      label: "GPT-4o" },
      { id: "anthropic/claude-3.5-sonnet",        label: "Claude 3.5 Sonnet" },
      { id: "anthropic/claude-3.7-sonnet",        label: "Claude 3.7 Sonnet" },
      { id: "google/gemini-2.5-pro",              label: "Gemini 2.5 Pro" },
      { id: "anthropic/claude-sonnet-4.5",        label: "Claude Sonnet 4.5" },
      { id: "openai/gpt-5",                       label: "GPT-5" },
      { id: "anthropic/claude-opus-4.1",          label: "Claude Opus 4.1" },
      { id: "x-ai/grok-3",                        label: "Grok 3" },
      { id: "openai/o3-mini",                     label: "o3-mini" },
      { id: "openai/o3",                          label: "o3" },
      { id: "deepseek/deepseek-r1",               label: "DeepSeek R1" },
    ] },
    { id: "openai", label: "OpenAI", models: [
      { id: "gpt-4o-mini", label: "GPT-4o mini — cheap" },
      { id: "gpt-5-mini",  label: "GPT-5 mini — cheap + smart" },
      { id: "gpt-4o",      label: "GPT-4o — balanced" },
      { id: "gpt-5",       label: "GPT-5 — ★ frontier" },
      { id: "o3-mini",     label: "o3-mini — cheap reasoning" },
      { id: "o3",          label: "o3 — deep reasoning" },
    ] },
    { id: "claude", label: "Claude", models: [
      { id: "claude-haiku-4-5-20251001",  label: "Claude Haiku 4.5 — fast + cheap" },
      { id: "claude-3-5-haiku-20241022",  label: "Claude 3.5 Haiku — cheap" },
      { id: "claude-3-5-sonnet-20241022", label: "Claude 3.5 Sonnet — strong" },
      { id: "claude-3-7-sonnet-20250219", label: "Claude 3.7 Sonnet — strong" },
      { id: "claude-sonnet-4-6",          label: "Claude Sonnet 4.6 — ★ premium" },
      { id: "claude-opus-4-8",            label: "Claude Opus 4.8 — top, pricey" },
    ] },
    { id: "ollama", label: "Ollama (local)", models: [
      { id: "mistral-small", label: "Mistral Small — light, local" },
      { id: "qwen2.5:32b",   label: "Qwen 2.5 32B — local" },
      { id: "llama3.3:70b",  label: "Llama 3.3 70B — local, heavy" },
    ] },
    { id: "custom", label: "Custom endpoint", models: [] },
  ];
  const EMBED_PROVIDERS: { id: string; label: string; models: Model[] }[] = [
    { id: "gemini",  label: "Gemini",        models: [{ id: "text-embedding-004", label: "text-embedding-004" }] },
    { id: "openai",  label: "OpenAI",        models: [{ id: "text-embedding-3-small", label: "text-embedding-3-small — cheap" }, { id: "text-embedding-3-large", label: "text-embedding-3-large — best" }] },
    { id: "ollama",  label: "Ollama (local)", models: [{ id: "nomic-embed-text", label: "nomic-embed-text — local" }, { id: "mxbai-embed-large", label: "mxbai-embed-large — local" }] },
  ];
  const MODEL_TASKS = [
    { id: "chat",       label: t("Chat"),                  desc: t("Scoped Q&A across sources") },
    { id: "cheatsheet", label: t("Cheatsheet synthesis"),  desc: t("Completeness-checked merges") },
    { id: "audio",      label: t("Audio overview script"), desc: t("Two-host podcast dialogue") },
    { id: "quiz",       label: t("Quiz generation"),       desc: t("MCQ · short answer · cloze") },
    { id: "flashcard",  label: t("Flashcard generation"),  desc: t("Q/A pairs + SRS scheduling") },
    { id: "embedding",  label: t("Embedding"),             desc: t("Vector index for retrieval") },
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
      app.pushToast({ kind: "error", title: t("Could not save memory") });
    } finally {
      memoryBusy = false;
    }
  }
  async function removeMemory(id: string) {
    try {
      await api.deleteMemory(id);
      await loadMemory();
    } catch {
      app.pushToast({ kind: "error", title: t("Could not delete memory") });
    }
  }

  // ---- models state ----
  type TaskAssign = { provider: string; model: string; budget: string };
  // Defaults: DeepSeek V4 Flash (via OpenRouter) for ALL text generation —
  // extremely cheap ($0.09/$0.18 per Mtok), 1M context, fast and non-reasoning.
  // Falls back to any configured key if OpenRouter isn't set (see
  // llm::from_spec_or_any). Embeddings stay on Gemini (DeepSeek doesn't embed).
  let assign = $state<Record<TaskId, TaskAssign>>({
    chat:       { provider: "openrouter", model: "deepseek/deepseek-v4-flash", budget: "8000" },
    cheatsheet: { provider: "openrouter", model: "deepseek/deepseek-v4-flash", budget: "32000" },
    audio:      { provider: "openrouter", model: "deepseek/deepseek-v4-flash", budget: "16000" },
    quiz:       { provider: "openrouter", model: "deepseek/deepseek-v4-flash", budget: "8000" },
    flashcard:  { provider: "openrouter", model: "deepseek/deepseek-v4-flash", budget: "6000" },
    embedding:  { provider: "gemini",     model: "text-embedding-004",        budget: "—" },
  });

  function setTask(id: TaskId, patch: Partial<TaskAssign>) {
    assign = { ...assign, [id]: { ...assign[id], ...patch } };
  }

  // ---- keys state ----
  let keys = $state({
    openrouter: "",
    gemini: "",
    claude: "",
    openai: "",
    custom_endpoint: "",
    custom_api_key: "",
  });
  const keyMeta = [
    { id: "openrouter", label: "OpenRouter",              note: "openrouter.ai/keys",    placeholder: "sk-or-…" },
    { id: "gemini",     label: "Gemini",                  note: "Google AI Studio",       placeholder: "AIza…" },
    { id: "claude",     label: "Claude",                  note: "console.anthropic.com",  placeholder: "sk-ant-…" },
    { id: "openai",     label: "OpenAI",                  note: "platform.openai.com",    placeholder: "sk-…" },
    { id: "custom_endpoint", label: t("Custom endpoint URL"), note: t("OpenAI-compatible base URL"), placeholder: "https://…/v1" },
    { id: "custom_api_key", label: t("Custom endpoint API key"), note: t("Bearer token for the custom endpoint"), placeholder: "sk-…" },
  ] as const;
  // show/hide per key
  let showKey = $state<Record<string, boolean>>({
    openrouter: false,
    gemini: false,
    claude: false,
    openai: false,
    custom_endpoint: false,
    custom_api_key: false,
  });

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

  // Settings is the ONLY view that mutates the <html> root. Writing an attribute on
  // documentElement invalidates styles for the ENTIRE document, forcing WebKit
  // (WebKitGTK / iOS WKWebView) to re-match every element against the large global
  // stylesheet (~1740 rules) — ~2× slower than Blink, which is why only Settings
  // freezes/lags on WebKit. The store already applies these on boot, so these were
  // redundant no-op writes that still triggered a full-document recalc every open.
  // Only write when the value actually changes (a getAttribute read is cheap and
  // does NOT force layout); live appearance changes still apply instantly.
  $effect(() => {
    const el = document.documentElement;
    if (el.getAttribute("data-read") !== readFont) el.setAttribute("data-read", readFont);
  });
  $effect(() => {
    const el = document.documentElement;
    const v = density === "compact" ? "compact" : "regular";
    if (el.getAttribute("data-density") !== v) el.setAttribute("data-density", v);
  });

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
            title: t("Keybind updated"),
            body: t("{action} → “{key}”. Takes effect outside this screen.", { action: ACTION_LABELS[action], key: k === " " ? "Space" : k }),
          });
        } else {
          app.pushToast({
            kind: "warning",
            title: t("Key already in use"),
            body: t("“{key}” is bound to another action — pick a different key.", { key: k === " " ? "Space" : k }),
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
  let whisperModel = $state("");
  let webImages  = $state(false);
  let testState  = $state<null | "testing" | "ok" | "fail">(null);
  let searxState = $state<null | "testing" | "ok" | "fail">(null);
  let whisperState = $state<null | "testing" | "ok" | "fail">(null);
  // ---- OCR (external vision endpoint) ----
  let ocrEndpoint = $state("");
  let ocrKey      = $state("");
  let ocrModel    = $state("");
  async function saveOcr() {
    await api.setSettings({
      ocr_endpoint: ocrEndpoint.trim(),
      ocr_api_key:  ocrKey.trim(),
      ocr_model:    ocrModel.trim(),
    });
  }
  async function verifyOcr() {
    try {
      await saveOcr();
      verify = { ...verify, ocr: "checking" };
      verify = { ...verify, ocr: await api.verifyProvider("ocr") };
    } catch (e) {
      verify = { ...verify, ocr: { ok: false, detail: String(e) } };
    }
  }

  function saveWhisper() {
    api.setSetting("whisper_url", whisperUrl.trim()).catch(() => {});
  }
  function saveWhisperModel() {
    api.setSetting("whisper_model", whisperModel.trim()).catch(() => {});
  }
  async function testWhisper() {
    if (!whisperUrl.trim()) return;
    whisperState = "testing";
    whisperState = await testEndpoint(whisperUrl);
  }
  // End-to-end whisper validation: server reachable AND the configured model
  // installed — downloading it server-side on the spot when missing, so the
  // first real lecture never pays the multi-minute cold pull.
  let whisperCheckState = $state<null | "checking" | "ok" | "fail">(null);
  let whisperCheckNote = $state("");
  async function checkWhisper() {
    whisperCheckState = "checking";
    whisperCheckNote = t("Checking the server (a first-time model download can take a few minutes)…");
    try {
      whisperCheckNote = await api.checkWhisperModel();
      whisperCheckState = "ok";
    } catch (e) {
      whisperCheckNote = String(e);
      whisperCheckState = "fail";
    }
  }

  // ---- transcription mode (Settings → Integrations → Transcription) ----
  // "local" = on this machine (zero setup) · "cloud" = OpenAI-compatible API
  // with a key (Groq/OpenAI/custom) · "homelab" = the /whisper service behind
  // the Homelab URL. Unset falls back to the legacy auto behavior (homelab if
  // configured, else local) — shown as whichever of those applies.
  let transcriptionMode = $state<"local" | "cloud" | "homelab">("local");
  let whisperCloudProvider = $state<"groq" | "openai" | "custom">("groq");
  let whisperCloudUrl = $state("");
  let whisperCloudModel = $state("");
  let whisperApiKey = $state("");
  const CLOUD_WHISPER_PRESETS = {
    groq: { url: "https://api.groq.com/openai/v1", model: "whisper-large-v3-turbo" },
    openai: { url: "https://api.openai.com/v1", model: "whisper-1" },
  } as const;
  function setTranscriptionMode(m: "local" | "cloud" | "homelab") {
    transcriptionMode = m;
    whisperCheckState = null;
    whisperCheckNote = "";
    api.setSetting("transcription_mode", m).catch(() => {});
    // Entering cloud mode with everything blank: land on the Groq preset so the
    // only thing left to do is paste a key.
    if (m === "cloud" && !whisperCloudUrl.trim() && !whisperCloudModel.trim()) {
      applyCloudPreset("groq");
    }
  }
  function applyCloudPreset(p: "groq" | "openai" | "custom") {
    whisperCloudProvider = p;
    api.setSetting("whisper_cloud_provider", p).catch(() => {});
    if (p !== "custom") {
      whisperCloudUrl = CLOUD_WHISPER_PRESETS[p].url;
      whisperCloudModel = CLOUD_WHISPER_PRESETS[p].model;
      saveCloudWhisper();
    }
  }
  function saveCloudWhisper() {
    api.setSettings({
      whisper_cloud_url: whisperCloudUrl.trim(),
      whisper_cloud_model: whisperCloudModel.trim(),
      whisper_api_key: whisperApiKey.trim(),
    }).catch(() => {});
  }

  // ---- homelab access token (auth for every service except /sync) ----
  let hlToken = $state("");
  function saveHlToken() {
    api.setSetting("homelab_token", hlToken.trim()).catch(() => {});
  }

  // ---- unified homelab access ----
  // One base URL fronts every service (search / whisper / ollama / sync) behind
  // a reverse proxy; Cortex appends each service's path. Tailscale/public bases
  // are the auto local→Tailscale→public fallbacks.
  let hlBase      = $state("");
  let hlTailscale = $state("");
  let hlPublic    = $state("");
  let hlState     = $state<"idle" | "testing" | "ok" | "fail">("idle");
  let hlReach     = $state<Record<string, "ok" | "fail">>({});
  let hlSaveTimer: ReturnType<typeof setTimeout> | undefined;

  // Real app version for the footer (the Tauri build version), loaded once.
  let appVersion = $state("");
  $effect(() => { void getVersion().then((v) => (appVersion = v)).catch(() => {}); });
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
  // Per-service health grid (SearXNG / WhisperX / WebDAV / syncd / ingest / Ollama),
  // probed through the SAME resolved URLs the app actually uses.
  let svcStatus = $state<api.HomelabServiceStatus[] | null>(null);
  let svcTesting = $state(false);
  async function testHomelab() {
    const tiers: [string, string][] = [["local", hlBase], ["tailscale", hlTailscale], ["public", hlPublic]];
    if (!tiers.some(([, u]) => u.trim())) return;
    hlState = "testing";
    svcTesting = true;
    const reach: Record<string, "ok" | "fail"> = {};
    for (const [tier, url] of tiers) {
      if (!url.trim()) continue;
      reach[tier] = await testEndpoint(url.trim());
    }
    hlReach = reach;
    hlState = Object.values(reach).some((r) => r === "ok") ? "ok" : "fail";
    // Then check each service individually so a broken one is named, not guessed.
    try { svcStatus = await api.homelabStatus(); } catch { svcStatus = null; }
    svcTesting = false;
  }
  // Whether the homelab's live-sync service answered on the last test (null = unknown).
  const syncdOk = $derived(svcStatus?.find((s) => s.id === "syncd")?.ok ?? null);

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
  const canSync = $derived(anySyncUrl || !!hlBase.trim() || !!hlTailscale.trim() || !!hlPublic.trim());

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
    return ms ? new Date(ms).toLocaleString() : t("never");
  }
  function syncPill() {
    // Live (WebSocket) trumps the periodic states — changes are propagating
    // across devices within about a second.
    if (app.syncLive && app.syncState !== "off" && app.syncState !== "error") {
      return { cls: "ready", label: t("Live") };
    }
    switch (app.syncState) {
      case "syncing": return { cls: "draft", label: t("Syncing…") };
      case "synced":  return { cls: "ready", label: t("Synced") };
      case "error":   return { cls: "error", label: t("Sync error") };
      case "idle":    return { cls: "ready", label: t("On") };
      default:        return { cls: "pending", label: t("Off") };
    }
  }

  // ---- Experimental features ----
  let expMoodle = $state(false);
  function toggleExpMoodle() {
    expMoodle = !expMoodle;
    api.setSetting("exp_moodle", expMoodle ? "true" : "false").catch(() => {});
  }

  // ---- Moodle integration ----
  let mdUrl   = $state("");
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
      app.pushToast({ kind: "success", title: t("Moodle connected"), body: name || undefined });
      await loadMoodle();
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Moodle connect failed"), body: String(e) });
    } finally { mdBusy = false; }
  }
  async function mdSyncNow() {
    mdBusy = true;
    try {
      mdSummary = await api.moodleSync();
      app.pushToast({ kind: "success", title: t("Moodle synced"),
        body: t("{n} courses · {n} grades · {n} deadlines · {n} announcements", { n: `${mdSummary.courses} courses · ${mdSummary.grades} grades · ${mdSummary.deadlines} deadlines · ${mdSummary.announcements} announcements` }) });
      await loadMoodle();
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Moodle sync failed"), body: String(e) });
    } finally { mdBusy = false; }
  }
  async function mdAutolink() {
    try {
      const n = await api.moodleAutolink();
      app.pushToast({ kind: n ? "success" : "info", title: t("Auto-linked {n} subject(s)", { n }) });
      await app.refresh();
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Auto-link failed"), body: String(e) });
    }
  }
  async function mdDisconnect() {
    try {
      await api.moodleDisconnect();
      mdToken = "";
      await loadMoodle();
      app.pushToast({ kind: "info", title: t("Moodle disconnected") });
    } catch { /* ignore */ }
  }
  function mdCourseName(courseId: string): string {
    return mdData.courses.find((c) => c.id === courseId)?.fullname || courseId;
  }
  async function mdLoginSso() {
    try {
      await api.moodleLoginSso(mdUrl);
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Could not open SSO login"), body: String(e) });
    }
  }
  // SSO login happens in a separate window; react to its result here.
  $effect(() => {
    let un1: (() => void) | undefined;
    let un2: (() => void) | undefined;
    api.onMoodleSsoDone(async (name) => {
      app.pushToast({ kind: "success", title: t("Moodle connected"), body: name || undefined });
      await loadMoodle();
    }).then((u) => (un1 = u));
    api.onMoodleSsoError((msg) =>
      app.pushToast({ kind: "error", title: t("SSO login failed"), body: msg })
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
      app.pushToast({ kind: "success", title: t("Backup uploaded"), body: dest });
      await refreshBackupStatus();
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Backup failed"), body: String(e) });
    } finally {
      backingUp = false;
    }
  }
  function fmtBackupTime(ms: number | null): string {
    if (!ms) return t("never");
    return new Date(ms).toLocaleString();
  }

  // Last transport error from a failed reachability test, so the homelab card can
  // explain WHY (e.g. iOS blocking cleartext LAN, DNS, timeout) instead of "fail".
  let lastTestError = $state("");
  async function testEndpoint(url: string): Promise<"ok" | "fail"> {
    try {
      const ok = await api.pingUrl(url);
      lastTestError = ok ? "" : t("Reached the server, but it didn't return a success status.");
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
      app.pushToast({ kind: "success", title: t("Copied"), body: t("Install command copied to clipboard.") });
    } catch {
      app.pushToast({ kind: "warning", title: t("Couldn't copy"), body: depReport.install_command });
    }
  }
  // One-click install (macOS/Homebrew only — no sudo needed there).
  let depInstalling = $state(false);
  async function installDeps() {
    depInstalling = true;
    try {
      const msg = await api.installDependencies();
      app.pushToast({ kind: "success", title: t("Dependencies"), body: msg });
      await loadDeps();
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Install failed"), body: String(e) });
    } finally {
      depInstalling = false;
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
    catch (e) { app.pushToast({ kind: "error", title: t("Couldn't list calendars"), body: String(e) }); }
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
      app.pushToast({ kind: "success", title: t("Google Calendar connected"), body: gStatus.email ?? undefined });
      void loadGoogleCalendars();
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Connect failed"), body: String(e) });
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
      app.pushToast({ kind: "success", title: t("Calendar synced"), body: t("{n} pulled · {n} pushed", { n: `${r.pulled} pulled · ${r.pushed} pushed` }) });
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Sync failed"), body: String(e) });
    } finally {
      gBusy = false;
    }
  }
  async function disconnectGoogle() {
    try {
      gStatus = await api.googleDisconnect();
      app.pushToast({ kind: "info", title: t("Google disconnected") });
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Disconnect failed"), body: String(e) });
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
    if (state === "testing") return { cls: "draft", label: t("Checking…") };
    if (state === "ok") return { cls: "ready", label: t("Connected") };
    if (state === "fail") return { cls: "error", label: t("Unreachable") };
    return configured
      ? { cls: "pending", label: t("Untested") }
      : { cls: "pending", label: t("Not set") };
  }

  // persist the Ollama endpoint on change, and re-probe its installed models
  $effect(() => {
    const ep = endpoint;
    if (!loaded) return;
    api.setSettings({ ollama_url: ep }).catch(() => {});
    invalidateOllamaModels();
  });

  // ---- focus timer (pomodoro) durations — bound to the app-wide timer ----
  const pomoFields = [
    { key: "workMin",            label: t("Focus length"),  unit: " min", step: 5, min: 5, max: 90 },
    { key: "breakMin",           label: t("Short break"),   unit: " min", step: 1, min: 1, max: 30 },
    { key: "longBreakMin",       label: t("Long break"),    unit: " min", step: 5, min: 5, max: 60 },
    { key: "sessionsBeforeLong", label: t("Sessions / set"), unit: "",    step: 1, min: 2, max: 8  },
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
      app.pushToast({ kind: "success", title: t("Storage optimized"), body: t("Reclaimed unused space (VACUUM).") });
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Optimize failed"), body: String(e) });
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
      app.pushToast({ kind: "success", title: t("Exported"), body: dest });
    } catch (e) {
      app.pushToast({ kind: "error", title: t("Export failed"), body: String(e) });
    }
  }

  async function deleteEverything() {
    const ok = window.confirm(
      t("Delete ALL data?\n\nThis wipes the local database — every subject, source, cheatsheet, and embedding. This cannot be undone. Your settings and API keys are kept."),
    );
    if (!ok) return;
    try {
      await api.deleteAllData();
      await loadStats();
      app.pushToast({ kind: "success", title: t("All data deleted"), body: t("Reload the app to start fresh.") });
    } catch {
      app.pushToast({ kind: "error", title: t("Delete failed") });
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
      if (s.custom_endpoint)    keys = { ...keys, custom_endpoint: s.custom_endpoint };
      if (s.custom_api_key)     keys = { ...keys, custom_api_key: s.custom_api_key };

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
      if (s.whisper_model)                 whisperModel = s.whisper_model;
      // Transcription mode + cloud provider. Unset mode = legacy auto: homelab
      // when any whisper/homelab URL is configured, otherwise local.
      if (s.transcription_mode === "local" || s.transcription_mode === "cloud" || s.transcription_mode === "homelab") {
        transcriptionMode = s.transcription_mode;
      } else {
        transcriptionMode =
          (s.whisper_url || s.homelab_base || s.homelab_tailscale_base || s.homelab_public_base)
            ? "homelab" : "local";
      }
      if (s.whisper_cloud_provider === "groq" || s.whisper_cloud_provider === "openai" || s.whisper_cloud_provider === "custom") whisperCloudProvider = s.whisper_cloud_provider;
      if (s.whisper_cloud_url)   whisperCloudUrl = s.whisper_cloud_url;
      if (s.whisper_cloud_model) whisperCloudModel = s.whisper_cloud_model;
      if (s.whisper_api_key)     whisperApiKey = s.whisper_api_key;
      if (s.ocr_endpoint)        ocrEndpoint = s.ocr_endpoint;
      if (s.ocr_api_key)         ocrKey      = s.ocr_api_key;
      if (s.ocr_model)           ocrModel    = s.ocr_model;
      if (s.homelab_token)       hlToken = s.homelab_token;
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

      // Auto-verify each stored key's connection, and pre-load Ollama's installed
      // models if any task uses Ollama (so the picker reflects reality immediately).
      verifyAllKeys();
      if (Object.values(assign).some((x) => x.provider === "ollama")) void ensureOllamaModels();
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
    }).then(() => app.pushToast({ kind: "success", title: t("Profile saved"), body: t("The AI will use your updated context.") }))
      .catch(() => app.pushToast({ kind: "error", title: t("Save failed") }));
  }

  function saveKeys() {
    api.setSettings({
      openrouter_api_key: keys.openrouter,
      gemini_api_key:     keys.gemini,
      claude_api_key:     keys.claude,
      openai_api_key:     keys.openai,
      custom_endpoint:    keys.custom_endpoint,
      custom_api_key:     keys.custom_api_key,
    }).then(() => app.pushToast({ title: t("Keys saved"), body: t("Stored in the system keychain."), kind: "success" }))
      .catch(() => app.pushToast({ kind: "error", title: t("Save failed") }));
  }

  // Live OpenRouter catalog for the searchable model picker — fetched once, on the
  // first time an OpenRouter model dropdown is opened.
  let orModels = $state<OrModel[]>([]);
  let orLoading = $state(false);
  let orLoaded = false;
  async function ensureOrModels() {
    if (orLoaded || orLoading) return;
    orLoading = true;
    try {
      orModels = await loadOpenRouterModels();
      orLoaded = true;
    } catch {
      app.pushToast({ kind: "error", title: t("OpenRouter models"), body: t("Couldn’t load the model list — check your connection.") });
    } finally {
      orLoading = false;
    }
  }

  async function onModelProviderChange(taskId: TaskId, p: string) {
    if (p === "ollama") await ensureOllamaModels();
    const provList = providersFor(taskId);
    const np = provList.find((x) => x.id === p) ?? provList[0];
    // Ollama → default to the first INSTALLED model (empty if none pulled, which leaves
    // the picker blank as intended). Others → the provider's first curated model.
    const firstModel = p === "ollama" || p === "custom" ? "" : (np.models[0]?.id ?? "");
    setTask(taskId, { provider: p, model: firstModel });
    const kv: Record<string, string> = { [`model_${taskId}`]: p + ":" + firstModel };
    if (taskId === "embedding") kv.embed_provider = p;
    api.setSettings(kv).catch(() => {});
  }

  function onModelChange(taskId: TaskId, m: string) {
    setTask(taskId, { model: m });
    api.setSettings({ [`model_${taskId}`]: assign[taskId].provider + ":" + m }).catch(() => {});
  }

  // ── Ollama: live installed-model list (GET /api/tags) ──────────────────────
  // When a task's provider is Ollama, the model picker shows ONLY the models actually
  // installed on the configured server (local on desktop, or the homelab). An empty
  // list ⇒ an empty picker (nothing to choose) — never models that aren't pulled.
  let ollamaInstalled = $state<string[]>([]);
  let ollamaLoaded = false;
  async function ensureOllamaModels() {
    if (ollamaLoaded) return;
    ollamaLoaded = true;
    try { ollamaInstalled = await api.ollamaModels(); } catch { ollamaInstalled = []; }
  }
  // Changing the Ollama/homelab URL just INVALIDATES the cached list — it's re-probed
  // lazily next time the picker opens, so we don't fire a network call per keystroke.
  function invalidateOllamaModels() { ollamaLoaded = false; ollamaInstalled = []; }

  // Is local-model (Ollama) usage available here? On desktop, always (localhost). On
  // mobile there is no localhost Ollama, so it needs a homelab base (or an explicit
  // non-localhost Ollama URL) to be reachable.
  const hasHomelabOllama = $derived(
    !!(hlBase.trim() || hlTailscale.trim() || hlPublic.trim()) ||
    (!!endpoint.trim() && !/localhost|127\.0\.0\.1/.test(endpoint))
  );
  const ollamaAvailable = $derived(!isMobile || hasHomelabOllama);

  // Provider list for a task, with Ollama dropped on mobile when no homelab is set
  // (so it isn't offered when it can't possibly work).
  function providersFor(taskId: TaskId) {
    const base = taskId === "embedding" ? EMBED_PROVIDERS : PROVIDERS;
    return ollamaAvailable ? base : base.filter((p) => p.id !== "ollama");
  }

  // Options for a task's model picker: OpenRouter → live catalog; Ollama → installed
  // models (may be empty); else the provider's curated list.
  function modelOptionsFor(prov: { id: string; models: Model[] }) {
    if (prov.id === "openrouter" && orModels.length) return orModels;
    if (prov.id === "ollama") return ollamaInstalled.map((id) => ({ id, label: id }));
    return prov.models.map((m) => ({ id: m.id, label: m.label }));
  }

  // Ollama URL to display in API keys. Desktop: the editable endpoint. Mobile: the
  // homelab-derived URL (base + /ollama), shown read-only — Ollama is homelab-only there.
  const ollamaDisplayUrl = $derived(
    endpoint.trim() ? endpoint.trim()
    : (hlBase.trim() ? hlBase.trim().replace(/\/+$/, "") + "/ollama" : "")
  );

  // ── API-key verification (a real authed connection check, not just "key present") ──
  type VerifyState = api.VerifyResult | "checking" | null;
  let verify = $state<Record<string, VerifyState>>({});
  // Status-badge class + label, shared by the provider rows and the Ollama row.
  const statusClass = (v: VerifyState) =>
    "key-status " + (v === "checking" ? "checking" : v ? (v.ok ? "ok" : "bad") : "off");
  const statusLabel = (v: VerifyState, isSet = false) =>
    v === "checking" ? "checking…" : v ? (v.ok ? "connected" : v.detail) : (isSet ? "not checked" : "not set");
  const verifyIdForKey = (id: string) =>
    id === "custom_endpoint" || id === "custom_api_key" ? "custom" : id === "ocr_endpoint" || id === "ocr_api_key" ? "ocr" : id;
  async function verifyKey(id: string) {
    const vid = verifyIdForKey(id);
    verify = { ...verify, [vid]: "checking" };
    try { verify = { ...verify, [vid]: await api.verifyProvider(vid) }; }
    catch (e) { verify = { ...verify, [vid]: { ok: false, detail: String(e) } }; }
  }
  // Verify every provider that has a stored key (run on load + after Save keys).
  function verifyAllKeys() {
    for (const k of keyMeta) {
      if (keys[k.id as keyof typeof keys]?.trim()) void verifyKey(k.id);
    }
    if (ollamaAvailable) void verifyKey("ollama");
  }

  const levelLabels: Record<string, string> = {
    undergrad: t("Undergraduate"),
    postgrad:  t("Postgraduate"),
    phd:       t("PhD / research"),
    self:      t("Self-study"),
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
        title={t("Back")}
      >
        <Icon name="chevron" size={14} style="transform:rotate(180deg)" />
      </button>
      <span class="mono" style="color:var(--fg-bright);font-weight:600">{t("Settings")}</span>
    </div>

    {#each navTabs as nt}
      <button
        class={"set-nav-item" + (tab === nt.id ? " on" : "")}
        onclick={() => (tab = nt.id)}
      >
        <Icon name={nt.icon} size={13} /> {nt.label}
      </button>
    {/each}

    <div class="set-nav-foot mono faint">Cortex {appVersion ? "v" + appVersion : ""} · BYOK · local-first</div>
  </aside>

  <!-- MAIN BODY -->
  <div class="set-body">

    {#if isMobile}
      <div class="set-mnav">
        <Picker
          value={tab}
          onChange={(id) => (tab = id)}
          options={navTabs.map((nt) => ({ id: nt.id, label: nt.label }))}
          icon={navTabs.find((nt) => nt.id === tab)?.icon}
        />
      </div>
    {/if}

    <!-- ===== PROFILE ===== -->
    {#if tab === "profile"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">{t("Profile")}</div>
          <h1 class="set-title">{t("Who the AI thinks you are")}</h1>
          <p class="set-sub">{t("Shared with every chat and generation so answers fit your level and style. Stays on this machine.")}</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Identity")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Display name")}</div></div>
              <div class="set-row-r"><input class="input" bind:value={name} placeholder={t("Your name")} /></div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Pronouns")}</div></div>
              <div class="set-row-r"><input class="input" bind:value={pronouns} /></div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Level")}</div></div>
              <div class="set-row-r">
                <Picker
                  value={level}
                  onChange={(v) => (level = v)}
                  options={[
                    { id: "undergrad", label: t("Undergraduate") },
                    { id: "postgrad",  label: t("Postgraduate") },
                    { id: "phd",       label: t("PhD / research") },
                    { id: "self",      label: t("Self-study") },
                  ]}
                />
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Field of study")}</div></div>
              <div class="set-row-r"><input class="input" bind:value={field} /></div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">{t("About you")}</h3>
            <p class="set-group-d">{t("Context the AI uses to personalize explanations.")}</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-t">{t("In your words")}</div>
              <textarea class="input set-textarea set-bio" bind:value={about} rows={6}></textarea>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Response style")}</div>
                <div class="set-row-d">{t("How much detail by default.")}</div>
              </div>
              <div class="set-row-r">
                <div class="seg">
                  {#each [{ id: "concise", label: t("Concise") }, { id: "balanced", label: t("Balanced") }, { id: "detailed", label: t("Detailed") }] as opt}
                    <button type="button" class={"seg-opt" + (style === opt.id ? " on" : "")} onclick={() => (style = opt.id)}>{opt.label}</button>
                  {/each}
                </div>
              </div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-l">
                <div class="set-row-t">{t("Explain with")}</div>
                <div class="set-row-d">{t("Pick what helps you learn fastest.")}</div>
              </div>
              <div class="set-row-r">
                <div class="tag-suggest" style="margin-top:0">
                  {#each [{ id: "worked-examples", label: t("worked examples") }, { id: "analogies", label: t("analogies") }, { id: "formal-proofs", label: t("formal proofs") }, { id: "diagrams", label: t("diagrams") }, { id: "code", label: t("code snippets") }] as opt}
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
            <h3 class="set-group-t">{t("Memory")}</h3>
            <p class="set-group-d">{t("Long-term facts the AI is given in every chat — like remembering your exam date, the textbook you use, or how you like answers framed.")}</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-r">
                <div class="row-inline">
                  <input
                    class="input"
                    bind:value={newMemory}
                    placeholder={t("e.g. My final exam is on June 20th")}
                    onkeydown={(e) => { if (e.key === "Enter") { e.preventDefault(); addMemoryFact(); } }}
                  />
                  <button class="btn btn--primary" onclick={addMemoryFact} disabled={!newMemory.trim() || memoryBusy}>
                    <Icon name="check" size={13} /> {t("Remember")}
                  </button>
                </div>
              </div>
            </div>
            {#if memories.length === 0}
              <div class="set-row">
                <div class="set-row-l"><div class="set-row-d">{t("No memories yet. Add a fact above and the AI will keep it in mind.")}</div></div>
              </div>
            {:else}
              {#each memories as m (m.id)}
                <div class="set-row">
                  <div class="set-row-l"><div class="set-row-t">{m.content}</div></div>
                  <div class="set-row-r">
                    <button class="btn btn--icon btn--sm btn--ghost" onclick={() => removeMemory(m.id)} title={t("Forget this")}>
                      <Icon name="x" size={13} />
                    </button>
                  </div>
                </div>
              {/each}
            {/if}
          </div>
        </section>

        <div class="set-preview">
          <div class="label" style="margin-bottom:8px">{t("What the AI receives")}</div>
          <pre class="set-sysprompt mono">User: {name} ({pronouns}) · {levelLabels[level] ?? level}
Studying: {field}
Style: {style}, prefers {explain.join(", ") || "no special format"}
Notes: {about}</pre>
        </div>

        <div class="set-foot-actions">
          <button class="btn btn--primary" onclick={saveProfile}>
            <Icon name="check" size={13} /> {t("Save profile")}
          </button>
        </div>
      </div>

    <!-- ===== MODELS ===== -->
    {:else if tab === "models"}
      <div class="set-pane set-pane--models">
        <header class="set-head">
          <div class="eyebrow">{t("Models")}</div>
          <h1 class="set-title">{t("A model for every task")}</h1>
          <p class="set-sub">{t("Route each job to the provider that does it best. Token budgets cap spend per call.")}</p>
        </header>

        <div class="set-card set-table">
          <div class="mt-head mono">
            <span>{t("Task")}</span><span>{t("Provider")}</span><span>{t("Model")}</span><span>{t("Token budget")}</span>
          </div>

          {#each MODEL_TASKS as task}
              {@const a = assign[task.id]}
              {@const provList = providersFor(task.id)}
              {@const allProv = task.id === "embedding" ? EMBED_PROVIDERS : PROVIDERS}
            {@const isOr = a.provider === "openrouter"}
            {@const isOllama = a.provider === "ollama"}
            {@const isCustom = a.provider === "custom"}
            {@const prov = allProv.find((p) => p.id === a.provider) ?? provList[0]}
            <div class="mt-row">
              <div class="mt-task">
                <div class="mt-task-t">{task.label}</div>
                <div class="mt-task-d mono">{task.desc}</div>
              </div>
              <Picker
                value={a.provider}
                onChange={(p) => onModelProviderChange(task.id, p)}
                options={provList.map((p) => ({ id: p.id, label: p.label }))}
              />
              <!-- OpenRouter → live searchable catalog (curated list as offline/pre-fetch
                   fallback); Ollama → only models actually installed (empty ⇒ nothing to
                   pick); every other provider → its own curated list, still searchable. -->
              <ModelSearch
                value={a.model}
                onChange={(m) => onModelChange(task.id, m)}
                options={modelOptionsFor(prov)}
                loading={isOr && orLoading}
                onOpen={isOr ? ensureOrModels : (isOllama ? ensureOllamaModels : undefined)}
                allowCustom={isCustom}
                placeholder={isCustom ? t("Type model id, e.g. qwen-plus") : (isOr ? t("Search OpenRouter…") : (isOllama ? (ollamaInstalled.length ? t("Pick an installed model") : t("No models installed")) : undefined))}
              />
              {#if task.id === "embedding"}
                <span class="mono faint mt-budget-na">n/a</span>
              {:else}
                <input
                  class="input mono mt-budget"
                  value={a.budget}
                  oninput={(e) => { const v = (e.target as HTMLInputElement).value; setTask(task.id, { budget: v }); api.setSettings({ ["budget_" + task.id]: v }).catch(() => {}); }}
                />
              {/if}
            </div>
          {/each}
        </div>

        <div class="set-note mono">
          <Icon name="diamond" size={11} color="var(--accent)" />
          {t("Ollama tasks run fully offline on this machine or your homelab — no key required.")}
        </div>
      </div>

    <!-- ===== API KEYS ===== -->
    {:else if tab === "keys"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">{t("API keys")}</div>
          <h1 class="set-title">{t("Bring your own keys")}</h1>
          <p class="set-sub">{t("Stored in the OS keychain, never synced. Nothing routes through Cortex servers.")}</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Providers")}</h3></div>
          <div class="set-card">
            {#each keyMeta as k}
              {@const isSet = !!keys[k.id as keyof typeof keys]}
              {@const v = verify[verifyIdForKey(k.id)]}
              <div class="set-row stacked">
                <div class="set-row-l">
                  <div class="set-row-t">
                    <span class="row-keytitle">
                      {k.label}
                      <span class={statusClass(v)}>{statusLabel(v, isSet)}</span>
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
                      oninput={(e) => { keys = { ...keys, [k.id]: (e.target as HTMLInputElement).value }; verify = { ...verify, [k.id]: null }; }}
                      placeholder={k.placeholder}
                      spellcheck={false}
                    />
                    <button
                      type="button"
                      class="masked-eye"
                      onclick={() => { showKey = { ...showKey, [k.id]: !showKey[k.id] }; }}
                      title={showKey[k.id] ? t("Hide") : t("Show")}
                    >
                      <Icon name={showKey[k.id] ? "x" : "search"} size={13} />
                    </button>
                  </div>
                  {#if isSet}
                    <div style="display:flex;gap:6px;margin-top:6px">
                      <button
                        type="button"
                        class="btn btn--ghost btn--sm"
                        onclick={() => verifyKey(k.id)}
                      >{t("Verify")}</button>
                      <button
                        type="button"
                        class="btn btn--ghost btn--sm"
                        onclick={() => { keys = { ...keys, [k.id]: "" }; verify = { ...verify, [k.id]: null }; }}
                      >{t("Clear")}</button>
                    </div>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </section>

        <!-- Local models (Ollama) — keyless; a URL, not a key. On mobile Ollama is
             reached only through the Homelab (no localhost on a phone). -->
        {#if ollamaAvailable}
        {@const ov = verify.ollama}
        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Local models (Ollama)")}</h3></div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-l">
                <div class="set-row-t">
                  <span class="row-keytitle">
                    {t("Ollama URL")}
                    <span class={statusClass(ov)}>{statusLabel(ov)}</span>
                  </span>
                </div>
                <div class="set-row-d">
                  {#if isMobile}
                    {t("Runs through your Homelab — set the Homelab URL in Integrations. Keyless.")}
                  {:else}
                    {t("Keyless, local. Defaults to")} <span class="mono">http://localhost:11434</span>; {t("leave blank to use your Homelab.")}
                  {/if}
                </div>
              </div>
              <div class="set-row-r">
                {#if isMobile}
                  <input class="input mono" value={ollamaDisplayUrl} placeholder={t("set Homelab URL in Integrations")} readonly />
                {:else}
                  <input
                    class="input mono"
                    value={endpoint}
                    oninput={(e) => { endpoint = (e.target as HTMLInputElement).value; verify = { ...verify, ollama: null }; }}
                    placeholder="http://localhost:11434"
                    spellcheck={false}
                  />
                {/if}
                <button type="button" class="btn btn--ghost btn--sm" style="margin-top:6px" onclick={() => verifyKey("ollama")}>{t("Verify")}</button>
              </div>
            </div>
          </div>
        </section>
        {/if}

        <div class="set-foot-actions">
          <button class="btn btn--primary" onclick={() => { saveKeys(); verifyAllKeys(); }}>
            <Icon name="check" size={13} /> {t("Save keys")}
          </button>
        </div>
      </div>

    {:else if tab === "appearance"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">{t("Appearance")}</div>
          <h1 class="set-title">{t("Make it yours")}</h1>
          <p class="set-sub">{t("Cortex re-skins live from your Omarchy theme, or pick one manually.")}</p>
        </header>

        <!-- Follow-Omarchy mirrors the desktop's Omarchy palette — meaningless on a phone. -->

        <!-- Interface language: en | ru, persisted per-device in ui_lang -->
        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Language")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Interface language")}</div>
                <div class="set-row-d">{t("Missing translations fall back to English.")}</div>
              </div>
              <div class="set-row-r">
                <div class="seg">
                  {#each LOCALES as l}
                    <button
                      type="button"
                      class={"seg-opt" + (i18n.locale === l.id ? " on" : "")}
                      onclick={() => setLocale(l.id)}
                    >{l.label}</button>
                  {/each}
                </div>
              </div>
            </div>
          </div>
        </section>
        {#if !isMobile}
        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Theme")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Follow Omarchy theme")}</div>
                <div class="set-row-d">{t("Mirror your desktop's current Omarchy palette on every launch. Picking a theme below turns this off.")}</div>
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
                      app.pushToast({ kind: "warning", title: t("Omarchy theme not found"), body: t("Couldn't read your Omarchy theme, or it has no Cortex match.") });
                    } else if (matched) {
                      app.pushToast({ kind: "success", title: t("Following Omarchy"), body: t("Matched → {theme}.", { theme: THEME_LABELS[matched] }) });
                    }
                  }}
                ><span class="st-knob"></span></button>
              </div>
            </div>
          </div>
        </section>
        {/if}

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Manual theme")}</h3></div>
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
          <div class="set-group-h"><h3 class="set-group-t">{t("Reading")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Cheatsheet typeface")}</div>
                <div class="set-row-d">{t("The voice of everything you read to learn.")}</div>
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
                <div class="set-row-t">{t("Density")}</div>
                <div class="set-row-d">{t("Spacing throughout the app.")}</div>
              </div>
              <div class="set-row-r">
                <div class="seg">
                  {#each [{ id: "regular", label: t("Regular") }, { id: "compact", label: t("Compact") }] as opt}
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
          <div class="set-group-h"><h3 class="set-group-t">{t("Window")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Close to tray")}</div>
                <div class="set-row-d">{t("Closing the window keeps Cortex running in the tray — ingest, generation and music continue. Quit from the tray menu.")}</div>
              </div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (closeToTray ? " on" : "")} onclick={toggleCloseToTray} role="switch" aria-checked={closeToTray} aria-label={t("Close to tray")}><span class="st-knob"></span></button>
              </div>
            </div>
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Display")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("UI scale")}</div>
                <div class="set-row-d">{t("Make everything larger or smaller — helps on high-resolution displays.")}</div>
              </div>
              <div class="set-row-r" style="min-width:120px">
                <!-- Dropdown of presets (was a slider — the UI rescaling live under the
                     thumb made the slider itself jump around while dragging). -->
                <Picker
                  value={String(app.uiScale)}
                  onChange={(id) => app.setUiScale(+id)}
                  options={[
                    ...([80, 90, 100, 110, 125, 150].includes(app.uiScale) ? [] : [{ id: String(app.uiScale), label: `${app.uiScale}% (custom)` }]),
                    { id: "80", label: "80%" },
                    { id: "90", label: "90%" },
                    { id: "100", label: t("100% — default") },
                    { id: "110", label: "110%" },
                    { id: "125", label: "125%" },
                    { id: "150", label: "150%" },
                  ]}
                  placeholder="100%"
                />
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
          <div class="eyebrow">{t("Keybinds")}</div>
          <h1 class="set-title">{t("Helix-style, your way")}</h1>
          <p class="set-sub">{t("Click any binding to rebind it. Press Esc while listening to cancel.")}</p>
        </header>

        <div class="set-card">
          <div class="set-row">
            <div class="set-row-l">
              <div class="set-row-t">{t("Preset")}</div>
              <div class="set-row-d">{t("Starting point for bindings.")}</div>
            </div>
            <div class="set-row-r">
              <div class="seg">
                {#each [{ id: "helix", label: "Helix" }, { id: "vim", label: "Vim" }, { id: "custom", label: "Custom" }] as opt}
                  <button
                    type="button"
                    class={"seg-opt" + (keybinds.preset === opt.id ? " on" : "")}
                    disabled={opt.id === "custom"}
                    onclick={() => { if (opt.id === "helix" || opt.id === "vim") { keybinds.applyPreset(opt.id); app.pushToast({ kind: "success", title: t("{preset} keybinds applied", { preset: opt.label }), body: t("Shortcuts take effect outside this screen.") }); } }}
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
                  <span class="mono faint">{t("press a key…")}</span>
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
              app.pushToast({ kind: "info", title: t("Reset"), body: t("Bindings restored to {preset} preset.", { preset: p === "vim" ? "Vim" : "Helix" }) });
            }}
          >
            {t("Reset to preset")}
          </button>
        </div>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">{t("Leader menu (Space then…)")}</h3>
            <p class="set-group-d">{t("Press")} <span class="kbd">Space</span> {t("to open the leader menu, then a key. Fixed, mnemonic — they only fire while the menu is open.")}</p>
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
            <h3 class="set-group-t">{t("System")}</h3>
            <p class="set-group-d">{t("Built-in shortcuts that follow OS conventions and can't be rebound.")}</p>
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
            <div class="set-row-t">{t("Illustrate with diagrams")}</div>
            <div class="set-row-d">{t("A relevant diagram per cheatsheet section + images in chat. On by default once SearXNG is connected.")}</div>
          </div>
          <div class="set-row-r">
            <button type="button" class={"st-toggle" + (webImages ? " on" : "")} onclick={() => setWebImages(!webImages)} disabled={!(searxng.trim() || hlBase.trim() || hlTailscale.trim() || hlPublic.trim())} role="switch" aria-checked={webImages} aria-label={t("Illustrate with diagrams")}><span class="st-knob"></span></button>
          </div>
        </div>
      {/snippet}

      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">{t("Integrations")}</div>
          <h1 class="set-title">{t("Transcription, search & sync — your way")}</h1>
          <p class="set-sub">{t("Everything works out of the box on this computer. Add a cloud API key for heavier lifting, or point Cortex at your own homelab (the")} <span class="mono">homelab/</span> {t("docker compose) — nothing here is required.")}</p>
        </header>

        <!-- ═══ TRANSCRIPTION — outcome-first: pick WHERE audio becomes text ═══ -->
        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">{t("Transcription")}</h3>
            <p class="set-group-d">{t("Where lecture recordings become text.")}</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="seg">
                <button type="button" class={"seg-opt" + (transcriptionMode === "local" ? " on" : "")} onclick={() => setTranscriptionMode("local")}>{t("This computer")}</button>
                <button type="button" class={"seg-opt" + (transcriptionMode === "cloud" ? " on" : "")} onclick={() => setTranscriptionMode("cloud")}>{t("Cloud API")}</button>
                <button type="button" class={"seg-opt" + (transcriptionMode === "homelab" ? " on" : "")} onclick={() => setTranscriptionMode("homelab")}>{t("My homelab")}</button>
              </div>
              {#if transcriptionMode === "local"}
                <div class="set-row-d">{t("Zero setup — Whisper runs on this machine, and the first transcription fetches its model automatically. Most private; slower on long lectures than the other two.")}</div>
              {:else if transcriptionMode === "cloud"}
                <div class="set-row-d">{t("No server, no hosting — just an API key. Groq's free tier turns a whole lecture into text in seconds with")} <span class="mono">large-v3-turbo</span>.</div>
              {:else}
                <div class="set-row-d">{t("Runs on the WhisperX lecture server behind your Homelab URL (configured below) — built for hour-plus recordings, with speaker labels when the server has an HF token. Audio never leaves your machines.")}</div>
              {/if}
            </div>


            {#if transcriptionMode === "cloud"}
              <div class="set-row stacked">
                <div class="set-row-t">{t("Provider")}</div>
                <div class="seg">
                  <button type="button" class={"seg-opt" + (whisperCloudProvider === "groq" ? " on" : "")} onclick={() => applyCloudPreset("groq")}>{t("Groq")} <span class="faint">{t("free")}</span></button>
                  <button type="button" class={"seg-opt" + (whisperCloudProvider === "openai" ? " on" : "")} onclick={() => applyCloudPreset("openai")}>OpenAI</button>
                  <button type="button" class={"seg-opt" + (whisperCloudProvider === "custom" ? " on" : "")} onclick={() => applyCloudPreset("custom")}>{t("Custom")}</button>
                </div>
              </div>
              {#if whisperCloudProvider === "custom"}
                <div class="set-row stacked">
                  <div class="set-row-t">{t("Endpoint URL")}</div>
                  <input class="input mono" bind:value={whisperCloudUrl} onchange={saveCloudWhisper} onblur={saveCloudWhisper} placeholder="https://api.example.com/v1" />
                  <div class="set-row-d">{t("Any OpenAI-compatible")} <span class="mono">/v1/audio/transcriptions</span> {t("endpoint.")}</div>
                </div>
              {/if}
              <div class="set-row stacked">
                <div class="set-row-t">{t("API key")}</div>
                <input class="input mono" type="password" bind:value={whisperApiKey} onchange={saveCloudWhisper} onblur={saveCloudWhisper} placeholder={whisperCloudProvider === "openai" ? "sk-…" : "gsk_…"} />
                {#if whisperCloudProvider === "groq"}
                  <div class="set-row-d">{t("Get one free — no card needed:")}
                    <button class="btn btn--sm btn--ghost" onclick={() => api.openExternal("https://console.groq.com/keys")}><Icon name="external" size={11} /> console.groq.com/keys</button>
                  </div>
                {/if}
              </div>
              <div class="set-row stacked">
                <div class="set-row-t">{t("Model")}</div>
                <input class="input mono" bind:value={whisperCloudModel} onchange={saveCloudWhisper} onblur={saveCloudWhisper} placeholder="whisper-large-v3-turbo" />
              </div>
            {/if}

            {#if transcriptionMode === "homelab"}
              <div class="set-row stacked">
                <div class="set-row-t">{t("Model")} <span class="faint">{t("legacy servers only")}</span></div>
                <input class="input mono" bind:value={whisperModel} onchange={saveWhisperModel} onblur={saveWhisperModel} placeholder="deepdml/faster-whisper-large-v3-turbo-ct2" />
                <div class="set-row-d">Only used by OpenAI-compatible servers (speaches). The WhisperX lecture server picks its model in <span class="mono">docker-compose</span> instead (<span class="mono">WHISPER_MODEL</span>, default <span class="mono">distil-large-v3</span>) — leave this blank there.</div>
              </div>
            {/if}

            {#if transcriptionMode !== "local"}
              <div class="set-row stacked">
                <div class="row-inline">
                  <button class="btn" onclick={checkWhisper} disabled={whisperCheckState === "checking"}>
                    <Icon name="refresh" size={12} /> {whisperCheckState === "checking" ? t("Verifying…") : t("Verify setup")}
                  </button>
                </div>
                {#if whisperCheckNote}
                  <div class="set-row-d" style="color:{whisperCheckState === 'fail' ? 'var(--err,#e5484d)' : whisperCheckState === 'ok' ? 'var(--ok)' : 'var(--fg-muted)'}">{whisperCheckNote}</div>
                {/if}
              </div>
            {/if}
          </div>
        </section>

        <!-- ═══ OCR — where scanned pages become text ═══ -->
        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">{t("Text recognition (OCR)")}</h3>
            <p class="set-group-d">{t("How scanned PDFs and images become searchable text.")}</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-d">{t("By default OCR runs through the vision model (the")} <span class="mono">Vision</span> {t("row on the Models tab). Point it at any OpenAI-compatible vision endpoint instead — a free cloud tier or a local server like Ollama with")} <span class="mono">llama3.2-vision</span>{t(" — to keep scans off your main provider. Leave empty to use the vision model.")}</div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("OCR endpoint URL")}</div>
              <input class="input mono" bind:value={ocrEndpoint} onchange={saveOcr} onblur={saveOcr} placeholder="http://localhost:11434/v1" />
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("API key")} <span class="faint">{t("optional")}</span></div>
              <input class="input mono" type="password" bind:value={ocrKey} onchange={saveOcr} onblur={saveOcr} placeholder="sk-…" />
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("Model")}</div>
              <input class="input mono" bind:value={ocrModel} onchange={saveOcr} onblur={saveOcr} placeholder="llama3.2-vision" />
            </div>
            {#if verify.ocr === "checking"}
              <div class="set-row">
                <div class="set-row-d">{t("Checking…")}</div>
                <button type="button" class="btn btn--ghost btn--sm" onclick={verifyOcr} disabled>
                  <Icon name="refresh" size={12} /> {t("Checking…")}
                </button>
              </div>
            {:else}
              <div class="set-row">
                <div class="set-row-d">{verify.ocr ? (verify.ocr.ok ? t("Connected") : verify.ocr.detail) : t("Save the endpoint, then verify the OCR connection.")}</div>
                <button type="button" class="btn btn--ghost btn--sm" onclick={verifyOcr}>
                  <Icon name="refresh" size={12} /> {t("Verify OCR")}
                </button>
              </div>
            {/if}
          </div>
        </section>

        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">{t("Homelab URL")}</h3>
              <p class="set-group-d">{t("One address for everything. Run the")} <span class="mono">homelab/</span> {t("docker compose and point Cortex here — search, lecture transcription, instant sync, mobile ingest and local models are all reached off this single URL (Cortex adds")} <span class="mono">/searxng</span>, <span class="mono">/whisper</span>, <span class="mono">/sync</span>, <span class="mono">/syncd</span>, <span class="mono">/ingest</span>, <span class="mono">/ollama</span> {t("for you). Add a Tailscale and/or public address and Cortex auto-picks the first reachable:")}: <strong>{t("local → Tailscale → public")}</strong>.</p>
            </div>
            {#if hlBase.trim()}
              {@const p = connPill(hlState === "idle" ? null : hlState, true)}
              <span class="status-pill status-pill--{p.cls}"><span class="dot"></span>{p.label}</span>
            {/if}
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-t">{t("Local URL")}</div>
              <input class="input mono" bind:value={hlBase} oninput={saveHomelabBasesSoon} onchange={saveHomelabBases} onblur={saveHomelabBases} placeholder="http://192.168.1.10:8080" />
              <div class="set-row-d">{t("Your homelab's LAN address (the Caddy proxy port, default")} <span class="mono">8080</span>{t(").")}</div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("Tailscale URL")} <span class="faint">{t("optional")}</span></div>
              <input class="input mono" bind:value={hlTailscale} oninput={saveHomelabBasesSoon} onchange={saveHomelabBases} onblur={saveHomelabBases} placeholder="https://homelab.tailnet-xxxx.ts.net" />
              <div class="set-row-d">{t("Used when the local URL isn't reachable. Cortex swaps just the host (and port if you give one), keeping the service paths.")}</div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("Public URL")} <span class="faint">{t("optional")}</span></div>
              <input class="input mono" bind:value={hlPublic} oninput={saveHomelabBasesSoon} onchange={saveHomelabBases} onblur={saveHomelabBases} placeholder="https://lab.example.com" />
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("Access token")} <span class="faint">{t("optional — required for a public URL")}</span></div>
              <input class="input mono" type="password" bind:value={hlToken} onchange={saveHlToken} onblur={saveHlToken} placeholder="the CORTEX_TOKEN your proxy was started with" />
              <div class="set-row-d">{t("Locks every homelab service behind a shared secret. Start the proxy with")} <span class="mono">CORTEX_TOKEN=… docker compose up -d</span> {t("and paste the same value here — Cortex sends it automatically on every request (sync keeps its own WebDAV credentials).")}</div>
              {#if hlPublic.trim() && !hlToken.trim()}
                <div class="set-row-d" style="color:var(--warn)">{t("Your homelab has a public URL but no access token — anyone on the internet can use your Whisper/Ollama/SearXNG. Set")} <span class="mono">CORTEX_TOKEN</span> {t("on the proxy and paste it here.")}</div>
              {/if}
            </div>
            <div class="set-row stacked">
              <div class="row-inline" style="flex-wrap:wrap; gap:8px 12px; align-items:center">
                <button class="btn" onclick={testHomelab} disabled={hlState === 'testing' || !(hlBase.trim() || hlTailscale.trim() || hlPublic.trim())}>
                  <Icon name="refresh" size={12} /> {hlState === 'testing' ? t("Testing…") : t("Test homelab")}
                </button>
                {#if hlReach.local}<span class="hl-reach" style="color:{hlReach.local === 'ok' ? 'var(--ok)' : 'var(--err,#e5484d)'}">LAN {hlReach.local === 'ok' ? '✓' : '✗'}</span>{/if}
                {#if hlReach.tailscale}<span class="hl-reach" style="color:{hlReach.tailscale === 'ok' ? 'var(--ok)' : 'var(--err,#e5484d)'}">Tailscale {hlReach.tailscale === 'ok' ? '✓' : '✗'}</span>{/if}
                {#if hlReach.public}<span class="hl-reach" style="color:{hlReach.public === 'ok' ? 'var(--ok)' : 'var(--err,#e5484d)'}">Public {hlReach.public === 'ok' ? '✓' : '✗'}</span>{/if}
              </div>
            </div>
            <!-- Per-service health: each feature probed on the exact URL the app
                 uses, so a red row names the ONE thing to fix (and how). -->
            {#if svcTesting && !svcStatus}
              <div class="set-row-d faint" style="margin-top:6px">{t("Checking each service…")}</div>
            {:else if svcStatus}
              <div style="margin-top:10px">
                {#each svcStatus as s (s.id)}
                  <div class="dep-row" class:dep-missing={s.configured && !s.ok}>
                    <span class="dep-dot" class:on={s.ok}></span>
                    <div class="dep-main">
                      <span class="dep-name">{s.label}</span>
                      <span class="dep-detail mono">{s.detail}</span>
                    </div>
                    <span class="dep-status {s.ok ? 'ok' : s.configured ? 'miss' : ''}">{s.ok ? t("working") : s.configured ? t("problem") : t("not set")}</span>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </section>

        <!-- Local CLI dependencies (pdftotext/libreoffice/age/…) never exist on a phone. -->
        {#if !isMobile}
        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">{t("Dependencies")}</h3>
              <p class="set-group-d">{t("External tools Cortex shells out to for ingest, OCR, transcription and media. Install whatever's missing with the one command below.")}</p>
            </div>
            <button class="btn btn--sm btn--ghost" onclick={loadDeps} disabled={depLoading}><Icon name="refresh" size={12} /> {depLoading ? t("Checking…") : t("Re-check")}</button>
          </div>
          <div class="set-card">
            {#if depReport}
              {#each depReport.deps as d (d.name)}
                <div class="dep-row" class:dep-missing={!d.present}>
                  <span class="dep-dot" class:on={d.present}></span>
                  <div class="dep-main">
                    <span class="dep-name">{d.name}</span>
                    <span class="dep-detail mono">{d.detail}</span>
                  </div>
                  <span class="dep-status {d.present ? 'ok' : 'miss'}">{d.present ? t("installed") : t("missing")}</span>
                </div>
              {/each}
              {#if depReport.install_command}
                <div class="set-row stacked" style="margin-top:10px">
                  <div class="set-row-t">{t("Install missing")} <span class="faint">· {depReport.manager}</span></div>
                  <div class="row-inline">
                    <input class="input mono" readonly value={depReport.install_command} />
                    <button class="btn" onclick={copyDepCmd}><Icon name="doc" size={12} /> {t("Copy")}</button>
                    {#if depReport.manager === "brew"}
                      <button class="btn btn--primary" onclick={installDeps} disabled={depInstalling}><Icon name="plus" size={12} /> {depInstalling ? t("Installing…") : t("Install")}</button>
                    {/if}
                  </div>
                  <div class="set-row-d">{depReport.manager === "brew" ? t("One-click install via Homebrew (no sudo). LibreOffice is large — this can take a few minutes.") : depReport.note}</div>
                </div>
              {:else}
                <div class="set-row-d" style="color:var(--ok); margin-top:8px">{t("All dependencies present 🎉")}</div>
              {/if}
            {:else}
              <div class="set-row-d faint">{depLoading ? t("Checking installed tools…") : t("Couldn't check dependencies.")}</div>
            {/if}
          </div>
        </section>
        {/if}

        <!-- One section of toggleable homelab features — same on desktop + mobile. No
             per-service URL overrides; everything rides the single Homelab URL above. -->
        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">{t("Homelab features")}</h3>
            <p class="set-group-d">{t("Optional features powered by the services behind your Homelab URL — they switch on the moment a Homelab URL is set.")}</p>
          </div>
          <div class="set-card">
            {@render diagramsToggle()}
          </div>
        </section>
        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">{t("Live sync")}</h3>
              <p class="set-group-d">{t("Instant cross-device sync. Every change streams to your other devices as a tiny per-record delta over a WebSocket (the homelab's")} <span class="mono">/syncd</span> {t("service) and shows up within about a second — newest edit wins, and nothing is deleted unless you actually deleted it. Source files and a full snapshot ride the")} <span class="mono">/sync</span> {t("WebDAV vault, which also serves as the automatic fallback when")} <span class="mono">/syncd</span> {t("isn't reachable.")}</p>
            </div>
            <span class="status-pill status-pill--{syncPill().cls}"><span class="dot"></span>{syncPill().label}</span>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Enable live sync")}</div>
                <div class="set-row-d">{t("Merges the remote vault in on launch (in the background — never blocks startup), then pushes your changes (debounced).")}</div>
              </div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (syncOn ? " on" : "")} onclick={toggleSync} disabled={!canSync} role="switch" aria-checked={syncOn} aria-label={t("Enable live sync")}><span class="st-knob"></span></button>
              </div>
            </div>

            <div class="set-row stacked">
              <div class="set-row-d">{t("Rides the")} <strong>{t("Homelab URL")}</strong> {t("above (its")} <span class="mono">/syncd</span> + <span class="mono">/sync</span> {t("services), with the same local → Tailscale → public reachability. One username/password below covers both — it's the pair set in")} <span class="mono">docker-compose.yml</span> {t("for the")} <span class="mono">sync</span> {t("and")} <span class="mono">syncd</span> {t("services.")}</div>
              {#if syncOn}
                {#if app.syncLive}
                  <div class="set-row-d" style="color:var(--ok)">{t("Live — connected to")} <span class="mono">/syncd</span>; {t("changes propagate across devices in about a second.")}</div>
                {:else if syncdOk === false}
                  <div class="set-row-d" style="color:var(--warn)">{t("Running in snapshot mode: the homelab's live-sync service didn't answer (see the service check above). Update the homelab —")} <span class="mono">git pull && docker compose up -d --build</span> — {t("to get instant sync; until then changes still sync via WebDAV snapshots.")}</div>
                {:else if app.syncState === "error"}
                  <div class="set-row-d" style="color:var(--warn)">{t("Sync is failing — hit")} <strong>{t("Test homelab")}</strong> {t("above; the service check will name what's broken (URL, credentials, or a service that isn't running).")}</div>
                {:else}
                  <div class="set-row-d faint">{t("Not live right now — snapshot sync still runs in the background.")} <strong>{t("Test homelab")}</strong> {t("above checks whether your homelab has the")} <span class="mono">/syncd</span> {t("live-sync service.")}</div>
                {/if}
              {/if}
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("Username")} <span class="faint">{t("optional")}</span></div>
              <input class="input mono" bind:value={syncUser} oninput={saveSyncSoon} onchange={saveSync} onblur={saveSync} placeholder="cortex" />
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("Password")} <span class="faint">{t("optional")}</span></div>
              <div class="row-inline">
                <input class="input mono" type="password" bind:value={syncPass} oninput={saveSyncSoon} onchange={saveSync} onblur={saveSync} placeholder="••••••••" />
                <button class="btn btn--primary" disabled={app.syncState === "syncing" || !canSync} onclick={() => app.syncManual()}>
                  <Icon name="upload" size={12} /> {app.syncState === "syncing" ? t("Syncing…") : t("Sync now")}
                </button>
              </div>
              <div class="set-row-d">{t("Last snapshot sync:")} <span class="mono">{fmtSyncTime(app.syncLastAt)}</span> {t("(live deltas don't update this — it's the periodic full-vault pass). Files live under")} <span class="mono">files/</span> {t("on the target; the DB merges by record.")}</div>
            </div>
          </div>
        </section>

        <!-- Encrypted backups use the age + rclone sidecars — desktop-only. -->
        {#if !isMobile}
        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">{t("Encrypted backups")}</h3>
              <p class="set-group-d">{t("Snapshot the database, encrypt it with")} <span class="mono">age</span>, {t("and upload with")} <span class="mono">rclone</span>. {t("Nothing leaves the machine unencrypted.")}</p>
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
              <div class="set-row-t">{t("age recipient (public key)")}</div>
              <input class="input mono" bind:value={ageRecipient} onchange={saveBackupConfig} onblur={saveBackupConfig} placeholder="age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p" />
              <div class="set-row-d">{t("From")} <span class="mono">age-keygen</span>. {t("Only this key's holder can decrypt the backups.")}</div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-t">{t("rclone remote")}</div>
              <div class="row-inline">
                <input class="input mono" bind:value={rcloneRemote} onchange={saveBackupConfig} onblur={saveBackupConfig} placeholder="homelab:cortex-backups" />
                <button class="btn btn--primary" disabled={backingUp} onclick={runBackup}>
                  <Icon name={backingUp ? "refresh" : "upload"} size={12} /> {backingUp ? t("Backing up…") : t("Back up now")}
                </button>
              </div>
              <div class="set-row-d">
                {t("An rclone remote + path, e.g.")} <span class="mono">homelab:cortex-backups</span> {t("(configure with")} <span class="mono">rclone config</span>{t(").")}
                {t("Last backup:")} <span class="mono">{fmtBackupTime(backupInfo?.last_at ?? null)}</span>{#if backupInfo?.last_dest} → <span class="mono">{backupInfo.last_dest}</span>{/if}
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
          <div class="eyebrow">{t("Audio")}</div>
          <h1 class="set-title">{t("Study sound & voices")}</h1>
          <p class="set-sub">{t("Defaults for the music player and generated audio overviews.")}</p>
        </header>

        <!-- Music is cut on mobile (no mpv/yt-dlp sidecars) — hide its settings. -->
        {#if !isMobile}
        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Study music")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Default station")}</div></div>
              <div class="set-row-r">
                <Picker
                  value={station}
                  onChange={(v) => { station = v; app.music = { ...app.music, current: v }; }}
                  options={allStations}
                />
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Autoplay on launch")}</div></div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (autoplay ? " on" : "")} onclick={() => (autoplay = !autoplay)} role="switch" aria-checked={autoplay} aria-label={t("Autoplay on launch")}><span class="st-knob"></span></button>
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
            <h3 class="set-group-t">{t("YouTube streaming")}</h3>
            <p class="set-group-d">Paste a YouTube video or livestream URL in the music panel to stream it ad-free. Uses a headless <span class="mono">mpv</span> + <span class="mono">yt-dlp</span> (auto-downloaded on first use). Nothing is bundled — only the URL is saved.</p>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Tools")}</div></div>
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
                <div class="set-row-l"><div class="set-row-d">{t("Install mpv to enable YouTube streaming:")} <span class="mono">sudo pacman -S mpv</span></div></div>
              </div>
            {:else if mediaTools && !mediaTools.ytdlp}
              <div class="set-row">
                <div class="set-row-l"><div class="set-row-d">{t("yt-dlp will be downloaded automatically the first time you play a YouTube station.")}</div></div>
              </div>
            {/if}
          </div>
        </section>
        {/if}

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">{t("Focus timer")}</h3>
            <p class="set-group-d">{t("Pomodoro session lengths (applies app-wide).")}</p>
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
            <h3 class="set-group-t">{t("Audio overview voices")}</h3>
            <p class="set-group-d">{t("The two hosts of generated podcasts.")}</p>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Host A")}</div></div>
              <div class="set-row-r">
                <Picker
                  value={voiceA}
                  onChange={(v) => (voiceA = v)}
                  options={[{ id: "maya", label: "Maya · warm" }, { id: "nova", label: "Nova · bright" }, { id: "io", label: "Io · neutral" }]}
                />
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">{t("Host B")}</div></div>
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
          <div class="eyebrow">{t("Google Calendar")}</div>
          <h1 class="set-title">{t("Sync your calendar")}</h1>
          <p class="set-sub">{t("Two-way sync with Google Calendar. The native Cortex calendar works fully without this — connecting just mirrors events both ways.")}</p>
        </header>

        <section class="set-group">
          <div class="set-group-h">
            <h3 class="set-group-t">{t("Status")}</h3>
          </div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Connection")}</div>
                <div class="set-row-d mono faint">
                  {#if gStatus?.connected}
                    {t("Connected")}{gStatus.email ? " · " + gStatus.email : ""}
                  {:else if gStatus?.configured}
                    {t("Credentials saved — not connected yet")}
                  {:else}
                    {t("Not configured")}
                  {/if}
                </div>
              </div>
              <div class="set-row-r">
                {#if gStatus?.connected}
                  <div class="row-inline">
                    <button class="btn" onclick={syncGoogle} disabled={gBusy}>
                      <Icon name="refresh" size={12} /> {t("Sync now")}
                    </button>
                    <button class="btn btn--danger" onclick={disconnectGoogle} disabled={gBusy}>{t("Disconnect")}</button>
                  </div>
                {:else}
                  <button class="btn btn--primary" onclick={connectGoogle} disabled={gBusy}>
                    <Icon name="globe" size={12} /> {gBusy ? t("Connecting…") : t("Connect Google")}
                  </button>
                {/if}
              </div>
            </div>
          </div>
        </section>

        {#if gStatus?.connected}
          <section class="set-group">
            <div class="set-group-h">
              <h3 class="set-group-t">{t("Calendars to sync")}</h3>
              <p class="set-group-d">{t("Pick which Google calendars to pull events from — tick your")} <strong>{t("university / timetable")}</strong> {t("calendar here so its classes, deadlines and exams land on the Cortex calendar.")}</p>
            </div>
            <div class="set-card">
              {#if gCalBusy && gCalendars.length === 0}
                <div class="set-row"><div class="set-row-d faint">{t("Loading calendars…")}</div></div>
              {:else if gCalendars.length === 0}
                <div class="set-row">
                  <div class="set-row-l"><div class="set-row-d faint">{t("No calendars found.")}</div></div>
                  <div class="set-row-r"><button class="btn btn--sm" onclick={loadGoogleCalendars}>{t("Reload")}</button></div>
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
            <h3 class="set-group-t">{t("Credentials")}</h3>
            <p class="set-group-d">{t("Create an OAuth client of type “Desktop app” in Google Cloud → APIs & Services → Credentials, enable the Calendar API, then paste the ID and secret here.")}</p>
          </div>
          <div class="set-card">
            <div class="set-row stacked">
              <div class="set-row-l"><div class="set-row-t">{t("Client ID")}</div></div>
              <div class="set-row-r">
                <input class="input mono" bind:value={gClientId} onblur={saveGoogleCreds} placeholder="…apps.googleusercontent.com" />
              </div>
            </div>
            <div class="set-row stacked">
              <div class="set-row-l"><div class="set-row-t">{t("Client secret")}</div></div>
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
          <div class="eyebrow">{t("Settings")}</div>
          <h2 class="set-title">{t("Experimental")}</h2>
          <p class="set-sub">{t("Early features that may be rough or change. Toggle one on to try it.")}</p>
        </header>

        <section class="set-group">
          <div class="set-group-h svc-h">
            <div>
              <h3 class="set-group-t">{t("University portal (Moodle)")}</h3>
              <p class="set-group-d">{t("Pull grades, assignments, deadlines and announcements from your Moodle portal into Cortex.")}</p>
            </div>
            <button class={"st-toggle" + (expMoodle ? " on" : "")} type="button" onclick={toggleExpMoodle} role="switch" aria-checked={expMoodle} aria-label={t("University portal (Moodle)")}><span class="st-knob"></span></button>
          </div>

          {#if expMoodle}
            <div class="set-card">
              <div class="set-row">
                <div class="set-row-l">
                  <div class="set-row-t">{t("Connection")}</div>
                  <div class="set-row-d">{mdStatus.configured ? t("Connected · last sync {when}", { when: fmtSyncTime(mdStatus.last_sync) }) : t("Not connected")}</div>
                </div>
                <span class="status-pill status-pill--{mdStatus.configured ? 'ready' : 'pending'}"><span class="dot"></span>{mdStatus.configured ? t("Connected") : t("Off")}</span>
              </div>

              <div class="set-row stacked">
                <div class="set-row-t">{t("Moodle site URL")}</div>
                <input class="input mono" bind:value={mdUrl} placeholder="https://moodle.your-school.edu" />
              </div>

              <div class="set-row">
                <div class="set-row-l">
                  <div class="set-row-t">{t("Sign-in method")}</div>
                  <div class="set-row-d">{t("Many institutions use SSO (Microsoft/SAML), so username/password often won't work — paste a web-services token instead.")}</div>
                </div>
                <div class="set-row-r" style="gap:6px">
                  <button class={"btn btn--sm" + (mdAuthMode === 'token' ? ' btn--primary' : ' btn--ghost')} type="button" onclick={() => (mdAuthMode = 'token')}>{t("Token")}</button>
                  <button class={"btn btn--sm" + (mdAuthMode === 'password' ? ' btn--primary' : ' btn--ghost')} type="button" onclick={() => (mdAuthMode = 'password')}>{t("Password")}</button>
                </div>
              </div>

              {#if mdAuthMode === 'password'}
                <div class="set-row stacked">
                  <div class="set-row-t">{t("Username")}</div>
                  <input class="input mono" bind:value={mdUser} placeholder={t("student number")} />
                </div>
                <div class="set-row stacked">
                  <div class="set-row-t">{t("Password")}</div>
                  <input class="input mono" type="password" bind:value={mdPass} placeholder="••••••••" />
                </div>
              {:else}
                <div class="set-row stacked">
                  <div class="set-row-t">{t("Web-services token")}</div>
                  <input class="input mono" bind:value={mdToken} placeholder={t("paste your Moodle token")} />
                  <div class="set-row-d">{t("Obtain it from the official Moodle app or a browser login. Stored locally; your password is never sent to Cortex.")}</div>
                </div>
              {/if}

              <div class="set-row">
                <div class="set-row-l"><div class="set-row-d">{t("Connect, then sync to pull your data.")}</div></div>
                <div class="set-row-r" style="gap:8px">
                  {#if mdStatus.configured}
                    <button class="btn btn--ghost btn--sm" type="button" onclick={mdDisconnect} disabled={mdBusy}>{t("Disconnect")}</button>
                    <button class="btn btn--primary btn--sm" type="button" onclick={mdSyncNow} disabled={mdBusy}><Icon name="refresh" size={12} /> {mdBusy ? t("Syncing…") : t("Sync now")}</button>
                  {:else}
                    <button class="btn btn--sm" type="button" onclick={mdLoginSso}>{t("Sign in via browser (SSO)")}</button>
                    <button class="btn btn--primary btn--sm" type="button" onclick={mdConnect} disabled={mdBusy || (mdAuthMode==='token' ? !mdToken.trim() : !mdUser.trim())}>{mdBusy ? t("Connecting…") : t("Connect")}</button>
                  {/if}
                </div>
              </div>

              {#if mdStatus.configured}
                <div class="set-row">
                  <div class="set-row-l">
                    <div class="set-row-t">{t("Link subjects to courses")}</div>
                    <div class="set-row-d">{t("Auto-match your Cortex subjects to Moodle courses by code/name.")}</div>
                  </div>
                  <div class="set-row-r"><button class="btn btn--sm" type="button" onclick={mdAutolink}>{t("Auto-link")}</button></div>
                </div>
              {/if}

              {#if mdData.courses.length}
                <div class="set-row stacked">
                  <div class="set-row-t">{t("Synced data")}</div>
                  <div class="md-stats">
                    <span class="md-stat"><b>{mdData.courses.length}</b> {t("courses")}</span>
                    <span class="md-stat"><b>{mdData.grades.length}</b> {t("grades")}</span>
                    <span class="md-stat"><b>{mdData.deadlines.length}</b> {t("deadlines")}</span>
                    <span class="md-stat"><b>{mdData.announcements.length}</b> {t("announcements")}</span>
                  </div>
                  {#if mdUpcoming.length}
                    <div class="md-up-h mono">{t("Upcoming deadlines")}</div>
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
          <div class="eyebrow">{t("Data & privacy")}</div>
          <h1 class="set-title">{t("Local-first by default")}</h1>
          <p class="set-sub">{t("Everything lives in a SQLite database on this machine. You own it.")}</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Storage")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Database")}</div>
                <div class="set-row-d">~/.cortex/cortex.db</div>
              </div>
              <div class="set-row-r">
                <span class="mono faint">
                  {#if stats}
                    {t("{n} · {n2} subjects · {n3} sources", { n: fmtBytes(stats.db_bytes), n2: stats.subjects, n3: stats.sources })}
                  {:else}
                    …
                  {/if}
                </span>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Vector index")}</div>
                <div class="set-row-d">{t("Local embeddings for retrieval")}</div>
              </div>
              <div class="set-row-r">
                <span class="mono faint">{stats ? t("{n} chunks", { n: stats.chunks }) : "…"}</span>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Offline mode")}</div>
                <div class="set-row-d">{t("Block all network calls; Ollama only.")}</div>
              </div>
              <div class="set-row-r">
                <button type="button" class={"st-toggle" + (offlineMode ? " on" : "")} onclick={toggleOffline} role="switch" aria-checked={offlineMode} aria-label={t("Offline mode")}><span class="st-knob"></span></button>
              </div>
            </div>
          </div>
        </section>

        {#if archivedSubjects.length}
          <section class="set-group">
            <div class="set-group-h">
              <h3 class="set-group-t">{t("Archived subjects")}</h3>
              <p class="set-group-d">{t("Hidden from the app, kept for storage. Restore any time.")}</p>
            </div>
            <div class="set-card">
              {#each archivedSubjects as s (s.id)}
                <div class="set-row">
                  <div class="set-row-l">
                    <div class="set-row-t">
                      <span style="color:{app.subjectColor(s)}">{s.glyph}</span> {s.name}
                    </div>
                    <div class="set-row-d">
                      {s.code ? s.code + " · " : ""}{t("{n} sources", { n: s.sourceCount })}
                    </div>
                  </div>
                  <div class="set-row-r">
                    <button class="btn" onclick={() => restoreSubject(s.id)}>{t("Restore")}</button>
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {/if}

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">{t("Manage")}</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Export everything")}</div>
                <div class="set-row-d">{t("Subjects, sources, cheatsheets → a portable archive.")}</div>
              </div>
              <div class="set-row-r">
                <button class="btn" onclick={exportData}>
                  <Icon name="external" size={12} /> {t("Export")}
                </button>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Optimize storage")}</div>
                <div class="set-row-d">{t("Reclaim unused disk space (VACUUM). Safe.")}</div>
              </div>
              <div class="set-row-r">
                <button class="btn" onclick={clearCaches}>{t("Optimize")}</button>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">{t("Delete all data")}</div>
                <div class="set-row-d">{t("Irreversible. Wipes the local database.")}</div>
              </div>
              <div class="set-row-r">
                <button class="btn btn--danger" onclick={deleteEverything}>{t("Delete…")}</button>
              </div>
            </div>
          </div>
        </section>
      </div>

    <!-- ===== ABOUT ===== -->
    {:else if tab === "about"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">{t("About")}</div>
          <h1 class="set-title">Cortex</h1>
          <p class="set-sub">{t("A desktop study OS for serious students.")}</p>
        </header>

        <div class="set-card">
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">{t("Version")}</div></div>
            <div class="set-row-r"><span class="mono faint">{appVersion || "…"}</span></div>
          </div>
          <div class="set-row">
            <div class="set-row-l">
              <div class="set-row-t">{t("Updates")}</div>
              <div class="set-row-d">{t("Check GitHub for a newer release and install it.")}</div>
            </div>
            <div class="set-row-r">
              <button class="btn" onclick={() => app.checkForUpdates()} disabled={app.updateChecking}>
                <Icon name="refresh" size={12} /> {app.updateChecking ? t("Checking…") : t("Check for updates")}
              </button>
            </div>
          </div>
          <div class="set-row">
            <div class="set-row-l">
              <div class="set-row-t">{t("Support Cortex")}</div>
              <div class="set-row-d">{t("Cortex is built by one student. If it saves you time, a coffee keeps the updates coming.")}</div>
            </div>
            <div class="set-row-r">
              <button class="btn" onclick={() => api.openExternal("https://ko-fi.com/aidanmcconnon")}>
                <Icon name="heart" size={12} color="var(--err)" /> {t("Support me on Ko-fi")}
              </button>
            </div>
          </div>
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">{t("Engine")}</div></div>
            <div class="set-row-r"><span class="mono faint">Rust · Tauri · Svelte</span></div>
          </div>
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">{t("Theme source")}</div></div>
            <div class="set-row-r">
              <span class="mono faint">
                Omarchy · {THEME_LABELS[app.theme]}
              </span>
            </div>
          </div>
          <div class="set-row">
            <div class="set-row-l"><div class="set-row-t">{t("License")}</div></div>
            <div class="set-row-r"><span class="mono faint">Source-available · BYOK</span></div>
          </div>
        </div>

        <div class="set-note mono">
          <Icon name="diamond" size={11} color="var(--accent)" />
          {t("Offline-first. Your notes never leave this machine unless you choose a cloud model.")}
        </div>
      </div>
    {/if}

  </div>
</div>
