<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  interface LeaderAction {
    key: string;
    label: string;
    detail?: string;
    run: () => void;
  }

  const actions: LeaderAction[] = [
    { key: "s", label: "Add source",   detail: "ingest a file / URL",   run: () => app.setView("add-source") },
    { key: "c", label: "Chat",         detail: "open chat dock",         run: () => { app.chatOpen = true; } },
    { key: "r", label: "Record",       detail: "lecture recorder",       run: () => app.setView("recorder") },
    { key: "f", label: "Flashcards",   detail: "study session",          run: () => app.setView("subject") },
    { key: "d", label: "Review diff",  detail: "cheatsheet draft",       run: () => app.reviewDiff() },
    { key: "w", label: "Web search",   detail: "search the web",         run: () => app.setView("websearch") },
    { key: "o", label: "Notes",        detail: "markdown notes",         run: () => app.setView("notes") },
    { key: "a", label: "Calendar",     detail: "events & tasks",         run: () => app.setView("calendar") },
    { key: "t", label: "Theme",        detail: "cycle Omarchy theme",    run: () => app.cycleTheme() },
    { key: "m", label: "Music",        detail: "study sound panel",      run: () => { app.musicOpen = true; } },
    { key: "p", label: "Pomodoro",     detail: "focus timer + bonsai",   run: () => { app.pomodoroOpen = true; } },
    { key: "g", label: "Dashboard",    detail: "go to dashboard",        run: () => app.setView("dashboard") },
  ];

  function runAction(a: LeaderAction) {
    a.run();
    app.leaderOpen = false;
  }

  function onKeyDown(e: KeyboardEvent) {
    if (!app.leaderOpen) return;
    if (e.key === "Escape") {
      e.preventDefault();
      app.leaderOpen = false;
      return;
    }
    // Match leader keys
    const matched = actions.find(a => a.key === e.key);
    if (matched) {
      e.preventDefault();
      runAction(matched);
    }
  }
</script>

<svelte:window onkeydown={onKeyDown} />

{#if app.leaderOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="leader-overlay" onmousedown={() => (app.leaderOpen = false)}>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="leader" onmousedown={e => e.stopPropagation()}>
      <div class="leader-head">
        <span class="kbd">␣</span> Space leader — context actions
      </div>
      <div class="leader-grid">
        {#each actions as a}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
          <div class="leader-item" onclick={() => runAction(a)}>
            <span class="lk">{a.key}</span>{a.label}
            {#if a.detail}
              <span class="ld">{a.detail}</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}
