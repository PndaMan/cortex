<script lang="ts">
  import { app } from "./lib/store.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import GlobalSearch from "./components/GlobalSearch.svelte";
  import ToastStack from "./components/ToastStack.svelte";
  import DiffModal from "./components/DiffModal.svelte";
  import MusicPanel from "./components/MusicPanel.svelte";
  import SourceMetaModal from "./components/SourceMetaModal.svelte";
  import LeaderPane from "./components/LeaderPane.svelte";
  import ChatPanel from "./components/ChatPanel.svelte";
  import ResizeHandle from "./components/ResizeHandle.svelte";
  import Icon from "./components/Icon.svelte";
  import FindBar from "./components/FindBar.svelte";
  import HelpOverlay from "./components/HelpOverlay.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import Dialog from "./components/Dialog.svelte";
  import EditModal from "./components/EditModal.svelte";
  import SubjectPanel from "./components/SubjectPanel.svelte";
  import NotificationCenter from "./components/NotificationCenter.svelte";
  import NotificationDetail from "./components/NotificationDetail.svelte";
  import PomodoroPanel from "./components/PomodoroPanel.svelte";
  import LiveActivity from "./components/LiveActivity.svelte";
  import { keybinds } from "./lib/keybinds.svelte";

  import Dashboard from "./views/Dashboard.svelte";
  import SubjectView from "./views/SubjectView.svelte";
  import SourceViewer from "./views/SourceViewer.svelte";
  import AddSource from "./views/AddSource.svelte";
  import AddSubject from "./views/AddSubject.svelte";
  import GenerateMaterial from "./views/GenerateMaterial.svelte";
  import NotesView from "./views/NotesView.svelte";
  import Onboarding from "./views/Onboarding.svelte";

  // The three heaviest views (Settings 70KB, Recorder 32KB, Calendar 31KB) are
  // code-split out of the startup bundle and loaded on first visit. They're
  // prefetched right after init settles, so by the time a human can navigate
  // there the chunk is already resolved — no flash, just a faster cold start.
  let SettingsView = $state<any>(null);
  let RecorderView = $state<any>(null);
  let CalendarViewC = $state<any>(null);
  let AnalyticsViewC = $state<any>(null);
  let ExamViewC = $state<any>(null);
  const loadSettings = () =>
    SettingsView ?? import("./views/Settings.svelte").then((m) => (SettingsView = m.default));
  const loadRecorder = () =>
    RecorderView ?? import("./views/Recorder.svelte").then((m) => (RecorderView = m.default));
  const loadCalendar = () =>
    CalendarViewC ?? import("./views/CalendarView.svelte").then((m) => (CalendarViewC = m.default));
  const loadAnalytics = () =>
    AnalyticsViewC ?? import("./views/AnalyticsView.svelte").then((m) => (AnalyticsViewC = m.default));
  const loadExam = () =>
    ExamViewC ?? import("./views/ExamView.svelte").then((m) => (ExamViewC = m.default));
  $effect(() => {
    if (app.view === "settings") void loadSettings();
    else if (app.view === "recorder") void loadRecorder();
    else if (app.view === "calendar") void loadCalendar();
    else if (app.view === "analytics") void loadAnalytics();
    else if (app.view === "exam") void loadExam();
  });

  // Initialize app state on mount (loads subjects, seeds demo if empty, restores theme)
  $effect(() => {
    app.init();
    const prefetch = setTimeout(() => {
      void loadSettings();
      void loadRecorder();
      void loadCalendar();
      void loadAnalytics();
      void loadExam();
    }, 1500);
    return () => clearTimeout(prefetch);
  });

  // Chat dock / FAB visibility. Read EVERY signal into a local first so &&/||
  // short-circuiting can't drop one from Svelte's reactive dependency set (that
  // bug left the dock stuck visible on the notes view regardless of chatOpen).
  const chatViewOk = $derived.by(() => {
    const v = app.view;
    const tab = app.subjectTab;
    return v === "notes" || (v === "subject" && tab !== "chats");
  });
  const showChatDock = $derived.by(() => {
    const open = app.chatOpen;
    return open && chatViewOk;
  });
  const showChatFab = $derived.by(() => {
    const open = app.chatOpen;
    const v = app.view;
    return !open && (chatViewOk || v === "source");
  });

  // ---- responsive shell ----
  // The drawer/compact layout is fully styled in app.css but was never switched
  // on. Drive it from the viewport width so the app collapses gracefully toward
  // mobile: < 1080px → chat dock becomes an overlay (compact); < 760px → the
  // sidebar becomes a slide-in drawer behind a hamburger (tight).
  let vw = $state(typeof window !== "undefined" ? window.innerWidth : 1280);
  $effect(() => {
    const onResize = () => (vw = window.innerWidth);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });
  // Below 1080 the sidebar becomes a slide-in drawer (overlay, never pushes the
  // workspace) so the content always has the full width to itself — that kills
  // the 760-1080 "squash" where a fixed 248px rail + workspace both fought for
  // ~900px. `tight` (<760) keeps its phone tweaks; `drawer` is the shared
  // hamburger+overlay sidebar mode for the whole sub-1080 band.
  const tight = $derived(vw < 760);
  const drawer = $derived(vw < 1080);
  const compact = $derived(vw < 1080);
  let navOpen = $state(false);
  // Close the drawer when leaving drawer mode or when navigating anywhere.
  $effect(() => { if (!drawer) navOpen = false; });
  $effect(() => { void app.view; void app.activeSubjectId; void app.subjectTab; navOpen = false; });

  // ---- view back-stack: Esc goes back to the previous page ----
  let viewHistory: string[] = [];
  let prevView = app.view;
  let navigatingBack = false;
  $effect(() => {
    const v = app.view;
    if (v !== prevView) {
      if (!navigatingBack) {
        viewHistory.push(prevView);
        if (viewHistory.length > 50) viewHistory.shift();
      }
      navigatingBack = false;
      prevView = v;
    }
  });
  function goBack() {
    const last = viewHistory.pop();
    if (last !== undefined) {
      navigatingBack = true;
      app.setView(last as typeof app.view);
    }
  }

  // Find-in-page (Ctrl/Cmd+F) — claimed in the CAPTURE phase so it beats the
  // WebKitGTK native find-in-page (which otherwise popped up and swallowed
  // Ctrl+C/Ctrl+V). Only this exact combo is intercepted; plain Ctrl and other
  // Ctrl chords (copy/paste/etc.) pass straight through.
  $effect(() => {
    function onFind(e: KeyboardEvent) {
      // NB: keep the two halves as separate booleans. Writing this as one
      // `(mod) && (key)` expression triggers a Svelte 5 compiler bug that strips
      // the parentheses (→ `ctrl || meta && key... || key...`), which made bare
      // Ctrl and Shift+F open find. Separate statements compile correctly.
      const mod = e.ctrlKey || e.metaKey;
      const isF = e.key === "f" || e.key === "F";
      if (mod && isF) {
        e.preventDefault();
        e.stopPropagation();
        e.stopImmediatePropagation(); // also beat any native find-in-page handler
        app.findOpen = true;
      }
    }
    window.addEventListener("keydown", onFind, true);
    return () => window.removeEventListener("keydown", onFind, true);
  });

  // Kill webview zoom — Ctrl/Cmd+scroll and Ctrl/Cmd +/-/0 otherwise scale the
  // whole UI, which makes Cortex feel like a zoomable web page / PWA instead of
  // a native app. Captured early so WebKitGTK never gets to zoom. Ctrl+F / Ctrl+P
  // (letters) are untouched — only the zoom combos are swallowed.
  $effect(() => {
    function onWheel(e: WheelEvent) {
      if (e.ctrlKey || e.metaKey) e.preventDefault();
    }
    function onZoomKey(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey)) return;
      if (["=", "+", "-", "_", "0"].includes(e.key)) {
        e.preventDefault();
        e.stopPropagation();
      }
    }
    // passive:false is required for preventDefault on wheel.
    window.addEventListener("wheel", onWheel, { passive: false, capture: true });
    window.addEventListener("keydown", onZoomKey, true);
    return () => {
      window.removeEventListener("wheel", onWheel, true);
      window.removeEventListener("keydown", onZoomKey, true);
    };
  });

  // Global keyboard engine (Helix-style). Modals/sessions set window.__cortexModalOpen
  // to claim the keyboard; we stay out of their way then.
  let gPrefix = false;
  $effect(() => {
    function onKey(e: KeyboardEvent) {
      const el = document.activeElement as HTMLElement | null;
      const typing =
        !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);

      // Alt+1..9 — jump to the Nth subject (like clicking it in the navbar).
      if (e.altKey && !e.ctrlKey && !e.metaKey && /^[1-9]$/.test(e.key)) {
        e.preventDefault();
        const s = app.subjects[parseInt(e.key, 10) - 1];
        if (s) app.openSubject(s.id);
        return;
      }

      if (e.key === "Escape") {
        // 1. Close any transient overlay first.
        if (app.findOpen) { app.findOpen = false; return; }
        if (app.detail) { app.closeDetail(); return; }
        if (app.notifOpen) { app.notifOpen = false; return; }
        if (app.cmdkOpen || app.leaderOpen || app.musicOpen || app.searchOpen) {
          app.cmdkOpen = false; app.leaderOpen = false; app.musicOpen = false; app.searchOpen = false; return;
        }
        if ((window as any).__cortexModalOpen) return; // modals handle their own Esc
        // 2. In a text field (e.g. the chat compose box) Esc just leaves edit mode.
        if (typing) { el?.blur(); app.setMode("NOR"); return; }
        // 3. Otherwise Esc ALWAYS navigates back to the previous page (chat is
        //    closed with `c`/its × button, so Esc is reserved for back-nav).
        app.setMode("NOR");
        goBack();
        return;
      }
      // Never act on a standalone modifier press (so Ctrl/Cmd for copy etc. work,
      // and a stray "Control" keybind can't open the palette).
      if (["Control", "Shift", "Alt", "Meta", "AltGraph", "CapsLock", "ContextMenu"].includes(e.key)) return;
      if (typing) return;
      if ((window as any).__cortexModalOpen) return; // diff/flashcards/quiz own the keyboard
      // Full-screen views (e.g. Settings) claim the keyboard so single-key
      // shortcuts (t/c/m/n…) don't fire while you're configuring things. Esc is
      // handled above this guard, so back-nav still works.
      if ((window as any).__cortexViewKeys) return;
      if (app.cmdkOpen) return;

      // Separate booleans on purpose — see the onFind note above (Svelte 5
      // paren-stripping compiler bug would otherwise make any Ctrl chord open this).
      const cmdMod = e.ctrlKey || e.metaKey;
      const isP = e.key === "p" || e.key === "P";
      if (cmdMod && isP) { e.preventDefault(); app.cmdkOpen = true; return; }
      const isK = e.key === "k" || e.key === "K";
      if (cmdMod && isK) { e.preventDefault(); app.searchOpen = true; return; }
      if (e.metaKey || e.ctrlKey || e.altKey) return;

      if (app.leaderOpen) return; // LeaderPane consumes its own keys
      const k = keybinds.map;
      // The command-palette key must be a symbol (":"). If a corrupt bind ever
      // made it an alphanumeric (e.g. "c"), that would hijack typing AND steal
      // the chat toggle — so sanitize at the use site, never trusting the value.
      const cmdkKey = /^[a-z0-9]$/i.test(k.cmdk) ? ":" : k.cmdk;
      if (e.key === cmdkKey) { e.preventDefault(); app.cmdkOpen = true; return; }
      if (e.key === k.leader) { e.preventDefault(); app.leaderOpen = true; return; }
      if (e.key === k.help) { e.preventDefault(); app.helpOpen = true; return; }
      if (e.key === k.dismissToast && app.toasts.length) { e.preventDefault(); app.dismissToast(app.toasts[app.toasts.length - 1].id); return; }

      if (gPrefix) {
        gPrefix = false;
        if (e.key === k.dashboard) { app.setView("dashboard"); return; }
        if (e.key === "s") { app.openSettings(); return; } // g s → Settings
      }
      if (e.key === "g") { gPrefix = true; setTimeout(() => (gPrefix = false), 600); return; }

      // Chat toggle. Use the configured bind, falling back to "c" ONLY when it's
      // unset/corrupt — so rebinding the chat key actually replaces "c" instead of
      // leaving both keys active (the old behaviour, which made rebinds "not work").
      if (e.key === (k.toggleChat || "c")) { app.toggleChat(); return; }
      if (e.key === k.toggleSidebar) { app.toggleSidebar(); return; }
      if (e.key === k.newSubject) { e.preventDefault(); app.setView("add-subject"); return; }
      if (e.key === k.recorder) { app.setView("recorder"); return; }
      if (e.key === k.cycleTheme) { app.cycleTheme(); return; }
      if (e.key === k.music) { app.musicOpen = true; return; }
      if (e.key === k.notifications) { e.preventDefault(); app.toggleNotifications(); return; }
      if (e.key === k.insert) {
        const ta = document.querySelector<HTMLTextAreaElement>(".compose-box textarea");
        if (ta) { e.preventDefault(); ta.focus(); app.setMode("INS"); }
        return;
      }

      if (app.view === "dashboard") {
        const n = app.subjects.length;
        if (e.key === "j" || e.key === "ArrowDown" || e.key === "l" || e.key === "ArrowRight") {
          e.preventDefault(); app.dashFocus = Math.min(n - 1, app.dashFocus + 1);
        } else if (e.key === "k" || e.key === "ArrowUp" || e.key === "h" || e.key === "ArrowLeft") {
          e.preventDefault(); app.dashFocus = Math.max(0, app.dashFocus - 1);
        } else if (e.key === "Enter" && app.subjects[app.dashFocus]) {
          e.preventDefault(); app.openSubject(app.subjects[app.dashFocus].id);
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

{#if app.loading}
  <div class="boot">Cortex — loading…</div>
{:else if app.onboarding}
  <Onboarding onFinish={() => (app.onboarding = false)} />
{:else}
  <div
    class="app-shell"
    class:tight
    class:drawer
    class:compact
    class:nav-open={navOpen}
    style:--sb-w={app.sidebarCollapsed ? "0px" : app.sidebarWidth + "px"}
    style:--chat-w={app.chatWidth + "px"}
  >
    {#if drawer}
      <!-- Sub-1080: hamburger toggles the slide-in sidebar drawer (overlay). -->
      <button class="nav-toggle" onclick={() => (navOpen = !navOpen)} aria-label="Toggle menu">
        <Icon name="grid" size={16} />
      </button>
      <Sidebar />
      <button class="nav-backdrop" aria-label="Close menu" onclick={() => (navOpen = false)}></button>
    {:else if !app.sidebarCollapsed}
      <Sidebar />
    {:else}
      <button class="sb-expand" onclick={() => app.toggleSidebar()} title="Show sidebar (b)">
        <Icon name="chevron" size={14} />
      </button>
    {/if}

    <div class="app-main">
      <div class="workspace">
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
        {:else if app.view === "recorder"}
          {#if RecorderView}{@const Recorder = RecorderView}<Recorder />{/if}
        {:else if app.view === "gen-material"}
          <GenerateMaterial />
        {:else if app.view === "notes"}
          <NotesView />
        {:else if app.view === "calendar"}
          {#if CalendarViewC}{@const CalendarView = CalendarViewC}<CalendarView />{/if}
        {:else if app.view === "analytics"}
          {#if AnalyticsViewC}{@const AnalyticsView = AnalyticsViewC}<AnalyticsView />{/if}
        {:else if app.view === "exam"}
          {#if ExamViewC}{@const ExamView = ExamViewC}<ExamView />{/if}
        {:else if app.view === "settings"}
          {#if SettingsView}{@const Settings = SettingsView}<Settings />{/if}
        {/if}
      </div>

      {#if showChatDock}
        <div class="chatdock">
          {#if !compact}
            <ResizeHandle
              side="left"
              sign={-1}
              min={300}
              max={720}
              get={() => app.chatWidth}
              set={(v) => (app.chatWidth = v)}
              commit={() => app.saveChatWidth()}
            />
          {/if}
          <ChatPanel
            onClose={() => (app.chatOpen = false)}
            onFullscreen={() => { app.setView("subject"); app.subjectTab = "chats"; }}
          />
        </div>
      {/if}

      {#if showChatFab}
        <button class="chat-fab" onclick={() => (app.chatOpen = true)} title="Open chat (c)">
          Ask <span class="kbd">c</span>
        </button>
      {/if}
    </div>

    <StatusBar />

    <!-- overlays -->
    <FindBar />
    <CommandPalette />
    <GlobalSearch />
    <LeaderPane />
    <HelpOverlay />
    <DiffModal />
    <MusicPanel />
    <SourceMetaModal />
    <Dialog />
    <EditModal />
    <SubjectPanel />
    <NotificationCenter />
    <NotificationDetail />
    <PomodoroPanel />
    <LiveActivity />
    <ContextMenu />
    <ToastStack />
  </div>
{/if}

<style>
  .boot {
    display: flex; align-items: center; justify-content: center; height: 100vh;
    color: var(--fg-bright); font-family: var(--font-mono); font-size: var(--t-md);
    background: var(--bg);
  }

  /* Floating button to reopen the sidebar when it's minimized. */
  .sb-expand {
    position: fixed; top: 10px; left: 10px; z-index: 40;
    width: 30px; height: 30px; display: inline-flex; align-items: center; justify-content: center;
    border-radius: 8px; border: 1px solid var(--border); background: var(--surface);
    color: var(--fg-muted); cursor: pointer; transition: color var(--dur), border-color var(--dur);
  }
  .sb-expand:hover { color: var(--fg-bright); border-color: var(--border-strong); }
</style>
