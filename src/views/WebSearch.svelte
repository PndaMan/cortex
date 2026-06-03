<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { FetchedPage } from "../lib/api";
  import { wsCache } from "../lib/websearch";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";

  // ── Types ────────────────────────────────────────────────────────────────────

  type NavEntry = {
    url: string;       // the URL we fetched
    page: FetchedPage; // the result
  };

  type BrowserState =
    | { kind: "idle" }
    | { kind: "loading"; url: string }
    | { kind: "page"; entry: NavEntry }
    | { kind: "error"; url: string; msg: string };

  // ── Helpers ──────────────────────────────────────────────────────────────────

  function looksLikeUrl(v: string): boolean {
    // has a scheme, OR a dotted domain with no spaces
    return /^https?:\/\//.test(v) || (/\.[a-z]{2,}/i.test(v) && !/\s/.test(v));
  }

  function normalizeUrl(v: string): string {
    return /^https?:\/\//.test(v) ? v : "https://" + v;
  }

  function queryUrl(q: string): string {
    return "https://lite.duckduckgo.com/lite/?q=" + encodeURIComponent(q);
  }

  function hostOf(url: string): string {
    try { return new URL(url).hostname.replace(/^www\./, ""); }
    catch { return url.replace(/^https?:\/\//, "").split("/")[0]; }
  }

  // Deterministic favicon letter + bg
  const FAV_BGS = ["#3b6ea5","#2dd5b7","#a31f34","#e07a26","#5b6ee1","#3a5","#b54bd6"];
  function favBgFor(host: string): string {
    let h = 0;
    for (let i = 0; i < host.length; i++) h = (h * 31 + host.charCodeAt(i)) >>> 0;
    return FAV_BGS[h % FAV_BGS.length];
  }
  function favLetter(host: string): string {
    const clean = host.replace(/^www\./, "");
    return (clean[0] ?? "?").toUpperCase();
  }

  // ── Persistent session state (restored from wsCache on mount) ───────────────

  // Navigation history: flat array + pointer
  type HistCache = { history: NavEntry[]; idx: number; draft: string; };
  const restored = wsCache.tabs as HistCache | null;

  let history    = $state<NavEntry[]>(restored?.history ?? []);
  let histIdx    = $state<number>(restored?.idx ?? -1);
  let draft      = $state<string>(restored?.draft ?? "");
  let inputEl    = $state<HTMLInputElement | null>(null);

  // UI state — not persisted
  let loadState  = $state<BrowserState>({ kind: "idle" });
  let pickedSubjectId = $state<string>(app.activeSubjectId ?? "");
  let savedUrl   = $state<string | null>(null);  // URL that was last saved as source
  let linksOpen  = $state(false);

  // Keep the picker in sync when activeSubjectId changes elsewhere
  $effect(() => {
    if (app.activeSubjectId && !pickedSubjectId) pickedSubjectId = app.activeSubjectId;
  });

  // Persist whenever history / draft changes
  $effect(() => {
    const cache: HistCache = { history, idx: histIdx, draft };
    wsCache.tabs = [cache] as unknown[];
    wsCache.activeId = "main";
  });

  // ── Derived ──────────────────────────────────────────────────────────────────

  const curPage  = $derived(histIdx >= 0 && histIdx < history.length ? history[histIdx] : null);
  const canBack  = $derived(histIdx > 0);
  const canFwd   = $derived(histIdx < history.length - 1);
  const finalUrl = $derived(curPage?.page.final_url ?? curPage?.url ?? "");
  const pageHost = $derived(finalUrl ? hostOf(finalUrl) : "");

  const subjectOptions = $derived(
    app.subjects.map((s) => ({ id: s.id, label: (s.glyph ? s.glyph + " " : "") + s.name }))
  );

  // ── Navigation ───────────────────────────────────────────────────────────────

  async function navigate(url: string) {
    // Truncate forward history, push new entry
    const newHistory = history.slice(0, histIdx + 1);
    const newIdx = newHistory.length;

    loadState = { kind: "loading", url };
    draft = url;
    savedUrl = null;

    try {
      const page = await api.fetchPage(url);
      const entry: NavEntry = { url, page };
      history = [...newHistory, entry];
      histIdx = newIdx;
      loadState = { kind: "page", entry };
      draft = page.final_url || url;
      linksOpen = false;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      loadState = { kind: "error", url, msg };
      app.pushToast({ kind: "error", title: "Failed to load page", body: msg });
    }
  }

  async function reload() {
    if (!curPage) return;
    await navigate(curPage.url);
  }

  function back() {
    if (!canBack) return;
    histIdx--;
    const entry = history[histIdx];
    draft = entry.page.final_url || entry.url;
    loadState = { kind: "page", entry };
    savedUrl = null;
    linksOpen = false;
  }

  function forward() {
    if (!canFwd) return;
    histIdx++;
    const entry = history[histIdx];
    draft = entry.page.final_url || entry.url;
    loadState = { kind: "page", entry };
    savedUrl = null;
    linksOpen = false;
  }

  function submitAddress() {
    const v = draft.trim();
    if (!v) return;
    const url = looksLikeUrl(v) ? normalizeUrl(v) : queryUrl(v);
    void navigate(url);
  }

  function followLink(href: string) {
    // Resolve relative URLs using the current page's final_url as base
    let resolved = href;
    if (!/^https?:\/\//i.test(href) && finalUrl) {
      try { resolved = new URL(href, finalUrl).href; }
      catch { resolved = href; }
    }
    void navigate(resolved);
  }

  function openExternal(url: string) {
    try { window.open(url, "_blank", "noopener"); } catch { /* ignore */ }
  }

  // ── Save as source ────────────────────────────────────────────────────────────

  async function saveAsSource() {
    const page = curPage?.page;
    if (!page) return;
    if (!pickedSubjectId) {
      app.pushToast({ kind: "warning", title: "Pick a subject first" });
      return;
    }
    const url = page.final_url || page.url;
    try {
      await api.addSource({ subject_id: pickedSubjectId, url, kind: "url" });
      savedUrl = url;
      const subj = app.subjects.find((s) => s.id === pickedSubjectId);
      app.pushToast({
        kind: "success",
        title: "Captured — ingesting",
        body: (subj?.name ?? "Subject") + " · " + (page.title || url),
        action: {
          label: "Go to subject",
          run: () => app.openSubject(pickedSubjectId),
        },
      });
      await app.refresh();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      app.pushToast({ kind: "error", title: "Save failed", body: msg });
    }
  }

  // ── Keyboard shortcuts ────────────────────────────────────────────────────────

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      const el = document.activeElement as HTMLElement | null;
      const typing = el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA");
      if (e.key === "/" && !typing) {
        e.preventDefault();
        inputEl?.focus();
        inputEl?.select();
        return;
      }
      if (typing) return;
      if (e.key === "Escape") { linksOpen = false; return; }
      if (e.key === "ArrowLeft" || e.key === "Backspace") { if (canBack) back(); return; }
      if (e.key === "ArrowRight") { if (canFwd) forward(); return; }
      if (e.key === "r") { void reload(); return; }
      if (e.key === "o" && curPage) { openExternal(curPage.page.final_url || curPage.url); return; }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="br-shell">
  <!-- ── Toolbar ──────────────────────────────────────────────────────────────── -->
  <div class="br-toolbar">
    <div class="br-nav">
      <button class="br-ico" disabled={!canBack} onclick={back} title="Back (←)">
        <span style="display:block;transform:rotate(180deg)">
          <Icon name="chevron" size={14} color="currentColor" />
        </span>
      </button>
      <button class="br-ico" disabled={!canFwd} onclick={forward} title="Forward (→)">
        <Icon name="chevron" size={14} color="currentColor" />
      </button>
      <button class="br-ico" disabled={!curPage && loadState.kind !== "error"} onclick={reload} title="Reload (r)">
        <Icon name="refresh" size={13} color="currentColor" />
      </button>
    </div>

    <div class="br-address">
      {#if loadState.kind === "loading"}
        <span class="br-addr-spin is-spin"></span>
      {:else if curPage}
        <Icon name="globe" size={13} color="var(--accent)" />
      {:else}
        <Icon name="search" size={13} color="var(--fg-faint)" />
      {/if}
      <input
        bind:this={inputEl}
        bind:value={draft}
        onkeydown={(e) => {
          if (e.key === "Enter") { e.preventDefault(); submitAddress(); (e.target as HTMLInputElement).blur(); }
          if (e.key === "Escape") (e.target as HTMLInputElement).blur();
        }}
        onfocus={(e) => (e.target as HTMLInputElement).select()}
        placeholder="Search the web or paste a URL — press Enter"
        spellcheck={false}
        aria-label="Address bar"
      />
      {#if draft.trim()}
        <button class="br-go btn btn--sm btn--primary" onclick={submitAddress} title="Go">
          Go
        </button>
      {/if}
    </div>

    {#if curPage}
      <button
        class="br-ico"
        onclick={() => openExternal(curPage!.page.final_url || curPage!.url)}
        title="Open in browser (o)"
      >
        <Icon name="external" size={13} color="currentColor" />
      </button>
    {/if}
  </div>

  <!-- ── Page URL bar (shown when viewing a page) ─────────────────────────────── -->
  {#if finalUrl && loadState.kind === "page"}
    <div class="br-urlbar">
      <span
        class="br-fav"
        style:background={favBgFor(pageHost)}
        aria-hidden="true"
      >{favLetter(pageHost)}</span>
      <span class="br-urlbar-host mono">{pageHost}</span>
      <span class="br-urlbar-path mono faint">{finalUrl.replace(/^https?:\/\/[^/]+/, "")}</span>
      <div class="grow"></div>
      <button
        class="br-links-toggle btn btn--sm btn--ghost"
        onclick={() => (linksOpen = !linksOpen)}
        title="Toggle page links panel"
        aria-expanded={linksOpen}
      >
        <Icon name="link" size={12} />
        {curPage?.page.links.length ?? 0} links
        <span style:display="block" style:transform={linksOpen ? "rotate(90deg)" : ""}>
          <Icon name="chevron" size={11} color="var(--fg-faint)" />
        </span>
      </button>
    </div>
  {/if}

  <!-- ── Main content area ─────────────────────────────────────────────────────── -->
  <div class="br-body">

    <!-- ── IDLE / landing state ─────────────────────────────────────────────────── -->
    {#if loadState.kind === "idle"}
      <div class="br-state">
        <div class="br-state-icon"><Icon name="globe" size={28} color="var(--fg-faint)" /></div>
        <div class="br-state-title read">In-App Reader Browser</div>
        <p class="br-state-body mono faint">
          Search the web or paste a URL — pages open here in a readable article view.
          Save any page as a source with one click. No SearXNG needed.
        </p>
        <div class="br-state-hints mono faint">
          <span><kbd>/</kbd> focus address bar</span>
          <span><kbd>←</kbd> / <kbd>→</kbd> back / forward</span>
          <span><kbd>r</kbd> reload</span>
          <span><kbd>o</kbd> open in browser</span>
        </div>
        {#if draft.trim()}
          <button class="btn btn--sm btn--primary" onclick={submitAddress}>
            <Icon name="search" size={13} />
            {looksLikeUrl(draft.trim()) ? "Open " + draft.trim() : 'Search "' + draft.trim() + '"'}
          </button>
        {/if}
      </div>

    <!-- ── LOADING ──────────────────────────────────────────────────────────────── -->
    {:else if loadState.kind === "loading"}
      <div class="br-state">
        <div class="br-state-icon"><span class="is-spin" style:width="28px" style:height="28px"></span></div>
        <div class="br-state-title read">Loading…</div>
        <p class="br-state-body mono faint">{loadState.url}</p>
      </div>

    <!-- ── ERROR ────────────────────────────────────────────────────────────────── -->
    {:else if loadState.kind === "error"}
      <div class="br-state">
        <div class="br-state-icon"><Icon name="x" size={26} color="var(--err, #ff5345)" /></div>
        <div class="br-state-title read">Failed to load</div>
        <p class="br-state-body mono faint">{loadState.msg}</p>
        <div class="br-state-acts">
          <button class="btn btn--sm" onclick={reload}>
            <Icon name="refresh" size={13} /> Try again
          </button>
          <button class="btn btn--sm btn--ghost" onclick={() => openExternal(loadState.kind === "error" ? loadState.url : "")}>
            <Icon name="external" size={13} /> Open in browser
          </button>
        </div>
      </div>

    <!-- ── PAGE VIEW ─────────────────────────────────────────────────────────────── -->
    {:else if loadState.kind === "page" && curPage}
      {@const page = curPage.page}
      <div class="br-page-wrap">

        <!-- Article column -->
        <div class="br-reader-scroll">
          <article class="br-article">
            <h1 class="br-article-title read">{page.title || pageHost}</h1>

            <div class="br-article-meta mono faint">
              <Icon name="globe" size={12} color="var(--accent)" />
              {pageHost}
              {#if page.final_url !== page.url && page.url !== page.final_url}
                <span class="br-redirected">redirected</span>
              {/if}
            </div>

            {#if page.text}
              <div class="br-article-body read">
                {#each page.text.split(/\n{2,}/) as para}
                  {#if para.trim()}
                    <p>{para.trim()}</p>
                  {/if}
                {/each}
              </div>
            {:else}
              <p class="mono faint br-no-text">No readable text was extracted from this page.</p>
            {/if}

            <div class="br-article-end mono faint">— end of page —</div>
          </article>
        </div>

        <!-- Links panel (collapsible) -->
        {#if linksOpen && page.links.length > 0}
          <aside class="br-links-panel">
            <div class="br-links-head mono">
              <Icon name="link" size={12} color="var(--accent)" />
              Page links
              <span class="faint">({page.links.length})</span>
            </div>
            <div class="br-links-list">
              {#each page.links as lnk}
                {@const lhost = hostOf(lnk.href)}
                <button
                  class="br-link-row"
                  onclick={() => followLink(lnk.href)}
                  title={lnk.href}
                >
                  <span
                    class="br-fav br-fav-sm"
                    style:background={favBgFor(lhost)}
                    aria-hidden="true"
                  >{favLetter(lhost)}</span>
                  <span class="br-link-text">{lnk.text || lhost}</span>
                  <Icon name="arrowR" size={11} color="var(--fg-faint)" />
                </button>
              {/each}
            </div>
          </aside>
        {/if}

      </div>
    {/if}

  </div>

  <!-- ── Save-as-source action bar (only shown while viewing a page) ─────────── -->
  {#if loadState.kind === "page" && curPage}
    <div class="br-actionbar">
      {#if app.subjects.length === 0}
        <span class="br-nosubj mono faint">No subjects yet — create one first.</span>
        <button class="btn btn--sm btn--ghost" onclick={() => app.setView("add-subject")}>
          <Icon name="plus" size={13} /> New subject
        </button>
      {:else}
        <Picker
          value={pickedSubjectId}
          onChange={(id) => (pickedSubjectId = id)}
          options={subjectOptions}
          icon="book"
          placeholder="Pick subject…"
        />
        {#if savedUrl === finalUrl}
          <button class="btn btn--sm" disabled style:color="var(--ok)" style:border-color="color-mix(in oklab, var(--ok) 40%, transparent)">
            <Icon name="check" size={13} /> Saved as source
          </button>
          <button class="btn btn--sm btn--ghost" onclick={() => app.openSubject(pickedSubjectId)}>
            Go to subject <Icon name="arrowR" size={12} />
          </button>
        {:else}
          <button
            class="btn btn--sm btn--primary"
            disabled={!pickedSubjectId}
            onclick={saveAsSource}
            title="Save this page as a source and ingest it"
          >
            <Icon name="plus" size={13} /> Save as source
          </button>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  /* ── Shell layout ─────────────────────────────────────────────────── */
  .br-shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--bg);
    overflow: hidden;
  }

  /* ── Toolbar ─────────────────────────────────────────────────────── */
  .br-toolbar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-sunken);
  }

  .br-nav {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
  }

  .br-ico {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: var(--rad-2, 6px);
    border: none;
    background: none;
    color: var(--fg-muted);
    cursor: pointer;
    transition: background var(--dur-fast, 80ms), color var(--dur-fast, 80ms);
  }
  .br-ico:hover:not(:disabled) {
    background: var(--surface-2);
    color: var(--fg-bright);
  }
  .br-ico:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .br-address {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 30px;
    padding: 0 8px;
    border-radius: var(--rad-2, 6px);
    background: var(--surface-2);
    border: 1px solid var(--border);
    transition: border-color var(--dur-fast, 80ms), box-shadow var(--dur-fast, 80ms);
  }
  .br-address:focus-within {
    border-color: var(--border-strong);
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 14%, transparent);
  }
  .br-address input {
    flex: 1;
    border: none;
    outline: none;
    background: none;
    color: var(--fg-bright);
    font-family: var(--font-mono, monospace);
    font-size: var(--t-sm, 12px);
    min-width: 0;
  }
  .br-address input::placeholder { color: var(--fg-faint); }

  .br-addr-spin {
    flex: none;
    width: 13px;
    height: 13px;
  }

  .br-go {
    flex: none;
    height: 22px;
    padding: 0 8px;
    font-size: var(--t-xs, 11px);
  }

  /* ── URL bar (below toolbar, shows when page loaded) ─────────────── */
  .br-urlbar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-sunken);
    font-size: var(--t-xs, 11px);
    overflow: hidden;
  }
  .br-urlbar-host {
    color: var(--fg-muted);
    white-space: nowrap;
  }
  .br-urlbar-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }
  .br-links-toggle {
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
    height: 22px;
    padding: 0 7px;
    font-size: var(--t-xs, 11px);
    white-space: nowrap;
  }

  /* ── Favicon chips ───────────────────────────────────────────────── */
  .br-fav {
    flex: none;
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 700;
    color: #fff;
    font-family: var(--font-mono, monospace);
  }
  .br-fav-sm {
    width: 14px;
    height: 14px;
    font-size: 8px;
    border-radius: 3px;
  }

  /* ── Body area ───────────────────────────────────────────────────── */
  .br-body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* ── State screens (idle / loading / error) ──────────────────────── */
  .br-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 40px 28px;
    text-align: center;
    overflow-y: auto;
  }
  .br-state-icon {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 14px;
    background: var(--bg-soft, color-mix(in oklab, var(--fg-faint) 8%, transparent));
  }
  .br-state-title {
    font-size: 15px;
    font-weight: 600;
    color: var(--fg-bright);
  }
  .br-state-body {
    max-width: 440px;
    line-height: 1.55;
    font-size: 12px;
  }
  .br-state-hints {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    justify-content: center;
    font-size: 11px;
  }
  .br-state-hints kbd {
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid color-mix(in oklab, currentColor 28%, transparent);
    font-family: inherit;
    font-size: inherit;
  }
  .br-state-acts {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    justify-content: center;
  }

  /* ── Page / reader layout ────────────────────────────────────────── */
  .br-page-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  .br-reader-scroll {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    min-width: 0;
  }

  .br-article {
    max-width: 680px;
    margin: 0 auto;
    padding: 28px 24px 40px;
  }

  .br-article-title {
    font-size: 20px;
    font-weight: 700;
    color: var(--fg-bright);
    line-height: 1.3;
    margin: 0 0 10px;
  }

  .br-article-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    margin-bottom: 20px;
    padding-bottom: 12px;
    border-bottom: 1px solid var(--border);
  }

  .br-redirected {
    padding: 1px 6px;
    border-radius: 4px;
    background: color-mix(in oklab, var(--warn, #e0af68) 15%, transparent);
    color: var(--warn, #e0af68);
    font-size: 10px;
  }

  .br-article-body {
    font-size: var(--r-md, 14px);
    line-height: 1.7;
    color: var(--fg);
  }
  .br-article-body p {
    margin: 0 0 1em;
  }

  .br-no-text {
    font-size: 12px;
    font-style: italic;
    margin-top: 16px;
  }

  .br-article-end {
    margin-top: 32px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    text-align: center;
  }

  /* ── Links panel ─────────────────────────────────────────────────── */
  .br-links-panel {
    flex: none;
    width: 280px;
    border-left: 1px solid var(--border);
    background: var(--bg-sunken);
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }

  .br-links-head {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 9px 12px 8px;
    border-bottom: 1px solid var(--border);
    font-size: 11px;
    color: var(--fg-muted);
  }

  .br-links-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
    min-height: 0;
  }

  .br-link-row {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 6px 8px;
    border-radius: var(--rad-2, 6px);
    border: none;
    background: none;
    color: var(--fg-muted);
    font-size: var(--t-sm, 12px);
    text-align: left;
    cursor: pointer;
    transition: background var(--dur-fast, 80ms), color var(--dur-fast, 80ms);
  }
  .br-link-row:hover {
    background: var(--surface-2);
    color: var(--fg-bright);
  }
  .br-link-text {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    font-family: var(--font-mono, monospace);
    font-size: 11px;
  }

  /* ── Action bar (save as source) ─────────────────────────────────── */
  .br-actionbar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-top: 1px solid var(--border);
    background: var(--bg-sunken);
  }

  .br-nosubj {
    font-size: 12px;
    flex: 1;
  }

  /* ── Spinner ─────────────────────────────────────────────────────── */
  .is-spin {
    display: inline-block;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Utility ─────────────────────────────────────────────────────── */
  .grow { flex: 1; }
  .mono { font-family: var(--font-mono, monospace); }
  .faint { color: var(--fg-faint); }
  .read { font-family: var(--font-read, serif); }
</style>
