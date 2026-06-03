<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import { jobs } from "../lib/jobs.svelte";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";

  // Current input method selected
  let method = $state<"upload" | "url" | "text" | "record" | "photo" | null>(null);
  // User-entered URL or text body
  let value = $state("");
  // Title for text pastes
  let textTitle = $state("");

  const methods = [
    { id: "upload" as const, ico: "doc",    t: "Upload File",    d: "PDF · PPTX · DOCX · TXT · MD", k: "u" },
    { id: "url"    as const, ico: "search", t: "Paste URL",       d: "web page · YouTube",            k: "p" },
    { id: "text"   as const, ico: "doc",    t: "Paste Text",      d: "markdown · plain text",         k: "t" },
    { id: "record" as const, ico: "record", t: "Record Lecture",  d: "live audio + transcript",       k: "r" },
    { id: "photo"  as const, ico: "grid",   t: "Snap Photo",      d: "OCR a whiteboard / page",       k: "o" },
  ] as const;

  // Subject + topic selectors, seeded ONCE from the per-topic "+" token
  // (app.addSourceTopicId) or the active subject. A single guarded effect
  // avoids the prior two-effect cascade that reset the subject back to the
  // active one when the picked topic lived in a different subject. The subject
  // Picker's onChange resets the topic imperatively for later changes.
  let selectedSubjectId = $state<string>(app.activeSubjectId ?? "");
  let selectedTopic = $state(""); // "" = no topic → null on the wire
  let seeded = false;
  $effect(() => {
    if (seeded) return;
    const pending = app.addSourceTopicId;
    const owning = pending
      ? (app.subjects.find((s) => s.topics.some((t) => t.id === pending)) ?? null)
      : null;
    if (owning) {
      selectedSubjectId = owning.id;
      selectedTopic = pending!;
    } else {
      selectedSubjectId = app.activeSubjectId ?? "";
      selectedTopic = app.subjects.find((s) => s.id === selectedSubjectId)?.topics[0]?.id ?? "";
    }
    seeded = true;
    if (pending) app.addSourceTopicId = null; // consume once
  });
  const subjectOptions = $derived(
    app.subjects.map((s) => ({ id: s.id, label: s.name }))
  );
  const selectedSubject = $derived(
    app.subjects.find((s) => s.id === selectedSubjectId) ?? null
  );
  const topicId = $derived(selectedTopic || null);
  // Themed dropdown options: the selected subject's topics, plus an explicit "no topic" entry.
  const topicOptions = $derived([
    ...(selectedSubject?.topics ?? []).map((t) => ({ id: t.id, label: t.name })),
    { id: "", label: "— no topic —" },
  ]);

  function guardSubject(): boolean {
    if (!selectedSubject) {
      app.pushToast({ kind: "error", title: "Select a subject first", body: "Choose a subject before adding a source." });
      return false;
    }
    return true;
  }

  /** Fire-and-forget: registers a background job then navigates to the subject. */
  function startIngest(input: Parameters<typeof api.addSource>[0], name: string) {
    jobs.start({
      kind: "source",
      label: name,
      subjectId: input.subject_id,
      topicId: input.topic_id ?? null,
      run: () => api.addSource(input),
      onDone: () => app.refresh(),
    });
    app.openSubject(input.subject_id);
    app.setTab("sources");
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

      const name = path.split(/[\\/]/).pop() ?? path;
      startIngest({ subject_id: selectedSubjectId, topic_id: topicId, path, name, tags: [] }, name);
    } catch (e) {
      app.pushToast({ kind: "error", title: "File pick failed", body: String(e) });
    }
  }

  function beginUrl() {
    if (!guardSubject()) return;
    if (!value.trim()) return;
    const url = value.trim();
    startIngest({ subject_id: selectedSubjectId, topic_id: topicId, url, name: url, tags: [] }, url);
  }

  function beginText() {
    if (!guardSubject()) return;
    if (!value.trim()) return;
    const name = textTitle.trim() || "Pasted text";
    startIngest({ subject_id: selectedSubjectId, topic_id: topicId, text: value.trim(), kind: "md", name, tags: [] }, name);
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
      startIngest({ subject_id: selectedSubjectId, topic_id: topicId, path, kind: "image", name, tags: [] }, name);
    } catch (e) {
      app.pushToast({ kind: "error", title: "Image pick failed", body: String(e) });
    }
  }

  function handleBegin() {
    if (method === "upload") beginUpload();
    else if (method === "url") beginUrl();
    else if (method === "text") beginText();
    else if (method === "record") app.setView("recorder");
    else if (method === "photo") beginPhoto();
  }
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
        {#if selectedSubject}
          <div class="mono faint" style:font-size="var(--t-xs)">
            into {selectedSubject.name}{selectedTopic ? " › " + (selectedSubject.topics.find((t) => t.id === selectedTopic)?.name ?? "") : ""}
          </div>
        {/if}
      </div>
    </div>

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

    <!-- Target subject + topic -->
    <div class="field" style:margin-top="14px">
      <span class="onb-label mono">SUBJECT</span>
      <Picker
        value={selectedSubjectId}
        onChange={(id) => { selectedSubjectId = id; selectedTopic = ""; }}
        options={subjectOptions}
        placeholder="— select subject —"
      />
    </div>
    {#if selectedSubject}
      <div class="field" style:margin-top="10px">
        <span class="onb-label mono">TOPIC <span class="faint">where this source lives</span></span>
        <Picker
          value={selectedTopic}
          onChange={(id) => (selectedTopic = id)}
          options={topicOptions}
          placeholder="— no topic —"
        />
      </div>
    {/if}

    <!-- URL input -->
    {#if method === "url"}
      <div class="add-input-row">
        <!-- svelte-ignore a11y_autofocus -->
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
        <!-- svelte-ignore a11y_autofocus -->
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

    <!-- Footer actions -->
    <div class="add-foot">
      <button class="btn btn--ghost" onclick={() => app.setView("subject")}>Cancel</button>
      {#if method === "url"}
        <button class="btn btn--primary" disabled={!value.trim() || !selectedSubject} onclick={handleBegin}>
          Ingest source <Icon name="arrowR" size={13} />
        </button>
      {:else if method === "text"}
        <button class="btn btn--primary" disabled={!value.trim() || !selectedSubject} onclick={handleBegin}>
          Ingest source <Icon name="arrowR" size={13} />
        </button>
      {:else if method === "upload"}
        <button class="btn btn--primary" disabled={!selectedSubject} onclick={beginUpload}>
          Pick file <Icon name="arrowR" size={13} />
        </button>
      {:else if method === "photo"}
        <button class="btn btn--primary" disabled={!selectedSubject} onclick={beginPhoto}>
          Pick image <Icon name="arrowR" size={13} />
        </button>
      {:else if method === "record"}
        <button class="btn btn--primary" disabled={!selectedSubject} onclick={() => app.setView("recorder")}>
          Open recorder <Icon name="arrowR" size={13} />
        </button>
      {:else}
        <button class="btn btn--primary" disabled={!method || !selectedSubject} onclick={handleBegin}>
          Ingest source <Icon name="arrowR" size={13} />
        </button>
      {/if}
    </div>
  </div>
</div>
