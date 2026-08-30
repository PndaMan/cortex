<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";

  // The status-bar "PWD": subject › topic › source, reflecting where the user is
  // (a source view, or the current chat scope). Uses Design-System scope styling.
  type Crumb = { label: string; color?: string; glyph?: string; act?: () => void };
  const crumbs = $derived.by(() => {
    const subj = app.activeSubject;
    if (!subj) return [] as Crumb[];
    const out: Crumb[] = [
      { label: subj.name, color: app.subjectColor(subj), glyph: subj.glyph || "◆", act: () => app.openSubject(subj.id) },
    ];
    const src = app.activeSource;
    if (app.view === "source" && src) {
      const topic = subj.topics.find((t) => t.id === src.topic_id);
      if (topic) out.push({ label: topic.name, act: () => app.openSubject(subj.id) });
      out.push({ label: src.name, act: () => app.openSource(src) });
    } else if (app.chatScope) {
      if (app.chatScope.topicName) out.push({ label: app.chatScope.topicName });
      if (app.chatScope.sourceName) out.push({ label: app.chatScope.sourceName });
    }
    return out;
  });
</script>

<!-- Helix-style status bar. Mode block kept; leader/help/command are buttons. -->
<div class="statusbar mode-{app.mode}">
  <div class="mode-block">
    {app.mode}
    {#if app.activeSubject}
      <span class="mode-ctx">{app.activeSubject.code ?? app.activeSubject.name}</span>
    {/if}
  </div>

  <button
    class="sb-seg sb-seg-btn"
    type="button"
    title={t("Leader actions (Space)")}
    onclick={() => (app.leaderOpen = true)}
  >
    <span class="sb-key">␣</span> {t("actions")}
  </button>

  <!-- scope breadcrumb (present working directory) -->
  <div class="sb-seg sb-scope">
    {#if crumbs.length}
      {#each crumbs as c, i}
        {#if i > 0}<span class="scope-sep">›</span>{/if}
        {#if c.act}
          <button type="button" class="scope-seg scope-seg--btn" class:is-last={i === crumbs.length - 1} onclick={c.act} title={c.label}>
            {#if c.glyph}<span class="seg-ico" style:color={c.color ?? "var(--accent)"}>{c.glyph}</span>{/if}
            {c.label}
          </button>
        {:else}
          <span class="scope-seg" class:is-last={i === crumbs.length - 1}>
            {#if c.glyph}<span class="seg-ico" style:color={c.color ?? "var(--accent)"}>{c.glyph}</span>{/if}
            {c.label}
          </span>
        {/if}
      {/each}
    {:else}
      <span class="seg-ico" style="color:var(--accent)">◆</span> Cortex
    {/if}
  </div>

  <div class="sb-spacer"></div>

  <button class="sb-seg sb-seg-btn" type="button" title={t("Keyboard shortcuts (?)")} onclick={() => (app.helpOpen = true)}>
    <span class="sb-key">?</span> {t("help")}
  </button>

  <button class="sb-seg sb-seg-btn" type="button" title={t("Command palette :)")} style:border-right="none" onclick={() => (app.cmdkOpen = true)}>
    <span class="sb-key">:</span> {t("command")}
  </button>
</div>

<style>
  .mode-block .mode-ctx {
    margin-left: 8px; font-weight: 600; letter-spacing: 0.04em; opacity: 0.92;
  }
  .sb-scope { display: flex; align-items: center; gap: 6px; min-width: 0; max-width: 52vw; }
  .sb-scope .scope-seg {
    display: inline-flex; align-items: center; gap: 5px; min-width: 0;
    color: var(--fg-muted);
  }
  .sb-scope .scope-seg.is-last {
    color: var(--fg-bright);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .sb-scope .seg-ico { font-size: 10px; flex: none; }
  .sb-scope .scope-sep { color: var(--fg-faint); flex: none; }
  /* clickable crumbs navigate to that subject/source */
  .sb-scope .scope-seg--btn {
    background: none; border: none; font: inherit; padding: 0; cursor: pointer;
    transition: color 0.12s ease;
  }
  .sb-scope .scope-seg--btn:hover { color: var(--accent); }

  .statusbar .sb-seg-btn {
    border: none; border-right: 1px solid var(--border); background: none;
    font: inherit; cursor: pointer; color: var(--fg-faint);
    transition: background 0.12s ease, color 0.12s ease;
  }
  .statusbar .sb-seg-btn:hover { color: var(--fg-bright); background: var(--surface-2); }
  .statusbar .sb-seg-btn .sb-key {
    transition: color 0.12s ease, border-color 0.12s ease;
  }
  .statusbar .sb-seg-btn:hover .sb-key { color: var(--accent); border-color: var(--accent); }
  .statusbar .sb-seg-btn:active { transform: translateY(0.5px); }
</style>
