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
  import HelpOverlay from "./components/HelpOverlay.svelte";
  import { keybinds } from "./lib/keybinds.svelte";

  import Dashboard from "./views/Dashboard.svelte";
  import SubjectView from "./views/SubjectView.svelte";
  import SourceViewer from "./views/SourceViewer.svelte";
  import AddSource from "./views/AddSource.svelte";
  import AddSubject from "./views/AddSubject.svelte";
  import WebSearch from "./views/WebSearch.svelte";
  import Recorder from "./views/Recorder.svelte";
  import GenerateMaterial from "./views/GenerateMaterial.svelte";
  import Settings from "./views/Settings.svelte";
  import Onboarding from "./views/Onboarding.svelte";

  // Initialize app state on mount (loads subjects, seeds demo if empty, restores theme)
  $effect(() => {
    app.init();
  });

  // Show the chat dock alongside the workspace on the subject view (except when
  // the Chats tab already shows the full panel).
  const showChatDock = $derived(
    app.chatOpen && app.view === "subject" && app.subjectTab !== "chats"
  );

  // Global keyboard engine (Helix-style). Modals/sessions set window.__cortexModalOpen
  // to claim the keyboard; we stay out of their way then.
  let gPrefix = false;
  $effect(() => {
    function onKey(e: KeyboardEvent) {
      const el = document.activeElement as HTMLElement | null;
      const typing =
        !!el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable);

      if (e.key === "Escape") {
        if (app.cmdkOpen || app.leaderOpen || app.musicOpen) {
          app.cmdkOpen = false; app.leaderOpen = false; app.musicOpen = false; return;
        }
        if (typing) el?.blur();
        app.setMode("NOR");
        return;
      }
      if (typing) return;
      if ((window as any).__cortexModalOpen) return; // diff/flashcards/quiz own the keyboard
      if (app.cmdkOpen) return;

      if ((e.ctrlKey || e.metaKey) && (e.key === "p" || e.key === "P")) { e.preventDefault(); app.cmdkOpen = true; return; }
      if (e.metaKey || e.ctrlKey || e.altKey) return;

      if (app.leaderOpen) return; // LeaderPane consumes its own keys
      const k = keybinds.map;
      if (e.key === k.cmdk) { e.preventDefault(); app.cmdkOpen = true; return; }
      if (e.key === k.leader) { e.preventDefault(); app.leaderOpen = true; return; }
      if (e.key === k.help) { e.preventDefault(); app.helpOpen = true; return; }
      if (e.key === k.dismissToast && app.toasts.length) { e.preventDefault(); app.dismissToast(app.toasts[app.toasts.length - 1].id); return; }

      if (gPrefix) {
        gPrefix = false;
        if (e.key === k.dashboard) { app.setView("dashboard"); return; }
      }
      if (e.key === "g") { gPrefix = true; setTimeout(() => (gPrefix = false), 600); return; }

      if (e.key === k.toggleChat) { app.toggleChat(); return; }
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
  <div class="app-shell" style:--sb-w="248px">
    <Sidebar />

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
        {:else if app.view === "settings"}
          <Settings />
        {/if}
      </div>

      {#if showChatDock}
        <div class="chatdock">
          <ChatPanel onClose={() => (app.chatOpen = false)} />
        </div>
      {/if}

      {#if app.view === "subject" && app.subjectTab !== "chats" && !app.chatOpen}
        <button class="chat-fab" onclick={() => (app.chatOpen = true)} title="Open chat (c)">
          Ask <span class="kbd">c</span>
        </button>
      {/if}
    </div>

    <StatusBar />

    <!-- overlays -->
    <CommandPalette />
    <LeaderPane />
    <HelpOverlay />
    <DiffModal />
    <MusicPanel />
    <SourceMetaModal />
    <ToastStack />
  </div>
{/if}

<style>
  .boot {
    display: flex; align-items: center; justify-content: center; height: 100vh;
    color: var(--fg-bright); font-family: var(--font-mono); font-size: var(--t-md);
    background: var(--bg);
  }
</style>
