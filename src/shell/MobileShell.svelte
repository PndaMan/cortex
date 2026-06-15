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

  type Screen = "router" | "subjects" | "more";
  // "subjects"/"more" are mobile-only list screens with no desktop app.view; every
  // other destination is just an app.view rendered by the router below.
  let screen = $state<Screen>("router");
  let captureOpen = $state(false);

  // Lazy heavy views (mirrors App.svelte's code-split + prefetch idea).
  let SettingsView = $state<any>(null);
  let RecorderView = $state<any>(null);
  let CalendarViewC = $state<any>(null);
  let AnalyticsViewC = $state<any>(null);
  let ExamViewC = $state<any>(null);
  const loadSettings = () =>
    SettingsView ?? import("../views/Settings.svelte").then((m) => (SettingsView = m.default));
  const loadRecorder = () =>
    RecorderView ?? import("../views/Recorder.svelte").then((m) => (RecorderView = m.default));
  const loadCalendar = () =>
    CalendarViewC ?? import("../views/CalendarView.svelte").then((m) => (CalendarViewC = m.default));
  const loadAnalytics = () =>
    AnalyticsViewC ?? import("../views/AnalyticsView.svelte").then((m) => (AnalyticsViewC = m.default));
  const loadExam = () =>
    ExamViewC ?? import("../views/ExamView.svelte").then((m) => (ExamViewC = m.default));
  $effect(() => {
    if (app.view === "settings") void loadSettings();
    else if (app.view === "recorder") void loadRecorder();
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
  function pickSubject(id: string) {
    app.openSubject(id);
    screen = "router";
  }
  function openCapture() {
    captureOpen = true;
  }
  // A "deep" view sits under a subject/source — show a back chevron for it.
  const isDeep = $derived(
    ["source", "add-source", "add-subject", "gen-material", "recorder", "exam"].includes(app.view)
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
    if (screen === "subjects") return "Subjects";
    if (screen === "more") return "More";
    switch (app.view) {
      case "dashboard": return "Home";
      case "subject": return app.activeSubject?.name ?? "Subject";
      case "source": return app.activeSource?.name ?? "Source";
      case "add-source": return "Add source";
      case "add-subject": return "New subject";
      case "recorder": return "Record";
      case "gen-material": return "Generate";
      case "notes": return "Notes";
      case "calendar": return "Calendar";
      case "analytics": return "Insights";
      case "exam": return "Exam";
      case "settings": return "Settings";
      default: return "Cortex";
    }
  });

  type Tab = "home" | "subjects" | "calendar" | "more" | "none";
  const tab = $derived.by<Tab>(() => {
    if (screen === "subjects") return "subjects";
    if (screen === "more") return "more";
    const v = app.view;
    if (v === "dashboard") return "home";
    if (v === "calendar") return "calendar";
    if (["subject", "source", "add-source", "gen-material"].includes(v)) return "subjects";
    if (["analytics", "exam", "notes", "settings", "add-subject", "recorder"].includes(v)) return "more";
    return "none";
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
    {#if screen === "subjects"}
      <div class="m-list">
        {#each app.subjects as s (s.id)}
          <button class="m-subj" onclick={() => pickSubject(s.id)}>
            <span class="m-subj-glyph" style:color={app.subjectColor(s)}>{s.glyph || "◆"}</span>
            <span class="m-subj-body">
              <span class="m-subj-name">{s.name}</span>
              <span class="m-subj-meta">{s.sourceCount} sources · {s.topics.length} topics</span>
            </span>
            <Icon name="chevron" size={14} />
          </button>
        {/each}
        {#if app.subjects.length === 0}
          <div class="m-empty">No subjects yet — add one to get started.</div>
        {/if}
        <button class="m-add" onclick={() => go("add-subject")}>
          <Icon name="plus" size={14} /> New subject
        </button>
      </div>
    {:else if screen === "more"}
      <div class="m-list">
        <button class="m-row" onclick={() => (app.searchOpen = true)}><Icon name="search" size={16} /> Search</button>
        <button class="m-row" onclick={() => go("analytics")}><Icon name="chart" size={16} /> Insights</button>
        <button class="m-row" onclick={() => go("exam")}><Icon name="cards" size={16} /> Exam mode</button>
        <button class="m-row" onclick={() => go("notes")}><Icon name="reader" size={16} /> Notes</button>
        <button class="m-row" onclick={() => go("settings")}><Icon name="settings" size={16} /> Settings</button>
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
      {:else if app.view === "recorder"}
        {#if RecorderView}{@const C = RecorderView}<C />{/if}
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
    <button class="m-tab" class:on={tab === "subjects"} onclick={() => (screen = "subjects")}>
      <Icon name="book" size={20} /><span>Subjects</span>
    </button>
    <button class="m-tab m-tab--cap" onclick={openCapture} aria-label="Capture">
      <span class="m-cap"><Icon name="plus" size={22} /></span>
    </button>
    <button class="m-tab" class:on={tab === "calendar"} onclick={() => go("calendar")}>
      <Icon name="calendar" size={20} /><span>Calendar</span>
    </button>
    <button class="m-tab" class:on={tab === "more"} onclick={() => (screen = "more")}>
      <Icon name="grid" size={20} /><span>More</span>
    </button>
  </nav>
</div>

<!-- Capture bottom sheet -->
{#if captureOpen}
  <button class="m-backdrop" aria-label="Close" onclick={() => (captureOpen = false)}></button>
  <div class="m-sheet m-sheet--bottom" role="dialog" aria-label="Capture">
    <div class="m-grip"></div>
    <button class="m-cap-row" onclick={() => { captureOpen = false; go("recorder"); }}>
      <Icon name="mic" size={18} /> Record lecture
    </button>
    <button class="m-cap-row" onclick={() => { captureOpen = false; go("add-source"); }}>
      <Icon name="plus" size={18} /> Add source
    </button>
    <button class="m-cap-row" onclick={() => { captureOpen = false; go("notes"); }}>
      <Icon name="reader" size={18} /> New note
    </button>
    <button class="m-cap-row" onclick={() => { captureOpen = false; go("add-source"); }}>
      <Icon name="camera" size={18} /> Snap a page (OCR)
    </button>
  </div>
{/if}

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

  /* List screens (Subjects / More) */
  .m-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
  }
  .m-subj,
  .m-row,
  .m-add,
  .m-cap-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 14px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--fg);
    border-radius: var(--rad-3);
    font-size: var(--t-sm);
    text-align: left;
    cursor: pointer;
  }
  .m-subj:active,
  .m-row:active,
  .m-add:active,
  .m-cap-row:active { background: var(--surface-2); }
  .m-subj-glyph { flex: none; font-size: 18px; }
  .m-subj-body { display: flex; flex-direction: column; gap: 2px; flex: 1; min-width: 0; }
  .m-subj-name { color: var(--fg-bright); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .m-subj-meta { color: var(--fg-faint); font-size: var(--t-2xs); }
  .m-add { justify-content: center; color: var(--fg-muted); background: var(--surface-2); }
  .m-empty { color: var(--fg-faint); padding: 24px 12px; text-align: center; }

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
  /* Elevated center capture button */
  .m-tab--cap { padding: 0; }
  .m-cap {
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
  .m-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    border: none;
    padding: 0;
    cursor: default;
    z-index: 60;
  }
  .m-sheet {
    position: fixed;
    z-index: 61;
    background: var(--bg);
    color: var(--fg);
  }
  .m-sheet--bottom {
    left: 0;
    right: 0;
    bottom: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 12px calc(16px + env(safe-area-inset-bottom, 0px));
    border-top: 1px solid var(--border);
    border-radius: var(--rad-4) var(--rad-4) 0 0;
  }
  .m-grip {
    width: 36px;
    height: 4px;
    border-radius: 999px;
    background: var(--border-strong);
    margin: 4px auto 8px;
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
