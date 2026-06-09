<script lang="ts">
  // Global semantic search (Ctrl+K): one box over EVERYTHING — semantic hits
  // from the vector index (all subjects' sources/transcripts) plus text matches
  // on sources, notes, calendar events and materials. Reuses the command
  // palette's design-system classes so it reads as a sibling overlay.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "./Icon.svelte";

  let q = $state("");
  let sel = $state(0);
  let hits = $state<api.SearchHit[]>([]);
  let searching = $state(false);
  let inputEl = $state<HTMLInputElement | null>(null);
  let debounce: ReturnType<typeof setTimeout> | null = null;
  let runId = 0; // drop stale responses when the user keeps typing

  const GROUPS: Record<string, { label: string; icon: string }> = {
    chunk: { label: "In your sources", icon: "search" },
    source: { label: "Sources", icon: "doc" },
    note: { label: "Notes", icon: "pencil" },
    event: { label: "Calendar", icon: "calendar" },
    material: { label: "Materials", icon: "grid" },
  };
  const ORDER = ["chunk", "source", "note", "event", "material"];
  const grouped = $derived(
    ORDER.map((k) => ({ kind: k, items: hits.filter((h) => h.kind === k) })).filter(
      (g) => g.items.length > 0
    )
  );
  const flat = $derived(grouped.flatMap((g) => g.items));

  function subjectName(id: string | null): string {
    return (id && app.subjects.find((s) => s.id === id)?.name) || "";
  }

  $effect(() => {
    const query = q;
    if (!app.searchOpen) return;
    if (debounce) clearTimeout(debounce);
    if (!query.trim()) { hits = []; searching = false; return; }
    searching = true;
    debounce = setTimeout(async () => {
      const id = ++runId;
      try {
        const res = await api.globalSearch(query);
        if (id === runId) { hits = res; sel = 0; }
      } catch {
        if (id === runId) hits = [];
      } finally {
        if (id === runId) searching = false;
      }
    }, 220);
  });

  $effect(() => {
    if (app.searchOpen) {
      q = "";
      hits = [];
      sel = 0;
      setTimeout(() => inputEl?.focus(), 0);
    }
  });

  async function open(h: api.SearchHit | undefined) {
    if (!h) return;
    app.searchOpen = false;
    if (h.kind === "chunk" || h.kind === "source") {
      if (h.subject_id) app.openSubject(h.subject_id);
      const src = await api.getSource(h.id).catch(() => null);
      if (src) app.openSource(src);
    } else if (h.kind === "note") {
      if (h.subject_id) app.openSubject(h.subject_id);
      app.setView("notes");
    } else if (h.kind === "event") {
      app.setView("calendar");
    } else if (h.kind === "material") {
      if (h.subject_id) app.openSubject(h.subject_id);
      app.setTab("materials");
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || (e.ctrlKey && e.key === "n")) {
      e.preventDefault();
      sel = Math.min(flat.length - 1, sel + 1);
    } else if (e.key === "ArrowUp" || (e.ctrlKey && e.key === "p")) {
      e.preventDefault();
      sel = Math.max(0, sel - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      void open(flat[sel]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation(); // keep the global Esc handler from also navigating back
      app.searchOpen = false;
    }
  }
</script>

{#if app.searchOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="overlay" onmousedown={() => (app.searchOpen = false)}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="cmdk" style:margin-top="12vh" onmousedown={(e) => e.stopPropagation()}>
      <div class="cmdk-input">
        <span class="lead"><Icon name="search" size={13} /></span>
        <input
          bind:this={inputEl}
          bind:value={q}
          onkeydown={onKey}
          placeholder="Search everything — notes, sources, transcripts, events…"
        />
        <span class="kbd">esc</span>
      </div>

      <div style:max-height="46vh" style:overflow-y="auto" style:overflow-x="hidden">
        {#if q.trim() && !searching && flat.length === 0}
          <div style:padding="22px" style:text-align="center" style:color="var(--fg-faint)" style:font-size="var(--t-sm)">
            No matches
          </div>
        {:else if searching && flat.length === 0}
          <div style:padding="22px" style:text-align="center" style:color="var(--fg-faint)" style:font-size="var(--t-sm)">
            Searching…
          </div>
        {/if}

        {#each grouped as g (g.kind)}
          {@const groupStart = flat.indexOf(g.items[0])}
          <div class="cmdk-group">
            <div class="gl">{GROUPS[g.kind].label}</div>
            {#each g.items as h, localIdx (h.kind + h.id + localIdx)}
              {@const globalIdx = groupStart + localIdx}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="cmdk-item{globalIdx === sel ? ' sel' : ''}"
                onmouseenter={() => (sel = globalIdx)}
                onclick={() => void open(h)}
              >
                <span class="ci-ico"><Icon name={GROUPS[g.kind].icon} size={14} /></span>
                <span class="gs-main">
                  <span class="gs-title">{h.title}</span>
                  {#if h.snippet}<span class="gs-snippet">{h.snippet}</span>{/if}
                </span>
                <span class="ci-kind">{subjectName(h.subject_id)}</span>
              </div>
            {/each}
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .gs-main { display: flex; flex-direction: column; gap: 1px; min-width: 0; flex: 1; }
  .gs-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .gs-snippet {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
