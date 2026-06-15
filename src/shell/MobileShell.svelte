<script lang="ts">
  // Mobile shell (see docs/MOBILE_PORT.md §4). Renders ONLY when isMobile, so it
  // never affects the desktop app. It reuses every existing view verbatim — the
  // store (app.*) is the shared source of truth — and only swaps the chrome:
  // a .page-title header (continuity, §3.4), a 5-item bottom tab bar, a Capture
  // bottom-sheet, an Ask→chat full sheet, and the shared self-managed overlays.
  import { app } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";

  // Views (reused as-is). Heavy ones are lazy-loaded on first visit, like App.svelte.
  import Dashboard from "../views/Dashboard.svelte";
  import SubjectView from "../views/SubjectView.svelte";
  import SourceViewer from "../views/SourceViewer.svelte";
  import AddSource from "../views/AddSource.svelte";
  import AddSubject from "../views/AddSubject.svelte";
  import GenerateMaterial from "../views/GenerateMaterial.svelte";
  import NotesView from "../views/NotesView.svelte";

  // Shared overlays — all self-managed via app state (no props), exactly as in App.svelte.
  import ChatPanel from "../components/ChatPanel.svelte";
  import ToastStack from "../components/ToastStack.svelte";
  import Dialog from "../components/Dialog.svelte";
  import EditModal from "../components/EditModal.svelte";
  import GlobalSearch from "../components/GlobalSearch.svelte";
  import NotificationCenter from "../components/NotificationCenter.svelte";
  import NotificationDetail from "../components/NotificationDetail.svelte";
  import DiffModal from "../components/DiffModal.svelte";
  import SourceMetaModal from "../components/SourceMetaModal.svelte";
  import PomodoroPanel from "../components/PomodoroPanel.svelte";

  type Screen = "router" | "more";
  // "more" is a mobile-only list screen with no desktop app.view; every other
  // destination is just an app.view rendered by the router below.
  let screen = $state<Screen>("router");

  // Lazy heavy views (mirrors App.svelte's code-split + prefetch idea).
  let SettingsView = $state<any>(null);
  let CalendarViewC = $state<any>(null);
  let AnalyticsViewC = $state<any>(null);
  let ExamViewC = $state<any>(null);
  const loadSettings = () =>
    SettingsView ?? import("../views/Settings.svelte").then((m) => (SettingsView = m.default));
  const loadCalendar = () =>
    CalendarViewC ?? import("../views/CalendarView.svelte").then((m) => (CalendarViewC = m.default));
  const loadAnalytics = () =>
    AnalyticsViewC ?? import("../views/AnalyticsView.svelte").then((m) => (AnalyticsViewC = m.default));
  const loadExam = () =>
    ExamViewC ?? import("../views/ExamView.svelte").then((m) => (ExamViewC = m.default));
  // Lecture recording/transcription is intentionally NOT available on mobile: phones
  // can't run Whisper locally and the homelab /ingest offload isn't built yet.
  $effect(() => {
    if (app.view === "settings") void loadSettings();
    else if (app.view === "calendar") void loadCalendar();
    else if (app.view === "analytics") void loadAnalytics();
    else if (app.view === "exam") void loadExam();
  });

  // If we land on the subject view with nothing active (fresh install), show Home.
  $effect(() => {
    if (app.view === "subject" && !app.activeSubject) app.setView("dashboard");
  });

  // ── navigation ──
  function go(v: Parameters<typeof app.setView>[0]) {
    screen = "router";
    app.setView(v);
  }
  // A "deep" view sits under a subject/source — show a back chevron for it.
  const isDeep = $derived(
    ["source", "add-source", "add-subject", "gen-material", "exam"].includes(app.view)
  );
  function back() {
    if (app.view === "source") {
      app.closeSource();
      return;
    }
    app.setView(app.activeSubject ? "subject" : "dashboard");
    screen = "router";
  }

  const titleText = $derived.by(() => {
    if (screen === "more") return "More";
    switch (app.view) {
      case "dashboard": return "Home";
      case "subject": return app.activeSubject?.name ?? "Subject";
      case "source": return app.activeSource?.name ?? "Source";
      case "add-source": return "Add source";
      case "add-subject": return "New subject";
      case "gen-material": return "Generate";
      case "notes": return "Notes";
      case "calendar": return "Calendar";
      case "analytics": return "Insights";
      case "exam": return "Exam";
      case "settings": return "Settings";
      default: return "Cortex";
    }
  });

  type Tab = "home" | "add" | "calendar" | "more" | "none";
  const tab = $derived.by<Tab>(() => {
    if (screen === "more") return "more";
    const v = app.view;
    if (v === "dashboard") return "home";
    if (v === "calendar") return "calendar";
    if (v === "add-source") return "add";
    if (["analytics", "exam", "notes", "settings", "add-subject"].includes(v)) return "more";
    return "none"; // subject / source / gen-material → deep content, no tab lit
  });

  // Ask FAB: only where a chat scope makes sense, and only when chat is closed.
  const showAsk = $derived(
    screen === "router" && ["subject", "source", "notes"].includes(app.view) && !app.chatOpen
  );
</script>

<div class="m-shell">
  <header class="m-header">
    {#if isDeep}
      <button class="m-iconbtn m-back" onclick={back} aria-label="Back">
        <Icon name="chevron" size={18} />
      </button>
    {/if}
    <h1 class="page-title m-title">{titleText}</h1>
    <span class="m-spacer"></span>
    <button class="m-iconbtn m-bell" onclick={() => app.toggleNotifications()} aria-label="Notifications">
      <Icon name="bell" size={18} />
      {#if app.unreadCount > 0}
        <span class="m-badge">{app.unreadCount > 99 ? "99+" : app.unreadCount}</span>
      {/if}
    </button>
  </header>

  <main class="m-main">
    {#if screen === "more"}
      <div class="m-list">
        <button class="m-row" onclick={() => go("notes")}><Icon name="reader" size={18} /><span class="m-row-l">Notes</span><Icon name="chevron" size={14} /></button>
        <button class="m-row" onclick={() => go("analytics")}><Icon name="chart" size={18} /><span class="m-row-l">Insights</span><Icon name="chevron" size={14} /></button>
        <button class="m-row" onclick={() => go("exam")}><Icon name="cards" size={18} /><span class="m-row-l">Exam mode</span><Icon name="chevron" size={14} /></button>
        <button class="m-row" onclick={() => go("add-subject")}><Icon name="plus" size={18} /><span class="m-row-l">New subject</span><Icon name="chevron" size={14} /></button>
        <button class="m-row" onclick={() => go("settings")}><Icon name="settings" size={18} /><span class="m-row-l">Settings</span><Icon name="chevron" size={14} /></button>
      </div>
    {:else}
      <!-- router: reuse the existing views verbatim -->
      {#if app.view === "dashboard"}
        <Dashboard />
      {:else if app.view === "subject"}
        <SubjectView />
      {:else if app.view === "source"}
        <SourceViewer />
      {:else if app.view === "add-source"}
        <AddSource />
      {:else if app.view === "add-subject"}
        <AddSubject />
      {:else if app.view === "gen-material"}
        <GenerateMaterial />
      {:else if app.view === "notes"}
        <NotesView />
      {:else if app.view === "calendar"}
        {#if CalendarViewC}{@const C = CalendarViewC}<C />{/if}
      {:else if app.view === "analytics"}
        {#if AnalyticsViewC}{@const C = AnalyticsViewC}<C />{/if}
      {:else if app.view === "exam"}
        {#if ExamViewC}{@const C = ExamViewC}<C />{/if}
      {:else if app.view === "settings"}
        {#if SettingsView}{@const C = SettingsView}<C />{/if}
      {/if}
    {/if}
  </main>

  {#if showAsk}
    <button class="m-ask" onclick={() => (app.chatOpen = true)} aria-label="Ask Cortex">
      <Icon name="chat" size={18} /> Ask
    </button>
  {/if}

  <nav class="m-tabs">
    <button class="m-tab" class:on={tab === "home"} onclick={() => go("dashboard")}>
      <Icon name="home" size={20} /><span>Home</span>
    </button>
    <button class="m-tab" class:on={tab === "add"} onclick={() => go("add-source")}>
      <Icon name="doc" size={20} /><span>Add</span>
    </button>
    <button class="m-tab m-tab--search" onclick={() => (app.searchOpen = true)} aria-label="Search">
      <span class="m-search"><Icon name="search" size={22} /></span>
    </button>
    <button class="m-tab" class:on={tab === "calendar"} onclick={() => go("calendar")}>
      <Icon name="calendar" size={20} /><span>Calendar</span>
    </button>
    <button class="m-tab" class:on={tab === "more"} onclick={() => (screen = "more")}>
      <Icon name="grid" size={20} /><span>More</span>
    </button>
  </nav>
</div>

<!-- Chat as a full-screen sheet -->
{#if app.chatOpen}
  <div class="m-sheet m-sheet--full" role="dialog" aria-label="Chat">
    <ChatPanel
      onClose={() => (app.chatOpen = false)}
      onFullscreen={() => { app.chatOpen = false; app.setView("subject"); app.subjectTab = "chats"; }}
    />
  </div>
{/if}

<!-- shared overlays (self-managed via app state) -->
<ToastStack />
<Dialog />
<EditModal />
<GlobalSearch />
<NotificationCenter />
<NotificationDetail />
<DiffModal />
<SourceMetaModal />
<PomodoroPanel />

<style>
  /* All chrome below is styled ONLY from existing design tokens — no new colours
     (continuity rule, docs/MOBILE_PORT.md §3.4). */
  .m-shell {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--fg);
    overflow: hidden;
  }

  .m-header {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 52px;
    padding: 0 10px;
    padding-top: env(safe-area-inset-top, 0px);
    box-sizing: content-box;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  /* Reuse the shared .page-title look (mono/treatment), just sized for the bar. */
  .m-title {
    margin: 0;
    font-size: var(--t-md);
    line-height: 1;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m-spacer { flex: 0 0 auto; }
  .m-iconbtn {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border: none;
    background: none;
    color: var(--fg-muted);
    border-radius: var(--rad-2);
    cursor: pointer;
  }
  .m-iconbtn:active { background: var(--surface-2); color: var(--fg-bright); }
  .m-bell { position: relative; }
  .m-badge {
    position: absolute;
    top: 1px;
    right: 1px;
    min-width: 15px;
    height: 15px;
    padding: 0 4px;
    background: var(--accent);
    color: var(--accent-fg);
    font-size: 9.5px;
    font-weight: 700;
    line-height: 15px;
    text-align: center;
    border-radius: 999px;
    font-family: var(--font-mono);
  }

  .m-main {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
  }

  /* List screen (More) */
  .m-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
  }
  .m-row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 56px;
    padding: 14px 16px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--fg);
    border-radius: var(--rad-4);
    font-size: var(--t-md);
    text-align: left;
    cursor: pointer;
  }
  .m-row-l { flex: 1; min-width: 0; }
  .m-row :global(svg:last-child) { color: var(--fg-faint); flex: none; }
  .m-row:active { background: var(--surface-2); }

  /* Bottom tab bar */
  .m-tabs {
    flex: none;
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    align-items: center;
    border-top: 1px solid var(--border);
    background: var(--surface);
    padding-bottom: env(safe-area-inset-bottom, 0px);
  }
  .m-tab {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    padding: 9px 0 7px;
    border: none;
    background: none;
    color: var(--fg-faint);
    font-size: 10px;
    font-family: var(--font-mono);
    cursor: pointer;
  }
  .m-tab.on { color: var(--accent); }
  .m-tab:active { color: var(--fg-bright); }
  /* Elevated center search button */
  .m-tab--search { padding: 0; }
  .m-search {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 46px;
    height: 46px;
    margin-top: -14px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--accent-fg);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.35);
  }

  /* Ask FAB */
  .m-ask {
    position: fixed;
    right: 14px;
    bottom: calc(64px + env(safe-area-inset-bottom, 0px));
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 10px 16px;
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-pill, 999px);
    background: var(--surface-2);
    color: var(--fg-bright);
    font-size: var(--t-sm);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.3);
    cursor: pointer;
    z-index: 30;
  }

  /* Sheets */
  .m-sheet {
    position: fixed;
    z-index: 61;
    background: var(--bg);
    color: var(--fg);
  }
  .m-sheet--full {
    inset: 0;
    display: flex;
    flex-direction: column;
    padding-top: env(safe-area-inset-top, 0px);
    padding-bottom: env(safe-area-inset-bottom, 0px);
  }
  .m-sheet--full :global(.chat-panel) {
    flex: 1;
    min-height: 0;
  }
</style>
