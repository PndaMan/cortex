<script lang="ts">
  import { app } from "./lib/store.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import StatusBar from "./components/StatusBar.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import ToastStack from "./components/ToastStack.svelte";
  import DiffModal from "./components/DiffModal.svelte";
  import MusicPanel from "./components/MusicPanel.svelte";
  import SourceMetaModal from "./components/SourceMetaModal.svelte";
  import LeaderPane from "./components/LeaderPane.svelte";
  import ChatPanel from "./components/ChatPanel.svelte";
  import Icon from "./components/Icon.svelte";
  import FindBar from "./components/FindBar.svelte";
  import HelpOverlay from "./components/HelpOverlay.svelte";
  import Dialog from "./components/Dialog.svelte";
  import EditModal from "./components/EditModal.svelte";
  import PomodoroPanel from "./components/PomodoroPanel.svelte";
  import LiveActivity from "./components/LiveActivity.svelte";
  import { keybinds } from "./lib/keybinds.svelte";

  import Dashboard from "./views/Dashboard.svelte";
  import SubjectView from "./views/SubjectView.svelte";
  import SourceViewer from "./views/SourceViewer.svelte";
  import AddSource from "./views/AddSource.svelte";
  import AddSubject from "./views/AddSubject.svelte";
  import WebSearch from "./views/WebSearch.svelte";
  import Recorder from "./views/Recorder.svelte";
  import GenerateMaterial from "./views/GenerateMaterial.svelte";
  import NotesView from "./views/NotesView.svelte";
  import CalendarView from "./views/CalendarView.svelte";
  import Settings from "./views/Settings.svelte";
  import Onboarding from "./views/Onboarding.svelte";

  // Initialize app state on mount (loads subjects, seeds demo if empty, restores theme)
  $effect(() => {
    app.init();
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
  const tight = $derived(vw < 760);
  const compact = $derived(vw < 1080);
  let navOpen = $state(false);
  // Close the drawer when leaving tight mode or when navigating anywhere.
  $effect(() => { if (!tight) navOpen = false; });
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

  // Global keyboard engine (Helix-style). Modals/sessions set window.__cortexModalOpen
  // to claim the keyboard; we stay out of their way then.
  let gPrefix = false;
  $effect(() => {
    function onKey(e: KeyboardEvent) {
      const el = document.activeElement as HTMLElement | null;
      const typing =
        !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);

      // Find-in-page (Ctrl/Cmd+F) — works even while typing, like a browser.
      if ((e.ctrlKey || e.metaKey) && (e.key === "f" || e.key === "F")) {
        e.preventDefault();
        app.findOpen = true;
        return;
      }
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
        if (app.cmdkOpen || app.leaderOpen || app.musicOpen) {
          app.cmdkOpen = false; app.leaderOpen = false; app.musicOpen = false; return;
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
      if (app.cmdkOpen) return;

      if ((e.ctrlKey || e.metaKey) && (e.key === "p" || e.key === "P")) { e.preventDefault(); app.cmdkOpen = true; return; }
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
      }
      if (e.key === "g") { gPrefix = true; setTimeout(() => (gPrefix = false), 600); return; }

      // "c" is the app-wide chat toggle (shown as "Ask c" everywhere). Accept it
      // literally in addition to the configured bind, so a corrupted keybind map
      // can never break closing the chat.
      if (e.key === k.toggleChat || e.key === "c") { app.toggleChat(); return; }
      if (e.key === k.toggleSidebar) { app.toggleSidebar(); return; }
      if (e.key === k.newSubject) { e.preventDefault(); app.setView("add-subject"); return; }
      if (e.key === k.recorder) { app.setView("recorder"); return; }
      if (e.key === k.websearch) { app.setView("websearch"); return; }
      if (e.key === k.cycleTheme) { app.cycleTheme(); return; }
      if (e.key === k.music) { app.musicOpen = true; return; }
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
    class:compact
    class:nav-open={navOpen}
    style:--sb-w={app.sidebarCollapsed ? "0px" : "248px"}
  >
    {#if tight}
      <!-- Mobile: hamburger toggles the slide-in sidebar drawer. -->
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
        {:else if app.view === "websearch"}
          <WebSearch />
        {:else if app.view === "recorder"}
          <Recorder />
        {:else if app.view === "gen-material"}
          <GenerateMaterial />
        {:else if app.view === "notes"}
          <NotesView />
        {:else if app.view === "calendar"}
          <CalendarView />
        {:else if app.view === "settings"}
          <Settings />
        {/if}
      </div>

      {#if showChatDock}
        <div class="chatdock">
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
    <LeaderPane />
    <HelpOverlay />
    <DiffModal />
    <MusicPanel />
    <SourceMetaModal />
    <Dialog />
    <EditModal />
    <PomodoroPanel />
    <LiveActivity />
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
