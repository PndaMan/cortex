<script lang="ts">
  // Themed right-click menu — replaces the bare WebKitGTK context menu so the
  // app stops feeling like a webview/PWA. Context-aware: editable targets get
  // Cut/Copy/Paste/Select all; plain text with a selection gets Copy/Select all.
  // Right-clicking a sidebar subject/topic row gets Edit / Archive / Delete.
  import { app } from "../lib/store.svelte";
  type Item = { label: string; key?: string; run: () => void; disabled?: boolean };

  // Edit / Archive / Delete for a sidebar subject or topic row, or null if the
  // target isn't one. Topic check first — a topic row is also inside a subject.
  function entityItems(target: Element | null): Item[] | null {
    const topicEl = target?.closest("[data-topic-id]") as HTMLElement | null;
    if (topicEl) {
      const tid = topicEl.dataset.topicId!;
      const sid = topicEl.dataset.subjectId!;
      const subj = app.subjects.find((s) => s.id === sid);
      const topic = subj?.topics.find((t) => t.id === tid);
      if (!topic) return null;
      return [
        { label: "Edit topic", run: () => app.openEdit({ kind: "topic", id: topic.id, name: topic.name, subjectId: sid, glyph: topic.glyph || "◆", tags: topic.tags ?? [] }) },
        { label: "Delete topic", run: async () => {
            if (await app.confirm({ title: `Delete topic "${topic.name}"?`, danger: true, okLabel: "Delete" })) app.deleteTopic(topic.id, sid);
          } },
      ];
    }
    const subjEl = target?.closest("[data-subject-id]") as HTMLElement | null;
    if (subjEl) {
      const sid = subjEl.dataset.subjectId!;
      const s = app.subjects.find((x) => x.id === sid);
      if (!s) return null;
      return [
        { label: "Edit subject", run: () => app.openEdit({ kind: "subject", id: s.id, name: s.name, code: s.code ?? "", glyph: s.glyph, color: app.subjectColor(s) }) },
        { label: "Archive subject", run: async () => {
            if (await app.confirm({ title: `Archive "${s.name}"?`, body: "Hidden everywhere but its data is kept. Restore from Settings → Data.", okLabel: "Archive" })) app.setSubjectArchived(s.id, true);
          } },
        { label: "Delete subject", run: async () => {
            if (await app.confirm({ title: `Delete "${s.name}"?`, danger: true, okLabel: "Delete" })) app.deleteSubject(s.id);
          } },
      ];
    }
    return null;
  }

  let open = $state(false);
  let x = $state(0);
  let y = $state(0);
  let items = $state<Item[]>([]);
  let menuEl = $state<HTMLDivElement | null>(null);

  function isEditable(el: Element | null): el is HTMLInputElement | HTMLTextAreaElement {
    if (!el) return false;
    const t = el.tagName;
    return t === "INPUT" || t === "TEXTAREA" || (el as HTMLElement).isContentEditable;
  }

  function selectionText(): string {
    const sel = window.getSelection?.();
    return sel ? sel.toString() : "";
  }

  async function writeClipboard(text: string) {
    try { await navigator.clipboard.writeText(text); }
    catch { try { document.execCommand("copy"); } catch { /* noop */ } }
  }

  async function readClipboard(): Promise<string> {
    try { return await navigator.clipboard.readText(); }
    catch { return ""; }
  }

  function insertIntoField(el: HTMLInputElement | HTMLTextAreaElement, text: string) {
    const start = el.selectionStart ?? el.value.length;
    const end = el.selectionEnd ?? el.value.length;
    el.value = el.value.slice(0, start) + text + el.value.slice(end);
    const caret = start + text.length;
    el.selectionStart = el.selectionEnd = caret;
    el.dispatchEvent(new Event("input", { bubbles: true }));
  }

  function fieldSelection(el: HTMLInputElement | HTMLTextAreaElement): string {
    const start = el.selectionStart ?? 0;
    const end = el.selectionEnd ?? 0;
    return el.value.slice(start, end);
  }

  function deleteFieldSelection(el: HTMLInputElement | HTMLTextAreaElement) {
    const start = el.selectionStart ?? 0;
    const end = el.selectionEnd ?? 0;
    if (start === end) return;
    el.value = el.value.slice(0, start) + el.value.slice(end);
    el.selectionStart = el.selectionEnd = start;
    el.dispatchEvent(new Event("input", { bubbles: true }));
  }

  function buildItems(target: Element | null): Item[] {
    // Subject/topic rows get their own actions (and skip the text menu).
    const entity = entityItems(target);
    if (entity) return entity;

    const editable = isEditable(target);
    const out: Item[] = [];

    if (editable && (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement)) {
      const el = target;
      const hasSel = (el.selectionStart ?? 0) !== (el.selectionEnd ?? 0);
      out.push({
        label: "Cut", key: "⌘X", disabled: !hasSel,
        run: async () => { const t = fieldSelection(el); if (t) { await writeClipboard(t); deleteFieldSelection(el); } },
      });
      out.push({
        label: "Copy", key: "⌘C", disabled: !hasSel,
        run: async () => { const t = fieldSelection(el); if (t) await writeClipboard(t); },
      });
      out.push({
        label: "Paste", key: "⌘V",
        run: async () => { el.focus(); const t = await readClipboard(); if (t) insertIntoField(el, t); },
      });
      out.push({
        label: "Select all", key: "⌘A",
        run: () => { el.focus(); el.select(); },
      });
      return out;
    }

    // Non-editable: operate on the page text selection.
    const sel = selectionText();
    out.push({
      label: "Copy", key: "⌘C", disabled: !sel,
      run: async () => { if (sel) await writeClipboard(sel); },
    });
    out.push({
      label: "Select all", key: "⌘A",
      run: () => {
        const range = document.createRange();
        const root = (target?.closest("p, li, td, .read, .bubble, .workspace-scroll") as Element) ?? document.body;
        range.selectNodeContents(root);
        const s = window.getSelection();
        s?.removeAllRanges();
        s?.addRange(range);
      },
    });
    return out;
  }

  $effect(() => {
    function onCtx(e: MouseEvent) {
      // Let the OS menu through only if something explicitly opts out.
      const target = e.target as Element | null;
      if (target?.closest("[data-native-menu]")) return;
      e.preventDefault();
      items = buildItems(target);
      open = true;
      // Position now; clamp to viewport after the menu measures.
      x = e.clientX;
      y = e.clientY;
      queueMicrotask(() => {
        if (!menuEl) return;
        const r = menuEl.getBoundingClientRect();
        if (e.clientX + r.width > window.innerWidth - 8) x = window.innerWidth - r.width - 8;
        if (e.clientY + r.height > window.innerHeight - 8) y = window.innerHeight - r.height - 8;
        x = Math.max(8, x); y = Math.max(8, y);
      });
    }
    function onDown(e: MouseEvent) {
      if (open && menuEl && !menuEl.contains(e.target as Node)) open = false;
    }
    function onKey(e: KeyboardEvent) { if (open && e.key === "Escape") { e.stopPropagation(); open = false; } }
    function onScroll() { if (open) open = false; }
    window.addEventListener("contextmenu", onCtx);
    window.addEventListener("mousedown", onDown, true);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("blur", onScroll);
    return () => {
      window.removeEventListener("contextmenu", onCtx);
      window.removeEventListener("mousedown", onDown, true);
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("blur", onScroll);
    };
  });

  function pick(it: Item) {
    if (it.disabled) return;
    open = false;
    Promise.resolve(it.run()).catch(() => {});
  }
</script>

{#if open}
  <div
    class="ctxmenu"
    bind:this={menuEl}
    style:left="{x}px"
    style:top="{y}px"
    role="menu"
    tabindex="-1"
  >
    {#each items as it (it.label)}
      <button class="ctx-item" class:disabled={it.disabled} role="menuitem" onclick={() => pick(it)}>
        <span class="ci-label">{it.label}</span>
        {#if it.key}<span class="ci-key mono">{it.key}</span>{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .ctxmenu {
    position: fixed; z-index: 95;
    min-width: 190px; padding: 5px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-3);
    box-shadow: var(--shadow-pop);
    animation: ctxIn var(--dur-fast) var(--ease);
  }
  @keyframes ctxIn { from { opacity: 0.4; transform: translateY(-4px); } }
  .ctx-item {
    display: flex; align-items: center; gap: 10px;
    width: 100%; height: 30px; padding: 0 9px;
    border: none; background: none; cursor: pointer;
    border-radius: var(--rad-2);
    color: var(--fg-muted); font-family: var(--font-sans); font-size: var(--t-sm);
    text-align: left;
  }
  .ctx-item:hover { background: var(--surface-3); color: var(--fg-bright); }
  .ctx-item.disabled { color: var(--fg-faint); cursor: default; opacity: 0.55; }
  .ctx-item.disabled:hover { background: none; color: var(--fg-faint); }
  .ci-label { flex: 1; }
  .ci-key { color: var(--fg-faint); font-size: var(--t-2xs); letter-spacing: 0.02em; }
</style>
