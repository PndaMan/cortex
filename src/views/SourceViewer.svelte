<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { ChunkInfo } from "../lib/api";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import Icon from "../components/Icon.svelte";
  import ChatPanel from "../components/ChatPanel.svelte";
  import { isMobile } from "../lib/platform";
  import { tick } from "svelte";

  // ---- state ----
  let chunks = $state<ChunkInfo[]>([]);
  let loading = $state(true);
  let split = $state(58); // percent for left pane
  let dragging = $state(false);

  // Citation deep-link: when a source is opened from a ⟦… · loc⟧ chip, scroll to the
  // chunk whose loc matches the cited location and flash it. Best-effort — no match just
  // opens the source. Re-runs when chunks finish loading.
  $effect(() => {
    const loc = app.sourceJumpLoc;
    void chunks;
    if (!loc || chunks.length === 0) return;
    const lc = loc.toLowerCase();
    const target = chunks.find((c) => {
      const cl = (c.loc ?? "").toLowerCase();
      return !!cl && (cl === lc || cl.includes(lc) || lc.includes(cl));
    });
    app.sourceJumpLoc = null; // consume
    if (!target) return;
    void tick().then(() => {
      const el = document.getElementById("sv-chunk-" + target.ord);
      if (!el) return;
      el.scrollIntoView({ behavior: "smooth", block: "center" });
      el.classList.add("sv-chunk-flash");
      setTimeout(() => el.classList.remove("sv-chunk-flash"), 1600);
    });
  });

  // Mobile: zoom is locked app-wide (native-app feel), but a source IS the one place
  // you need pinch-zoom — to read a dense PDF/slide. Unlock the viewport while this
  // viewer is mounted and restore the lock on exit. Desktop is unaffected (no touch-zoom).
  $effect(() => {
    if (!isMobile) return;
    const meta = document.querySelector('meta[name="viewport"]');
    if (!meta) return;
    const locked = meta.getAttribute("content") ?? "width=device-width, initial-scale=1.0, maximum-scale=1, user-scalable=no";
    meta.setAttribute("content", "width=device-width, initial-scale=1.0, maximum-scale=5, user-scalable=yes");
    return () => meta.setAttribute("content", locked);
  });

  // ---- per-kind preview routing ----
  // `assetUrl` is the webview-loadable URL for the persisted original (or, for
  // pptx/docx, the backend-rendered PDF). Guarded against a null stored_path.
  const assetUrl = $derived(
    app.activeSource?.stored_path
      ? convertFileSrc(app.activeSource.stored_path)
      : null
  );
  // A document renders as an embedded PDF when its kind is a PDF-previewable
  // document. pptx/docx are rendered to PDF by the backend (their stored_path
  // points at that PDF), so they preview as slides via the same iframe.
  const PDF_KINDS = ["pdf", "pptx", "docx"];
  const isPdfDoc = $derived(
    !!app.activeSource && PDF_KINDS.includes(app.activeSource.kind) && !!assetUrl
  );
  const isImage = $derived(app.activeSource?.kind === "image" && !!assetUrl);
  const isAudio = $derived(app.activeSource?.kind === "audio" && !!assetUrl);
  // Readable text: explicit text-ish kinds, or any source with no stored file
  // but extracted content to show.
  const TEXT_KINDS = ["txt", "md", "web", "url", "epub"];
  const isText = $derived(
    !!app.activeSource &&
      !isPdfDoc &&
      !isImage &&
      !isAudio &&
      !!app.activeSource.content &&
      (TEXT_KINDS.includes(app.activeSource.kind) || !app.activeSource.stored_path)
  );
  // Anything with a dedicated preview skips the chunk-list fallback.
  const hasPreview = $derived(isPdfDoc || isImage || isAudio || isText);

  // ---- mobile PDF rendering ----
  // iOS WKWebView only renders the FIRST page of an <iframe> PDF and won't scroll,
  // so on mobile we render every page to a canvas with PDF.js into a scroll column.
  // Desktop keeps the native <iframe> preview.
  let pdfBox = $state<HTMLDivElement | null>(null);
  let pdfRendering = $state(false);
  let pdfError = $state<string | null>(null);
  $effect(() => {
    if (!(isMobile && isPdfDoc && assetUrl && pdfBox)) return;
    const url = assetUrl;
    const box = pdfBox;
    let cancelled = false;
    pdfRendering = true;
    pdfError = null;
    box.replaceChildren();
    (async () => {
      try {
        const pdfjs = await import("pdfjs-dist");
        const worker = await import("pdfjs-dist/build/pdf.worker.min.mjs?url");
        pdfjs.GlobalWorkerOptions.workerSrc = worker.default;
        // isEvalSupported/enableScripting harden the renderer (no eval, no in-PDF JS);
        // real runtime options in pdfjs v6 but absent from its public types — hence the cast.
        const doc = await pdfjs.getDocument({ url, isEvalSupported: false, enableScripting: false } as any).promise;
        const dpr = Math.min(window.devicePixelRatio || 1, 2);
        for (let n = 1; n <= doc.numPages; n++) {
          if (cancelled) return;
          const page = await doc.getPage(n);
          const base = page.getViewport({ scale: 1 });
          const cssWidth = box.clientWidth || 360;
          const vp = page.getViewport({ scale: (cssWidth / base.width) * dpr });
          const canvas = document.createElement("canvas");
          canvas.className = "sv-pdf-page";
          canvas.width = vp.width;
          canvas.height = vp.height;
          const ctx = canvas.getContext("2d");
          if (!ctx) continue;
          box.appendChild(canvas);
          await page.render({ canvasContext: ctx, viewport: vp, canvas }).promise;
        }
      } catch (e) {
        if (!cancelled) pdfError = String(e);
      } finally {
        if (!cancelled) pdfRendering = false;
      }
    })();
    return () => { cancelled = true; };
  });

  // ---- load chunks on mount / when the active source changes ----
  // Re-runs reactively whenever app.activeSource (or its id) changes. We only
  // need raw chunks when there's no richer preview to fall back to, plus audio
  // uses them as a transcript fallback.
  $effect(() => {
    const src = app.activeSource;
    if (!src) return;
    // Skip the (potentially large) chunk fetch when a real preview is shown and
    // we don't need a transcript fallback.
    if (hasPreview && !isAudio) {
      chunks = [];
      loading = false;
      return;
    }
    loading = true;
    api.listChunks(src.id)
      .then((c) => { chunks = c; })
      .catch(() => { chunks = []; })
      .finally(() => { loading = false; });
  });

  // ---- delete ----
  async function confirmDelete() {
    const src = app.activeSource;
    if (!src) return;
    if (await app.confirm({ title: "Delete this source?", danger: true, okLabel: "Delete" })) {
      await app.deleteSource(src.id); // store action closes the viewer on success
    }
  }

  // ---- edit ----
  function editSource() {
    const src = app.activeSource;
    if (!src) return;
    app.openEdit({
      kind: "source",
      id: src.id,
      name: src.name,
      subjectId: src.subject_id,
      topicId: src.topic_id,
      tags: src.tags ?? [],
      topicOptions: (app.activeSubject?.topics ?? []).map((t) => ({ id: t.id, label: t.name })),
    });
  }

  // transcript text for audio: prefer extracted content, fall back to chunks
  const transcript = $derived(
    app.activeSource?.content ?? (chunks.length ? chunks.map((c) => c.text).join("\n\n") : "")
  );

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
  <div class="sv-pane sv-source" style:width="{app.chatOpen ? split : 100}%">
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

      <button class="btn btn--icon btn--sm btn--ghost" title="Search">
        <Icon name="search" size={13} />
      </button>
      {#if src}
        <button
          class="btn btn--icon btn--sm btn--ghost"
          onclick={editSource}
          title="Edit source"
        >
          <Icon name="pencil" size={13} color="currentColor" />
        </button>
        <button
          class="btn btn--icon btn--sm btn--ghost sv-delete"
          onclick={confirmDelete}
          title="Delete source"
        >
          <Icon name="x" size={13} color="currentColor" />
        </button>
      {/if}
    </div>

    <!-- Preview / embedding proof scrollable area -->
    <div class="sv-doc" class:sv-doc--flush={isPdfDoc || isImage}>
      {#if src?.error}
        <!-- Failed ingest: surface the error prominently instead of an empty view -->
        <div class="pdf-page sv-error" style="width:100%;max-width:560px">
          <h3 class="pdf-h" style="color:var(--err, var(--warn))">Failed to ingest</h3>
          <p class="read pdf-note">{src.error}</p>
          <p class="pdf-note mono" style="font-size:var(--t-xs);color:var(--fg-muted)">
            This source couldn't be parsed. Try deleting and re-adding it.
          </p>
        </div>
      {:else if isPdfDoc && assetUrl}
        <!-- PDF / rendered slide preview (also covers pptx & docx via rendered PDF) -->
        {#if isMobile}
          <!-- All pages via PDF.js (WKWebView's iframe shows only page 1) -->
          <div class="sv-pdf-pages" bind:this={pdfBox}></div>
          {#if pdfRendering}
            <p class="pdf-note mono" style="font-size:var(--t-xs);color:var(--fg-muted);text-align:center;padding:12px">Rendering pages…</p>
          {/if}
          {#if pdfError}
            <p class="pdf-note mono" style="font-size:var(--t-xs);color:var(--err,var(--warn));text-align:center;padding:12px">Couldn't render PDF: {pdfError}</p>
          {/if}
        {:else}
          <iframe class="sv-frame" src={assetUrl} title={src?.name ?? "document"}></iframe>
        {/if}
      {:else if isImage && assetUrl}
        <!-- Image preview, centered & fit -->
        <div class="sv-img-wrap">
          <img class="sv-img" src={assetUrl} alt={src?.name ?? "image"} />
        </div>
      {:else if isAudio && assetUrl}
        <!-- Audio player + transcript -->
        <div class="pdf-page" style="width:100%;max-width:560px">
          <audio class="sv-audio" controls src={assetUrl}></audio>
        </div>
        <div class="pdf-page" style="width:100%;max-width:560px">
          <h3 class="pdf-h">Transcript</h3>
          {#if transcript}
            <p class="read pdf-note sv-text">{transcript}</p>
          {:else}
            <p class="pdf-note mono" style="font-size:var(--t-xs);color:var(--fg-muted)">
              No transcript available yet.
            </p>
          {/if}
        </div>
      {:else if isText && src?.content}
        <!-- Readable extracted text (txt / md / web / url, or content-only sources) -->
        <div class="pdf-page" style="width:100%;max-width:680px">
          <p class="read pdf-note sv-text">{src.content}</p>
        </div>
      {:else if loading}
        <div class="pdf-page" style="width:100%;max-width:560px">
          <div class="pdf-line sk" style="width:60%;height:14px;margin-bottom:18px"></div>
          <div class="pdf-line sk" style="width:90%"></div>
          <div class="pdf-line sk" style="width:80%"></div>
          <div class="pdf-line sk" style="width:70%"></div>
        </div>
      {:else}
        <!-- Fallback: raw chunk list (nothing richer to show) -->
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
          <div id={"sv-chunk-" + chunk.ord} class="pdf-page" style="width:100%;max-width:560px">
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

  <!-- Splitter + scoped chat — hidden when the chat is toggled off (c). The
       left pane expands to full width and App's "Ask c" FAB reopens it. -->
  {#if app.chatOpen}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="sv-splitter"
      role="separator"
      aria-orientation="vertical"
      tabindex="-1"
      onmousedown={startDrag}
    >
      <span></span>
    </div>

    <!-- RIGHT: scoped chat -->
    <div class="sv-pane sv-chat" style:width="{100 - split}%">
      <ChatPanel onClose={() => (app.chatOpen = false)} />
    </div>
  {/if}
</div>

<style>
  /* PDF / image previews fill the pane edge-to-edge instead of the padded,
     centered "document" column used by the chunk list. */
  .sv-doc--flush {
    padding: 0;
    gap: 0;
    align-items: stretch;
  }
  .sv-frame {
    flex: 1;
    width: 100%;
    border: none;
    background: var(--surface);
  }
  .sv-img-wrap {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--sp-6);
    min-height: 0;
  }
  .sv-img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    border-radius: var(--rad-2);
    border: 1px solid var(--border);
  }
  .sv-audio {
    width: 100%;
    display: block;
  }
  /* Preserve paragraphs/newlines for readable plaintext and transcripts. */
  .sv-text {
    white-space: pre-wrap;
    word-break: break-word;
  }
  .sv-error {
    border-color: color-mix(in oklab, var(--warn) 40%, var(--border));
  }
  .sv-delete:hover {
    color: var(--err, var(--warn));
  }
</style>
