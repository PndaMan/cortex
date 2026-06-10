<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "../components/Icon.svelte";
  import * as api from "../lib/api";
  import type { CsSection as ApiCsSection, CheatsheetVersionMeta } from "../lib/api";
  import { flip } from "svelte/animate";

  type DiffChange = { type: "ctx" | "add" | "del"; text: string };
  type DiffSec = { id: string; title: string; changes: DiffChange[] };

  // ── tabbed hub ───────────────────────────────────────────────
  // `space d` opens this review hub: the existing draft-approve flow ("Draft
  // changes") AND an editable version history ("History"). The draft flow below
  // is unchanged — only wrapped in a tab.
  let tab = $state<"draft" | "history">("draft");

  // Local UI state — all Svelte 5 runes
  let view = $state<"inline" | "split">("inline");
  let resolving = $state<Record<string, "accept" | "reject">>({});
  let focus = $state(0);
  let bodyEl = $state<HTMLElement | null>(null);
  let loading = $state(false);
  let sourceLabel = $state("cheatsheet draft");
  // True once we know the subject has ANY stored cheatsheet — drives the
  // "nothing here at all" empty state so `space d` never shows a blank modal.
  let hasSheet = $state(false);
  // REAL data: the active subject's cheatsheet sections still in draft-pending
  // state — the genuine proposed additions awaiting review (no mock fixtures).
  let sections = $state<DiffSec[]>([]);

  // Load the real cheatsheet draft whenever the modal opens.
  $effect(() => {
    if (!app.diffOpen) return;
    resolving = {};
    focus = 0;
    loading = true;
    sections = [];
    hasSheet = false;
    tab = "draft";
    const sid = app.activeSubjectId;
    (async () => {
      try {
        const cs = sid ? await api.getCheatsheet(sid) : null;
        if (cs) {
          hasSheet = true;
          sourceLabel = cs.topic ? `${cs.subject} · ${cs.topic}` : cs.subject;
          sections = cs.sections
            .filter((s) => s.state === "draft-pending")
            .map((s) => ({
              id: s.id,
              title: s.title,
              changes: s.items.map((it) => ({
                type: "add" as const,
                text: it.t ? `${it.t}: ${it.d}` : it.d,
              })),
            }));
        }
      } catch (e) {
        app.pushToast({ kind: "error", title: "Couldn't load draft", body: String(e) });
      } finally {
        loading = false;
      }
    })();
  });

  // Keyboard handler — capture phase, installed only while open.
  // Draft-review shortcuts only fire on the Draft tab; Escape always closes.
  $effect(() => {
    if (!app.diffOpen) return;

    window.__cortexModalOpen = true;

    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        close();
        return;
      }
      if (tab !== "draft") return;
      const list = sections;
      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        focus = Math.min(list.length - 1, focus + 1);
      } else if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        focus = Math.max(0, focus - 1);
      } else if (e.key === "a") {
        e.preventDefault();
        const s = list[focus];
        if (s && !resolving[s.id]) resolve(s.id, "accept");
      } else if (e.key === "x") {
        e.preventDefault();
        const s = list[focus];
        if (s && !resolving[s.id]) resolve(s.id, "reject");
      } else if (e.key === "A") {
        e.preventDefault();
        acceptAll();
      } else if (e.key === "s") {
        e.preventDefault();
        view = view === "inline" ? "split" : "inline";
      }
    }

    window.addEventListener("keydown", onKey, true);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.__cortexModalOpen = false;
    };
  });

  // Scroll focused section into view
  $effect(() => {
    if (!app.diffOpen || !bodyEl || tab !== "draft") return;
    const node = bodyEl.querySelector<HTMLElement>(".diff-section.is-focus");
    if (node) {
      const b = bodyEl.getBoundingClientRect();
      const n = node.getBoundingClientRect();
      if (n.top < b.top + 8 || n.bottom > b.bottom - 8) {
        bodyEl.scrollTo({ top: bodyEl.scrollTop + (n.top - b.top) - 16, behavior: "smooth" });
      }
    }
  });

  function close() {
    app.diffOpen = false;
  }

  function resolve(secId: string, kind: "accept" | "reject") {
    if (resolving[secId]) return;
    resolving = { ...resolving, [secId]: kind };
    // After the resolve flash, drop the section so the remaining ones snap up.
    setTimeout(() => {
      sections = sections.filter((s) => s.id !== secId);
      const { [secId]: _, ...rest } = resolving;
      resolving = rest;
      if (focus >= sections.length) focus = Math.max(0, sections.length - 1);
      if (sections.length === 0) app.mergeDiff();
    }, 420);
  }

  function acceptAll() {
    if (!sections.length) { close(); return; }
    const next: Record<string, "accept" | "reject"> = {};
    sections.forEach((s) => (next[s.id] = "accept"));
    resolving = next;
    setTimeout(() => app.mergeDiff(), 460);
  }

  const remaining = $derived(sections.filter((s) => !resolving[s.id]));

  // ── HISTORY TAB ──────────────────────────────────────────────
  // Version list scoped exactly like Cheatsheet.svelte: the active subject +
  // the current topic selection (null = whole subject). "View" renders a stored
  // version read-only; "Restore" overwrites the live sheet (snapshotting current
  // first) — the "accept edits from history" ask.
  let versions = $state<CheatsheetVersionMeta[]>([]);
  let histLoading = $state(false);
  let restoringId = $state<string | null>(null);
  // The version being previewed read-only (its full section set), or null for the list.
  let viewing = $state<{ meta: CheatsheetVersionMeta; sections: ApiCsSection[] } | null>(null);

  function fmtTime(ms: number): string {
    try {
      return new Date(ms).toLocaleString(undefined, {
        month: "short", day: "numeric", hour: "2-digit", minute: "2-digit",
      });
    } catch { return String(ms); }
  }

  async function loadHistory() {
    const sid = app.activeSubjectId;
    if (!sid) return;
    histLoading = true;
    viewing = null;
    try {
      versions = await api.listCheatsheetVersions(sid, app.cheatTopicId ?? undefined);
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't load history", body: String(e) });
    } finally {
      histLoading = false;
    }
  }

  // Load the version list the first time the History tab is opened (and on reopen).
  $effect(() => {
    if (app.diffOpen && tab === "history") loadHistory();
  });

  async function viewVersion(meta: CheatsheetVersionMeta) {
    try {
      const secs = await api.getCheatsheetVersion(meta.id);
      viewing = { meta, sections: secs };
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't load version", body: String(e) });
    }
  }

  async function restoreVersion(meta: CheatsheetVersionMeta) {
    if (restoringId) return;
    const ok = await app.confirm({
      title: "Restore this version?",
      body:
        `This replaces the current cheatsheet with the version from ` +
        `${fmtTime(meta.created_at)}. Your current sheet is snapshotted first, ` +
        `so you can undo by restoring that.`,
      okLabel: "Restore",
    });
    if (!ok) return;
    restoringId = meta.id;
    try {
      await api.restoreCheatsheetVersion(meta.id);
      app.cheatsheetReloadNonce++; // the Cheatsheet view reloads the live sheet
      app.scheduleSync(); // restored content is a change → push to homelab
      app.pushToast({
        kind: "success",
        title: "Version restored",
        body: "The cheatsheet now matches the selected version.",
      });
      viewing = null;
      await loadHistory(); // the restore added new snapshot rows
    } catch (e) {
      app.pushToast({ kind: "error", title: "Restore failed", body: String(e) });
    } finally {
      restoringId = null;
    }
  }

  // ── FOOTER: homelab sync awareness ───────────────────────────
  const syncLabel = $derived.by(() => {
    if (app.syncState === "off") return "sync off";
    if (app.syncState === "syncing") return "syncing…";
    if (app.syncState === "error") return "sync error";
    if (app.syncLastAt > 0) {
      const mins = Math.max(0, Math.round((Date.now() - app.syncLastAt) / 60000));
      const ago = mins < 1 ? "just now" : mins < 60 ? `${mins}m ago` : `${Math.round(mins / 60)}h ago`;
      return `synced · ${ago}`;
    }
    return "synced";
  });
  const syncConfigured = $derived(app.syncState !== "off");
</script>

{#if app.diffOpen}
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="overlay diff-overlay"
    onmousedown={close}
    role="dialog"
    aria-modal="true"
    aria-label="Cheatsheet review"
    tabindex="-1"
  >
    <!-- Modal panel — stop propagation so backdrop click doesn't close when clicking inside -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="diff-modal"
      onmousedown={(e) => e.stopPropagation()}
    >
      <!-- Header -->
      <header class="diff-head">
        <div>
          <div class="eyebrow">Cheatsheet review</div>
          <div class="diff-title mono">
            <span class="badge badge--web" style="margin-left: 0;">
              <span class="dot"></span>{sourceLabel}
            </span>
          </div>
        </div>
        <div class="diff-head-tools">
          <div class="seg" role="tablist" aria-label="Review section">
            <button
              class="seg-opt{tab === 'draft' ? ' on' : ''}"
              role="tab"
              aria-selected={tab === "draft"}
              onclick={() => (tab = "draft")}
            >
              Draft changes{#if remaining.length}<span class="diff-tab-count">{remaining.length}</span>{/if}
            </button>
            <button
              class="seg-opt{tab === 'history' ? ' on' : ''}"
              role="tab"
              aria-selected={tab === "history"}
              onclick={() => (tab = "history")}
            >
              History
            </button>
          </div>
          <button class="btn btn--icon btn--sm btn--ghost" onclick={close}>
            <Icon name="x" size={12} />
          </button>
        </div>
      </header>

      {#if tab === "draft"}
        <!-- ── DRAFT CHANGES (existing approve/reject flow, unchanged) ── -->
        <!-- Legend -->
        <div class="diff-legend mono">
          <span><i class="lg add"></i> added</span>
          <span><i class="lg del"></i> removed</span>
          <span class="diff-keys">
            <span class="kbd">j</span><span class="kbd">k</span> move ·
            <span class="kbd">a</span> accept ·
            <span class="kbd">x</span> reject ·
            <span class="kbd">A</span> all ·
            <span class="kbd">s</span> view
          </span>
          <div class="grow"></div>
          <div class="seg-toggle" title="Toggle with s">
            <button class={view === "inline" ? "on" : ""} onclick={() => (view = "inline")}>Inline</button>
            <button class={view === "split" ? "on" : ""} onclick={() => (view = "split")}>Side-by-side</button>
          </div>
        </div>

        <!-- Diff body -->
        <div class="diff-body" bind:this={bodyEl}>
          {#if loading}
            <div class="mono faint" style="display:flex;align-items:center;justify-content:center;min-height:200px;">Loading draft…</div>
          {:else if !hasSheet}
            <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;min-height:240px;text-align:center;padding:24px;">
              <Icon name="diamond" size={26} color="var(--fg3)" />
              <div class="read" style="font-size:18px;color:var(--fg-bright);">No cheatsheet yet</div>
              <p class="mono faint" style="max-width:360px;line-height:1.5;">
                {#if app.activeSubjectId}
                  Generate a cheatsheet for this subject first — proposed changes
                  and version history will appear here.
                {:else}
                  Open a subject to review its cheatsheet draft changes and history.
                {/if}
              </p>
            </div>
          {:else if sections.length === 0}
            <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;min-height:240px;text-align:center;padding:24px;">
              <Icon name="check" size={26} color="var(--ok)" />
              <div class="read" style="font-size:18px;color:var(--fg-bright);">No pending changes</div>
              <p class="mono faint" style="max-width:360px;line-height:1.5;">
                Nothing is awaiting review for this subject. Regenerate the cheatsheet
                to propose additions, or browse <button class="diff-linkbtn" onclick={() => (tab = "history")}>History</button>.
              </p>
            </div>
          {:else}
          {#each sections as sec, idx (sec.id)}
            {@const r = resolving[sec.id]}
            {@const isFocus = idx === focus && !r}
            <div
              class="diff-section{r ? ' resolving-' + r : ''}{isFocus ? ' is-focus' : ''}"
              animate:flip={{ duration: 240 }}
              onmouseenter={() => (focus = idx)}
              role="group"
              aria-label={sec.title}
            >
              <div class="diff-sec-head">
                <h3>{sec.title}</h3>
                <div class="diff-stats mono">
                  <span class="st-add">+{sec.changes.filter((c) => c.type === "add").length}</span>
                  <span class="st-del">−{sec.changes.filter((c) => c.type === "del").length}</span>
                </div>
                <div class="grow"></div>

                {#if !r}
                  <button
                    class="btn btn--sm btn--ghost"
                    onclick={() => resolve(sec.id, "reject")}
                    title="Reject (x)"
                  >
                    <span class="kbd">x</span> Reject
                  </button>
                  <button
                    class="btn btn--sm btn--primary"
                    onclick={() => resolve(sec.id, "accept")}
                    title="Accept section (a)"
                  >
                    <span class="kbd" style="border-color: currentColor;">a</span> Accept section
                  </button>
                {:else if r === "accept"}
                  <span class="resolved-tag ok"><Icon name="check" size={13} /> merged</span>
                {:else if r === "reject"}
                  <span class="resolved-tag err"><Icon name="x" size={12} /> rejected</span>
                {/if}
              </div>

              {#if view === "inline"}
                <div class="diff-inline">
                  {#each sec.changes as c, i (i)}
                    <div class="diff-line {c.type}">
                      <span class="gutter">{c.type === "add" ? "+" : c.type === "del" ? "−" : ""}</span>
                      <span class="txt read">{c.text}</span>
                    </div>
                  {/each}
                </div>
              {:else}
                <div class="diff-split">
                  <div class="ds-col">
                    <div class="ds-col-l mono">Current</div>
                    {#each sec.changes.filter((c) => c.type !== "add") as c, i (i)}
                      <div class="diff-line {c.type === 'del' ? 'del' : ''}">
                        <span class="txt read">{c.text}</span>
                      </div>
                    {/each}
                  </div>
                  <div class="ds-col">
                    <div class="ds-col-l mono">Proposed</div>
                    {#each sec.changes.filter((c) => c.type !== "del") as c, i (i)}
                      <div class="diff-line {c.type === 'add' ? 'add' : ''}">
                        <span class="txt read">{c.text}</span>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          {/each}
          {/if}
        </div>
      {:else}
        <!-- ── HISTORY (editable: view + restore) ── -->
        <div class="diff-body">
          {#if histLoading}
            <div class="mono faint" style="display:flex;align-items:center;justify-content:center;min-height:200px;">Loading history…</div>
          {:else if viewing}
            <!-- Read-only render of one stored version -->
            <div class="cmdk-group">
              <button class="diff-linkbtn" onclick={() => (viewing = null)}>
                ← Back to versions
              </button>
            </div>
            <div class="set-card" style="padding:14px;">
              <div class="diff-sec-head" style="margin-bottom:10px;">
                <h3>{viewing.meta.note || "version"}</h3>
                <div class="grow"></div>
                <span class="mono faint">{fmtTime(viewing.meta.created_at)} · {viewing.meta.section_count} sec</span>
              </div>
              {#each viewing.sections as sec (sec.id)}
                {#if !sec.id.startsWith("__topic__")}
                  <div class="diff-hist-sec">
                    <div class="diff-hist-sectitle mono">{sec.title}</div>
                    <dl class="diff-hist-list">
                      {#each sec.items as item, i (i)}
                        {#if !item.t.startsWith("__topic__")}
                          <div class="diff-hist-item">
                            <dt class="read">{item.t}</dt>
                            <dd class="read faint">{item.d}</dd>
                          </div>
                        {/if}
                      {/each}
                    </dl>
                  </div>
                {/if}
              {/each}
              <div class="diff-sec-head" style="margin-top:12px;">
                <div class="grow"></div>
                <button
                  class="btn btn--sm btn--primary"
                  onclick={() => restoreVersion(viewing!.meta)}
                  disabled={restoringId === viewing.meta.id}
                >
                  <Icon name="refresh" size={13} />
                  {restoringId === viewing.meta.id ? "Restoring…" : "Restore this version"}
                </button>
              </div>
            </div>
          {:else if versions.length === 0}
            <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;gap:10px;min-height:240px;text-align:center;padding:24px;">
              <Icon name="refresh" size={26} color="var(--fg3)" />
              <div class="read" style="font-size:18px;color:var(--fg-bright);">No versions yet</div>
              <p class="mono faint" style="max-width:360px;line-height:1.5;">
                Edits and regenerations of this cheatsheet are recorded here — you'll
                be able to view and restore them.
              </p>
            </div>
          {:else}
            <div class="cmdk-group">
              <div class="gl">Versions — newest first</div>
              {#each versions as v, i (v.id)}
                <div class="diff-hist-row set-card">
                  <div class="diff-hist-meta">
                    <span class="diff-hist-when mono">{fmtTime(v.created_at)}</span>
                    <span class="diff-hist-note">
                      {i === 0 ? "current" : v.note} · {v.section_count} section{v.section_count !== 1 ? "s" : ""}
                    </span>
                  </div>
                  <div class="grow"></div>
                  <button class="btn btn--sm btn--ghost" onclick={() => viewVersion(v)}>
                    <Icon name="doc" size={12} /> View
                  </button>
                  <button
                    class="btn btn--sm btn--primary"
                    onclick={() => restoreVersion(v)}
                    disabled={i === 0 || restoringId === v.id}
                    title={i === 0 ? "This is already the current sheet" : "Restore this version"}
                  >
                    <Icon name="refresh" size={12} />
                    {restoringId === v.id ? "Restoring…" : "Restore"}
                  </button>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      <!-- Footer -->
      <footer class="diff-foot">
        <div class="diff-foot-sync mono faint" title="Homelab sync">
          <span class="dot" class:dot--on={syncConfigured && app.syncState !== 'error'}></span>
          {syncLabel}
          {#if syncConfigured}
            <button class="diff-linkbtn" onclick={() => app.syncNow()} disabled={app.syncState === 'syncing'}>
              Sync now
            </button>
          {/if}
        </div>
        <div class="grow"></div>
        {#if tab === "draft" && hasSheet && remaining.length}
          <button class="btn btn--sm btn--ghost" onclick={close}>Later</button>
          <button class="btn btn--sm btn--primary" onclick={acceptAll}>
            <span class="kbd" style="border-color: currentColor;">A</span> Approve all &amp; merge
          </button>
        {:else}
          <button class="btn btn--sm btn--ghost" onclick={close}>Close</button>
        {/if}
      </footer>
    </div>
  </div>
{/if}

<style>
  /* Tab pending-count chip on the segmented control. */
  .diff-tab-count {
    margin-left: 6px;
    padding: 0 5px;
    border-radius: 999px;
    font-size: var(--t-2xs);
    background: color-mix(in oklab, var(--accent) 24%, transparent);
    color: var(--fg-bright);
  }
  /* Inline text-link buttons (back, sync now, jump to history). */
  .diff-linkbtn {
    background: none;
    border: none;
    padding: 0;
    margin-left: 6px;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
  }
  .diff-linkbtn:hover:not(:disabled) { text-decoration: underline; }
  .diff-linkbtn:disabled { opacity: 0.5; cursor: default; }

  /* History list rows. */
  .diff-hist-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    margin: 6px 0;
  }
  .diff-hist-meta { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .diff-hist-when { font-size: var(--t-sm); color: var(--fg-bright); }
  .diff-hist-note { font-size: var(--t-xs); color: var(--fg-faint); }

  /* Read-only version render. */
  .diff-hist-sec { margin: 12px 0; }
  .diff-hist-sectitle {
    font-size: var(--t-sm);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
    margin-bottom: 6px;
  }
  .diff-hist-list { display: flex; flex-direction: column; gap: 8px; margin: 0; }
  .diff-hist-item { display: flex; flex-direction: column; gap: 2px; }
  .diff-hist-item dt { font-weight: 600; color: var(--fg-bright); }
  .diff-hist-item dd { margin: 0; white-space: pre-wrap; }

  /* Footer sync indicator dot. */
  .diff-foot-sync { display: inline-flex; align-items: center; gap: 7px; }
  .diff-foot-sync .dot {
    width: 7px; height: 7px; border-radius: 50%;
    background: var(--fg3);
  }
  .diff-foot-sync .dot--on { background: var(--ok); }
</style>
