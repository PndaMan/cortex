<script lang="ts">
  // Find-in-page: a small bar (Ctrl/Cmd+F) that searches & scrolls to text on the
  // current view using the webview's native window.find — highlights the match
  // and scrolls to it without mutating the DOM Svelte manages.
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let q = $state("");
  let notFound = $state(false);
  let inputEl = $state<HTMLInputElement | null>(null);

  function find(backwards = false) {
    if (!q) return;
    // (text, caseSensitive, backwards, wrapAround)
    const ok = (window as unknown as { find?: (...a: unknown[]) => boolean }).find?.(
      q, false, backwards, true
    );
    notFound = ok === false;
  }

  $effect(() => {
    if (app.findOpen) setTimeout(() => { inputEl?.focus(); inputEl?.select(); }, 20);
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); find(e.shiftKey); }
    else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); app.findOpen = false; }
  }
</script>

{#if app.findOpen}
  <div class="findbar">
    <Icon name="search" size={13} color="var(--fg-faint)" />
    <input
      bind:this={inputEl}
      bind:value={q}
      class="input findbar-input"
      placeholder={t("Find on page…")}
      oninput={() => (notFound = false)}
      onkeydown={onKey}
    />
    {#if notFound}<span class="findbar-stat mono">{t("no matches")}</span>{/if}
    <button class="btn btn--icon btn--sm btn--ghost" onclick={() => find(true)} title={t("Previous (Shift+Enter)")}>
      <span style="display:inline-flex;transform:rotate(-90deg)"><Icon name="chevron" size={12} /></span>
    </button>
    <button class="btn btn--icon btn--sm btn--ghost" onclick={() => find(false)} title={t("Next (Enter)")}>
      <span style="display:inline-flex;transform:rotate(90deg)"><Icon name="chevron" size={12} /></span>
    </button>
    <button class="btn btn--icon btn--sm btn--ghost" onclick={() => (app.findOpen = false)} title={t("Close (Esc)")}>
      <Icon name="x" size={12} />
    </button>
  </div>
{/if}

<style>
  .findbar {
    position: fixed;
    top: 12px;
    right: 18px;
    z-index: 80;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px 6px 10px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    box-shadow: var(--shadow-2, 0 8px 24px rgba(0, 0, 0, 0.4));
  }
  .findbar-input { width: 200px; }
  .findbar-stat { font-size: 11px; color: var(--warn); white-space: nowrap; }
</style>
