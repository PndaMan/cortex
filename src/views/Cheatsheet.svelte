<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { CheatsheetData, CsSection as ApiCsSection } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import { jobs } from "../lib/jobs.svelte";
  import GeneratingCard from "../components/GeneratingCard.svelte";
  import RichText from "../components/RichText.svelte";

  // Background cheatsheet jobs for the active subject (running + any errors to surface).
  const csJobs = $derived(
    jobs.forSubject(app.activeSubjectId).filter(
      (j) => j.kind === "cheatsheet" && (j.status === "running" || j.status === "error")
    )
  );
  const csGenerating = $derived(csJobs.some((j) => j.status === "running"));

  // ── reactive data ────────────────────────────────────────────
  let sections = $state<ApiCsSection[]>([]);
  let cheatTopic = $state<string>("");
  let cheatSub = $state<string>("");
  let sourceCount = $state<number>(0);
  let hasCheatsheet = $state(false);  // true = real cheatsheet loaded

  // Track sections that were just approved (for flash animation).
  let recentlyApproved = $state<Record<string, boolean>>({});

  // ── load cheatsheet whenever activeSubject changes ────────────
  $effect(() => {
    const sub = app.activeSubject;
    if (!sub) {
      sections = [];
      cheatTopic = "";
      cheatSub = "";
      sourceCount = 0;
      hasCheatsheet = false;
      return;
    }

    const topicId = sub.topics[0]?.id;

    api.getCheatsheet(sub.id, topicId).then((data) => {
      if (data) {
        sections = data.sections;
        cheatTopic = data.topic;
        cheatSub = data.subject;
        sourceCount = data.sources;
        hasCheatsheet = true;
        app.pending = data.sections.filter((s) => s.state === "draft-pending").length;
      } else {
        sections = [];
        cheatTopic = sub.topics[0]?.name ?? "";
        cheatSub = sub.name;
        sourceCount = sub.sourceCount;
        hasCheatsheet = false;
        app.pending = 0;
      }
    }).catch(() => {
      sections = [];
      cheatTopic = sub.topics[0]?.name ?? "";
      cheatSub = sub.name;
      sourceCount = sub.sourceCount;
      hasCheatsheet = false;
      app.pending = 0;
    });
  });

  const sectionCount = $derived(sections.length);

  // Apply a freshly-loaded cheatsheet to local view state.
  function applyCheatsheet(data: CheatsheetData) {
    sections = data.sections;
    cheatTopic = data.topic;
    cheatSub = data.subject;
    sourceCount = data.sources;
    hasCheatsheet = true;
    app.pending = data.sections.filter((s) => s.state === "draft-pending").length;
  }

  // ── generate / regenerate action (runs in the BACKGROUND) ─────
  // Kicks off generation through the global jobs store so navigating away does
  // not cancel it; on completion we reload the persisted cheatsheet from the DB.
  function generate() {
    const sub = app.activeSubject;
    if (!sub || csGenerating) return;
    const topicId = sub.topics[0]?.id;
    jobs.start({
      kind: "cheatsheet",
      label: sub.name,
      subjectId: sub.id,
      topicId,
      run: () => api.generateCheatsheet(sub.id, topicId),
      onDone: () => {
        // Only refresh if the user is still on this subject.
        if (app.activeSubjectId !== sub.id) return;
        api.getCheatsheet(sub.id, topicId).then((data) => {
          if (data) applyCheatsheet(data);
        }).catch(() => {});
      },
    });
  }
</script>

<div class="workspace-scroll">
  <div class="cs-doc">
    {#each csJobs as job (job.id)}
      <GeneratingCard {job} />
    {/each}
    {#if !app.activeSubject}
      <!-- No subject open -->
      <div class="cs-empty-state">
        <Icon name="diamond" size={28} color="var(--fg3)" />
        <div class="ces-title">No subject open</div>
        <div class="ces-sub">Open a subject from the sidebar to view its cheatsheet.</div>
      </div>
    {:else if !hasCheatsheet}
      <!-- Subject open but no cheatsheet generated yet -->
      {@const noSources = app.activeSubject.sourceCount === 0}
      <div class="cs-working">
        <div class="cs-working-ico">
          <Icon name="diamond" size={26} color="var(--fg3)" />
        </div>
        <h1 class="cs-working-title read">No cheatsheet yet</h1>
        <p class="cs-working-sub mono muted">
          {#if noSources}
            Add sources to this subject first — your cheatsheet is synthesized from them.
          {:else}
            A completeness-checked cheatsheet will be generated from this subject's {app.activeSubject.sourceCount} source{app.activeSubject.sourceCount !== 1 ? "s" : ""}.
          {/if}
        </p>
        <button class="btn btn--primary btn--sm" onclick={generate} disabled={csGenerating}>
          <Icon name="refresh" size={13} /> {csGenerating ? "Synthesizing…" : "Generate cheatsheet"}
        </button>
      </div>
    {:else}
      <!-- Document header -->
      <div class="cs-doc-head">
        <div>
          <div class="eyebrow">Cheatsheet</div>
          <h1 class="cs-title">{cheatTopic}</h1>
          <div class="cs-sub mono">
            {app.activeSubject.name}{app.activeSubject.code ? " · " + app.activeSubject.code : ""} ·
            synthesized from {sourceCount} source{sourceCount !== 1 ? "s" : ""} · {sectionCount} enforced sections
          </div>
        </div>
        <div class="cs-doc-actions">
          <button class="btn btn--sm" onclick={() => window.print()} title="Export / print this cheatsheet as PDF">
            <Icon name="doc" size={13} /> Export PDF
          </button>
          <button class="btn btn--sm" onclick={generate} disabled={csGenerating}>
            <Icon name="refresh" size={13} /> {csGenerating ? "Synthesizing…" : "Regenerate"}
          </button>
        </div>
      </div>

      <!-- Sections -->
      <div class="cs-sections">
        {#each sections as sec (sec.id)}
          {@const isRecent = !!recentlyApproved[sec.id]}
          <section
            class="cs-section{isRecent ? ' just-approved' : ''}{sec.state === 'draft-pending' ? ' is-pending' : ''}"
          >
            <header class="cs-sec-head">
              <h2 class="cs-sec-title">{sec.title}</h2>

              {#if sec.state === "draft-pending"}
                <span class="status-pill status-pill--draft">
                  <span class="dot"></span>pending
                </span>
              {/if}

              {#if isRecent}
                <span class="status-pill status-pill--ready">
                  <span class="dot"></span>just approved
                </span>
              {/if}

              {#if (sec.state === "approved" || sec.state === "idle") && !isRecent}
                <span class="cs-sec-count mono">{sec.items.length}</span>
              {/if}
            </header>

            <dl class="cs-list">
              {#each sec.items as item, i (i)}
                <div class="cs-item{(item as any).flag === 'changed' ? ' item-changed' : ''}">
                  <dt>{item.t}</dt>
                  <dd><RichText text={item.d} /></dd>
                </div>
              {/each}
            </dl>
          </section>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  /* Centered empty / working states — mirrors GenerateMaterial's .genmat--working */
  .cs-empty-state,
  .cs-working {
    min-height: 60vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 12px;
  }
  .cs-working-ico {
    width: 54px;
    height: 54px;
    border-radius: 50%;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    display: grid;
    place-items: center;
    margin-bottom: 4px;
  }
  .cs-working-title {
    font-size: var(--r-xl);
    color: var(--fg-bright);
    font-weight: 500;
  }
  .cs-working-sub,
  .cs-empty-state .ces-sub {
    max-width: 380px;
    font-size: var(--t-sm);
    line-height: 1.55;
  }
  .cs-empty-state .ces-title {
    font-size: var(--r-lg);
    color: var(--fg-bright);
    font-weight: 500;
  }
  .cs-working .btn {
    margin-top: 4px;
  }

  /* ── PRINT / EXPORT PDF ─────────────────────────────────────
     window.print() exports the cheatsheet. Hide all app chrome and
     show only the cheatsheet document, black-on-white, full width. */
  @media print {
    :global(.sidebar),
    :global(.statusbar),
    :global(.chatdock),
    :global(.chat-fab) {
      display: none !important;
    }
    :global(html),
    :global(body),
    :global(.app-shell),
    :global(.app-main),
    :global(.workspace) {
      display: block !important;
      height: auto !important;
      overflow: visible !important;
      background: #fff !important;
      color: #000 !important;
      margin: 0 !important;
    }
    :global(.app-shell) { grid-template-columns: 1fr !important; }
    .workspace-scroll {
      overflow: visible !important;
      height: auto !important;
      padding: 0 !important;
    }
    .cs-doc {
      max-width: none !important;
      width: 100% !important;
      margin: 0 !important;
      padding: 0 !important;
      color: #000 !important;
    }
    /* hide the action buttons (regenerate / export) when printing */
    .cs-doc-actions { display: none !important; }
    /* keep each section from splitting awkwardly across pages */
    :global(.cs-section) { break-inside: avoid; }
    :global(.cs-item) { break-inside: avoid; }
    :global(.cs-title),
    :global(.cs-sec-title) { color: #000 !important; }
    /* readable tables/callouts on white paper */
    :global(.rt-table th) { background: #f0f0f0 !important; color: #000 !important; }
    :global(.rt-callout-label) { color: #000 !important; }
    :global(.rt-callout) { background: #f6f6f6 !important; }
    :global(.workspace-scroll), :global(.cs-doc), :global(.rt-callout-body),
    :global(.rt-table td), :global(.rt-p), :global(.cs-item dt), :global(.cs-item dd) {
      color: #000 !important;
    }
  }
</style>
