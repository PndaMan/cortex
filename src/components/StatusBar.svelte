<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
</script>

<!-- Helix-style status bar. Mode block (NOR/INS/SEL) is kept; the leader, help
     and command segments are real, hover-animated buttons. -->
<div class="statusbar mode-{app.mode}">
  <div class="mode-block">
    {app.mode}
    {#if app.activeSubject}
      <span class="mode-ctx">{app.activeSubject.code ?? app.activeSubject.name}</span>
    {/if}
  </div>

  <!-- clickable leader: a mouse alternative to the Space key -->
  <button
    class="sb-seg sb-seg-btn"
    type="button"
    title="Leader actions (Space)"
    onclick={() => (app.leaderOpen = true)}
  >
    <span class="sb-key">␣</span> actions
  </button>

  <div class="sb-seg">
    <Icon name="diamond" size={10} color="var(--accent)" />
    {app.activeSubject ? app.activeSubject.name : "Cortex"}
  </div>

  <div class="sb-spacer"></div>

  <button
    class="sb-seg sb-seg-btn"
    type="button"
    title="Keyboard shortcuts (?)"
    onclick={() => (app.helpOpen = true)}
  >
    <span class="sb-key">?</span> help
  </button>

  <button
    class="sb-seg sb-seg-btn"
    type="button"
    title="Command palette (:)"
    style:border-right="none"
    onclick={() => (app.cmdkOpen = true)}
  >
    <span class="sb-key">:</span> command
  </button>
</div>

<style>
  .mode-block .mode-ctx {
    margin-left: 8px;
    font-weight: 600;
    letter-spacing: 0.04em;
    opacity: 0.92;
  }
  /* the leader / help / command segments are interactive buttons that still
     read as plain .sb-seg cells, with a low-glare hover lift */
  .statusbar .sb-seg-btn {
    border: none;
    border-right: 1px solid var(--border);
    background: none;
    font: inherit;
    cursor: pointer;
    color: var(--fg-faint);
    transition: background 0.12s ease, color 0.12s ease;
  }
  .statusbar .sb-seg-btn:hover {
    color: var(--fg-bright);
    background: var(--surface-2);
  }
  .statusbar .sb-seg-btn .sb-key {
    transition: color 0.12s ease, border-color 0.12s ease, background 0.12s ease;
  }
  .statusbar .sb-seg-btn:hover .sb-key {
    color: var(--accent);
    border-color: var(--accent-dim, var(--accent));
  }
  .statusbar .sb-seg-btn:active {
    transform: translateY(0.5px);
  }
</style>
