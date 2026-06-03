<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as mock from "../lib/mock";
  import type { SearchResult } from "../lib/mock";
  import Icon from "../components/Icon.svelte";

  // ---- types ----
  type StackEntry =
    | { kind: "serp"; query: string }
    | { kind: "page"; page: SearchResult; query: string };

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

  function makeTab(query: string): Tab {
    return {
      id: "t" + __tabSeq++,
      stack: [{ kind: "serp", query }],
      idx: 0,
      draft: query,
      sel: 0,
      cat: "All",
      ingest: null,
      added: [],
    };
  }

  // ---- state ----
  let tabs = $state<Tab[]>([makeTab(mock.search.suggested)]);
  let activeId = $state<string>(tabs[0].id);
  let inputEl = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLElement | null>(null);

  // ---- derived ----
  const active = $derived(tabs.find((t) => t.id === activeId) ?? tabs[0]);
  const entry = $derived(active.stack[active.idx]);
  const cur = $derived(entry.kind === "page" ? entry.page : null);
  const results = $derived(
    mock.search.results.filter(
      (r) => active.cat === "All" || r.cat === active.cat
    )
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

  function navigate(e: StackEntry) {
    patchActive((t) => {
      const stack = t.stack.slice(0, t.idx + 1).concat([e]);
      return { stack, idx: stack.length - 1, draft: e.query, ingest: null, sel: 0 };
    });
  }

  function runSearch(q: string) {
    if (!q.trim()) return;
    navigate({ kind: "serp", query: q.trim() });
  }

  function openResult(r: SearchResult) {
    navigate({ kind: "page", page: r, query: r.url });
  }

  function openUrl(u: string) {
    const url = /^https?:\/\//.test(u) ? u : "https://" + u;
    const host = url.replace(/^https?:\/\//, "").split("/")[0];
    const existing = mock.search.results.find((r) => r.url === url || r.host === host);
    if (existing) return openResult(existing);
    navigate({
      kind: "page",
      query: url,
      page: {
        id: "u" + Date.now(),
        title: host,
        url,
        host,
        fav: host[0]?.toUpperCase() ?? "?",
        favBg: "#3a5",
        cat: "General",
        engines: [],
        snippet: "",
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
      const n = makeTab(mock.search.suggested);
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

  function addSource(r: SearchResult) {
    const steps = ["parsing", "chunking", "embedding", "done"];
    let i = 0;
    patchActive(() => ({ ingest: steps[0] }));
    const iv = setInterval(() => {
      i++;
      if (i < steps.length) {
        patchActive(() => ({ ingest: steps[i] }));
      } else {
        clearInterval(iv);
        patchActive((t) => ({ ingest: "done" }));
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

  function favSquare(r: SearchResult, size: number) {
    return { bg: r.favBg, letter: r.fav, size };
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
        onclick={() => patchActive(() => ({}))}
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
    <div class="serp">
      <div class="serp-main">
        <div class="serp-bar">
          <span class="mono faint">≈ 41,200 results · {results.length} shown · via SearXNG</span>
          <div class="serp-cats">
            {#each SERP_CATS as cat}
              <button
                class="serp-cat{active.cat === cat ? ' on' : ''}"
                onclick={() => patchActive(() => ({ cat, sel: 0 }))}
              >{cat}</button>
            {/each}
          </div>
        </div>

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
      </div>

      <!-- Preview aside -->
      <aside class="serp-preview">
        {#if results[active.sel]}
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
          <div class="prev-empty mono faint">Hover a result to preview.</div>
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
