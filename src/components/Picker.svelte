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

  const cur = $derived(options.find((o) => o.id === value));
</script>

<div class="picker">
  <button
    type="button"
    class={"picker-btn" + (open ? " open" : "")}
    onclick={() => (open = !open)}
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
    <div class="picker-menu">
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
