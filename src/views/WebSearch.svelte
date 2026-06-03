<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { WebResult } from "../lib/api";
  import Icon from "../components/Icon.svelte";

  // Default query shown in the first tab (replaces mock.search.suggested).
  const SUGGESTED = "dynamic programming memoization";

  // ---- types ----
  // A renderable result: the real WebResult plus the extra fields the existing
  // markup expects (fav chip, reader paragraphs, engines list, category).
  type Result = {
    id: string;
    title: string;
    url: string;
    host: string;
    snippet: string;
    fav: string;
    favBg: string;
    cat: string;
    engines: string[];
    reader: string[];
  };

  type SerpState = "idle" | "loading" | "results" | "error" | "setup";

  type StackEntry =
    | {
        kind: "serp";
        query: string;
        state: SerpState;
        results: Result[];
        error: string;
      }
    | { kind: "page"; page: Result; query: string };

  type Tab = {
    id: string;
    stack: StackEntry[];
    idx: number;
    draft: string;
    sel: number;
    cat: string;
    ingest: string | null;
    added: string[];
  };

  // ---- constants ----
  const SERP_CATS = ["All", "General", "Science", "Files", "Videos"];
  let __tabSeq = 1;

  // Deterministic favicon background derived from the host string.
  const FAV_BGS = ["#3b6ea5", "#2dd5b7", "#a31f34", "#e07a26", "#5b6ee1", "#3a5", "#b54bd6"];
  function favBgFor(host: string): string {
    let h = 0;
    for (let i = 0; i < host.length; i++) h = (h * 31 + host.charCodeAt(i)) >>> 0;
    return FAV_BGS[h % FAV_BGS.length];
  }
  function favLetter(host: string): string {
    const clean = host.replace(/^www\./, "");
    return (clean[0] ?? "?").toUpperCase();
  }

  // Map a real WebResult into a renderable Result with sensible fallbacks.
  function toResult(w: WebResult, i: number): Result {
    const host = w.host || w.url.replace(/^https?:\/\//, "").split("/")[0];
    const reader = w.snippet
      ? [w.snippet]
      : ["No preview text was returned for this result. Open it in your browser or add it as a source to extract the full page."];
    return {
      id: "w" + i + "-" + w.url,
      title: w.title || host,
      url: w.url,
      host,
      snippet: w.snippet,
      fav: favLetter(host),
      favBg: favBgFor(host),
      cat: "General",
      engines: w.engine ? [w.engine] : [],
      reader,
    };
  }

  function makeSerp(query: string): StackEntry {
    return { kind: "serp", query, state: "idle", results: [], error: "" };
  }

  function makeTab(query: string): Tab {
    return {
      id: "t" + __tabSeq++,
      stack: [makeSerp(query)],
      idx: 0,
      draft: query,
      sel: 0,
      cat: "All",
      ingest: null,
      added: [],
    };
  }

  // ---- state ----
  let tabs = $state<Tab[]>([makeTab(SUGGESTED)]);
  let activeId = $state<string>(tabs[0].id);
  let inputEl = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLElement | null>(null);

  // ---- derived ----
  const active = $derived(tabs.find((t) => t.id === activeId) ?? tabs[0]);
  const entry = $derived(active.stack[active.idx]);
  const cur = $derived(entry.kind === "page" ? entry.page : null);
  const serp = $derived(entry.kind === "serp" ? entry : null);
  // Results to render: the active serp entry's results, filtered by category.
  const results = $derived(
    serp
      ? serp.results.filter((r) => active.cat === "All" || r.cat === active.cat)
      : []
  );

  // ---- scroll selected result into view ----
  $effect(() => {
    const idx = active.sel;
    if (entry.kind === "serp" && listEl) {
      const rows = listEl.querySelectorAll<HTMLElement>(".serp-row");
      const node = rows[idx];
      if (node) {
        const b = listEl.getBoundingClientRect();
        const n = node.getBoundingClientRect();
        if (n.top < b.top || n.bottom > b.bottom) {
          node.scrollIntoView({ block: "nearest" });
        }
      }
    }
  });

  // ---- keyboard navigation ----
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
      if (e.key === "Escape") {
        if (cur) back();
        return;
      }
      if (entry.kind === "serp") {
        if (e.key === "j" || e.key === "ArrowDown") {
          e.preventDefault();
          patchActive((t) => ({ sel: Math.min(results.length - 1, t.sel + 1) }));
        } else if (e.key === "k" || e.key === "ArrowUp") {
          e.preventDefault();
          patchActive((t) => ({ sel: Math.max(0, t.sel - 1) }));
        } else if (e.key === "Enter") {
          e.preventDefault();
          const r = results[active.sel];
          if (r) openResult(r);
        }
      } else if (cur) {
        if (e.key === "o") {
          e.preventDefault();
          openExternal(cur.url);
        } else if (e.key === "a") {
          e.preventDefault();
          if (!active.ingest) addSource(cur);
        }
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // ---- helpers ----
  function patchActive(fn: (t: Tab) => Partial<Tab>) {
    tabs = tabs.map((t) => (t.id === activeId ? { ...t, ...fn(t) } : t));
  }

  // Patch the current stack entry of a specific tab in place.
  function patchEntry(id: string, fn: (e: StackEntry) => StackEntry) {
    tabs = tabs.map((t) => {
      if (t.id !== id) return t;
      const stack = t.stack.map((e, i) => (i === t.idx ? fn(e) : e));
      return { ...t, stack };
    });
  }

  function navigate(e: StackEntry) {
    patchActive((t) => {
      const stack = t.stack.slice(0, t.idx + 1).concat([e]);
      return { stack, idx: stack.length - 1, draft: e.query, ingest: null, sel: 0 };
    });
  }

  // Run a real web search for the active tab and stream the result into its
  // current serp entry. Sets loading first, then results / setup / error.
  async function runSearch(q: string) {
    const query = q.trim();
    if (!query) return;
    navigate(makeSerp(query));
    const tabId = activeId;
    patchEntry(tabId, (e) =>
      e.kind === "serp" ? { ...e, state: "loading", results: [], error: "" } : e
    );
    try {
      const raw = await api.webSearch(query);
      const mapped = raw.map((w, i) => toResult(w, i));
      patchEntry(tabId, (e) =>
        e.kind === "serp" && e.query === query
          ? { ...e, state: "results", results: mapped, error: "" }
          : e
      );
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      const isSetup = /searxng_url not configured/i.test(msg);
      patchEntry(tabId, (e) =>
        e.kind === "serp" && e.query === query
          ? { ...e, state: isSetup ? "setup" : "error", results: [], error: msg }
          : e
      );
    }
  }

  function openResult(r: Result) {
    navigate({ kind: "page", page: r, query: r.url });
  }

  function openUrl(u: string) {
    const url = /^https?:\/\//.test(u) ? u : "https://" + u;
    const host = url.replace(/^https?:\/\//, "").split("/")[0];
    // Reuse a result already loaded in the current serp entry if it matches.
    const existing =
      serp?.results.find((r) => r.url === url || r.host === host) ?? null;
    if (existing) return openResult(existing);
    navigate({
      kind: "page",
      query: url,
      page: {
        id: "u" + Date.now(),
        title: host,
        url,
        host,
        snippet: "",
        fav: favLetter(host),
        favBg: favBgFor(host),
        cat: "General",
        engines: [],
        reader: [
          "Reader view of " + url + ". In the desktop build, Cortex fetches the page through your self-hosted instance.",
          "Use the 'Add as source' button to ingest it.",
        ],
      },
    });
  }

  function submitAddress() {
    const v = active.draft.trim();
    if (!v) return;
    const looksUrl =
      /^https?:\/\//.test(v) || (/\.[a-z]{2,}/.test(v) && !/\s/.test(v));
    if (looksUrl) openUrl(v);
    else runSearch(v);
  }

  function back() {
    patchActive((t) =>
      t.idx > 0 ? { idx: t.idx - 1, draft: t.stack[t.idx - 1].query } : {}
    );
  }

  function forward() {
    patchActive((t) =>
      t.idx < t.stack.length - 1
        ? { idx: t.idx + 1, draft: t.stack[t.idx + 1].query }
        : {}
    );
  }

  // Reload re-runs the current serp query (or no-ops on a reader page).
  function reload() {
    if (entry.kind === "serp" && entry.query.trim()) {
      const q = entry.query;
      const tabId = activeId;
      patchEntry(tabId, (e) =>
        e.kind === "serp" ? { ...e, state: "loading", results: [], error: "" } : e
      );
      void (async () => {
        try {
          const raw = await api.webSearch(q);
          const mapped = raw.map((w, i) => toResult(w, i));
          patchEntry(tabId, (e) =>
            e.kind === "serp" && e.query === q
              ? { ...e, state: "results", results: mapped, error: "" }
              : e
          );
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err);
          const isSetup = /searxng_url not configured/i.test(msg);
          patchEntry(tabId, (e) =>
            e.kind === "serp" && e.query === q
              ? { ...e, state: isSetup ? "setup" : "error", results: [], error: msg }
              : e
          );
        }
      })();
    } else {
      patchActive(() => ({}));
    }
  }

  function addTab() {
    const t = makeTab("");
    tabs = [...tabs, t];
    activeId = t.id;
    setTimeout(() => inputEl?.focus(), 30);
  }

  function closeTab(id: string, e?: MouseEvent) {
    e?.stopPropagation();
    const left = tabs.filter((t) => t.id !== id);
    if (!left.length) {
      const n = makeTab(SUGGESTED);
      tabs = [n];
      if (id === activeId) activeId = n.id;
    } else {
      if (id === activeId) {
        const idx = tabs.findIndex((t) => t.id === id);
        activeId = left[Math.max(0, idx - 1)].id;
      }
      tabs = left;
    }
  }

  function openExternal(url: string) {
    try {
      window.open(url, "_blank", "noopener");
    } catch {}
  }

  function addSource(r: Result) {
    const steps = ["parsing", "chunking", "embedding", "done"];
    let i = 0;
    patchActive(() => ({ ingest: steps[0] }));
    const iv = setInterval(() => {
      i++;
      if (i < steps.length) {
        patchActive(() => ({ ingest: steps[i] }));
      } else {
        clearInterval(iv);
        patchActive(() => ({ ingest: "done" }));
        app.pushToast({
          kind: "success",
          title: "Source embedded",
          body: r.host + " added — cheatsheet draft pending.",
        });
        setTimeout(() => {
          patchActive((t) => ({ ingest: null, added: [...t.added, r.id] }));
        }, 1400);
      }
    }, 800);
  }
</script>

<div class="browser">
  <!-- Tab strip -->
  <div class="br-tabs">
    {#each tabs as tab (tab.id)}
      {@const te = tab.stack[tab.idx]}
      {@const tabTitle = te.kind === "serp" ? (te.query || "New tab") : te.page.title}
      {@const tabFav = te.kind === "page" ? te.page : null}
      <div
        class="br-tab{tab.id === activeId ? ' on' : ''}"
        onclick={() => (activeId = tab.id)}
        role="tab"
        aria-selected={tab.id === activeId}
        title={tabTitle}
      >
        {#if tabFav}
          <span
            class="br-fav"
            style:background={tabFav.favBg}
            style:width="14px"
            style:height="14px"
            style:font-size="8px"
          >{tabFav.fav}</span>
        {:else}
          <Icon name="search" size={12} color="var(--fg-faint)" />
        {/if}
        <span class="br-tab-title">{tabTitle}</span>
        <span
          class="br-tab-x"
          onclick={(e) => closeTab(tab.id, e)}
          role="button"
          tabindex="-1"
          aria-label="Close tab"
        >
          <Icon name="x" size={10} />
        </span>
      </div>
    {/each}
    <button class="br-tab-add" onclick={addTab} title="New tab">
      <Icon name="plus" size={13} />
    </button>
  </div>

  <!-- Toolbar -->
  <div class="br-toolbar">
    <div class="br-nav">
      <button
        class="br-ico"
        disabled={active.idx === 0}
        onclick={back}
        title="Back"
      >
        <span style="display:block;transform:rotate(180deg)">
          <Icon name="chevron" size={14} color="currentColor" />
        </span>
      </button>
      <button
        class="br-ico"
        disabled={active.idx >= active.stack.length - 1}
        onclick={forward}
        title="Forward"
      >
        <Icon name="chevron" size={14} color="currentColor" />
      </button>
      <button
        class="br-ico"
        onclick={reload}
        title="Reload"
      >
        <Icon name="refresh" size={13} />
      </button>
    </div>

    <div class="br-address">
      <Icon name={cur ? "search" : "search"} size={13} color="var(--fg-faint)" />
      <input
        bind:this={inputEl}
        value={active.draft}
        oninput={(e) => patchActive(() => ({ draft: (e.target as HTMLInputElement).value }))}
        onkeydown={(e) => {
          if (e.key === "Enter") { e.preventDefault(); submitAddress(); (e.target as HTMLInputElement).blur(); }
          if (e.key === "Escape") (e.target as HTMLInputElement).blur();
        }}
        placeholder="Search the web, or type a URL…"
        spellcheck={false}
      />
      <span class="br-engine mono"><span class="dot"></span>SearXNG</span>
    </div>

    {#if cur}
      <button class="btn btn--sm" onclick={() => openExternal(cur!.url)} title="Open in real browser (o)">
        <Icon name="arrowR" size={13} /> Open
      </button>
    {/if}
  </div>

  <!-- Content -->
  {#if entry.kind === "serp"}
    {@const st = entry.state}
    <div class="serp">
      <div class="serp-main">
        {#if st === "results"}
          <div class="serp-bar">
            <span class="mono faint">{entry.results.length} results · {results.length} shown · via SearXNG</span>
            <div class="serp-cats">
              {#each SERP_CATS as cat}
                <button
                  class="serp-cat{active.cat === cat ? ' on' : ''}"
                  onclick={() => patchActive(() => ({ cat, sel: 0 }))}
                >{cat}</button>
              {/each}
            </div>
          </div>
        {/if}

        {#if st === "idle"}
          <!-- (a) idle / empty: no query has been run yet -->
          <div class="serp-state">
            <div class="ss-icon"><Icon name="search" size={26} color="var(--fg-faint)" /></div>
            <div class="ss-title read">Search the web</div>
            <p class="ss-body mono faint">
              Type a query in the address bar above, or press <kbd>/</kbd> to focus it.
              Results come from your self-hosted SearXNG instance.
            </p>
            <button class="btn btn--sm btn--primary" onclick={() => runSearch(active.draft || SUGGESTED)}>
              <Icon name="search" size={13} /> Search “{(active.draft || SUGGESTED).trim()}”
            </button>
          </div>

        {:else if st === "loading"}
          <!-- (b) loading -->
          <div class="serp-state">
            <div class="ss-icon"><span class="is-spin"></span></div>
            <div class="ss-title read">Searching…</div>
            <p class="ss-body mono faint">Querying SearXNG for “{entry.query}”.</p>
          </div>

        {:else if st === "setup"}
          <!-- (c) setup: SearXNG endpoint not configured yet -->
          <div class="serp-state">
            <div class="ss-icon"><Icon name="bolt" size={26} color="var(--accent)" /></div>
            <div class="ss-title read">Web search needs a SearXNG endpoint</div>
            <p class="ss-body mono faint">
              Cortex runs web searches through your own self-hosted
              <strong>SearXNG</strong> instance — nothing is sent to a third party.
              Set the <code>searxng_url</code> value under
              <strong>Settings → Homelab</strong> to point at your instance, then try again.
            </p>
            <button class="btn btn--sm btn--primary" onclick={() => app.setView("settings")}>
              <Icon name="bolt" size={13} /> Open Settings → Homelab
            </button>
          </div>

        {:else if st === "error"}
          <!-- error: any other failure -->
          <div class="serp-state">
            <div class="ss-icon"><Icon name="x" size={24} color="var(--bad, #e0533a)" /></div>
            <div class="ss-title read">Search failed</div>
            <p class="ss-body mono faint">{entry.error || "An unknown error occurred."}</p>
            <button class="btn btn--sm" onclick={reload}>
              <Icon name="refresh" size={13} /> Try again
            </button>
          </div>

        {:else}
          <!-- results -->
          <div class="serp-list" bind:this={listEl}>
            {#if results.length === 0}
              <div class="serp-empty mono faint">No results in this category.</div>
            {/if}
            {#each results as r, i (r.id)}
              <div
                class="serp-row{i === active.sel ? ' sel' : ''}"
                onmouseenter={() => patchActive(() => ({ sel: i }))}
                onclick={() => openResult(r)}
                role="button"
                tabindex="0"
                onkeydown={(e) => e.key === "Enter" && openResult(r)}
              >
                <div class="serp-head">
                  <span
                    class="br-fav"
                    style:background={r.favBg}
                    style:width="18px"
                    style:height="18px"
                    style:font-size="10px"
                  >{r.fav}</span>
                  <div class="serp-host mono">
                    {r.host}<span class="serp-path">{r.url.replace(/^https?:\/\/[^/]+/, "")}</span>
                  </div>
                </div>
                <div class="serp-title">{r.title}</div>
                <div class="serp-snippet">{r.snippet}</div>
                <div class="serp-foot">
                  <div class="serp-engines">
                    {#each r.engines as eng}
                      <span class="engine-chip mono">{eng}</span>
                    {/each}
                  </div>
                  <div class="serp-actions">
                    <button
                      class="btn btn--sm btn--ghost"
                      onclick={(e) => { e.stopPropagation(); openResult(r); }}
                    >
                      <Icon name="search" size={12} /> Read
                    </button>
                    <button
                      class="btn btn--sm btn--ghost"
                      onclick={(e) => { e.stopPropagation(); openExternal(r.url); }}
                    >
                      <Icon name="arrowR" size={12} /> Open
                    </button>
                    <button
                      class="btn btn--sm btn--ghost"
                      onclick={(e) => { e.stopPropagation(); openResult(r); setTimeout(() => addSource(r), 60); }}
                    >
                      <Icon name="plus" size={12} /> Source
                    </button>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Preview aside -->
      <aside class="serp-preview">
        {#if st === "results" && results[active.sel]}
          {@const prev = results[active.sel]}
          <div class="prev-head">
            <span
              class="br-fav"
              style:background={prev.favBg}
              style:width="22px"
              style:height="22px"
              style:font-size="12px"
            >{prev.fav}</span>
            <div class="prev-host mono">{prev.host}</div>
          </div>
          <div class="prev-title read">{prev.title}</div>
          <div class="prev-body">
            {#each prev.reader.slice(0, 2) as p, i (i)}
              <p class="read">{p}</p>
            {/each}
          </div>
          <button class="btn btn--sm btn--primary prev-open" onclick={() => openResult(prev)}>
            Open reader <Icon name="arrowR" size={12} />
          </button>
        {:else}
          <div class="prev-empty mono faint">
            {#if st === "results"}Hover a result to preview.{:else}No preview.{/if}
          </div>
        {/if}
      </aside>
    </div>

  {:else if cur}
    <!-- Reader view -->
    <div class="reader-view">
      <div class="reader-scroll">
        <article class="reader-doc">
          <div class="reader-source">
            <span
              class="br-fav"
              style:background={cur.favBg}
              style:width="20px"
              style:height="20px"
              style:font-size="11px"
            >{cur.fav}</span>
            <span class="mono">{cur.host}</span>
            <span class="reader-url mono faint">{cur.url}</span>
          </div>
          <h1 class="reader-title read">{cur.title}</h1>
          <div class="reader-byline mono">
            <Icon name="search" size={12} color="var(--accent)" />
            Reader view · extracted by Cortex
            {#if cur.engines.length > 0}· via {cur.engines[0]}{/if}
          </div>
          {#each cur.reader as p, i (i)}
            <p class="reader-p read">{p}</p>
          {/each}
          <div class="reader-end mono faint">— end of extracted article —</div>
        </article>
      </div>

      <div class="reader-actionbar">
        {#if active.ingest}
          <div class="reader-ingest mono">
            {#each ["parsing", "chunking", "embedding", "done"] as st}
              {@const order = ["parsing", "chunking", "embedding", "done"]}
              {@const done = order.indexOf(st) < order.indexOf(active.ingest ?? "")}
              {@const on = st === active.ingest}
              <span class="ri-step{done ? ' done' : ''}{on ? ' on' : ''}">
                {#if done || (st === "done" && on)}
                  <Icon name="check" size={11} />
                {:else if on}
                  <span class="is-spin"></span>
                {:else}
                  <span class="ri-dot"></span>
                {/if}
                {st === "done" ? "ready" : st}
              </span>
            {/each}
          </div>
        {:else}
          <span class="ra-label mono faint">{cur.host}</span>
          <div class="grow"></div>
          <button
            class="btn btn--sm"
            onclick={() => openExternal(cur!.url)}
            title="Open in real browser (o)"
          >
            <Icon name="arrowR" size={13} /> Open in browser
          </button>
          {#if active.added.includes(cur.id)}
            <button
              class="btn btn--sm"
              disabled
              style:color="var(--ok)"
              style:border-color="color-mix(in oklab, var(--ok) 45%, transparent)"
            >
              <Icon name="check" size={13} /> Added as source
            </button>
          {:else}
            <button
              class="btn btn--sm btn--primary"
              onclick={() => addSource(cur!)}
              title="Add as source (a)"
            >
              <Icon name="plus" size={13} /> Add as source
            </button>
          {/if}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .serp-state {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 12px;
    padding: 32px 28px;
  }
  .ss-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 52px;
    height: 52px;
    border-radius: 14px;
    background: var(--bg-soft, rgba(127, 127, 127, 0.08));
  }
  .ss-title {
    font-size: 15px;
    font-weight: 600;
  }
  .ss-body {
    max-width: 420px;
    line-height: 1.5;
    font-size: 12px;
  }
  .ss-body code {
    padding: 1px 5px;
    border-radius: 5px;
    background: var(--bg-soft, rgba(127, 127, 127, 0.12));
  }
  .ss-body kbd {
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid color-mix(in oklab, currentColor 25%, transparent);
    font-family: inherit;
  }
</style>
