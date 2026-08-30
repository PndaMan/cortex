<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n.svelte";

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
  {#each app.toasts as toast (toast.id)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="toast toast--{toast.kind}" role="status" style:cursor="pointer" onclick={() => app.dismissToast(toast.id)} title={t("Dismiss")}>
      <span class="toast-ico" style:color={colorFor(toast.kind)}>
        <Icon name={icoFor[toast.kind] ?? "diamond"} size={14} />
      </span>
      <div class="grow">
        <div class="toast-title">{toast.title}</div>
        {#if toast.body}
          <div class="toast-body">{toast.body}</div>
        {/if}
      </div>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span class="kbd" style:cursor="pointer" onclick={() => app.dismissToast(toast.id)}>q</span>
    </div>
  {/each}
</div>
