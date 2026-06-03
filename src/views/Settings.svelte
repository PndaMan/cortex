<script lang="ts">
  import { app, THEMES } from "../lib/store.svelte";
  import type { Theme } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import { stations } from "../lib/mock";

  // ---- tab navigation ----
  const TABS = [
    { id: "profile",    label: "Profile",       icon: "diamond" },
    { id: "models",     label: "Models",        icon: "bolt" },
    { id: "keys",       label: "API keys",      icon: "lock" },
    { id: "appearance", label: "Appearance",    icon: "grid" },
    { id: "keybinds",   label: "Keybinds",      icon: "cmd" },
    { id: "homelab",    label: "Homelab",       icon: "globe" },
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
    { id: "openrouter", label: "OpenRouter",          models: ["google/gemini-2.5-flash","anthropic/claude-sonnet-4.5","openai/gpt-5-mini","meta-llama/llama-3.3-70b-instruct","deepseek/deepseek-chat"] },
    { id: "gemini",     label: "Gemini",              models: ["gemini-2.5-pro","gemini-2.5-flash","gemini-2.0-flash"] },
    { id: "claude",     label: "Claude",              models: ["claude-opus-4.5","claude-sonnet-4.5","claude-haiku-4.5"] },
    { id: "openai",     label: "OpenAI",              models: ["gpt-5.1","gpt-5-mini","o4-mini"] },
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

  // ---- models state ----
  type TaskAssign = { provider: string; model: string; budget: string };
  let assign = $state<Record<TaskId, TaskAssign>>({
    chat:       { provider: "claude",  model: "claude-sonnet-4.5",  budget: "8000" },
    cheatsheet: { provider: "gemini",  model: "gemini-2.5-pro",     budget: "32000" },
    audio:      { provider: "gemini",  model: "gemini-2.5-flash",   budget: "16000" },
    quiz:       { provider: "openai",  model: "gpt-5-mini",         budget: "8000" },
    flashcard:  { provider: "claude",  model: "claude-haiku-4.5",   budget: "6000" },
    embedding:  { provider: "gemini",  model: "text-embedding-004", budget: "—" },
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
    { id: "osaka-jade",  n: "Osaka Jade",      c: "#2dd5b7", b: "#111c18" },
    { id: "tokyo-night", n: "Tokyo Night",      c: "#7aa2f7", b: "#1a1b26" },
    { id: "catppuccin",  n: "Catppuccin Mocha", c: "#94e2d5", b: "#1e1e2e" },
  ];
  let followOmarchy = $state(true);
  let readFont      = $state("serif");
  let density       = $state("regular");

  $effect(() => { document.documentElement.setAttribute("data-read", readFont); });
  $effect(() => { document.documentElement.setAttribute("data-density", density === "compact" ? "compact" : "regular"); });

  // persist appearance on change
  $effect(() => {
    // track both values
    const rf = readFont;
    const d  = density;
    api.setSettings({ reading_font: rf, density: d }).catch(() => {});
  });

  // ---- keybinds state ----
  type Bind = { id: string; label: string; keys: string };
  let binds = $state<Bind[]>([
    { id: "cmd",     label: "Command palette",   keys: ":" },
    { id: "leader",  label: "Space leader",       keys: "Space" },
    { id: "search",  label: "Search in page",     keys: "/" },
    { id: "down",    label: "Move down",           keys: "j" },
    { id: "up",      label: "Move up",             keys: "k" },
    { id: "chat",    label: "Toggle chat",         keys: "c" },
    { id: "insert",  label: "Insert mode",         keys: "i" },
    { id: "newsubj", label: "New subject",         keys: "n" },
    { id: "panel",   label: "Toggle sources panel",keys: "]" },
    { id: "dash",    label: "Go to dashboard",     keys: "Space g" },
    { id: "record",  label: "Record lecture",      keys: "Space r" },
    { id: "addsrc",  label: "Add source",          keys: "Space s" },
  ]);
  let listening = $state<string | null>(null);

  $effect(() => {
    if (!listening) return;
    (window as any).__cortexModalOpen = true;
    function onKey(e: KeyboardEvent) {
      e.preventDefault();
      const k = e.key === " " ? "Space" : e.key === "Escape" ? null : e.key;
      if (k) {
        const lid = listening;
        binds = binds.map((b) => b.id === lid ? { ...b, keys: k } : b);
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

  // ---- homelab state ----
  let homelab    = $state(true);
  let endpoint   = $state("http://homelab.local:11434");
  let jobs       = $state({ whisper: true, llm: false, backups: true });
  let testState  = $state<null | "testing" | "ok">(null);

  function testConnection() {
    testState = "testing";
    setTimeout(() => (testState = "ok"), 1100);
  }

  // persist homelab on change
  $effect(() => {
    const h = homelab;
    const ep = endpoint;
    api.setSettings({ homelab_enabled: String(h), ollama_url: ep }).catch(() => {});
  });

  // ---- audio state ----
  let autoplay = $state(false);
  let station  = $state("lofi");
  let voiceA   = $state("maya");
  let voiceB   = $state("theo");

  // persist audio on change
  $effect(() => {
    const st = station;
    const ap = autoplay;
    api.setSettings({ default_station: st, autoplay: String(ap) }).catch(() => {});
  });

  // ---- mount: hydrate from backend ----
  $effect(() => {
    api.getAllSettings().then((s) => {
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
      }
      if (s.embed_provider) {
        assign = { ...assign, embedding: { ...assign.embedding, provider: s.embed_provider } };
      }

      // Homelab
      if (s.homelab_enabled !== undefined) homelab  = s.homelab_enabled === "true";
      if (s.ollama_url)                    endpoint = s.ollama_url;

      // Appearance
      if (s.reading_font) readFont = s.reading_font;
      if (s.density)      density  = s.density;

      // Profile
      if (s.profile_name)      name     = s.profile_name;
      if (s.profile_pronouns)  pronouns = s.profile_pronouns;
      if (s.profile_level)     level    = s.profile_level;
      if (s.profile_field)     field    = s.profile_field;
      if (s.profile_about)     about    = s.profile_about;
      if (s.profile_style)     style    = s.profile_style;
    }).catch(() => {});
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
                  oninput={(e) => setTask(t.id, { budget: (e.target as HTMLInputElement).value })}
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
                        {isSet ? "connected" : "not set"}
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
                <div class="set-row-t">Follow Omarchy</div>
                <div class="set-row-d">Match the desktop palette automatically.</div>
              </div>
              <div class="set-row-r">
                <button
                  type="button"
                  class={"st-toggle" + (followOmarchy ? " on" : "")}
                  onclick={() => (followOmarchy = !followOmarchy)}
                  role="switch"
                  aria-checked={followOmarchy}
                  aria-label="follow omarchy"
                >
                  <span class="st-knob"></span>
                </button>
              </div>
            </div>
            <div class="set-themes">
              {#each THEME_OPTS as t}
                <button
                  class={"set-theme" + (app.theme === t.id ? " on" : "")}
                  onclick={() => app.setTheme(t.id)}
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
                  <button type="button" class={"seg-opt" + (opt.id === "helix" ? " on" : "")} onclick={() => {}}>{opt.label}</button>
                {/each}
              </div>
            </div>
          </div>
        </div>

        <div class="set-card set-binds">
          {#each binds as b}
            <div class="bind-row">
              <span class="bind-label">{b.label}</span>
              <button
                class={"bind-keys" + (listening === b.id ? " listening" : "")}
                onclick={() => (listening = b.id)}
              >
                {#if listening === b.id}
                  <span class="mono faint">press a key…</span>
                {:else}
                  {#each b.keys.split(" ") as k, i}
                    <span class="kbd">{k}</span>
                  {/each}
                {/if}
              </button>
            </div>
          {/each}
        </div>

        <div class="set-foot-actions">
          <button class="btn btn--ghost" onclick={() => app.pushToast({ kind: "info", title: "Reset", body: "Bindings restored to Helix preset." })}>
            Reset to preset
          </button>
        </div>
      </div>

    <!-- ===== HOMELAB ===== -->
    {:else if tab === "homelab"}
      <div class="set-pane">
        <header class="set-head">
          <div class="eyebrow">Homelab</div>
          <h1 class="read set-title">Offload the heavy jobs</h1>
          <p class="set-sub">Send Whisper transcription, large-model synthesis and backups to a machine on your network. Cortex stays fully local without it.</p>
        </header>

        <section class="set-group">
          <div class="set-group-h"><h3 class="set-group-t">Connection</h3></div>
          <div class="set-card">
            <div class="set-row">
              <div class="set-row-l"><div class="set-row-t">Use homelab for heavy jobs</div></div>
              <div class="set-row-r">
                <button
                  type="button"
                  class={"st-toggle" + (homelab ? " on" : "")}
                  onclick={() => (homelab = !homelab)}
                  role="switch"
                  aria-checked={homelab}
                  aria-label="use homelab"
                >
                  <span class="st-knob"></span>
                </button>
              </div>
            </div>
            {#if homelab}
              <div class="set-row stacked">
                <div class="set-row-l"><div class="set-row-t">Endpoint</div></div>
                <div class="set-row-r">
                  <div class="row-inline">
                    <input class="input mono" bind:value={endpoint} />
                    <button class="btn" onclick={testConnection}>
                      <Icon name="refresh" size={12} /> Test
                    </button>
                  </div>
                  {#if testState === "testing"}
                    <div class="set-test mono faint">
                      <span class="is-spin" style="width:11px;height:11px;display:inline-block;vertical-align:-1px"></span>
                      Pinging…
                    </div>
                  {:else if testState === "ok"}
                    <div class="set-test mono" style="color:var(--ok)">
                      <Icon name="check" size={12} /> Reachable · 8ms · Ollama 0.5.4, Whisper ready
                    </div>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        </section>

        {#if homelab}
          <section class="set-group">
            <div class="set-group-h"><h3 class="set-group-t">Per-job routing</h3></div>
            <div class="set-card">
              <div class="set-row">
                <div class="set-row-l">
                  <div class="set-row-t">Whisper transcription</div>
                  <div class="set-row-d">Lecture audio → text.</div>
                </div>
                <div class="set-row-r">
                  <button type="button" class={"st-toggle" + (jobs.whisper ? " on" : "")} onclick={() => (jobs = { ...jobs, whisper: !jobs.whisper })} role="switch" aria-checked={jobs.whisper} aria-label="whisper"><span class="st-knob"></span></button>
                </div>
              </div>
              <div class="set-row">
                <div class="set-row-l">
                  <div class="set-row-t">LLM synthesis</div>
                  <div class="set-row-d">Cheatsheets, overviews, quizzes.</div>
                </div>
                <div class="set-row-r">
                  <button type="button" class={"st-toggle" + (jobs.llm ? " on" : "")} onclick={() => (jobs = { ...jobs, llm: !jobs.llm })} role="switch" aria-checked={jobs.llm} aria-label="llm"><span class="st-knob"></span></button>
                </div>
              </div>
              <div class="set-row">
                <div class="set-row-l">
                  <div class="set-row-t">Backups</div>
                  <div class="set-row-d">Nightly SQLite + vector index snapshot.</div>
                </div>
                <div class="set-row-r">
                  <button type="button" class={"st-toggle" + (jobs.backups ? " on" : "")} onclick={() => (jobs = { ...jobs, backups: !jobs.backups })} role="switch" aria-checked={jobs.backups} aria-label="backups"><span class="st-knob"></span></button>
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
                  onChange={(v) => (station = v)}
                  options={stations.map((s) => ({ id: s.id, label: s.name }))}
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
              <div class="set-row-r"><span class="mono faint">412 MB · 4 subjects · 41 sources</span></div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Vector index</div>
                <div class="set-row-d">Local embeddings for retrieval</div>
              </div>
              <div class="set-row-r"><span class="mono faint">88 MB</span></div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Offline mode</div>
                <div class="set-row-d">Block all network calls; Ollama only.</div>
              </div>
              <div class="set-row-r">
                <button type="button" class="st-toggle" onclick={() => {}} role="switch" aria-checked={false} aria-label="offline"><span class="st-knob"></span></button>
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
                <button class="btn" onclick={() => app.pushToast({ kind: "info", title: "Exporting", body: "Building cortex-export.zip…" })}>
                  <Icon name="external" size={12} /> Export
                </button>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Clear caches</div>
                <div class="set-row-d">Transcription & generation caches (safe).</div>
              </div>
              <div class="set-row-r">
                <button class="btn" onclick={() => app.pushToast({ kind: "info", title: "Caches cleared" })}>Clear</button>
              </div>
            </div>
            <div class="set-row">
              <div class="set-row-l">
                <div class="set-row-t">Delete all data</div>
                <div class="set-row-d">Irreversible. Wipes the local database.</div>
              </div>
              <div class="set-row-r">
                <button class="btn btn--danger" onclick={() => app.pushToast({ kind: "warning", title: "Are you sure?", body: "This cannot be undone." })}>Delete…</button>
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
                Omarchy · {app.theme === "osaka-jade" ? "Osaka Jade" : app.theme === "tokyo-night" ? "Tokyo Night" : "Catppuccin Mocha"}
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
