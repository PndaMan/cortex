<script lang="ts">
  // Markdown notes — a master/detail workspace. Left: collapsible note list
  // scoped to the active subject (or all unfiled notes). Right: title +
  // Markdown editor with Save, autosave, "Convert to source", "Export PDF",
  // and Delete.
  // Renders full-page in `.workspace-scroll`, or compact when `embedded`.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { Note } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import MarkdownEditor from "../components/MarkdownEditor.svelte";
  import RichText from "../components/RichText.svelte";

  let { embedded = false }: { embedded?: boolean } = $props();

  let notes = $state<Note[]>([]);
  let loading = $state(true);
  let selectedId = $state<string | null>(null);
  let listCollapsed = $state(true); // collapsed by default — more room for the editor
  let fullscreen = $state(false);

  // Draft fields for the selected note; saved status tracks persistence.
  let title = $state("");
  let body = $state("");
  let saved = $state(true);
  let savingTimer: ReturnType<typeof setTimeout> | null = null;

  const selected = $derived(notes.find((n) => n.id === selectedId) ?? null);

  // (Re)load notes whenever the active subject changes.
  $effect(() => {
    const sid = app.activeSubjectId ?? null;
    void load(sid);
  });

  // Escape key exits fullscreen.
  $effect(() => {
    if (!fullscreen) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") fullscreen = false;
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function load(sid: string | null) {
    loading = true;
    try {
      const list = await api.listNotes(sid);
      notes = list;
      // Keep the current selection if it still exists; else pick the first.
      if (!list.some((n) => n.id === selectedId)) {
        select(list[0] ?? null);
      }
    } catch (e) {
      app.pushToast({ kind: "error", title: "Failed to load notes", body: String(e) });
    } finally {
      loading = false;
    }
  }

  function select(n: Note | null) {
    if (savingTimer) { clearTimeout(savingTimer); savingTimer = null; }
    selectedId = n?.id ?? null;
    title = n?.title ?? "";
    body = n?.body ?? "";
    saved = true;
  }

  async function newNote() {
    try {
      const n = await api.createNote("Untitled", "", app.activeSubjectId ?? null);
      notes = [n, ...notes];
      select(n);
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't create note", body: String(e) });
    }
  }

  function markDirty() {
    saved = false;
    if (savingTimer) clearTimeout(savingTimer);
    savingTimer = setTimeout(() => { void save(); }, 800);
  }

  async function save() {
    const id = selectedId;
    if (!id) return;
    if (savingTimer) { clearTimeout(savingTimer); savingTimer = null; }
    try {
      const updated = await api.updateNote(id, title, body);
      notes = notes.map((n) => (n.id === id ? updated : n));
      saved = true;
    } catch (e) {
      app.pushToast({ kind: "error", title: "Save failed", body: String(e) });
    }
  }

  async function convert() {
    const id = selectedId;
    if (!id) return;
    if (!app.activeSubjectId) return; // guarded by disabled state too
    try {
      if (!saved) await save();
      await api.noteToSource(id);
      app.pushToast({ kind: "success", title: "Converted to source", body: title || "Untitled" });
      await app.refresh();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Convert failed", body: String(e) });
    }
  }

  async function remove() {
    const id = selectedId;
    if (!id) return;
    const ok = await app.confirm({ title: "Delete note?", danger: true, okLabel: "Delete" });
    if (!ok) return;
    try {
      await api.deleteNote(id);
      const rest = notes.filter((n) => n.id !== id);
      notes = rest;
      select(rest[0] ?? null);
    } catch (e) {
      app.pushToast({ kind: "error", title: "Delete failed", body: String(e) });
    }
  }

  function exportPdf() {
    window.print();
  }

  function relTime(ms: number): string {
    const diff = Date.now() - ms;
    const m = Math.round(diff / 60000);
    if (m < 1) return "just now";
    if (m < 60) return `${m}m ago`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h}h ago`;
    const d = Math.round(h / 24);
    if (d < 7) return `${d}d ago`;
    return new Date(ms).toLocaleDateString();
  }

  const canConvert = $derived(!!selectedId && !!app.activeSubjectId);
</script>

{#snippet notesWorkspace()}
  <div class={"notes" + (embedded ? " notes--embedded" : "") + (listCollapsed ? " notes--collapsed" : "")}>
    <!-- Left: note list or slim rail when collapsed -->
    {#if listCollapsed}
      <aside class="notes-rail" aria-label="Note list (collapsed)">
        <button
          type="button"
          class="notes-rail-toggle"
          title="Expand note list"
          aria-label="Expand note list"
          onclick={() => (listCollapsed = false)}
        >
          <!-- Chevron points right → "open left panel" -->
          <Icon name="chevron" size={14} />
        </button>
        <button
          type="button"
          class="notes-rail-new"
          title="New note"
          aria-label="New note"
          onclick={newNote}
        >
          <Icon name="plus" size={13} />
        </button>
      </aside>
    {:else}
      <aside class="notes-list">
        <div class="notes-list-head">
          <span class="notes-list-title">Notes</span>
          <div class="notes-list-head-actions">
            <button class="btn btn--primary btn--sm" type="button" onclick={newNote} title="New note">
              <Icon name="plus" size={12} /> New note
            </button>
            <!-- Collapse button: visually prominent, chevron points left (← close) -->
            <button
              type="button"
              class="notes-collapse-btn"
              title="Collapse note list"
              aria-label="Collapse note list"
              onclick={() => (listCollapsed = true)}
            >
              <!-- Chevron default points right; rotate 180° to point left = collapse -->
              <Icon name="chevron" size={14} style="transform:rotate(180deg)" />
            </button>
          </div>
        </div>
        <div class="notes-items">
          {#if loading}
            <div class="notes-hint">Loading…</div>
          {:else if notes.length === 0}
            <div class="notes-empty">
              <div class="notes-empty-glyph">📝</div>
              <div class="notes-empty-title">No notes yet</div>
              <div class="notes-empty-body">
                {app.activeSubjectId ? "Capture ideas in Markdown for this subject." : "Capture ideas in Markdown."}
              </div>
              <button class="btn btn--primary btn--sm" type="button" onclick={newNote}>
                <Icon name="plus" size={12} /> New note
              </button>
            </div>
          {:else}
            {#each notes as n (n.id)}
              <button
                type="button"
                class={"notes-item" + (n.id === selectedId ? " on" : "")}
                onclick={() => select(n)}
              >
                <span class="notes-item-title">{n.title || "Untitled"}</span>
                <span class="notes-item-time">{relTime(n.updated_at)}</span>
              </button>
            {/each}
          {/if}
        </div>
      </aside>
    {/if}

    <section class="notes-detail">
      {#if selected}
        <div class="notes-detail-head">
          <input
            class="notes-title"
            placeholder="Untitled"
            value={title}
            oninput={(e) => { title = (e.target as HTMLInputElement).value; markDirty(); }}
          />
          <span class={"notes-saved" + (saved ? " on" : "")}>
            {#if saved}<Icon name="check" size={12} /> Saved{:else}Editing…{/if}
          </span>
          <!-- Fullscreen toggle -->
          <button
            type="button"
            class={"notes-fullscreen-btn" + (fullscreen ? " on" : "")}
            title={fullscreen ? "Exit fullscreen (Esc)" : "Enter fullscreen"}
            aria-label={fullscreen ? "Exit fullscreen" : "Enter fullscreen"}
            onclick={() => (fullscreen = !fullscreen)}
          >
            {#if fullscreen}
              <Icon name="x" size={14} />
            {:else}
              <Icon name="external" size={14} />
            {/if}
          </button>
        </div>

        <div class="notes-editor-wrap">
          <MarkdownEditor value={body} onChange={(v) => { body = v; markDirty(); }} />
        </div>

        <!-- Print-only rendered preview — hidden on screen, shown when printing -->
        <div class="notes-print-preview" aria-hidden="true">
          <h1 class="notes-print-title">{title || "Untitled"}</h1>
          <RichText text={body} />
        </div>

        <div class="notes-actions">
          <button
            class="btn btn--danger btn--sm"
            type="button"
            style="margin-right:auto"
            onclick={remove}
            title="Delete this note"
          >
            Delete
          </button>
          <button
            class="btn btn--ghost btn--sm"
            type="button"
            onclick={exportPdf}
            title="Export note as PDF"
          >
            <Icon name="doc" size={13} /> Save as PDF
          </button>
          <span class="notes-convert-wrap" title={canConvert ? "" : "Notes need a subject to become a source"}>
            <button class="btn btn--ghost btn--sm" type="button" disabled={!canConvert} onclick={convert}>
              <Icon name="arrowR" size={13} /> Convert to source
            </button>
          </span>
          <button class="btn btn--primary btn--sm" type="button" disabled={saved} onclick={save} title="Save note">
            Save
          </button>
        </div>
      {:else if !loading}
        <div class="notes-detail-empty">
          <div class="notes-empty-glyph">🗒️</div>
          <div class="notes-empty-title">Select or create a note</div>
          <div class="notes-empty-body">Your Markdown notes live here.</div>
        </div>
      {/if}
    </section>
  </div>
{/snippet}

<!-- Fullscreen overlay: renders the editor maximised over the whole window -->
{#if fullscreen && selected}
  <div class="notes-fs-overlay" role="dialog" aria-modal="true" aria-label="Note — fullscreen">
    <div class="notes-fs-head">
      <input
        class="notes-fs-title"
        placeholder="Untitled"
        value={title}
        oninput={(e) => { title = (e.target as HTMLInputElement).value; markDirty(); }}
      />
      <span class={"notes-saved notes-fs-saved" + (saved ? " on" : "")}>
        {#if saved}<Icon name="check" size={12} /> Saved{:else}Editing…{/if}
      </span>
      <button
        type="button"
        class="notes-fs-exit-btn"
        title="Exit fullscreen (Esc)"
        aria-label="Exit fullscreen"
        onclick={() => (fullscreen = false)}
      >
        <Icon name="x" size={15} /> Exit fullscreen
      </button>
    </div>
    <div class="notes-fs-editor">
      <MarkdownEditor value={body} onChange={(v) => { body = v; markDirty(); }} />
    </div>
    <div class="notes-fs-actions">
      <button
        class="btn btn--danger btn--sm"
        type="button"
        style="margin-right:auto"
        onclick={remove}
        title="Delete this note"
      >
        Delete
      </button>
      <button
        class="btn btn--ghost btn--sm"
        type="button"
        onclick={exportPdf}
        title="Export note as PDF"
      >
        <Icon name="doc" size={13} /> Save as PDF
      </button>
      <span class="notes-convert-wrap" title={canConvert ? "" : "Notes need a subject to become a source"}>
        <button class="btn btn--ghost btn--sm" type="button" disabled={!canConvert} onclick={convert}>
          <Icon name="arrowR" size={13} /> Convert to source
        </button>
      </span>
      <button class="btn btn--primary btn--sm" type="button" disabled={saved} onclick={save} title="Save note">
        Save
      </button>
    </div>
  </div>
{/if}

{#if embedded}
  {@render notesWorkspace()}
{:else}
  <div class="workspace-scroll notes-workspace-scroll">
    <div class="notes-page">
      <div class="notes-page-head">
        <div class="eyebrow">Notes</div>
        <h1 class="notes-page-title">
          {app.activeSubject ? app.activeSubject.name + " · Notes" : "Notes"}
        </h1>
        <div class="mono faint" style="font-size:var(--t-xs)">Markdown notes you can convert into sources</div>
      </div>
      {@render notesWorkspace()}
    </div>
  </div>
{/if}

<style>
  /* ── Full-page wrapper ──────────────────────────────────────────────────── */
  :global(.notes-workspace-scroll) {
    display: flex;
    flex-direction: column;
  }

  .notes-page {
    display: flex;
    flex-direction: column;
    flex: 1 1 0;
    min-height: 0;
    padding: 24px 28px 0;
  }

  .notes-page-head {
    flex: none;
    margin-bottom: 16px;
  }

  .notes-page-title {
    margin: 4px 0 2px; font-size: var(--r-lg, 20px); color: var(--fg-bright); font-weight: 600;
  }

  /* ── Master–detail grid ─────────────────────────────────────────────────── */
  .notes {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 0;
    align-items: stretch;
    flex: 1 1 0;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: var(--r-lg, 12px);
    overflow: hidden;
    margin-bottom: 24px;
  }

  /* When list is collapsed, shrink first column to the slim rail width. */
  .notes--collapsed {
    grid-template-columns: 40px 1fr;
  }

  /* Embedded (dock) mode: narrower list, single frame, no extra margin. */
  .notes--embedded {
    grid-template-columns: 180px 1fr;
    flex: none;
    min-height: 420px;
    margin-bottom: 0;
    border-radius: var(--r-lg, 12px);
  }
  .notes--embedded.notes--collapsed {
    grid-template-columns: 40px 1fr;
  }

  /* ── Left: note list ────────────────────────────────────────────────────── */
  .notes-list {
    display: flex;
    flex-direction: column;
    background: var(--surface-2);
    border-right: 1px solid var(--border);
    overflow: hidden;
  }

  .notes-list-head {
    flex: none;
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
    padding: 10px 8px 10px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }

  .notes-list-title {
    font-size: var(--t-2xs, 10.5px); font-weight: 600; letter-spacing: 0.12em;
    text-transform: uppercase; color: var(--fg-faint);
  }

  .notes-list-head-actions {
    display: flex; align-items: center; gap: 6px;
  }

  /* Collapse toggle button in the list header — visible solid border so it's
     easy to spot alongside the New note button. */
  .notes-collapse-btn {
    display: inline-flex; align-items: center; justify-content: center;
    width: 28px; height: 28px; padding: 0; flex-shrink: 0;
    background: var(--surface-3); border: 1px solid var(--border-strong); border-radius: 7px;
    color: var(--fg-muted); cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease, border-color 0.1s ease;
  }
  .notes-collapse-btn:hover { background: var(--surface); color: var(--fg-bright); border-color: var(--border-strong); }

  /* List items area scrolls independently within the fixed column height. */
  .notes-items {
    flex: 1 1 0;
    min-height: 0;
    overflow-y: auto;
    display: flex; flex-direction: column;
    padding: 6px; gap: 2px;
  }

  .notes-item {
    display: flex; flex-direction: column; gap: 2px; text-align: left;
    padding: 8px 10px; border-radius: 8px; cursor: pointer;
    background: none; border: 1px solid transparent; color: var(--fg);
    transition: background 0.1s ease, border-color 0.1s ease;
  }
  .notes-item:hover { background: var(--surface-3); }
  .notes-item.on { background: var(--surface); border-color: var(--border-strong); }
  .notes-item-title {
    font-size: var(--t-xs, 12.5px); color: var(--fg-bright); font-weight: 500;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .notes-item-time { font-size: var(--t-2xs, 10px); color: var(--fg-faint); font-family: var(--font-mono); }
  .notes-hint { padding: 10px; color: var(--fg-faint); font-size: var(--t-xs, 12px); }

  /* ── Collapsed rail ─────────────────────────────────────────────────────── */
  .notes-rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 10px 0;
    background: var(--surface-2);
    border-right: 1px solid var(--border);
  }

  .notes-rail-toggle,
  .notes-rail-new {
    display: inline-flex; align-items: center; justify-content: center;
    width: 30px; height: 30px; padding: 0;
    background: var(--surface-3); border: 1px solid var(--border-strong); border-radius: 7px;
    color: var(--fg-muted); cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease;
  }
  .notes-rail-toggle:hover,
  .notes-rail-new:hover {
    background: var(--surface); color: var(--fg-bright);
  }

  /* ── Right: detail pane ─────────────────────────────────────────────────── */
  .notes-detail {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--surface);
  }

  .notes-detail-head {
    flex: none;
    display: flex; align-items: center; gap: 10px;
    padding: 12px 14px 12px 20px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .notes-title {
    flex: 1; min-width: 0;
    font-size: var(--t-md, 15px); font-weight: 600; color: var(--fg-bright);
    background: transparent; border: none; outline: none;
    padding: 0;
  }
  .notes-title:focus { color: var(--fg-bright); }

  .notes-saved {
    display: inline-flex; align-items: center; gap: 4px; flex-shrink: 0;
    font-size: var(--t-2xs, 10.5px); color: var(--fg-faint); font-family: var(--font-mono);
    transition: color 0.15s ease;
  }
  .notes-saved.on { color: var(--accent); }

  /* Fullscreen toggle button in the detail header. */
  .notes-fullscreen-btn {
    display: inline-flex; align-items: center; justify-content: center;
    width: 30px; height: 30px; padding: 0; flex-shrink: 0;
    background: var(--surface-3); border: 1px solid var(--border-strong); border-radius: 7px;
    color: var(--fg-muted); cursor: pointer;
    transition: background 0.1s ease, color 0.1s ease;
  }
  .notes-fullscreen-btn:hover,
  .notes-fullscreen-btn.on { background: var(--surface); color: var(--accent); border-color: var(--accent); }

  .notes-editor-wrap {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 16px 20px;
  }

  .notes-actions {
    flex: none;
    display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    padding: 10px 20px 14px;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }
  .notes-convert-wrap { display: inline-flex; }

  /* ── Fullscreen overlay ─────────────────────────────────────────────────── */
  /* Covers the entire viewport above normal content (z-index 900, below modals
     which typically live at 1000+). Uses the app's bg/surface tokens. */
  .notes-fs-overlay {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: flex;
    flex-direction: column;
    background: var(--bg, #1a1a2e);
  }

  .notes-fs-head {
    flex: none;
    display: flex; align-items: center; gap: 10px;
    padding: 14px 20px 13px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .notes-fs-title {
    flex: 1; min-width: 0;
    font-size: var(--t-lg, 18px); font-weight: 700; color: var(--fg-bright);
    background: transparent; border: none; outline: none;
    padding: 0;
  }
  .notes-fs-title:focus { color: var(--fg-bright); }

  .notes-fs-saved {
    flex-shrink: 0;
  }

  .notes-fs-exit-btn {
    display: inline-flex; align-items: center; gap: 6px;
    padding: 5px 12px; flex-shrink: 0;
    background: var(--surface-3); border: 1px solid var(--border-strong); border-radius: 7px;
    color: var(--fg-muted); cursor: pointer; font: inherit; font-size: var(--t-xs, 12.5px); font-weight: 600;
    transition: background 0.1s ease, color 0.1s ease;
  }
  .notes-fs-exit-btn:hover { background: var(--surface); color: var(--fg-bright); border-color: var(--fg-faint); }

  .notes-fs-editor {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 20px 28px;
    background: var(--bg, #1a1a2e);
  }

  .notes-fs-actions {
    flex: none;
    display: flex; align-items: center; gap: 8px; flex-wrap: wrap;
    padding: 10px 20px 14px;
    border-top: 1px solid var(--border);
    background: var(--surface);
  }

  /* ── Print-only preview block ───────────────────────────────────────────── */
  /* Hidden on screen; only the @media print rule makes it visible. */
  .notes-print-preview {
    display: none;
  }
  .notes-print-title {
    font-size: 22px; font-weight: 700; margin: 0 0 16px; color: #000;
  }

  /* ── Empty states ───────────────────────────────────────────────────────── */
  .notes-empty, .notes-detail-empty {
    display: flex; flex-direction: column; align-items: center; gap: 8px; text-align: center;
    padding: 40px 16px; color: var(--fg-muted);
  }
  .notes-detail-empty {
    flex: 1;
    margin: 16px;
    border: 1px dashed var(--border-strong); border-radius: var(--r-lg, 12px); background: var(--surface);
  }
  .notes-empty-glyph { font-size: 30px; line-height: 1; }
  .notes-empty-title { font-size: var(--t-md, 14px); font-weight: 600; color: var(--fg-bright); }
  .notes-empty-body { font-size: var(--t-xs, 12px); color: var(--fg-faint); max-width: 220px; }

  /* ── PDF / Print ────────────────────────────────────────────────────────── */
  @media print {
    /* Hide entire app chrome. These are :global because they live outside this
       component's shadow — the sidebar, statusbar, chat dock, etc. */
    :global(.sidebar),
    :global(.statusbar),
    :global(.chat-dock),
    :global(.chat-bar),
    :global(.workspace-scroll > *:not(.notes-page)),
    :global(.notes-page-head) {
      display: none !important;
    }

    /* Strip page layout so only the note fills the printed page. */
    :global(body),
    :global(.workspace-scroll),
    :global(.notes-page) {
      display: block !important;
      background: #fff !important;
      color: #000 !important;
      padding: 0 !important;
      margin: 0 !important;
    }

    /* Hide everything inside the notes component except the print preview. */
    .notes-list,
    .notes-rail,
    .notes-detail-head,
    .notes-editor-wrap,
    .notes-actions {
      display: none !important;
    }

    /* Let the notes grid collapse to a single column for the detail pane. */
    .notes {
      display: block !important;
      border: none !important;
      margin: 0 !important;
    }

    /* Show the hidden print-only preview block, full-width, black on white. */
    .notes-print-preview {
      display: block !important;
      color: #000;
      background: #fff;
      padding: 24px 32px;
      font-size: 12pt;
      line-height: 1.6;
    }
  }
</style>
