<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import { keybinds, ACTION_LABELS, ACTION_ORDER } from "../lib/keybinds.svelte";

  function close() {
    app.helpOpen = false;
  }

  // Render a friendlier glyph for whitespace binds
  function keyLabel(k: string): string {
    if (k === " ") return "␣";
    return k;
  }

  // While open: claim the keyboard (other overlays use this convention) and
  // close on Escape via a capture-phase handler.
  $effect(() => {
    if (!app.helpOpen) return;

    window.__cortexModalOpen = true;

    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        close();
      }
    }

    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.__cortexModalOpen = false;
    };
  });
</script>

{#if app.helpOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay" role="presentation" onmousedown={close}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="help-pane" style:margin-top="12vh" onmousedown={(e) => e.stopPropagation()}>
      <div class="help-head">
        <span class="help-title">Keyboard shortcuts</span>
        <span class="help-spacer"></span>
        <button class="help-close" type="button" title="Close" aria-label="Close" onclick={close}>
          <Icon name="x" size={14} />
        </button>
      </div>

      <div class="help-body">
        {#each ACTION_ORDER as action (action)}
          <div class="help-row">
            <span class="help-label">{ACTION_LABELS[action]}</span>
            <span class="kbd">{keyLabel(keybinds.map[action])}</span>
          </div>
        {/each}
      </div>

      <div class="help-foot">
        <span class="kbd">esc</span> to close
      </div>
    </div>
  </div>
{/if}

<style>
  .help-pane {
    width: 100%;
    max-width: 460px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    box-shadow: var(--shadow-pop);
    overflow: hidden;
    animation: popIn var(--dur) var(--ease);
  }
  .help-head {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 46px;
    padding: 0 12px 0 15px;
    border-bottom: 1px solid var(--border);
  }
  .help-title {
    color: var(--fg-bright);
    font-size: var(--t-md);
    font-weight: 600;
  }
  .help-spacer { flex: 1; }
  .help-close {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: none;
    background: none;
    border-radius: var(--rad-2);
    color: var(--fg-faint);
    cursor: pointer;
    transition: color var(--dur-fast), background var(--dur-fast);
  }
  .help-close:hover {
    color: var(--fg-bright);
    background: var(--surface-3);
  }
  .help-body {
    padding: 6px;
    max-height: 56vh;
    overflow-y: auto;
  }
  .help-row {
    display: flex;
    align-items: center;
    gap: 11px;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--rad-2);
    color: var(--fg-muted);
    font-size: var(--t-sm);
  }
  .help-row:hover { background: var(--surface-3); }
  .help-label { flex: 1; }
  .help-foot {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 9px 15px;
    border-top: 1px solid var(--border);
    font-size: var(--t-xs);
    color: var(--fg-faint);
  }
</style>
