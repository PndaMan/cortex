<script lang="ts">
  // Mobile shell (see docs/MOBILE_PORT.md §4). Renders ONLY when isMobile, so it
  // never affects the desktop app. It reuses every existing view verbatim — the
  // store (app.*) is the shared source of truth — and only swaps the chrome:
  // a slim "Cortex" header with a hamburger that opens a full nav drawer (every
  // destination lives there), an Ask→chat full sheet, and the shared overlays.
  import { app } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";
  import logo from "../assets/cortex-logo.png";

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
  // Subject detail / university-portal (Moodle) sheet — self-managed via
  // app.subjectPanelOpen; only mounted by the desktop App.svelte otherwise.
  import SubjectPanel from "../components/SubjectPanel.svelte";

  // The hamburger nav drawer (replaces the bottom tab bar + the old More screen).
  let drawerOpen = $state(false);

  // Lazy heavy views (mirrors App.svelte's code-split + prefetch idea).
  let SettingsView = $state<any>(null);
  let CalendarViewC = $state<any>(null);
  let AnalyticsViewC = $state<any>(null);
  let ExamViewC = $state<any>(null);
  let RecorderViewC = $state<any>(null);
  const loadSettings = () =>
    SettingsView ?? import("../views/Settings.svelte").then((m) => (SettingsView = m.default));
  const loadCalendar = () =>
    CalendarViewC ?? import("../views/CalendarView.svelte").then((m) => (CalendarViewC = m.default));
  const loadAnalytics = () =>
    AnalyticsViewC ?? import("../views/AnalyticsView.svelte").then((m) => (AnalyticsViewC = m.default));
  const loadExam = () =>
    ExamViewC ?? import("../views/ExamView.svelte").then((m) => (ExamViewC = m.default));
  const loadRecorder = () =>
    RecorderViewC ?? import("../views/Recorder.svelte").then((m) => (RecorderViewC = m.default));
  $effect(() => {
    if (app.view === "settings") void loadSettings();
    else if (app.view === "calendar") void loadCalendar();
    else if (app.view === "analytics") void loadAnalytics();
    else if (app.view === "exam") void loadExam();
    else if (app.view === "recorder") void loadRecorder();
  });

  // If we land on the subject view with nothing active (fresh install), show Home.
  $effect(() => {
    if (app.view === "subject" && !app.activeSubject) app.setView("dashboard");
  });

  // ── navigation ──
  function go(v: Parameters<typeof app.setView>[0]) {
    drawerOpen = false;
    app.setView(v);
  }
  function openSearch() {
    drawerOpen = false;
    app.searchOpen = true;
  }

  // "Deep" views sit under something (a subject/source) and show a back chevron;
  // top-level destinations show the hamburger instead.
  const isDeep = $derived(
    ["subject", "source", "add-subject", "gen-material", "exam", "recorder"].includes(app.view)
  );
  function back() {
    if (app.view === "source") {
      app.closeSource();
      return;
    }
    // Subject-scoped views go up to the subject; subject / new-subject → Home.
    if (["gen-material", "exam", "recorder"].includes(app.view) && app.activeSubject) {
      app.setView("subject");
      return;
    }
    app.setView("dashboard");
  }

  // Unified handler for the left-edge right-swipe: close the topmost overlay first,
  // then the drawer, then walk the deep-view stack. On a TOP-LEVEL view (the header
  // shows the hamburger, not a back chevron) with nothing open, the same swipe OPENS
  // the drawer — so the menu is reachable by gesture, mirroring the back gesture.
  function swipeRight() {
    if (app.detail) { app.closeDetail(); return; }
    if (app.notifOpen) { app.notifOpen = false; return; }
    if (app.chatOpen) { app.chatOpen = false; return; }
    if (app.searchOpen) { app.searchOpen = false; return; }
    if (app.subjectPanelOpen) { app.closeSubjectPanel(); return; }
    if (drawerOpen) { drawerOpen = false; return; }
    if (isDeep) { back(); return; }
    drawerOpen = true; // hamburger present, nothing to dismiss → open the menu
  }

  // Left-edge swipe = back / open-menu, app-wide (mobile-only; this shell only renders on mobile).
  $effect(() => {
    let sx = -1, sy = 0;
    const EDGE = 40, DX = 55, DY = 40;
    function onStart(e: TouchEvent) {
      if (e.touches.length !== 1) { sx = -1; return; }
      const t = e.touches[0];
      sx = t.clientX <= EDGE ? t.clientX : -1;
      sy = t.clientY;
    }
    function onMove(e: TouchEvent) {
      if (sx < 0 || e.touches.length !== 1) return;
      const t = e.touches[0];
      if (t.clientX - sx > DX && Math.abs(t.clientY - sy) < DY) {
        sx = -1;
        swipeRight();
      }
    }
    function onEnd() { sx = -1; }
    window.addEventListener("touchstart", onStart, { passive: true });
    window.addEventListener("touchmove", onMove, { passive: true });
    window.addEventListener("touchend", onEnd, { passive: true });
    return () => {
      window.removeEventListener("touchstart", onStart);
      window.removeEventListener("touchmove", onMove);
      window.removeEventListener("touchend", onEnd);
    };
  });

  // Highlight the active destination in the drawer.
  const activeNav = $derived.by(() => {
    const v = app.view;
    if (v === "dashboard") return "home";
    if (["subject", "source", "gen-material", "exam"].includes(v)) return "home";
    return v;
  });

  // Ask FAB: only where a chat scope makes sense, and only when chat is closed.
  // Hidden on the subject's Chats tab (it IS the chat — redundant) and Materials tab
  // (flashcards/slides already have their own controls; the FAB blocked the next button).
  const showAsk = $derived(
    ["subject", "source", "notes"].includes(app.view) &&
      !app.chatOpen &&
      !drawerOpen &&
      !(app.view === "subject" && (app.subjectTab === "chats" || app.subjectTab === "materials"))
  );

  const NAV = [
    { id: "dashboard",   icon: "home",     label: "Home" },
    { id: "add-source",  icon: "doc",      label: "Add source" },
    { id: "recorder",    icon: "record",   label: "Record lecture" },
    { id: "calendar",    icon: "calendar", label: "Calendar" },
    { id: "notes",       icon: "reader",   label: "Notes" },
    { id: "analytics",   icon: "chart",    label: "Insights" },
    { id: "add-subject", icon: "plus",     label: "New subject" },
    { id: "settings",    icon: "settings", label: "Settings" },
  ] as const;
</script>

<div class="m-shell">
  <header class="m-header">
    {#if isDeep}
      <button class="m-iconbtn" onclick={back} aria-label="Back">
        <Icon name="chevron" size={18} style="transform:rotate(180deg)" />
      </button>
    {:else}
      <button class="m-iconbtn" onclick={() => (drawerOpen = true)} aria-label="Menu">
        <Icon name="menu" size={18} />
      </button>
    {/if}
    <!-- Static brand: the user knows which screen they're on, so we don't repeat it. -->
    <h1 class="page-title m-title"><img class="m-logo" src={logo} alt="" />Cortex</h1>
    <span class="m-spacer"></span>
    <button class="m-iconbtn m-bell" onclick={() => app.toggleNotifications()} aria-label="Notifications">
      <Icon name="bell" size={18} />
      {#if app.unreadCount > 0}
        <span class="m-badge">{app.unreadCount > 99 ? "99+" : app.unreadCount}</span>
      {/if}
    </button>
  </header>

  <main class="m-main">
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
    {:else if app.view === "recorder"}
      {#if RecorderViewC}{@const C = RecorderViewC}<C />{/if}
    {:else}
      <!-- Safety net: any view the shell doesn't render falls back to Home. -->
      <Dashboard />
    {/if}
  </main>

  {#if showAsk}
    <button class="m-ask" onclick={() => (app.chatOpen = true)} aria-label="Ask Cortex">
      <Icon name="chat" size={18} /> Ask
    </button>
  {/if}
</div>

<!-- Nav drawer: every destination (formerly split across the bottom bar + More). -->
{#if drawerOpen}
  <button class="m-drawer-back" aria-label="Close menu" onclick={() => (drawerOpen = false)}></button>
  <nav class="m-drawer" aria-label="Menu">
    <div class="m-drawer-head">
      <span class="page-title m-drawer-brand"><img class="m-logo" src={logo} alt="" />Cortex</span>
      <button class="m-iconbtn" onclick={() => (drawerOpen = false)} aria-label="Close menu"><Icon name="x" size={16} /></button>
    </div>
    <button class="m-row m-nav{activeNav === 'home' ? ' on' : ''}" onclick={() => go("dashboard")}>
      <Icon name="home" size={18} /><span class="m-row-l">Home</span>
    </button>
    <button class="m-row m-nav" onclick={openSearch}>
      <Icon name="search" size={18} /><span class="m-row-l">Search</span>
    </button>
    {#each NAV.slice(1) as n (n.id)}
      <button class="m-row m-nav{activeNav === n.id ? ' on' : ''}" onclick={() => go(n.id)}>
        <Icon name={n.icon} size={18} /><span class="m-row-l">{n.label}</span>
      </button>
    {/each}
  </nav>
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
<SubjectPanel />

<style>
  /* All chrome below is styled ONLY from existing design tokens — no new colours
     (continuity rule, docs/MOBILE_PORT.md §3.4). */
  .m-shell {
    /* inset:0 fills the exact viewport (100dvh measured short in LiveContainer). */
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

  /* ── Nav drawer ─────────────────────────────────────────────────────────── */
  .m-drawer-back {
    position: fixed;
    inset: 0;
    z-index: 58;
    border: none;
    padding: 0;
    background: color-mix(in oklab, var(--bg) 55%, transparent);
    animation: m-fade 0.15s ease;
    cursor: pointer;
  }
  .m-drawer {
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    z-index: 59;
    width: min(82vw, 320px);
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: calc(env(safe-area-inset-top, 0px) + 12px) 12px calc(env(safe-area-inset-bottom, 0px) + 16px);
    background: var(--surface);
    border-right: 1px solid var(--border);
    overflow-y: auto;
    animation: m-drawer-in 0.18s ease;
  }
  @keyframes m-drawer-in { from { transform: translateX(-100%); } to { transform: none; } }
  @keyframes m-fade { from { opacity: 0; } to { opacity: 1; } }
  .m-drawer-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 6px 12px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--border);
  }
  .m-drawer-brand { font-size: var(--t-lg, 18px); }
  .m-title, .m-drawer-brand { display: inline-flex; align-items: center; gap: 7px; }
  .m-logo { height: 1.15em; width: auto; flex: none; }

  .m-row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    min-height: 50px;
    padding: 12px 14px;
    border: 1px solid transparent;
    background: none;
    color: var(--fg);
    border-radius: var(--rad-3);
    font-size: var(--t-md);
    text-align: left;
    cursor: pointer;
  }
  .m-row-l { flex: 1; min-width: 0; }
  .m-row:active { background: var(--surface-2); }
  .m-nav.on { background: color-mix(in oklab, var(--accent) 12%, var(--surface)); color: var(--fg-bright); }
  .m-nav.on :global(svg) { color: var(--accent); }

  /* Ask FAB — sits just above the bottom edge now that there's no tab bar. */
  .m-ask {
    position: fixed;
    right: 14px;
    bottom: calc(16px + env(safe-area-inset-bottom, 0px));
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 10px 16px;
    border: 1px solid var(--accent);
    border-radius: var(--rad-pill, 999px);
    background: var(--accent);
    color: var(--accent-fg);
    font-weight: 600;
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
