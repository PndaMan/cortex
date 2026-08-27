<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import { LEADER_ACTIONS, type LeaderAction } from "../lib/keybinds.svelte";
  import { translateText } from "../lib/i18n";

  // Run handlers keyed by leader key. The key/label/detail spec lives in
  // lib/keybinds.svelte (shared with the help overlay); only the behavior is here.
  const RUN: Record<string, () => void> = {
    s: () => app.goToSources(),
    h: () => app.goToCheatsheet(),
    c: () => { app.chatOpen = true; },
    r: () => app.setView("recorder"),
    // Flashcards/quizzes live on the materials tab — land there (the bare
    // setView("subject") used to leave you on the wrong tab, a dead-end).
    f: () => { app.setView("subject"); app.setTab("materials"); },
    e: () => { app.setView("subject"); app.setTab("materials"); },
    d: () => app.reviewDiff(),
    o: () => app.setView("notes"),
    a: () => app.setView("calendar"),
    i: () => app.setView("analytics"),
    t: () => app.cycleTheme(),
    m: () => { app.musicOpen = true; },
    p: () => { app.pomodoroOpen = true; },
    b: () => app.toggleSidebar(),
    g: () => app.setView("dashboard"),
    ",": () => app.openSettings(),
  };

  const actions = LEADER_ACTIONS;

  function runAction(a: LeaderAction) {
    RUN[a.key]?.();
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
            <span class="lk">{a.key}</span>{translateText(a.label, app.language)}
            {#if a.detail}
              <span class="ld">{translateText(a.detail, app.language)}</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}
