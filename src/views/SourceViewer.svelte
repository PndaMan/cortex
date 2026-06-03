<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { ChunkInfo } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import ChatPanel from "../components/ChatPanel.svelte";

  // ---- state ----
  let chunks = $state<ChunkInfo[]>([]);
  let loading = $state(true);
  let split = $state(58); // percent for left pane
  let dragging = $state(false);

  // ---- load chunks on mount ----
  $effect(() => {
    const src = app.activeSource;
    if (!src) return;
    loading = true;
    api.listChunks(src.id)
      .then((c) => { chunks = c; })
      .catch(() => { chunks = []; })
      .finally(() => { loading = false; });
  });

  // ---- splitter drag ----
  $effect(() => {
    function onMove(e: MouseEvent) {
      if (!dragging) return;
      const sidebar = 248; // --sb-w default
      const pct = ((e.clientX - sidebar) / (window.innerWidth - sidebar)) * 100;
      split = Math.min(72, Math.max(38, pct));
    }
    function onUp() {
      dragging = false;
      document.body.style.cursor = "";
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  });

  function startDrag() {
    dragging = true;
    document.body.style.cursor = "col-resize";
  }

  // ---- derived ----
  const src = $derived(app.activeSource);
  const dim = $derived(chunks.length > 0 ? chunks[0].dim : 0);
  const kindBadge = $derived(
    (src?.kind ?? "web") === "audio" ? "audio" : (src?.kind ?? "web")
  );
</script>

<div class="source-viewer">
  <!-- LEFT: embedding proof pane -->
  <div class="sv-pane sv-source" style:width="{split}%">
    <div class="sv-head">
      <button
        class="btn btn--icon btn--sm btn--ghost"
        onclick={() => app.closeSource()}
        title="Back"
      >
        <span style="display:block;transform:rotate(180deg)">
          <Icon name="chevron" size={13} color="currentColor" />
        </span>
      </button>

      {#if src}
        <span class="badge badge--{kindBadge}">
          <span class="dot"></span>
          {(src.kind ?? "web").toUpperCase()}
        </span>
        <span class="sv-name mono">{src.name}</span>
        <div class="grow"></div>
        {#if src.meta}
          <div class="sv-tools mono faint">{src.meta}</div>
        {/if}
      {/if}

      <button class="btn btn--icon btn--sm btn--ghost">
        <Icon name="search" size={13} />
      </button>
    </div>

    <!-- Embedding proof scrollable area -->
    <div class="sv-doc">
      {#if loading}
        <div class="pdf-page" style="width:100%;max-width:560px">
          <div class="pdf-line sk" style="width:60%;height:14px;margin-bottom:18px"></div>
          <div class="pdf-line sk" style="width:90%"></div>
          <div class="pdf-line sk" style="width:80%"></div>
          <div class="pdf-line sk" style="width:70%"></div>
        </div>
      {:else}
        <!-- Embedding proof header -->
        <div class="pdf-page" style="width:100%;max-width:560px">
          {#if chunks.length > 0}
            <h3 class="pdf-h" style="color:var(--ok)">
              ✓ {chunks.length} chunk{chunks.length === 1 ? "" : "s"} embedded · {dim}-dim vectors
            </h3>
            <p class="pdf-note mono" style="font-size:var(--t-xs);color:var(--fg-muted)">
              Source fully parsed and embedded. Each chunk below is stored with its vector.
            </p>
          {:else}
            <h3 class="pdf-h" style="color:var(--warn)">not embedded / ingesting</h3>
            <p class="pdf-note mono" style="font-size:var(--t-xs);color:var(--fg-muted)">
              No chunks found — the source may still be ingesting or failed to parse.
            </p>
          {/if}
        </div>

        <!-- Chunk list -->
        {#each chunks as chunk (chunk.ord)}
          <div class="pdf-page" style="width:100%;max-width:560px">
            <div class="pdf-pageno mono">#{chunk.ord}</div>
            {#if chunk.loc}
              <div class="pdf-formula mono" style="margin-bottom:10px">{chunk.loc}</div>
            {/if}
            <p class="read pdf-note">{chunk.text}</p>
          </div>
        {/each}
      {/if}
    </div>
  </div>

  <!-- Splitter -->
  <div
    class="sv-splitter"
    role="separator"
    aria-orientation="vertical"
    onmousedown={startDrag}
  >
    <span></span>
  </div>

  <!-- RIGHT: scoped chat -->
  <div class="sv-pane sv-chat" style:width="{100 - split}%">
    <ChatPanel />
  </div>
</div>
