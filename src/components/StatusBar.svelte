<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
</script>

<!-- Helix-style status bar -->
<div class="statusbar mode-{app.mode}">
  <div class="mode-block">{app.mode}</div>

  <div class="sb-seg">
    <Icon name="diamond" size={10} color="var(--accent)" />
    {#if app.activeSubject}
      {app.activeSubject.name}
    {:else}
      Cortex
    {/if}
  </div>

  <div class="sb-spacer"></div>

  <div class="sb-seg" style:color="var(--fg-faint)">? help</div>

  <div class="sb-seg" style:border-right="none">
    <span class="sb-key">:</span> command
  </div>
</div>
