<script lang="ts">
  import { jobs, jobKindLabel, type Job } from "../lib/jobs.svelte";
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  let { job }: { job: Job } = $props();

  async function cancel() {
    const ok = await app.confirm({
      title: `Stop generating ${jobKindLabel(job.kind)}?`,
      body: "It stops being tracked and the result is discarded.",
      danger: true,
      okLabel: "Stop",
    });
    if (ok) jobs.cancel(job.id);
  }
</script>

{#if job.status === "running"}
  <div class="gen-card">
    <span class="is-spin"></span>
    <span class="gen-card-label mono">Generating {jobKindLabel(job.kind)}…</span>
    {#if job.label}
      <span class="gen-card-sub mono">{job.label}</span>
    {/if}
    <button
      class="gen-card-x"
      style="margin-left:auto"
      type="button"
      aria-label="Stop generating"
      title="Stop generating"
      onclick={cancel}
    >
      <Icon name="x" size={13} />
    </button>
  </div>
{:else if job.status === "error"}
  <div class="gen-card gen-card--err">
    <span class="gen-card-ico"><Icon name="bolt" size={14} color="var(--err)" /></span>
    <div class="gen-card-body">
      <span class="gen-card-label mono">Couldn't generate {jobKindLabel(job.kind)}</span>
      <span class="gen-card-sub mono">{job.error ?? "Unknown error"}</span>
    </div>
    <button
      class="gen-card-x"
      type="button"
      aria-label="Dismiss"
      title="Dismiss"
      onclick={() => jobs.dismiss(job.id)}
    >
      <Icon name="x" size={13} />
    </button>
  </div>
{/if}

<style>
  .gen-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px;
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4, 10px);
    background: var(--surface-2);
    margin-bottom: var(--sp-3, 12px);
  }
  .gen-card-label {
    color: var(--fg-bright);
    font-size: var(--t-sm);
  }
  .gen-card-sub {
    color: var(--fg-faint);
    font-size: var(--t-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .gen-card--err {
    border-color: color-mix(in oklab, var(--err) 45%, transparent);
    background: color-mix(in oklab, var(--err) 8%, var(--surface));
    align-items: flex-start;
  }
  .gen-card--err .gen-card-label {
    color: var(--err);
  }
  .gen-card-ico {
    display: inline-flex;
    margin-top: 1px;
  }
  .gen-card-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }
  .gen-card-body .gen-card-sub {
    white-space: normal;
    color: var(--fg-muted);
  }
  .gen-card-x {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--rad-2, 6px);
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
  }
  .gen-card-x:hover {
    color: var(--fg-bright);
    background: var(--surface);
    border-color: var(--border-strong);
  }
</style>
