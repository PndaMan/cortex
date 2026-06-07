<script lang="ts">
  import Icon from "./Icon.svelte";

  let {
    value,
    onChange,
    options,
    icon,
    placeholder,
  }: {
    value: string;
    onChange: (id: string) => void;
    options: { id: string; label: string; glyph?: boolean }[];
    icon?: string;
    placeholder?: string;
  } = $props();

  let open = $state(false);
  let dropUp = $state(false);
  let btnEl = $state<HTMLButtonElement | null>(null);

  const cur = $derived(options.find((o) => o.id === value));

  // Open upward when there isn't room below the trigger (e.g. a picker near the
  // bottom of the screen) and there's more room above — so the menu never clips
  // off-screen. Recomputed every time it opens.
  function toggle() {
    if (!open && btnEl) {
      const rect = btnEl.getBoundingClientRect();
      const below = window.innerHeight - rect.bottom;
      const above = rect.top;
      // Approx menu height: capped at the CSS max-height (220) + chrome.
      const needed = Math.min(232, options.length * 30 + 16);
      dropUp = below < needed && above > below;
    }
    open = !open;
  }
</script>

<div class="picker">
  <button
    type="button"
    bind:this={btnEl}
    class={"picker-btn" + (open ? " open" : "")}
    onclick={toggle}
  >
    {#if icon}
      <Icon name={icon} size={12} color="var(--fg-faint)" />
    {/if}
    <span class={"picker-val" + (cur ? "" : " ph")}>
      {cur ? cur.label : (placeholder ?? "Select…")}
    </span>
    <Icon name="chevron" size={11} style="transform:rotate(90deg);color:var(--fg-faint)" />
  </button>
  {#if open}
    <div class="picker-back" role="presentation" onclick={() => (open = false)}></div>
    <div class={"picker-menu" + (dropUp ? " up" : "")}>
      {#each options as o}
        <button
          type="button"
          class={"picker-item" + (o.id === value ? " on" : "")}
          onclick={() => { onChange(o.id); open = false; }}
        >
          {#if o.glyph}
            <Icon name="diamond" size={11} color="var(--accent)" />
          {/if}
          <span class="grow" style="text-align:left">{o.label}</span>
          {#if o.id === value}
            <Icon name="check" size={12} color="var(--accent)" />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>
