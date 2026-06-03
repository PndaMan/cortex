<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";

  // ---- navigation history (kept in JS; the real page lives in a Tauri child
  // webview window driven by api.openBrowser). `pos` is the cursor into
  // `history`; Back/Forward move it and re-navigate, new navigation truncates
  // any forward entries and pushes. ----
  let history = $state<string[]>([]);
  let pos = $state(-1); // -1 = nothing navigated yet
  let draft = $state(""); // address-bar text
  let opened = $state(false); // whether a child window is currently open
  let busy = $state(false); // navigation in flight

  const current = $derived(pos >= 0 ? history[pos] : "");
  const canBack = $derived(pos > 0);
  const canForward = $derived(pos >= 0 && pos < history.length - 1);

  // Subject the captured source is filed under — defaults to the active subject.
  let subjectId = $state<string | null>(app.activeSubjectId);
  $effect(() => {
    // Keep the picker valid if the active subject changes elsewhere and we have
    // no explicit pick yet.
    if (!subjectId && app.activeSubjectId) subjectId = app.activeSubjectId;
  });

  const subjectOptions = $derived(
    app.subjects.map((s) => ({ id: s.id, label: `${s.glyph} ${s.name}` }))
  );

  // Turn raw input into a navigable URL. Has a scheme → use as-is; looks like a
  // bare domain (has a dot, no spaces) → prepend https://; otherwise treat the
  // text as a Google search query.
  function normalize(raw: string): string {
    const v = raw.trim();
    if (/^https?:\/\//i.test(v)) return v;
    if (/\.[a-z]{2,}/i.test(v) && !/\s/.test(v)) return "https://" + v;
    return "https://www.google.com/search?q=" + encodeURIComponent(v);
  }

  async function go(url: string, { push }: { push: boolean }) {
    busy = true;
    try {
      await api.openBrowser(url);
      opened = true;
      if (push) {
        // Truncate any forward history, then push the new entry.
        history = history.slice(0, pos + 1).concat(url);
        pos = history.length - 1;
      }
      draft = url;
    } catch (e) {
      app.pushToast({ kind: "error", title: "Could not open page", body: String(e) });
    } finally {
      busy = false;
    }
  }

  function submitAddress() {
    const v = draft.trim();
    if (!v) return;
    void go(normalize(v), { push: true });
  }

  function back() {
    if (!canBack) return;
    pos -= 1;
    void go(history[pos], { push: false });
  }
  function forward() {
    if (!canForward) return;
    pos += 1;
    void go(history[pos], { push: false });
  }
  function reload() {
    if (pos < 0) return;
    void go(history[pos], { push: false });
  }

  async function closeBrowser() {
    try {
      await api.closeBrowser();
      opened = false;
    } catch (e) {
      app.pushToast({ kind: "error", title: "Could not close window", body: String(e) });
    }
  }

  // Re-open the child window at the current entry (after it was closed).
  function reopen() {
    if (pos >= 0) void go(history[pos], { push: false });
  }

  async function saveAsSource() {
    if (!app.subjects.length) {
      app.pushToast({
        kind: "warning",
        title: "No subject yet",
        body: "Create a subject first, then capture pages into it.",
      });
      return;
    }
    const sid = subjectId ?? app.activeSubjectId ?? app.subjects[0]?.id ?? null;
    if (!sid) {
      app.pushToast({ kind: "warning", title: "Pick a subject", body: "Choose where to file this page." });
      return;
    }
    busy = true;
    try {
      // The child webview may have followed links past the address bar, so ask
      // it for its live URL; fall back to the bar / current entry if empty.
      let url = "";
      try {
        url = (await api.browserUrl()) ?? "";
      } catch {
        url = "";
      }
      url = url.trim() || draft.trim() || current;
      if (!url) {
        app.pushToast({ kind: "warning", title: "Nothing to save", body: "Navigate to a page first." });
        return;
      }
      await api.addSource({ subject_id: sid, url, kind: "url" });
      app.pushToast({ kind: "success", title: "Captured → ingesting", body: url });
      app.activeSubjectId = sid;
      app.setView("subject");
    } catch (e) {
      app.pushToast({ kind: "error", title: "Capture failed", body: String(e) });
    } finally {
      busy = false;
    }
  }
</script>

<div class="browser">
  <!-- Explainer -->
  <section class="bv-explain">
    <div class="bv-explain-icon"><Icon name="globe" size={22} color="var(--accent)" /></div>
    <div class="bv-explain-text">
      <div class="bv-explain-title read">Browse &amp; capture</div>
      <p class="bv-explain-body mono faint">
        Browse to any page in the popup window, then <strong>Save it as a source</strong>.
        No SearXNG needed.
      </p>
    </div>
    {#if current}
      <div class="bv-current mono" title={current}>
        <span class="bv-current-label faint">URL</span>
        <span class="bv-current-url">{current}</span>
      </div>
    {/if}
  </section>

  <!-- Toolbar -->
  <div class="bv-toolbar">
    <div class="bv-nav">
      <button class="btn btn--sm btn--icon" disabled={!canBack || busy} onclick={back} title="Back" aria-label="Back">
        <span style="display:block;transform:rotate(180deg)">
          <Icon name="chevron" size={14} color="currentColor" />
        </span>
      </button>
      <button class="btn btn--sm btn--icon" disabled={!canForward || busy} onclick={forward} title="Forward" aria-label="Forward">
        <Icon name="chevron" size={14} color="currentColor" />
      </button>
      <button class="btn btn--sm btn--icon" disabled={pos < 0 || busy} onclick={reload} title="Reload" aria-label="Reload">
        <Icon name="refresh" size={13} />
      </button>
    </div>

    <div class="bv-address">
      <Icon name="search" size={13} color="var(--fg-faint)" />
      <input
        type="text"
        value={draft}
        oninput={(e) => (draft = (e.target as HTMLInputElement).value)}
        onkeydown={(e) => {
          if (e.key === "Enter") { e.preventDefault(); submitAddress(); }
        }}
        placeholder="Type a URL, or search the web…"
        aria-label="Address or search"
        spellcheck={false}
      />
      <button class="btn btn--sm btn--primary bv-go" disabled={!draft.trim() || busy} onclick={submitAddress}>
        <Icon name="arrowR" size={13} /> Open page
      </button>
    </div>

    <button class="btn btn--sm btn--primary bv-save" disabled={busy} onclick={saveAsSource} title="Save current page as a source">
      <Icon name="plus" size={13} /> Save as source
    </button>
  </div>

  <!-- Capture controls -->
  <div class="bv-controls">
    <div class="bv-control">
      <span class="bv-control-label mono faint">Save into</span>
      {#if app.subjects.length}
        <Picker
          value={subjectId ?? ""}
          onChange={(id) => (subjectId = id)}
          options={subjectOptions}
          icon="book"
          placeholder="Pick a subject…"
        />
      {:else}
        <span class="bv-no-subject mono faint">No subjects yet — create one first.</span>
      {/if}
    </div>

    <div class="grow"></div>

    {#if opened}
      <button class="btn btn--sm" onclick={closeBrowser} title="Close the popup browser window">
        <Icon name="x" size={13} /> Close browser window
      </button>
    {:else if pos >= 0}
      <button class="btn btn--sm" onclick={reopen} title="Reopen the popup browser window">
        <Icon name="external" size={13} /> Reopen window
      </button>
    {/if}
  </div>

  <!-- Body / empty state -->
  <div class="bv-body">
    {#if pos < 0}
      <div class="bv-empty">
        <div class="bv-empty-icon"><Icon name="globe" size={28} color="var(--fg-faint)" /></div>
        <div class="bv-empty-title read">Open a page to begin</div>
        <p class="bv-empty-body mono faint">
          Type a URL or a search term above and press <kbd>Enter</kbd>. Cortex opens it in a
          separate browser window. When you find a page worth keeping, hit
          <strong>Save as source</strong> and it gets ingested into your chosen subject.
        </p>
      </div>
    {:else}
      <div class="bv-live">
        <div class="bv-live-icon">
          {#if busy}
            <span class="is-spin"></span>
          {:else}
            <Icon name="globe" size={26} color="var(--accent)" />
          {/if}
        </div>
        <div class="bv-live-title read">
          {opened ? "Browsing in the popup window" : "Window closed"}
        </div>
        <p class="bv-live-url mono">{current}</p>
        <p class="bv-live-hint mono faint">
          {#if opened}
            The page is live in the separate window. Click links there freely — when you press
            <strong>Save as source</strong>, Cortex captures whatever URL the window is currently showing.
          {:else}
            The popup window is closed. Reopen it, or just press <strong>Save as source</strong> to
            capture the last URL.
          {/if}
        </p>
      </div>
    {/if}
  </div>
</div>

<style>
  .browser {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    gap: 12px;
    padding: 16px;
  }

  /* explainer */
  .bv-explain {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .bv-explain-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 42px;
    height: 42px;
    border-radius: 12px;
    background: var(--surface-2);
    flex: 0 0 auto;
  }
  .bv-explain-text { flex: 0 1 auto; min-width: 0; }
  .bv-explain-title { font-size: var(--t-md); font-weight: 600; color: var(--fg-bright); }
  .bv-explain-body { font-size: var(--t-xs); line-height: 1.45; margin: 2px 0 0; }
  .bv-current {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 46%;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface-2);
    font-size: var(--t-2xs);
  }
  .bv-current-label {
    text-transform: uppercase;
    letter-spacing: 0.06em;
    flex: 0 0 auto;
  }
  .bv-current-url {
    color: var(--fg-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* toolbar */
  .bv-toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .bv-nav { display: flex; gap: 6px; flex: 0 0 auto; }
  .bv-address {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1 1 auto;
    min-width: 0;
    height: 34px;
    padding: 0 8px 0 12px;
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .bv-address:focus-within { border-color: var(--accent); }
  .bv-address input {
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--fg-bright);
    font-family: var(--font-mono);
    font-size: var(--t-md);
    outline: none;
  }
  .bv-address input::placeholder { color: var(--fg-faint); }
  .bv-go { flex: 0 0 auto; }
  .bv-save { flex: 0 0 auto; }

  /* controls */
  .bv-controls {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .bv-control { display: flex; align-items: center; gap: 8px; }
  .bv-control-label {
    font-size: var(--t-2xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .bv-no-subject { font-size: var(--t-xs); }
  .grow { flex: 1 1 auto; }

  /* body */
  .bv-body {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    background: var(--surface);
  }
  .bv-empty,
  .bv-live {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 12px;
    padding: 32px 28px;
    max-width: 520px;
  }
  .bv-empty-icon,
  .bv-live-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    border-radius: 16px;
    background: var(--surface-2);
  }
  .bv-empty-title,
  .bv-live-title { font-size: 15px; font-weight: 600; color: var(--fg-bright); }
  .bv-empty-body,
  .bv-live-hint { font-size: var(--t-xs); line-height: 1.5; margin: 0; }
  .bv-live-url {
    font-size: var(--t-xs);
    color: var(--accent);
    max-width: 100%;
    overflow-wrap: anywhere;
    margin: 0;
  }
  kbd {
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--border-strong);
    font-family: var(--font-mono);
    font-size: var(--t-2xs);
  }

  /* spinner — mirrors the WebSearch loading affordance */
  .is-spin {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    border-top-color: var(--accent);
    animation: bv-spin 0.7s linear infinite;
  }
  @keyframes bv-spin {
    to { transform: rotate(360deg); }
  }
</style>
