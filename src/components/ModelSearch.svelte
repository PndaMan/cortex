<script lang="ts">
  // A searchable model combobox — same chrome as Picker.svelte (reuses the global
  // .picker-* classes), with a sticky search box + a scrollable, filtered, capped
  // list and a price/context sub-line per row. Used for the model column in Settings
  // so the full 300+ OpenRouter catalog is searchable, not a fixed dropdown.
  import Icon from "./Icon.svelte";
  import { tick } from "svelte";

  let {
    value,
    onChange,
    options,
    icon,
    placeholder,
    loading = false,
    onOpen,
  }: {
    value: string;
    onChange: (id: string) => void;
    options: { id: string; label: string; sub?: string; recommended?: boolean }[];
    icon?: string;
    placeholder?: string;
    loading?: boolean;
    onOpen?: () => void;
  } = $props();

  const CAP = 60; // render at most this many rows; keep typing to narrow

  let open = $state(false);
  let dropUp = $state(false);
  let q = $state("");
  let btnEl = $state<HTMLButtonElement | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);

  const cur = $derived(options.find((o) => o.id === value));
  const filtered = $derived.by(() => {
    const s = q.trim().toLowerCase();
    if (!s) return options;
    return options.filter(
      (o) => o.id.toLowerCase().includes(s) || o.label.toLowerCase().includes(s),
    );
  });
  const shown = $derived(filtered.slice(0, CAP));

  function toggle() {
    if (!open) {
      if (btnEl) {
        const rect = btnEl.getBoundingClientRect();
        const below = window.innerHeight - rect.bottom;
        dropUp = below < 320 && rect.top > below;
      }
      q = "";
      onOpen?.();
      void tick().then(() => inputEl?.focus());
    }
    open = !open;
  }
  function pick(id: string) {
    onChange(id);
    open = false;
  }
  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); open = false; }
    else if (e.key === "Enter" && shown.length) { e.preventDefault(); pick(shown[0].id); }
  }
</script>

<div class="picker">
  <button
    type="button"
    bind:this={btnEl}
    class={"picker-btn" + (open ? " open" : "")}
    onclick={toggle}
  >
    {#if icon}<Icon name={icon} size={12} color="var(--fg-faint)" />{/if}
    <span class={"picker-val" + (cur || value ? "" : " ph")}>
      {cur ? cur.label : value || placeholder || "Select…"}
    </span>
    <Icon name="chevron" size={11} style="transform:rotate(90deg);color:var(--fg-faint)" />
  </button>
  {#if open}
    <div class="picker-back" role="presentation" onclick={() => (open = false)}></div>
    <div class={"picker-menu ms-menu" + (dropUp ? " up" : "")}>
      <div class="ms-search">
        <Icon name="search" size={12} color="var(--fg-faint)" />
        <!-- svelte-ignore a11y_autofocus -->
        <input
          bind:this={inputEl}
          bind:value={q}
          onkeydown={onSearchKey}
          class="ms-input mono"
          placeholder={loading ? "Loading models…" : "Search models…"}
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <div class="ms-list">
        {#if loading && options.length === 0}
          <div class="ms-empty">Loading models…</div>
        {:else if shown.length === 0}
          <div class="ms-empty">No models match “{q}”</div>
        {:else}
          {#each shown as o (o.id)}
            <button
              type="button"
              class={"picker-item ms-item" + (o.id === value ? " on" : "")}
              onclick={() => pick(o.id)}
            >
              {#if o.recommended}<Icon name="diamond" size={10} color="var(--accent)" />{/if}
              <span class="grow ms-text">
                <span class="ms-label">{o.label}</span>
                {#if o.sub}<span class="ms-sub mono">{o.sub}</span>{/if}
              </span>
              {#if o.id === value}<Icon name="check" size={12} color="var(--accent)" />{/if}
            </button>
          {/each}
          {#if filtered.length > shown.length}
            <div class="ms-more">+{filtered.length - shown.length} more — keep typing to narrow</div>
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  /* Reuses global .picker / .picker-btn / .picker-menu / .picker-item / .picker-back /
     .picker-val (app.css). These are the search-specific overrides + additions. */
  .ms-menu {
    width: max(300px, 100%);
    padding: 0;
    max-height: 340px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .ms-search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex: none;
  }
  .ms-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--fg-bright);
    font-size: var(--t-sm);
  }
  .ms-list { overflow-y: auto; padding: 4px; }
  .ms-item { height: auto; min-height: 30px; padding-top: 4px; padding-bottom: 4px; }
  .ms-text { display: flex; flex-direction: column; gap: 1px; min-width: 0; text-align: left; }
  .ms-label { color: inherit; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .ms-sub { font-size: var(--t-2xs); color: var(--fg-faint); }
  .ms-empty,
  .ms-more { padding: 12px 10px; text-align: center; color: var(--fg-faint); font-size: var(--t-2xs); }
</style>
