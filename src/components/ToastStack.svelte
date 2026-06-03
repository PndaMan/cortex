<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  const icoFor: Record<string, string> = {
    success: "check",
    info: "diamond",
    warning: "bolt",
    error: "x",
  };

  function colorFor(kind: string) {
    if (kind === "success") return "var(--ok)";
    if (kind === "warning") return "var(--warn)";
    if (kind === "error") return "var(--err)";
    return "var(--info)";
  }
</script>

<div class="toast-stack">
  {#each app.toasts as t (t.id)}
    <div class="toast toast--{t.kind}" role="status">
      <span class="toast-ico" style:color={colorFor(t.kind)}>
        <Icon name={icoFor[t.kind] ?? "diamond"} size={14} />
      </span>
      <div class="grow">
        <div class="toast-title">{t.title}</div>
        {#if t.body}
          <div class="toast-body">{t.body}</div>
        {/if}
      </div>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="kbd" style:cursor="pointer" onclick={() => app.dismissToast(t.id)}>q</span>
    </div>
  {/each}
</div>
