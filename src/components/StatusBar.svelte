<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
</script>

<!-- Helix-style status bar -->
<div class="statusbar mode-{app.mode}">
  <div class="mode-block">
    {app.mode}
    {#if app.activeSubject}
      <span class="mode-ctx">
        {app.activeSubject.code ?? app.activeSubject.name}
      </span>
    {/if}
    <span class="mode-hint">␣ actions · : cmd</span>
  </div>

  <div class="sb-seg">
    <Icon name="diamond" size={10} color="var(--accent)" />
    {#if app.activeSubject}
      {app.activeSubject.name}
    {:else}
      Cortex
    {/if}
  </div>

  <div class="sb-spacer"></div>

  <button class="sb-seg sb-seg-btn" type="button" title="Keyboard shortcuts" onclick={() => (app.helpOpen = true)}>
    <span class="sb-key">?</span> help
  </button>

  <div class="sb-seg" style:border-right="none">
    <span class="sb-key">:</span> command
  </div>
</div>

<style>
  /* Mode block: keep its themed glyph, append contextual hint compactly */
  .mode-block .mode-ctx {
    margin-left: 8px;
    font-weight: 600;
    letter-spacing: 0.04em;
    opacity: 0.92;
  }
  .mode-block .mode-hint {
    margin-left: 8px;
    font-weight: 500;
    letter-spacing: 0;
    font-size: var(--t-2xs);
    opacity: 0.7;
  }
  /* help segment becomes a real button while matching .sb-seg visuals */
  .statusbar .sb-seg-btn {
    border: none;
    border-right: 1px solid var(--border);
    background: none;
    font: inherit;
    cursor: pointer;
    color: var(--fg-faint);
  }
  .statusbar .sb-seg-btn:hover {
    color: var(--fg-bright);
    background: var(--surface-2);
  }
</style>
