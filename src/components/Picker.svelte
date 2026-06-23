<script lang="ts">
  import Icon from "./Icon.svelte";
  import { menuPosition, menuStyle, type MenuPos } from "../lib/dropdown";

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
  let menuPos = $state<MenuPos | null>(null);
  let btnEl = $state<HTMLButtonElement | null>(null);
  let menuEl = $state<HTMLDivElement | null>(null);

  const cur = $derived(options.find((o) => o.id === value));

  // Anchor the menu to the trigger as a viewport-fixed box (escapes overflow clipping),
  // flipping up / shifting on-screen as needed. Recomputed every time it opens.
  function toggle() {
    if (!open && btnEl) menuPos = menuPosition(btnEl.getBoundingClientRect());
    open = !open;
  }

  // A fixed menu doesn't follow the page — close it if the user scrolls.
  $effect(() => {
    if (!open) return;
    const close = (e: Event) => {
      const target = e.target;
      if (menuEl && target instanceof Node && menuEl.contains(target)) return;
      open = false;
    };
    window.addEventListener("scroll", close, true);
    return () => window.removeEventListener("scroll", close, true);
  });
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
  {#if open && menuPos}
    <div class="picker-back" role="presentation" onclick={() => (open = false)}></div>
    <div bind:this={menuEl} class="picker-menu" style={menuStyle(menuPos)}>
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
