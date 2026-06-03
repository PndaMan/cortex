<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    svg,
    onExit,
  }: { svg: string; onExit?: () => void } = $props();
</script>

<div class="workspace-scroll">
  <div class="infographic-page">
    {#if onExit}
      <div class="infographic-toolbar">
        <button class="btn btn--icon btn--sm btn--ghost" onclick={onExit} title="Back to materials">
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={14} /></span>
        </button>
        <span class="mono faint" style="font-size: var(--t-xs);">Infographic</span>
      </div>
    {/if}

    {#if !svg}
      <div class="infographic-empty">
        <Icon name="grid" size={26} color="var(--fg-faint)" />
        <p class="mono faint" style="margin-top: 12px;">No infographic content.</p>
      </div>
    {:else}
      <div class="infographic-canvas">
        {@html svg}
      </div>
    {/if}
  </div>
</div>

<style>
  .infographic-page {
    display: flex;
    flex-direction: column;
    min-height: 100%;
    padding: var(--sp-6) var(--sp-8);
    gap: var(--sp-5);
  }

  .infographic-toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: none;
  }

  .infographic-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--sp-10);
  }

  .infographic-canvas {
    flex: 1;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: var(--sp-4);
  }

  .infographic-canvas :global(svg) {
    max-width: 100%;
    height: auto;
    border-radius: var(--rad-3);
    box-shadow: var(--shadow-2);
  }
</style>
