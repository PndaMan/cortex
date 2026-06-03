<script lang="ts">
  // Markdown notes — a master/detail workspace. Left: note list scoped to the
  // active subject (or all unfiled notes). Right: title + Markdown editor with
  // Save button, debounced autosave, "Convert to source", and Delete.
  // Renders full-page in `.workspace-scroll`, or compact when `embedded`.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { Note } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import MarkdownEditor from "../components/MarkdownEditor.svelte";

  let { embedded = false }: { embedded?: boolean } = $props();

  let notes = $state<Note[]>([]);
  let loading = $state(true);
  let selectedId = $state<string | null>(null);

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
  <div class={"notes" + (embedded ? " notes--embedded" : "")}>
    <aside class="notes-list">
      <div class="notes-list-head">
        <span class="notes-list-title">Notes</span>
        <button class="btn btn--primary btn--sm" type="button" onclick={newNote}>
          <Icon name="plus" size={12} /> New note
        </button>
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

    <section class="notes-detail">
      {#if selected}
        <div class="notes-detail-head">
          <input
            class="input notes-title"
            placeholder="Untitled"
            value={title}
            oninput={(e) => { title = (e.target as HTMLInputElement).value; markDirty(); }}
          />
          <span class={"notes-saved" + (saved ? " on" : "")}>
            {#if saved}<Icon name="check" size={12} /> Saved{:else}Editing…{/if}
          </span>
        </div>

        <MarkdownEditor value={body} onChange={(v) => { body = v; markDirty(); }} />

        <div class="notes-actions">
          <button class="btn btn--danger btn--sm" type="button" style="margin-right:auto" onclick={remove}>
            Delete
          </button>
          <span class="notes-convert-wrap" title={canConvert ? "" : "Notes need a subject to become a source"}>
            <button class="btn btn--ghost btn--sm" type="button" disabled={!canConvert} onclick={convert}>
              <Icon name="arrowR" size={13} /> Convert to source
            </button>
          </span>
          <button class="btn btn--primary btn--sm" type="button" disabled={saved} onclick={save}>
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

{#if embedded}
  {@render notesWorkspace()}
{:else}
  <div class="workspace-scroll">
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
  .notes-page { max-width: 1100px; margin: 0 auto; padding: 8px 4px 32px; }
  .notes-page-head { margin-bottom: 18px; }
  .notes-page-title {
    margin: 4px 0 2px; font-size: var(--r-lg, 20px); color: var(--fg-bright); font-weight: 600;
  }

  .notes {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 16px;
    align-items: start;
  }
  .notes--embedded {
    grid-template-columns: 180px 1fr;
    gap: 10px;
  }

  /* ----- list ----- */
  .notes-list {
    display: flex; flex-direction: column;
    border: 1px solid var(--border); border-radius: var(--r-lg, 12px);
    background: var(--surface); overflow: hidden;
  }
  .notes-list-head {
    display: flex; align-items: center; justify-content: space-between; gap: 8px;
    padding: 10px 12px; border-bottom: 1px solid var(--border); background: var(--surface-2);
  }
  .notes-list-title {
    font-size: var(--t-2xs, 10.5px); font-weight: 600; letter-spacing: 0.12em;
    text-transform: uppercase; color: var(--fg-faint);
  }
  .notes-items { display: flex; flex-direction: column; padding: 6px; gap: 2px; max-height: 70vh; overflow-y: auto; }
  .notes--embedded .notes-items { max-height: none; }
  .notes-item {
    display: flex; flex-direction: column; gap: 2px; text-align: left;
    padding: 8px 10px; border-radius: 8px; cursor: pointer;
    background: none; border: 1px solid transparent; color: var(--fg);
    transition: background 0.1s ease, border-color 0.1s ease;
  }
  .notes-item:hover { background: var(--surface-2); }
  .notes-item.on { background: var(--surface-3); border-color: var(--border-strong); }
  .notes-item-title {
    font-size: var(--t-xs, 12.5px); color: var(--fg-bright); font-weight: 500;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .notes-item-time { font-size: var(--t-2xs, 10px); color: var(--fg-faint); font-family: var(--font-mono); }
  .notes-hint { padding: 10px; color: var(--fg-faint); font-size: var(--t-xs, 12px); }

  /* ----- detail ----- */
  .notes-detail { display: flex; flex-direction: column; gap: 12px; min-width: 0; }
  .notes-detail-head { display: flex; align-items: center; gap: 12px; }
  .notes-title {
    flex: 1; min-width: 0; font-size: var(--t-md, 15px); font-weight: 600; color: var(--fg-bright);
  }
  .notes-saved {
    display: inline-flex; align-items: center; gap: 4px; flex-shrink: 0;
    font-size: var(--t-2xs, 10.5px); color: var(--fg-faint); font-family: var(--font-mono);
    transition: color 0.15s ease;
  }
  .notes-saved.on { color: var(--accent); }
  .notes-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .notes-convert-wrap { display: inline-flex; }

  /* ----- empty states ----- */
  .notes-empty, .notes-detail-empty {
    display: flex; flex-direction: column; align-items: center; gap: 8px; text-align: center;
    padding: 40px 16px; color: var(--fg-muted);
  }
  .notes-detail-empty {
    border: 1px dashed var(--border-strong); border-radius: var(--r-lg, 12px); background: var(--surface);
  }
  .notes-empty-glyph { font-size: 30px; line-height: 1; }
  .notes-empty-title { font-size: var(--t-md, 14px); font-weight: 600; color: var(--fg-bright); }
  .notes-empty-body { font-size: var(--t-xs, 12px); color: var(--fg-faint); max-width: 220px; }
</style>
