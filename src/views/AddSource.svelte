<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { IngestResult } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // Current input method selected
  let method = $state<"upload" | "url" | "text" | "record" | "photo" | null>(null);
  // User-entered URL or text body
  let value = $state("");
  // Title for text pastes
  let textTitle = $state("");
  // Ingest progress state
  let stage = $state<string | null>(null); // null | parsing | chunking | embedding | storing | done | error
  let pct = $state(0);
  let errorMsg = $state<string | null>(null);
  // Last ingest result (for chunk count / warning)
  let lastResult = $state<IngestResult | null>(null);
  // Unlisten fn for progress events
  let unlisten = $state<UnlistenFn | null>(null);

  const methods = [
    { id: "upload" as const, ico: "doc",    t: "Upload File",    d: "PDF · PPTX · DOCX · TXT · MD", k: "u" },
    { id: "url"    as const, ico: "search", t: "Paste URL",       d: "web page · YouTube",            k: "p" },
    { id: "text"   as const, ico: "doc",    t: "Paste Text",      d: "markdown · plain text",         k: "t" },
    { id: "record" as const, ico: "record", t: "Record Lecture",  d: "live audio + transcript",       k: "r" },
    { id: "photo"  as const, ico: "grid",   t: "Snap Photo",      d: "OCR a whiteboard / page",       k: "o" },
  ] as const;

  const ORDER = ["parsing", "chunking", "embedding", "storing", "done"];

  const subj = $derived(app.activeSubject);
  // Selected target topic. Empty string means "no topic" → null on the wire.
  // Defaults to the first topic of the active subject when one exists.
  let selectedTopic = $state("");
  $effect(() => {
    // (re)default the selection whenever the active subject changes
    selectedTopic = subj?.topics[0]?.id ?? "";
  });
  const topicId = $derived(selectedTopic || null);

  function isDone(st: string) {
    return ORDER.indexOf(st) < ORDER.indexOf(stage ?? "");
  }
  function isActive(st: string) {
    return st === stage;
  }

  function reset() {
    stage = null;
    pct = 0;
    errorMsg = null;
    lastResult = null;
    value = "";
    textTitle = "";
    method = null;
    if (unlisten) { unlisten(); unlisten = null; }
  }

  function guardSubject(): boolean {
    if (!subj) {
      app.pushToast({ kind: "error", title: "Open a subject first", body: "Select a subject before adding a source." });
      return false;
    }
    return true;
  }

  async function subscribeProgress() {
    unlisten = await api.onIngestProgress((p) => {
      stage = p.stage;
      pct = p.pct;
      if (p.stage === "error") {
        errorMsg = p.detail;
        if (unlisten) { unlisten(); unlisten = null; }
      }
    });
  }

  async function beginUpload() {
    if (!guardSubject()) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Documents", extensions: ["pdf", "docx", "pptx", "doc", "ppt", "txt", "md"] }],
      });
      const path = typeof picked === "string" ? picked : picked?.[0] ?? null;
      if (!path) return;

      // Derive basename for name
      const name = path.split(/[\\/]/).pop() ?? path;

      await subscribeProgress();
      stage = "parsing";
      lastResult = null;

      const result = await api.addSource({
        subject_id: subj!.id,
        topic_id: topicId,
        path,
        name,
        tags: [],
      });
      lastResult = result;
      await finishIngest(result);
    } catch (e) {
      errorMsg = String(e);
      stage = null;
      if (unlisten) { unlisten(); unlisten = null; }
    }
  }

  async function beginUrl() {
    if (!guardSubject()) return;
    if (!value.trim()) return;
    try {
      await subscribeProgress();
      stage = "parsing";
      lastResult = null;

      const result = await api.addSource({
        subject_id: subj!.id,
        topic_id: topicId,
        url: value.trim(),
        name: value.trim(),
        tags: [],
      });
      lastResult = result;
      await finishIngest(result);
    } catch (e) {
      errorMsg = String(e);
      stage = null;
      if (unlisten) { unlisten(); unlisten = null; }
    }
  }

  async function beginText() {
    if (!guardSubject()) return;
    if (!value.trim()) return;
    try {
      const name = textTitle.trim() || "Pasted text";

      await subscribeProgress();
      stage = "parsing";
      lastResult = null;

      const result = await api.addSource({
        subject_id: subj!.id,
        topic_id: topicId,
        text: value.trim(),
        kind: "md",
        name,
        tags: [],
      });
      lastResult = result;
      await finishIngest(result);
    } catch (e) {
      errorMsg = String(e);
      stage = null;
      if (unlisten) { unlisten(); unlisten = null; }
    }
  }

  async function beginPhoto() {
    if (!guardSubject()) return;
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp"] }],
      });
      const path = typeof picked === "string" ? picked : picked?.[0] ?? null;
      if (!path) return;

      const name = path.split(/[\\/]/).pop() ?? path;

      await subscribeProgress();
      stage = "parsing";
      lastResult = null;

      const result = await api.addSource({
        subject_id: subj!.id,
        topic_id: topicId,
        path,
        kind: "image",
        name,
        tags: [],
      });
      lastResult = result;
      await finishIngest(result);
    } catch (e) {
      errorMsg = String(e);
      stage = null;
      if (unlisten) { unlisten(); unlisten = null; }
    }
  }

  async function finishIngest(result?: IngestResult) {
    if (unlisten) { unlisten(); unlisten = null; }
    await app.refresh();
    app.setTab("sources");

    const res = result ?? lastResult;
    if (res?.warning) {
      app.pushToast({ kind: "warning", title: "Source added (with note)", body: res.warning });
    } else {
      const chunks = res?.chunk_count ?? 0;
      app.pushToast({
        kind: "success",
        title: "Source embedded",
        body: chunks > 0 ? `Source embedded · ${chunks} chunks` : "New source ingested and ready.",
      });
    }

    app.setView("subject");
  }

  function handleBegin() {
    if (method === "upload") beginUpload();
    else if (method === "url") beginUrl();
    else if (method === "text") beginText();
    else if (method === "record") app.setView("recorder");
    else if (method === "photo") beginPhoto();
  }

  // Cleanup unlisten on component destroy
  $effect(() => {
    return () => {
      if (unlisten) { unlisten(); unlisten = null; }
    };
  });
</script>

<div class="workspace-scroll">
  <div class="addpage">
    <!-- Page header -->
    <div class="addpage-head">
      <button class="btn btn--icon btn--sm btn--ghost" onclick={() => app.setView("subject")} title="Back">
        <span style:transform="rotate(180deg)" style:display="flex"><Icon name="chevron" size={14} /></span>
      </button>
      <div>
        <div class="eyebrow">Add source</div>
        <h1 class="addpage-title read">New source</h1>
        {#if subj}
          <div class="mono faint" style:font-size="var(--t-xs)">
            into {subj.name}{selectedTopic ? " › " + (subj.topics.find((t) => t.id === selectedTopic)?.name ?? "") : ""}
          </div>
        {/if}
      </div>
    </div>

    {#if !stage}
      <!-- ===== METHOD SELECTION ===== -->
      <div class="add-methods">
        {#each methods as m (m.id)}
          <button
            class="add-method{method === m.id ? ' on' : ''}"
            onclick={() => {
              method = m.id;
              if (m.id === "record") app.setView("recorder");
            }}
          >
            <span class="am-ico"><Icon name={m.ico} size={20} /></span>
            <div class="am-text">
              <div class="am-t">{m.t}</div>
              <div class="am-d mono">{m.d}</div>
            </div>
            <span class="kbd">{m.k}</span>
          </button>
        {/each}
      </div>

      <!-- Target topic -->
      {#if subj}
        <div class="field" style:margin-top="14px">
          <label class="onb-label mono" for="addsrc-topic">TOPIC <span class="faint">where this source lives</span></label>
          <select id="addsrc-topic" class="input mono" bind:value={selectedTopic}>
            {#each subj.topics as t (t.id)}
              <option value={t.id}>{t.name}</option>
            {/each}
            <option value="">— no topic —</option>
          </select>
        </div>
      {/if}

      <!-- URL input -->
      {#if method === "url"}
        <div class="add-input-row">
          <input
            class="input"
            autofocus
            placeholder="https://… or a YouTube link"
            bind:value
          />
        </div>
      {/if}

      <!-- Text paste: optional title + textarea -->
      {#if method === "text"}
        <div class="add-input-row" style:display="flex" style:flex-direction="column" style:gap="8px">
          <input
            class="input"
            autofocus
            placeholder="Title (optional)"
            bind:value={textTitle}
          />
          <textarea
            class="input"
            placeholder="Paste your text or markdown here…"
            rows={8}
            style:resize="vertical"
            bind:value
          ></textarea>
        </div>
      {/if}

      <!-- Upload drop area — triggers file picker on click -->
      {#if method === "upload"}
        <div class="add-input-row">
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="add-drop" onclick={beginUpload}>
            <Icon name="doc" size={20} color="var(--fg-faint)" />
            <span class="mono">Click to browse or drop a file here</span>
          </div>
        </div>
      {/if}

      <!-- Photo picker drop area — triggers image picker on click -->
      {#if method === "photo"}
        <div class="add-input-row">
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="add-drop" onclick={beginPhoto}>
            <Icon name="grid" size={20} color="var(--fg-faint)" />
            <span class="mono">Click to browse for an image (PNG · JPG · WebP)</span>
          </div>
        </div>
      {/if}

      {#if errorMsg}
        <div style:color="var(--err)" style:margin-top="12px" style:font-size="var(--t-sm)">{errorMsg}</div>
      {/if}

      <!-- Footer actions -->
      <div class="add-foot">
        <button class="btn btn--ghost" onclick={() => app.setView("subject")}>Cancel</button>
        {#if method === "url"}
          <button class="btn btn--primary" disabled={!value.trim() || !subj} onclick={handleBegin}>
            Ingest source <Icon name="arrowR" size={13} />
          </button>
        {:else if method === "text"}
          <button class="btn btn--primary" disabled={!value.trim() || !subj} onclick={handleBegin}>
            Ingest source <Icon name="arrowR" size={13} />
          </button>
        {:else if method === "upload"}
          <button class="btn btn--primary" disabled={!subj} onclick={beginUpload}>
            Pick file <Icon name="arrowR" size={13} />
          </button>
        {:else if method === "photo"}
          <button class="btn btn--primary" disabled={!subj} onclick={beginPhoto}>
            Pick image <Icon name="arrowR" size={13} />
          </button>
        {:else if method === "record"}
          <button class="btn btn--primary" disabled={!subj} onclick={() => app.setView("recorder")}>
            Open recorder <Icon name="arrowR" size={13} />
          </button>
        {:else}
          <button class="btn btn--primary" disabled={!method || !subj} onclick={handleBegin}>
            Ingest source <Icon name="arrowR" size={13} />
          </button>
        {/if}
      </div>

    {:else}
      <!-- ===== INGEST PROGRESS ===== -->
      <div class="ingest ingest-page">
        <div class="ingest-file mono">
          <span class="badge badge--{method === 'url' ? 'web' : method === 'photo' ? 'img' : 'pdf'}">
            <span class="dot"></span>
            {method === "url" ? "WEB" : method === "photo" ? "IMG" : method === "text" ? "TXT" : "FILE"}
          </span>
          {value || (method === "upload" || method === "photo" ? "uploading…" : "processing…")}
        </div>

        <!-- Progress bar -->
        {#if pct > 0}
          <div style:width="100%" style:height="4px" style:background="var(--surface-2)" style:border-radius="var(--rad-pill)" style:margin-bottom="16px" style:overflow="hidden">
            <div style:height="100%" style:background="var(--accent)" style:border-radius="var(--rad-pill)" style:width="{pct}%" style:transition="width 0.4s var(--ease)"></div>
          </div>
        {/if}

        <div class="ingest-steps">
          {#each ORDER as st (st)}
            <div class="ingest-step{isDone(st) ? ' done' : ''}{isActive(st) ? ' active' : ''}">
              <span class="is-dot">
                {#if isDone(st) || (st === "done" && isActive(st))}
                  <Icon name="check" size={11} />
                {:else if isActive(st) && st !== "done"}
                  <span class="is-spin"></span>
                {/if}
              </span>
              <span class="is-label mono">{st === "done" ? "ready" : st}</span>
            </div>
          {/each}
        </div>

        {#if stage === "done"}
          <div class="ingest-done-row">
            <div class="ingest-done mono">
              <Icon name="check" size={13} color="var(--ok)" /> Embedded — source is ready.
            </div>
            <button class="btn btn--primary" onclick={() => finishIngest()}>
              Done <Icon name="check" size={13} />
            </button>
          </div>
        {/if}

        {#if stage === "error"}
          <div style:color="var(--err)" style:margin-top="16px" style:font-size="var(--t-sm)">
            {errorMsg ?? "Ingest failed"}
          </div>
          <button class="btn btn--ghost" style:margin-top="12px" onclick={reset}>Try again</button>
        {/if}
      </div>
    {/if}
  </div>
</div>
