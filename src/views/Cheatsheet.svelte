<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { CheatsheetData, CsSection as ApiCsSection } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import { jobs } from "../lib/jobs.svelte";
  import GeneratingCard from "../components/GeneratingCard.svelte";
  import RichText from "../components/RichText.svelte";
  import { savePdf } from "../lib/pdf";

  const esc = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  // ── jump index ───────────────────────────────────────────────
  let indexPinned = $state(false);
  // Per-section expand state (collapsed by default so the index stays compact).
  let idxExpanded = $state<Record<string, boolean>>({});
  function toggleIdxSec(id: string) {
    idxExpanded = { ...idxExpanded, [id]: !idxExpanded[id] };
  }
  // Offset the floating index left of the chat dock when it's showing.
  const chatDockOpen = $derived(
    app.chatOpen && app.view === "subject" && app.subjectTab !== "chats"
  );
  function jumpTo(id: string) {
    document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  // ── topic selection ──────────────────────────────────────────
  // null = the whole-subject cheatsheet ("All"); otherwise a topic id.
  let selectedTopicId = $state<string | null>(null);

  // Topics that actually have sources — only these get their own cheatsheet tab.
  const topicTabs = $derived(
    (app.activeSubject?.topics ?? []).filter((t) => t.sources.length > 0)
  );

  // Subtle per-subject accent for the active pill.
  const accent = $derived(app.subjectColor(app.activeSubject));

  // Background cheatsheet jobs for the active subject, filtered to the CURRENT
  // selection (running + any errors to surface). topicId is normalized: the
  // whole-subject job carries topicId === null.
  const csJobs = $derived(
    jobs
      .forSubject(app.activeSubjectId)
      .filter(
        (j) =>
          j.kind === "cheatsheet" &&
          (j.status === "running" || j.status === "error") &&
          (j.topicId ?? null) === selectedTopicId
      )
  );
  const csGenerating = $derived(csJobs.some((j) => j.status === "running"));

  // ── reactive data (the currently displayed sheet) ────────────
  let sections = $state<ApiCsSection[]>([]);
  let cheatTopic = $state<string>("");
  let sourceCount = $state<number>(0);
  let hasCheatsheet = $state(false); // true = a real stored cheatsheet is loaded
  let loading = $state(false);

  const sectionCount = $derived(sections.length);

  // Reset the selection to "whole subject" whenever the active subject changes,
  // and drop any tab that no longer has sources.
  $effect(() => {
    const sub = app.activeSubject;
    if (!sub) {
      selectedTopicId = null;
      return;
    }
    if (
      selectedTopicId !== null &&
      !sub.topics.some((t) => t.id === selectedTopicId && t.sources.length > 0)
    ) {
      selectedTopicId = null;
    }
  });

  // Apply a freshly-loaded cheatsheet to local view state.
  function applyCheatsheet(data: CheatsheetData) {
    sections = data.sections;
    cheatTopic = data.topic;
    sourceCount = data.sources;
    hasCheatsheet = true;
    app.pending = data.sections.filter((s) => s.state === "draft-pending").length;
  }

  // Clear local view state to the "nothing generated yet" placeholder.
  function applyEmpty() {
    const sub = app.activeSubject;
    sections = [];
    sourceCount =
      selectedTopicId === null
        ? (sub?.sourceCount ?? 0)
        : (sub?.topics.find((t) => t.id === selectedTopicId)?.sources.length ?? 0);
    cheatTopic =
      selectedTopicId === null
        ? (sub?.name ?? "")
        : (sub?.topics.find((t) => t.id === selectedTopicId)?.name ?? "");
    hasCheatsheet = false;
    app.pending = 0;
  }

  // ── load the stored cheatsheet for the active subject + selection ──
  // Re-runs when the active subject OR the selected topic changes.
  $effect(() => {
    const sub = app.activeSubject;
    // Touch selectedTopicId so this effect tracks it.
    const topicId = selectedTopicId;
    if (!sub) {
      applyEmpty();
      return;
    }
    loading = true;
    let cancelled = false;
    api
      .getCheatsheet(sub.id, topicId ?? undefined)
      .then((data) => {
        if (cancelled) return;
        if (data) applyCheatsheet(data);
        else applyEmpty();
      })
      .catch(() => {
        if (!cancelled) applyEmpty();
      })
      .finally(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  // ── generate / regenerate for the CURRENT selection (BACKGROUND) ──
  // Runs through the global jobs store so navigating away won't cancel it; on
  // completion we reload the persisted cheatsheet for that exact selection.
  function generate() {
    const sub = app.activeSubject;
    if (!sub || csGenerating) return;
    const topicId = selectedTopicId; // capture the selection at click time
    const topicName =
      topicId === null
        ? sub.name
        : (sub.topics.find((t) => t.id === topicId)?.name ?? sub.name);
    jobs.start({
      kind: "cheatsheet",
      label: topicName,
      subjectId: sub.id,
      topicId,
      run: () => api.generateCheatsheet(sub.id, topicId ?? undefined),
      onDone: () => {
        // Only refresh the view if the user is still on this subject AND still
        // looking at the selection we generated for.
        if (app.activeSubjectId !== sub.id || selectedTopicId !== topicId) return;
        api
          .getCheatsheet(sub.id, topicId ?? undefined)
          .then((data) => {
            if (data) applyCheatsheet(data);
          })
          .catch(() => {});
      },
    });
  }

  // Command-palette "Regenerate cheatsheet" bumps a nonce; regenerate the
  // whole-subject sheet here, where generation + live refresh already live.
  let lastRegenNonce = app.cheatsheetRegenNonce;
  $effect(() => {
    const n = app.cheatsheetRegenNonce;
    if (n !== lastRegenNonce) {
      lastRegenNonce = n;
      if (app.activeSubject && !csGenerating) {
        selectedTopicId = null; // whole-subject scope
        generate();
      }
    }
  });

  // Sidebar topic/subject clicks ask the cheatsheet to show a specific topic
  // (null = whole subject) by bumping app.cheatNonce.
  let lastCheatNonce = app.cheatNonce;
  $effect(() => {
    const n = app.cheatNonce;
    if (n !== lastCheatNonce) {
      lastCheatNonce = n;
      if (app.activeSubject) selectedTopicId = app.cheatTopicId;
    }
  });

  function selectTopic(id: string | null) {
    if (selectedTopicId === id) return;
    selectedTopicId = id;
  }

  // ── EXPORT ───────────────────────────────────────────────────
  // Render the current sheet (single topic OR whole subject) to a real PDF via
  // the backend (headless Chromium) — reliable where webview print() isn't.
  async function exportCurrent() {
    const el = document.querySelector(".cs-doc .cs-sections");
    const sub = app.activeSubject;
    if (!el || !sub || !hasCheatsheet) {
      app.pushToast({ kind: "warning", title: "Nothing to export", body: "Generate a cheatsheet first." });
      return;
    }
    const subtitle = `${sub.name}${sub.code ? " · " + sub.code : ""} · synthesized from ${sourceCount} source${sourceCount !== 1 ? "s" : ""} · ${sectionCount} enforced sections`;
    const body =
      `<article class="cs-doc"><div class="cs-doc-head"><div>` +
      `<div class="eyebrow">Cheatsheet · ${selectedTopicId === null ? "Whole subject" : "Topic"}</div>` +
      `<h1 class="cs-title">${esc(cheatTopic)}</h1>` +
      `<div class="cs-sub">${esc(subtitle)}</div></div></div>` +
      `<div class="cs-sections">${el.innerHTML}</div></article>`;
    await savePdf(body, `${sub.name} — ${cheatTopic}`);
  }

  // "Export all" loads the whole-subject sheet PLUS every topic-with-sources
  // sheet, renders them all into a hidden print-only block, then prints. Topics
  // without a stored sheet are skipped. We fall back to printing whatever is
  // already on screen if nothing could be loaded.
  let exportAll = $state<CheatsheetData[]>([]);
  let exporting = $state(false);

  async function exportWholeSubject() {
    const sub = app.activeSubject;
    if (!sub || exporting) return;
    exporting = true;
    try {
      const ids: (string | undefined)[] = [
        undefined, // whole-subject sheet first
        ...topicTabs.map((t) => t.id),
      ];
      const loaded = await Promise.all(
        ids.map((tid) => api.getCheatsheet(sub.id, tid).catch(() => null))
      );
      exportAll = loaded.filter((d): d is CheatsheetData => !!d);
      if (exportAll.length === 0) {
        app.pushToast({
          kind: "warning",
          title: "Nothing to export",
          body: "No cheatsheets have been generated for this subject yet.",
        });
        return;
      }
      // Let the hidden export block render, then serialize it to the PDF.
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      const block = document.querySelector(".cs-export-all");
      if (block) {
        await savePdf(block.innerHTML, `${sub.name} — all cheatsheets`);
      }
    } finally {
      exporting = false;
      // Clear after the print dialog returns so the hidden block doesn't linger.
      setTimeout(() => {
        exportAll = [];
      }, 0);
    }
  }
</script>

<div class="workspace-scroll">
  <div class="cs-doc{exportAll.length > 0 ? ' is-exporting-all' : ''}">
    {#if !app.activeSubject}
      <!-- No subject open -->
      <div class="cs-empty-state">
        <Icon name="diamond" size={28} color="var(--fg3)" />
        <div class="ces-title">No subject open</div>
        <div class="ces-sub">Open a subject from the sidebar to view its cheatsheet.</div>
      </div>
    {:else}
      {@const sub = app.activeSubject}

      <!-- ── TOPIC TAB BAR ──────────────────────────────────── -->
      <div class="cs-tabs" role="tablist" aria-label="Cheatsheet scope">
        <button
          class="cs-tab{selectedTopicId === null ? ' is-active' : ''}"
          role="tab"
          aria-selected={selectedTopicId === null}
          style={selectedTopicId === null ? `--tab-accent:${accent}` : ""}
          onclick={() => selectTopic(null)}
        >
          <Icon name="grid" size={12} /> Whole subject
        </button>
        {#each topicTabs as t (t.id)}
          <button
            class="cs-tab{selectedTopicId === t.id ? ' is-active' : ''}"
            role="tab"
            aria-selected={selectedTopicId === t.id}
            style={selectedTopicId === t.id ? `--tab-accent:${accent}` : ""}
            onclick={() => selectTopic(t.id)}
          >
            {#if t.glyph}<span class="cs-tab-glyph">{t.glyph}</span>{/if}
            {t.name}
            <span class="cs-tab-count mono">{t.sources.length}</span>
          </button>
        {/each}
      </div>

      <!-- Running / errored jobs for the current selection -->
      {#each csJobs as job (job.id)}
        <GeneratingCard {job} />
      {/each}

      {#if loading && !hasCheatsheet}
        <!-- brief loading placeholder while we fetch the stored sheet -->
        <div class="cs-working">
          <div class="cs-working-ico">
            <Icon name="diamond" size={26} color="var(--fg3)" />
          </div>
          <p class="cs-working-sub mono muted">Loading cheatsheet…</p>
        </div>
      {:else if !hasCheatsheet}
        <!-- Selection has no stored cheatsheet yet -->
        {@const scopeSources =
          selectedTopicId === null
            ? sub.sourceCount
            : (sub.topics.find((t) => t.id === selectedTopicId)?.sources.length ?? 0)}
        {@const noSources = scopeSources === 0}
        <div class="cs-working">
          <div class="cs-working-ico">
            <Icon name="diamond" size={26} color="var(--fg3)" />
          </div>
          <h1 class="cs-working-title read">No cheatsheet yet</h1>
          <p class="cs-working-sub mono muted">
            {#if noSources}
              Add sources to {selectedTopicId === null
                ? "this subject"
                : "this topic"} first — your cheatsheet is synthesized from them.
            {:else}
              A completeness-checked cheatsheet will be generated from
              {selectedTopicId === null ? "this subject's" : "this topic's"}
              {scopeSources} source{scopeSources !== 1 ? "s" : ""}.
            {/if}
          </p>
          <button class="btn btn--primary btn--sm" onclick={generate} disabled={csGenerating || noSources}>
            <Icon name="refresh" size={13} /> {csGenerating ? "Synthesizing…" : "Generate cheatsheet"}
          </button>
        </div>
      {:else}
        <!-- Document header -->
        <div class="cs-doc-head">
          <div>
            <div class="eyebrow">
              Cheatsheet · {selectedTopicId === null ? "Whole subject" : "Topic"}
            </div>
            <h1 class="cs-title">{cheatTopic}</h1>
            <div class="cs-sub mono">
              {sub.name}{sub.code ? " · " + sub.code : ""} ·
              synthesized from {sourceCount} source{sourceCount !== 1 ? "s" : ""} ·
              {sectionCount} enforced sections
            </div>
          </div>
          <div class="cs-doc-actions">
            <button class="btn btn--sm" onclick={exportCurrent} title="Opens the print dialog — choose “Save as PDF”">
              <Icon name="doc" size={13} /> Save as PDF
            </button>
            <button
              class="btn btn--sm"
              onclick={exportWholeSubject}
              disabled={exporting}
              title="Print the whole-subject sheet plus every topic's sheet together"
            >
              <Icon name="book" size={13} /> {exporting ? "Loading…" : "Export all"}
            </button>
            <button class="btn btn--sm" onclick={generate} disabled={csGenerating}>
              <Icon name="refresh" size={13} /> {csGenerating ? "Synthesizing…" : "Regenerate"}
            </button>
          </div>
        </div>

        <!-- Sections -->
        <div class="cs-sections">
          {#each sections as sec (sec.id)}
            <section
              id={"cs-sec-" + sec.id}
              class="cs-section{sec.state === 'draft-pending' ? ' is-pending' : ''}"
            >
              <header class="cs-sec-head">
                <h2 class="cs-sec-title">{sec.title}</h2>

                {#if sec.state === "draft-pending"}
                  <span class="status-pill status-pill--draft">
                    <span class="dot"></span>pending
                  </span>
                {:else}
                  <span class="cs-sec-count mono">{sec.items.length}</span>
                {/if}
              </header>

              <dl class="cs-list">
                {#each sec.items as item, i (i)}
                  <div class="cs-item" id={"cs-it-" + sec.id + "-" + i}>
                    <dt>{item.t}</dt>
                    <dd><RichText text={item.d} /></dd>
                  </div>
                {/each}
              </dl>
            </section>
          {/each}
        </div>

        <!-- Floating, hover-expand, collapsible jump index (table of contents).
             Lets you leap to any section or sub-topic. Sits just left of the chat
             dock when it's open. -->
        <nav
          class="cs-index"
          class:is-pinned={indexPinned}
          style:right={chatDockOpen ? "calc(var(--chat-w, 396px) + 14px)" : "16px"}
          aria-label="Cheatsheet index"
        >
          <button
            class="cs-index-tab"
            onclick={() => (indexPinned = !indexPinned)}
            title={indexPinned ? "Unpin index" : "Pin index"}
          >
            <Icon name="grid" size={13} />
          </button>
          <div class="cs-index-panel">
            {#each sections as sec (sec.id)}
              <div class="cs-idx-secrow">
                <button
                  class="cs-idx-caret"
                  onclick={() => toggleIdxSec(sec.id)}
                  title={idxExpanded[sec.id] ? "Collapse" : "Expand"}
                  aria-expanded={!!idxExpanded[sec.id]}
                >{idxExpanded[sec.id] ? "▾" : "▸"}</button>
                <button class="cs-idx-sec" onclick={() => jumpTo("cs-sec-" + sec.id)}>{sec.title}</button>
              </div>
              {#if idxExpanded[sec.id]}
                {#each sec.items as item, i (i)}
                  <button class="cs-idx-item" title={item.t} onclick={() => jumpTo("cs-it-" + sec.id + "-" + i)}>{item.t}</button>
                {/each}
              {/if}
            {/each}
          </div>
        </nav>
      {/if}
    {/if}
  </div>

  <!-- ── EXPORT-ALL print block ─────────────────────────────────
       Hidden on screen; only visible when printing. Holds the whole-subject
       sheet followed by each topic's stored sheet so "Export all" prints them
       together as one document. -->
  {#if exportAll.length > 0}
    <div class="cs-export-all" aria-hidden="true">
      {#each exportAll as sheet, si (si)}
        <div class="cs-doc cs-export-sheet">
          <div class="cs-doc-head">
            <div>
              <div class="eyebrow">Cheatsheet · {si === 0 ? "Whole subject" : "Topic"}</div>
              <h1 class="cs-title">{sheet.topic}</h1>
              <div class="cs-sub mono">
                {sheet.subject} · synthesized from {sheet.sources} source{sheet.sources !== 1 ? "s" : ""} ·
                {sheet.sections.length} enforced sections
              </div>
            </div>
          </div>
          <div class="cs-sections">
            {#each sheet.sections as sec (sec.id)}
              <section class="cs-section">
                <header class="cs-sec-head">
                  <h2 class="cs-sec-title">{sec.title}</h2>
                  <span class="cs-sec-count mono">{sec.items.length}</span>
                </header>
                <dl class="cs-list">
                  {#each sec.items as item, i (i)}
                    <div class="cs-item">
                      <dt>{item.t}</dt>
                      <dd><RichText text={item.d} /></dd>
                    </div>
                  {/each}
                </dl>
              </section>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  /* ── floating jump index (TOC) ─────────────────────────────── */
  .cs-index {
    position: fixed;
    top: 96px;
    z-index: 28;
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .cs-index-tab {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: color-mix(in oklab, var(--surface) 70%, transparent);
    color: var(--fg-muted);
    backdrop-filter: blur(8px);
    cursor: pointer;
    transition: color var(--dur), border-color var(--dur);
  }
  .cs-index-tab:hover { color: var(--fg-bright); border-color: var(--border-strong); }
  .cs-index.is-pinned .cs-index-tab { color: var(--accent); border-color: var(--accent); }
  .cs-index-panel {
    max-width: 0;
    max-height: 72vh;
    overflow: hidden auto;
    opacity: 0;
    padding: 0;
    border-radius: 10px;
    border: 1px solid transparent;
    background: color-mix(in oklab, var(--surface) 82%, transparent);
    backdrop-filter: blur(10px);
    transition: max-width var(--dur-slow) var(--ease), opacity var(--dur) var(--ease),
      padding var(--dur) var(--ease), border-color var(--dur) var(--ease);
  }
  .cs-index:hover .cs-index-panel,
  .cs-index.is-pinned .cs-index-panel {
    max-width: 250px;
    opacity: 1;
    padding: 7px;
    border-color: var(--border);
  }
  .cs-idx-secrow { display: flex; align-items: center; gap: 2px; }
  .cs-idx-caret {
    flex: none;
    width: 18px;
    padding: 6px 0 2px;
    font-size: 9px;
    color: var(--fg-faint);
    background: none;
    border: none;
    cursor: pointer;
  }
  .cs-idx-caret:hover { color: var(--fg-bright); }
  .cs-idx-sec {
    flex: 1;
    min-width: 0;
    display: block;
    text-align: left;
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
    padding: 7px 8px 2px 2px;
    cursor: pointer;
    background: none;
    border: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cs-idx-item {
    display: block;
    width: 100%;
    max-width: 236px;
    text-align: left;
    font-size: 12px;
    color: var(--fg-muted);
    padding: 3px 8px 3px 16px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-radius: 6px;
    cursor: pointer;
    background: none;
    border: none;
  }
  .cs-idx-item:hover,
  .cs-idx-sec:hover { color: var(--fg-bright); background: color-mix(in oklab, var(--accent) 13%, transparent); }
  @media print { .cs-index { display: none !important; } }

  /* faint divider between definition/concept items so they don't read as a wall */
  :global(.cs-item) + :global(.cs-item) {
    border-top: 1px solid color-mix(in oklab, var(--border) 60%, transparent);
    padding-top: 16px;
  }

  /* ── topic tab bar ─────────────────────────────────────────── */
  .cs-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: var(--sp-4, 16px);
    padding-bottom: var(--sp-3, 12px);
    border-bottom: 1px solid var(--border);
  }
  .cs-tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 11px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    color: var(--fg-muted);
    font-size: var(--t-sm);
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .cs-tab:hover {
    color: var(--fg-bright);
    border-color: var(--fg3);
  }
  .cs-tab.is-active {
    color: var(--fg-bright);
    border-color: color-mix(in oklab, var(--tab-accent) 60%, var(--border-strong));
    background: color-mix(in oklab, var(--tab-accent) 14%, var(--surface));
  }
  .cs-tab-glyph {
    font-size: var(--t-sm);
    line-height: 1;
  }
  .cs-tab-count {
    font-size: var(--t-xs);
    color: var(--fg-faint);
    padding: 0 5px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--fg3) 18%, transparent);
  }
  .cs-tab.is-active .cs-tab-count {
    color: var(--fg-bright);
    background: color-mix(in oklab, var(--tab-accent) 28%, transparent);
  }

  /* Centered empty / working states — mirrors GenerateMaterial's .genmat--working */
  .cs-empty-state,
  .cs-working {
    min-height: 50vh;
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

  /* export-all block is hidden on screen; print rules reveal it */
  .cs-export-all {
    display: none;
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
    :global(.app-shell) {
      grid-template-columns: 1fr !important;
    }
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
    /* the tab bar and action buttons are screen-only chrome */
    .cs-tabs,
    .cs-doc-actions {
      display: none !important;
    }
    /* when an Export-all is in progress, reveal that block and hide the
       on-screen single-view doc so we don't print both. */
    .cs-export-all {
      display: block !important;
    }
    /* don't also print the single on-screen view during an Export-all */
    .cs-doc.is-exporting-all {
      display: none !important;
    }
    .cs-export-sheet {
      break-after: page;
    }
    .cs-export-sheet:last-child {
      break-after: auto;
    }
    /* keep each section from splitting awkwardly across pages */
    :global(.cs-section) {
      break-inside: avoid;
    }
    :global(.cs-item) {
      break-inside: avoid;
    }
    :global(.cs-title),
    :global(.cs-sec-title) {
      color: #000 !important;
    }
    /* readable tables/callouts on white paper */
    :global(.rt-table th) {
      background: #f0f0f0 !important;
      color: #000 !important;
    }
    :global(.rt-callout-label) {
      color: #000 !important;
    }
    :global(.rt-callout) {
      background: #f6f6f6 !important;
    }
    :global(.workspace-scroll),
    :global(.cs-doc),
    :global(.rt-callout-body),
    :global(.rt-table td),
    :global(.rt-p),
    :global(.cs-item dt),
    :global(.cs-item dd) {
      color: #000 !important;
    }
  }
</style>
