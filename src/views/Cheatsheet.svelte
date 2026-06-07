<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { CheatsheetData, CsSection as ApiCsSection, CheatsheetVersionMeta } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import { jobs } from "../lib/jobs.svelte";
  import GeneratingCard from "../components/GeneratingCard.svelte";
  import RichText from "../components/RichText.svelte";
  import MarkdownEditor from "../components/MarkdownEditor.svelte";
  import { savePdf } from "../lib/pdf";

  const esc = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  // ── jump index ───────────────────────────────────────────────
  let indexPinned = $state(false);
  // Per-section expand state (collapsed by default so the index stays compact).
  let idxExpanded = $state<Record<string, boolean>>({});
  function toggleIdxSec(id: string) {
    idxExpanded = { ...idxExpanded, [id]: !idxExpanded[id] };
  }
  // Offset the floating index left of the chat dock when it's showing.
  const chatDockOpen = $derived(
    app.chatOpen && app.view === "subject" && app.subjectTab !== "chats"
  );
  function jumpTo(id: string) {
    document.getElementById(id)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  // ── topic selection ──────────────────────────────────────────
  // null = the whole-subject cheatsheet ("All"); otherwise a topic id. The pending
  // topic (app.cheatTopicId, set by openSubject/openTopicSheet to THIS subject's
  // remembered topic) is applied by the cheatNonce effect below — including on a
  // fresh mount, so arriving via `space h` from another page restores the topic.
  let selectedTopicId = $state<string | null>(null);
  // Optionally illustrate sections with web images (diagrams). Needs SearXNG.
  let withImages = $state(false);

  // Topics that actually have sources — only these get their own cheatsheet tab.
  const topicTabs = $derived(
    (app.activeSubject?.topics ?? []).filter((t) => t.sources.length > 0)
  );

  // Subtle per-subject accent for the active pill.
  const accent = $derived(app.subjectColor(app.activeSubject));

  // Background cheatsheet jobs for the active subject, filtered to the CURRENT
  // selection (running + any errors to surface). topicId is normalized: the
  // whole-subject job carries topicId === null.
  const csJobs = $derived(
    jobs
      .forSubject(app.activeSubjectId)
      .filter(
        (j) =>
          j.kind === "cheatsheet" &&
          (j.status === "running" || j.status === "error") &&
          (j.topicId ?? null) === selectedTopicId
      )
  );
  const csGenerating = $derived(csJobs.some((j) => j.status === "running"));

  // ── reactive data (the currently displayed sheet) ────────────
  let sections = $state<ApiCsSection[]>([]);
  let cheatTopic = $state<string>("");
  let sourceCount = $state<number>(0);
  let sourcesUsed = $state<number>(0); // sources actually synthesized (coverage)
  let hasCheatsheet = $state(false); // true = a real stored cheatsheet is loaded
  let loading = $state(false);

  const sectionCount = $derived(sections.length);

  // Reset the selection to "whole subject" whenever the active subject changes,
  // and drop any tab that no longer has sources.
  $effect(() => {
    const sub = app.activeSubject;
    if (!sub) {
      selectedTopicId = null;
      return;
    }
    if (
      selectedTopicId !== null &&
      !sub.topics.some((t) => t.id === selectedTopicId && t.sources.length > 0)
    ) {
      selectedTopicId = null;
    }
  });

  // Apply a freshly-loaded cheatsheet to local view state.
  function applyCheatsheet(data: CheatsheetData) {
    sections = data.sections;
    cheatTopic = data.topic;
    sourceCount = data.sources;
    sourcesUsed = data.sources_used ?? data.sources;
    hasCheatsheet = true;
    app.pending = data.sections.filter((s) => s.state === "draft-pending").length;
  }

  // Clear local view state to the "nothing generated yet" placeholder.
  function applyEmpty() {
    const sub = app.activeSubject;
    sections = [];
    sourceCount =
      selectedTopicId === null
        ? (sub?.sourceCount ?? 0)
        : (sub?.topics.find((t) => t.id === selectedTopicId)?.sources.length ?? 0);
    cheatTopic =
      selectedTopicId === null
        ? (sub?.name ?? "")
        : (sub?.topics.find((t) => t.id === selectedTopicId)?.name ?? "");
    hasCheatsheet = false;
    app.pending = 0;
  }

  // ── load the stored cheatsheet for the active subject + selection ──
  // Re-runs when the active subject OR the selected topic changes.
  $effect(() => {
    const sub = app.activeSubject;
    // Touch selectedTopicId so this effect tracks it.
    const topicId = selectedTopicId;
    // Track the reload nonce so a background (auto-)regen refreshes this view.
    void app.cheatsheetReloadNonce;
    if (!sub) {
      applyEmpty();
      return;
    }
    loading = true;
    let cancelled = false;
    // Whole subject = composed from the per-topic sheets; a topic = its own sheet.
    const load =
      topicId === null
        ? api.getSubjectCheatsheet(sub.id)
        : api.getCheatsheet(sub.id, topicId);
    load
      .then((data) => {
        if (cancelled) return;
        if (data) applyCheatsheet(data);
        else applyEmpty();
      })
      .catch(() => {
        if (!cancelled) applyEmpty();
      })
      .finally(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  // ── generate / regenerate for the CURRENT selection (BACKGROUND) ──
  // Runs through the global jobs store so navigating away won't cancel it; on
  // completion we reload the persisted cheatsheet for that exact selection.
  function generate() {
    const sub = app.activeSubject;
    if (!sub || csGenerating) return;
    const topicId = selectedTopicId; // capture the selection at click time
    const topicName =
      topicId === null
        ? sub.name
        : (sub.topics.find((t) => t.id === topicId)?.name ?? sub.name);
    jobs.start({
      kind: "cheatsheet",
      label: topicId === null ? `${topicName} (all topics)` : topicName,
      subjectId: sub.id,
      topicId,
      // Whole subject regenerates every topic's sheet then composes; a topic
      // regenerates just its own.
      run: () =>
        topicId === null
          ? api.generateSubjectCheatsheet(sub.id, withImages)
          : api.generateCheatsheet(sub.id, topicId, withImages),
      onDone: () => {
        // Only refresh the view if the user is still on this subject AND still
        // looking at the selection we generated for.
        if (app.activeSubjectId !== sub.id || selectedTopicId !== topicId) return;
        const reload =
          topicId === null
            ? api.getSubjectCheatsheet(sub.id)
            : api.getCheatsheet(sub.id, topicId);
        reload
          .then((data) => {
            if (data) applyCheatsheet(data);
          })
          .catch(() => {});
      },
    });
  }

  // Command-palette "Regenerate cheatsheet" bumps a nonce; regenerate the
  // whole-subject sheet here, where generation + live refresh already live.
  let lastRegenNonce = app.cheatsheetRegenNonce;
  $effect(() => {
    const n = app.cheatsheetRegenNonce;
    if (n !== lastRegenNonce) {
      lastRegenNonce = n;
      if (app.activeSubject && !csGenerating) {
        selectedTopicId = null; // whole-subject scope
        generate();
      }
    }
  });

  // Apply the pending topic request (sidebar/dashboard/space-h set app.cheatTopicId
  // + bump app.cheatNonce). Initialized to -1 (not the current nonce) so the FIRST
  // run on a fresh mount also applies it — otherwise arriving from another page
  // would miss the already-bumped nonce and fall back to whole-subject.
  let appliedNonce = -1;
  $effect(() => {
    const n = app.cheatNonce;
    if (n !== appliedNonce) {
      appliedNonce = n;
      if (app.activeSubject) selectedTopicId = app.cheatTopicId;
    }
  });

  function selectTopic(id: string | null) {
    if (selectedTopicId === id) return;
    selectedTopicId = id;
    app.cheatTopicId = id; // keep the canonical "current topic" in sync with the tab bar
  }

  // ── EXPORT ───────────────────────────────────────────────────
  // Render the current sheet (single topic OR whole subject) to a real PDF via
  // the backend (headless Chromium) — reliable where webview print() isn't.
  async function exportCurrent() {
    const el = document.querySelector(".cs-doc .cs-sections");
    const sub = app.activeSubject;
    if (!el || !sub || !hasCheatsheet) {
      app.pushToast({ kind: "warning", title: "Nothing to export", body: "Generate a cheatsheet first." });
      return;
    }
    const subtitle = `${sub.name}${sub.code ? " · " + sub.code : ""} · synthesized from ${sourceCount} source${sourceCount !== 1 ? "s" : ""} · ${sectionCount} enforced sections`;
    const body =
      `<article class="cs-doc"><div class="cs-doc-head"><div>` +
      `<div class="eyebrow">Cheatsheet · ${selectedTopicId === null ? "Whole subject" : "Topic"}</div>` +
      `<h1 class="cs-title">${esc(cheatTopic)}</h1>` +
      `<div class="cs-sub">${esc(subtitle)}</div></div></div>` +
      `<div class="cs-sections">${el.innerHTML}</div></article>`;
    await savePdf(body, `${sub.name} — ${cheatTopic}`);
  }

  // "Export all" loads the whole-subject sheet PLUS every topic-with-sources
  // sheet, renders them all into a hidden print-only block, then prints. Topics
  // without a stored sheet are skipped. We fall back to printing whatever is
  // already on screen if nothing could be loaded.
  let exportAll = $state<CheatsheetData[]>([]);
  let exporting = $state(false);

  async function exportWholeSubject() {
    const sub = app.activeSubject;
    if (!sub || exporting) return;
    exporting = true;
    try {
      // The composed whole-subject sheet already contains every topic's sections
      // (verbatim) with topic dividers, so exporting it = the full subject.
      const composed = await api.getSubjectCheatsheet(sub.id).catch(() => null);
      exportAll = composed ? [composed] : [];
      if (exportAll.length === 0) {
        app.pushToast({
          kind: "warning",
          title: "Nothing to export",
          body: "No cheatsheets have been generated for this subject yet.",
        });
        return;
      }
      // Let the hidden export block render, then serialize it to the PDF.
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      const block = document.querySelector(".cs-export-all");
      if (block) {
        await savePdf(block.innerHTML, `${sub.name} — all cheatsheets`);
      }
    } finally {
      exporting = false;
      // Clear after the print dialog returns so the hidden block doesn't linger.
      setTimeout(() => {
        exportAll = [];
      }, 0);
    }
  }

  // ── navigation memory ────────────────────────────────────────
  // Remember the subject + selected topic so leaving for another page and pressing
  // the cheatsheet keybind (space h) returns to this exact sheet.
  $effect(() => {
    const sub = app.activeSubject;
    if (sub) app.rememberCheatsheet(sub.id, selectedTopicId);
  });

  // ── per-sheet scroll memory ──────────────────────────────────
  // The cheatsheet shares ONE scroll container while its content swaps, so without
  // this each sheet would inherit the previous one's scroll offset. Key the saved
  // offset by subject+topic so every sheet remembers its OWN position independently
  // (session-scoped). Restore after content (re)loads.
  let scrollEl = $state<HTMLDivElement | null>(null);
  const scrollKey = () => `${app.activeSubjectId ?? ""}:${selectedTopicId ?? "all"}`;
  function onCsScroll() {
    if (scrollEl) app.cheatScroll[scrollKey()] = scrollEl.scrollTop;
  }
  $effect(() => {
    // Re-runs on subject/topic change AND when the sheet's content (re)loads, so the
    // offset is restored only after the content that gives the page its height is in
    // the DOM. Two rAFs: first lets Svelte paint the new content, second restores.
    const key = `${app.activeSubjectId ?? ""}:${selectedTopicId ?? "all"}`;
    void sections.length;
    void hasCheatsheet;
    if (!scrollEl) return;
    const target = app.cheatScroll[key] ?? 0;
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (scrollEl) scrollEl.scrollTop = target;
      })
    );
  });

  // Switching subject or topic leaves the current sheet — drop any in-progress edit
  // so a stale draft can't bleed across sheets.
  $effect(() => {
    void app.activeSubjectId;
    void selectedTopicId;
    mode = "preview";
    draft = [];
  });

  // ── EDIT MODE ────────────────────────────────────────────────
  // Default is the read-only preview above; "Edit" clones the stored sections
  // into a draft of editable structured fields (term + markdown body), keeping the
  // exact preview formatting. Saving persists the draft AND snapshots a version.
  let mode = $state<"preview" | "edit">("preview");
  let draft = $state<ApiCsSection[]>([]);
  let saving = $state(false);
  let imgTarget = -1; // which draft section index a file pick should illustrate
  let fileInput = $state<HTMLInputElement | null>(null);

  function enterEdit() {
    draft = structuredClone($state.snapshot(sections)) as ApiCsSection[];
    mode = "edit";
  }
  function cancelEdit() {
    mode = "preview";
    draft = [];
  }
  function addItem(si: number) {
    draft[si].items = [...draft[si].items, { t: "New term", d: "" }];
  }
  function removeItem(si: number, ii: number) {
    draft[si].items = draft[si].items.filter((_, i) => i !== ii);
  }
  function addSection() {
    draft = [
      ...draft,
      { id: "sec-" + Math.random().toString(36).slice(2), title: "New section", state: "approved", items: [], image: null },
    ];
  }
  function removeSection(si: number) {
    draft = draft.filter((_, i) => i !== si);
  }
  // Attach/replace a section image: pick a file → base64 data URL (renders in the
  // webview AND in the headless-Chromium PDF export).
  function pickImage(si: number) {
    imgTarget = si;
    fileInput?.click();
  }
  function onImageFile(e: Event) {
    const f = (e.target as HTMLInputElement).files?.[0];
    const si = imgTarget;
    imgTarget = -1;
    if (fileInput) fileInput.value = ""; // allow re-picking the same file
    if (!f || si < 0 || si >= draft.length) return;
    const reader = new FileReader();
    reader.onload = () => {
      draft[si].image = reader.result as string;
    };
    reader.readAsDataURL(f);
  }
  function clearImage(si: number) {
    draft[si].image = null;
  }

  async function saveEdit() {
    const sub = app.activeSubject;
    if (!sub || saving) return;
    // Drop fully-empty sections/items so the saved sheet stays clean.
    const clean = draft
      .map((s) => ({ ...s, title: s.title.trim(), items: s.items.filter((it) => it.t.trim() || it.d.trim()) }))
      .filter((s) => s.title || s.items.length || s.image);
    saving = true;
    try {
      await api.updateCheatsheet(sub.id, selectedTopicId ?? undefined, clean as ApiCsSection[]);
      const data = await api.getCheatsheet(sub.id, selectedTopicId ?? undefined);
      if (data) applyCheatsheet(data);
      mode = "preview";
      draft = [];
      app.pushToast({ kind: "success", title: "Cheatsheet saved", body: "A new version was recorded." });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Save failed", body: String(e) });
    } finally {
      saving = false;
    }
  }

  // ── VERSION HISTORY / DIFF (git-like) ────────────────────────
  let historyOpen = $state(false);
  let versions = $state<CheatsheetVersionMeta[]>([]);
  let compareId = $state<string | null>(null); // the older version to diff against current
  let diffRows = $state<{ type: "ctx" | "add" | "del"; text: string }[]>([]);
  let histLoading = $state(false);

  function fmtTime(ms: number): string {
    try {
      return new Date(ms).toLocaleString(undefined, {
        month: "short", day: "numeric", hour: "2-digit", minute: "2-digit",
      });
    } catch { return String(ms); }
  }

  // Serialize a section set to comparable text lines for the diff.
  function sheetLines(secs: ApiCsSection[]): string[] {
    const out: string[] = [];
    for (const s of secs) {
      out.push("# " + s.title);
      if (s.image) out.push("[image attached]");
      for (const it of s.items) {
        out.push("## " + it.t);
        for (const ln of (it.d ?? "").split("\n")) out.push(ln);
      }
      out.push("");
    }
    return out;
  }

  // Minimal LCS line diff → unified add/del/ctx rows (git-style).
  function lineDiff(a: string[], b: string[]) {
    const n = a.length, m = b.length;
    const dp: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
    for (let i = n - 1; i >= 0; i--)
      for (let j = m - 1; j >= 0; j--)
        dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    const rows: { type: "ctx" | "add" | "del"; text: string }[] = [];
    let i = 0, j = 0;
    while (i < n && j < m) {
      if (a[i] === b[j]) { rows.push({ type: "ctx", text: a[i] }); i++; j++; }
      else if (dp[i + 1][j] >= dp[i][j + 1]) { rows.push({ type: "del", text: a[i] }); i++; }
      else { rows.push({ type: "add", text: b[j] }); j++; }
    }
    while (i < n) rows.push({ type: "del", text: a[i++] });
    while (j < m) rows.push({ type: "add", text: b[j++] });
    return rows;
  }

  async function openHistory() {
    const sub = app.activeSubject;
    if (!sub) return;
    historyOpen = true;
    histLoading = true;
    diffRows = [];
    compareId = null;
    try {
      versions = await api.listCheatsheetVersions(sub.id, selectedTopicId ?? undefined);
      // Default: diff the previous version (index 1, since 0 is the current save).
      if (versions.length >= 2) selectCompare(versions[1].id);
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't load history", body: String(e) });
    } finally {
      histLoading = false;
    }
  }

  async function selectCompare(versionId: string) {
    compareId = versionId;
    try {
      const old = await api.getCheatsheetVersion(versionId);
      diffRows = lineDiff(sheetLines(old), sheetLines(sections));
    } catch (e) {
      app.pushToast({ kind: "error", title: "Couldn't load version", body: String(e) });
    }
  }
  const diffAdds = $derived(diffRows.filter((r) => r.type === "add").length);
  const diffDels = $derived(diffRows.filter((r) => r.type === "del").length);

  // The jump index (TOC) tracks the live source: the editable draft while editing,
  // the stored sheet otherwise — so it works on the edit page too.
  const indexSections = $derived(mode === "edit" ? draft : sections);
</script>

<div class="workspace-scroll" bind:this={scrollEl} onscroll={onCsScroll}>
  <div class="cs-doc{exportAll.length > 0 ? ' is-exporting-all' : ''}">
    {#if !app.activeSubject}
      <!-- No subject open -->
      <div class="cs-empty-state">
        <Icon name="diamond" size={28} color="var(--fg3)" />
        <div class="ces-title">No subject open</div>
        <div class="ces-sub">Open a subject from the sidebar to view its cheatsheet.</div>
      </div>
    {:else}
      {@const sub = app.activeSubject}

      <!-- ── TOPIC TAB BAR ──────────────────────────────────── -->
      <div class="cs-tabs" role="tablist" aria-label="Cheatsheet scope">
        <button
          class="cs-tab{selectedTopicId === null ? ' is-active' : ''}"
          role="tab"
          aria-selected={selectedTopicId === null}
          style={selectedTopicId === null ? `--tab-accent:${accent}` : ""}
          onclick={() => selectTopic(null)}
        >
          <Icon name="grid" size={12} /> Whole subject
        </button>
        {#each topicTabs as t (t.id)}
          <button
            class="cs-tab{selectedTopicId === t.id ? ' is-active' : ''}"
            role="tab"
            aria-selected={selectedTopicId === t.id}
            style={selectedTopicId === t.id ? `--tab-accent:${accent}` : ""}
            onclick={() => selectTopic(t.id)}
          >
            {#if t.glyph}<span class="cs-tab-glyph">{t.glyph}</span>{/if}
            {t.name}
            <span class="cs-tab-count mono">{t.sources.length}</span>
          </button>
        {/each}
      </div>

      <!-- Running / errored jobs for the current selection -->
      {#each csJobs as job (job.id)}
        <GeneratingCard {job} />
      {/each}

      {#if loading && !hasCheatsheet}
        <!-- brief loading placeholder while we fetch the stored sheet -->
        <div class="cs-working">
          <div class="cs-working-ico">
            <Icon name="diamond" size={26} color="var(--fg3)" />
          </div>
          <p class="cs-working-sub mono muted">Loading cheatsheet…</p>
        </div>
      {:else if !hasCheatsheet}
        <!-- Selection has no stored cheatsheet yet -->
        {@const scopeSources =
          selectedTopicId === null
            ? sub.sourceCount
            : (sub.topics.find((t) => t.id === selectedTopicId)?.sources.length ?? 0)}
        {@const noSources = scopeSources === 0}
        <div class="cs-working">
          <div class="cs-working-ico">
            <Icon name="diamond" size={26} color="var(--fg3)" />
          </div>
          <h1 class="cs-working-title read">No cheatsheet yet</h1>
          <p class="cs-working-sub mono muted">
            {#if noSources}
              Add sources to {selectedTopicId === null
                ? "this subject"
                : "this topic"} first — your cheatsheet is synthesized from them.
            {:else}
              A completeness-checked cheatsheet will be generated from
              {selectedTopicId === null ? "this subject's" : "this topic's"}
              {scopeSources} source{scopeSources !== 1 ? "s" : ""}.
            {/if}
          </p>
          {#if !noSources}
            <label class="cs-imgopt mono">
              <input type="checkbox" bind:checked={withImages} />
              Include diagrams/images <span class="faint">· needs SearXNG</span>
            </label>
          {/if}
          <button class="btn btn--primary btn--sm" onclick={generate} disabled={csGenerating || noSources}>
            <Icon name="refresh" size={13} /> {csGenerating ? "Synthesizing…" : "Generate cheatsheet"}
          </button>
        </div>
      {:else}
        <!-- Document header -->
        <div class="cs-doc-head">
          <div>
            <div class="eyebrow">
              Cheatsheet · {selectedTopicId === null ? "Whole subject" : "Topic"}
            </div>
            <h1 class="cs-title">{cheatTopic}</h1>
            <div class="cs-sub mono">
              {sub.name}{sub.code ? " · " + sub.code : ""} ·
              {#if sourcesUsed < sourceCount}
                <span class="cs-cov-warn" title="Some sources could not be synthesized — regenerate to retry">⚠ synthesized from {sourcesUsed}/{sourceCount} sources</span>
              {:else}
                synthesized from {sourceCount} source{sourceCount !== 1 ? "s" : ""}
              {/if} ·
              {selectedTopicId === null ? "composed from topics" : `${sectionCount} enforced sections`}
            </div>
          </div>
          <div class="cs-doc-actions">
            {#if mode === "edit"}
              <button class="btn btn--sm btn--ghost" onclick={cancelEdit} disabled={saving}>
                <Icon name="x" size={13} /> Cancel
              </button>
              <button class="btn btn--sm btn--primary" onclick={saveEdit} disabled={saving}>
                <Icon name="check" size={13} /> {saving ? "Saving…" : "Save"}
              </button>
            {:else}
              <button class="btn btn--sm" onclick={exportCurrent} title="Opens the print dialog — choose “Save as PDF”">
                <Icon name="doc" size={13} /> Save as PDF
              </button>
              <button
                class="btn btn--sm"
                onclick={exportWholeSubject}
                disabled={exporting}
                title="Print the whole-subject sheet plus every topic's sheet together"
              >
                <Icon name="book" size={13} /> {exporting ? "Loading…" : "Export all"}
              </button>
              <button class="btn btn--sm" onclick={openHistory} title="Browse versions and diff changes">
                <Icon name="refresh" size={13} /> History
              </button>
              <button class="btn btn--sm" onclick={enterEdit} title="Edit this cheatsheet">
                <Icon name="pencil" size={13} /> Edit
              </button>
              <button class="btn btn--sm" onclick={generate} disabled={csGenerating}>
                <Icon name="refresh" size={13} /> {csGenerating ? "Synthesizing…" : "Regenerate"}
              </button>
            {/if}
          </div>
        </div>

        {#if mode === "edit"}
          <!-- ── EDITOR: structured fields, markdown bodies ──────── -->
          <div class="cs-editor">
            <input
              bind:this={fileInput}
              type="file"
              accept="image/*"
              style="display:none"
              onchange={onImageFile}
            />
            {#each draft as sec, si (sec.id)}
              <section class="cs-edit-sec" id={"cs-sec-" + sec.id}>
                <div class="cs-edit-sechead">
                  <input class="cs-edit-title mono" bind:value={sec.title} placeholder="Section title" />
                  <div class="grow"></div>
                  {#if sec.image}
                    <button class="btn btn--sm btn--ghost" onclick={() => pickImage(si)} title="Replace image">
                      <Icon name="refresh" size={12} /> Image
                    </button>
                    <button class="btn btn--icon btn--sm btn--ghost" onclick={() => clearImage(si)} title="Remove image">
                      <Icon name="x" size={12} />
                    </button>
                  {:else}
                    <button class="btn btn--sm btn--ghost" onclick={() => pickImage(si)} title="Attach an image">
                      <Icon name="plus" size={12} /> Image
                    </button>
                  {/if}
                  <button class="btn btn--icon btn--sm btn--ghost" onclick={() => removeSection(si)} title="Remove section">
                    <Icon name="x" size={13} />
                  </button>
                </div>

                {#if sec.image}
                  <span class="cs-sec-img"><img src={sec.image} alt={sec.title} /></span>
                {/if}

                {#each sec.items as item, ii (ii)}
                  <div class="cs-edit-item" id={"cs-it-" + sec.id + "-" + ii}>
                    <div class="cs-edit-itemhead">
                      <input class="cs-edit-term" bind:value={item.t} placeholder="Term / concept" />
                      <button class="btn btn--icon btn--sm btn--ghost" onclick={() => removeItem(si, ii)} title="Remove item">
                        <Icon name="x" size={12} />
                      </button>
                    </div>
                    <div class="cs-edit-body">
                      <MarkdownEditor value={item.d} onChange={(v) => (item.d = v)} />
                    </div>
                  </div>
                {/each}

                <button class="btn btn--sm btn--ghost cs-edit-add" onclick={() => addItem(si)}>
                  <Icon name="plus" size={12} /> Add item
                </button>
              </section>
            {/each}
            <button class="btn btn--sm cs-edit-addsec" onclick={addSection}>
              <Icon name="plus" size={13} /> Add section
            </button>
          </div>
        {:else}
          <!-- Sections -->
          <div class="cs-sections">
            {#each sections as sec (sec.id)}
              {#if sec.id.startsWith("__topic__")}
                <!-- Topic divider in the composed whole-subject sheet -->
                <div class="cs-topic-divider" id={"cs-sec-" + sec.id}>
                  <span class="cs-topic-divider-label read">{sec.title}</span>
                </div>
              {:else}
              <section
                id={"cs-sec-" + sec.id}
                class="cs-section{sec.state === 'draft-pending' ? ' is-pending' : ''}"
              >
                <header class="cs-sec-head">
                  <h2 class="cs-sec-title">{sec.title}</h2>

                  {#if sec.state === "draft-pending"}
                    <span class="status-pill status-pill--draft">
                      <span class="dot"></span>pending
                    </span>
                  {:else}
                    <span class="cs-sec-count mono">{sec.items.length}</span>
                  {/if}
                </header>

                {#if sec.image}
                  <a class="cs-sec-img" href={sec.image} target="_blank" rel="noreferrer" title="Open image">
                    <img src={sec.image} alt={sec.title} loading="lazy" />
                  </a>
                {/if}

                <dl class="cs-list">
                  {#each sec.items as item, i (i)}
                    <div class="cs-item" id={"cs-it-" + sec.id + "-" + i}>
                      <dt>{item.t}</dt>
                      <dd><RichText text={item.d} /></dd>
                    </div>
                  {/each}
                </dl>
              </section>
              {/if}
            {/each}
          </div>
        {/if}

        <!-- Floating, hover-expand, collapsible jump index (table of contents).
             Lets you leap to any section or sub-topic. Sits just left of the chat
             dock when it's open. -->
        <nav
          class="cs-index"
          class:is-pinned={indexPinned}
          style:right={chatDockOpen ? "calc(var(--chat-w, 396px) + 14px)" : "16px"}
          aria-label="Cheatsheet index"
        >
          <button
            class="cs-index-tab"
            onclick={() => (indexPinned = !indexPinned)}
            title={indexPinned ? "Unpin index" : "Pin index"}
          >
            <Icon name="grid" size={13} />
          </button>
          <div class="cs-index-panel">
            {#each indexSections as sec (sec.id)}
              <div class="cs-idx-secrow">
                <button
                  class="cs-idx-caret"
                  onclick={() => toggleIdxSec(sec.id)}
                  title={idxExpanded[sec.id] ? "Collapse" : "Expand"}
                  aria-expanded={!!idxExpanded[sec.id]}
                >{idxExpanded[sec.id] ? "▾" : "▸"}</button>
                <button class="cs-idx-sec" onclick={() => jumpTo("cs-sec-" + sec.id)}>{sec.title}</button>
              </div>
              {#if idxExpanded[sec.id]}
                {#each sec.items as item, i (i)}
                  <button class="cs-idx-item" title={item.t} onclick={() => jumpTo("cs-it-" + sec.id + "-" + i)}>{item.t}</button>
                {/each}
              {/if}
            {/each}
          </div>
        </nav>
      {/if}
    {/if}
  </div>

  <!-- ── EXPORT-ALL print block ─────────────────────────────────
       Hidden on screen; only visible when printing. Holds the whole-subject
       sheet followed by each topic's stored sheet so "Export all" prints them
       together as one document. -->
  {#if exportAll.length > 0}
    <div class="cs-export-all" aria-hidden="true">
      {#each exportAll as sheet, si (si)}
        <div class="cs-doc cs-export-sheet">
          <div class="cs-doc-head">
            <div>
              <div class="eyebrow">Cheatsheet · {si === 0 ? "Whole subject" : "Topic"}</div>
              <h1 class="cs-title">{sheet.topic}</h1>
              <div class="cs-sub mono">
                {sheet.subject} · synthesized from {sheet.sources} source{sheet.sources !== 1 ? "s" : ""} ·
                {sheet.sections.length} enforced sections
              </div>
            </div>
          </div>
          <div class="cs-sections">
            {#each sheet.sections as sec (sec.id)}
              {#if sec.id.startsWith("__topic__")}
                <div class="cs-topic-divider"><span class="cs-topic-divider-label read">{sec.title}</span></div>
              {:else}
              <section class="cs-section">
                <header class="cs-sec-head">
                  <h2 class="cs-sec-title">{sec.title}</h2>
                  <span class="cs-sec-count mono">{sec.items.length}</span>
                </header>
                {#if sec.image}
                  <span class="cs-sec-img"><img src={sec.image} alt={sec.title} /></span>
                {/if}
                <dl class="cs-list">
                  {#each sec.items as item, i (i)}
                    <div class="cs-item">
                      <dt>{item.t}</dt>
                      <dd><RichText text={item.d} /></dd>
                    </div>
                  {/each}
                </dl>
              </section>
              {/if}
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- ── VERSION HISTORY / DIFF (git-like) ───────────────────────── -->
{#if historyOpen}
  <div
    class="overlay cs-hist-overlay"
    onmousedown={() => (historyOpen = false)}
    role="dialog"
    aria-modal="true"
    aria-label="Cheatsheet version history"
    tabindex="-1"
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="cs-hist" onmousedown={(e) => e.stopPropagation()}>
      <header class="cs-hist-head">
        <div>
          <div class="eyebrow">Version history</div>
          <div class="cs-hist-title mono">{cheatTopic} · diff vs current</div>
        </div>
        <div class="cs-hist-stats mono">
          <span class="st-add">+{diffAdds}</span>
          <span class="st-del">−{diffDels}</span>
        </div>
        <button class="btn btn--icon btn--sm btn--ghost" onclick={() => (historyOpen = false)}>
          <Icon name="x" size={13} />
        </button>
      </header>
      <div class="cs-hist-body">
        <aside class="cs-hist-list">
          {#if histLoading}
            <div class="mono faint" style="padding:12px">Loading…</div>
          {:else if versions.length === 0}
            <div class="mono faint" style="padding:12px">No versions yet.</div>
          {:else}
            {#each versions as v, i (v.id)}
              <button
                class="cs-hist-ver{compareId === v.id ? ' on' : ''}"
                onclick={() => selectCompare(v.id)}
                disabled={i === 0}
                title={i === 0 ? "Current version" : "Diff this version against current"}
              >
                <span class="cs-hist-when mono">{fmtTime(v.created_at)}</span>
                <span class="cs-hist-note">{i === 0 ? "current" : v.note} · {v.section_count} sec</span>
              </button>
            {/each}
          {/if}
        </aside>
        <div class="cs-hist-diff">
          {#if compareId === null}
            <div class="mono faint" style="padding:16px">
              {versions.length < 2 ? "Only one version so far — edits and regenerations will appear here." : "Pick a version on the left to see what changed."}
            </div>
          {:else}
            <div class="diff-inline">
              {#each diffRows as r, i (i)}
                <div class="diff-line {r.type}">
                  <span class="gutter">{r.type === "add" ? "+" : r.type === "del" ? "−" : ""}</span>
                  <span class="txt read">{r.text || " "}</span>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  /* ── editor (edit mode) ────────────────────────────────────── */
  .cs-editor { display: flex; flex-direction: column; gap: 20px; }
  .cs-edit-sec {
    border: 1px solid var(--border); border-radius: var(--rad-3);
    padding: 14px 14px 12px; background: var(--surface);
  }
  .cs-edit-sechead { display: flex; align-items: center; gap: 8px; margin-bottom: 12px; }
  .cs-edit-title {
    flex: 1; min-width: 0; font-size: var(--r-md); font-weight: 700;
    letter-spacing: 0.06em; text-transform: uppercase; color: var(--accent);
    background: var(--bg-sunken, var(--surface-2)); border: 1px solid var(--border);
    border-radius: var(--rad-2); padding: 6px 10px;
  }
  .cs-edit-item {
    display: flex; flex-direction: column; gap: 6px;
    padding: 10px 0; border-top: 1px solid color-mix(in oklab, var(--border) 60%, transparent);
  }
  .cs-edit-itemhead { display: flex; align-items: center; gap: 8px; }
  .cs-edit-term {
    flex: 1; min-width: 0; font-family: var(--font-read); font-weight: 600;
    font-size: calc(var(--read-size, var(--r-md)) + 1px); color: var(--fg-bright);
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--rad-2); padding: 6px 10px;
  }
  /* the embedded markdown editor gets a comfortable working height */
  .cs-edit-body { display: flex; min-height: 200px; }
  .cs-edit-body :global(.md) { min-height: 200px; }
  .cs-edit-add { align-self: flex-start; margin-top: 10px; }
  .cs-edit-addsec { align-self: flex-start; }

  /* ── version history / diff overlay ────────────────────────── */
  .cs-hist-overlay {
    position: fixed; inset: 0; z-index: 60; display: flex;
    align-items: center; justify-content: center; padding: 32px;
    background: color-mix(in oklab, var(--bg) 55%, transparent); backdrop-filter: blur(3px);
  }
  .cs-hist {
    width: min(960px, 94vw); height: min(80vh, 720px);
    display: flex; flex-direction: column;
    background: var(--surface); border: 1px solid var(--border-strong);
    border-radius: var(--rad-4, 14px); overflow: hidden;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.4);
  }
  .cs-hist-head {
    display: flex; align-items: center; gap: 14px;
    padding: 12px 16px; border-bottom: 1px solid var(--border);
  }
  .cs-hist-title { font-size: var(--t-md); color: var(--fg-bright); margin-top: 3px; }
  .cs-hist-stats { margin-left: auto; display: flex; gap: 10px; font-size: var(--t-sm); }
  .cs-hist-stats .st-add { color: var(--ok); }
  .cs-hist-stats .st-del { color: var(--err); }
  .cs-hist-body { flex: 1; min-height: 0; display: grid; grid-template-columns: 240px 1fr; }
  .cs-hist-list {
    border-right: 1px solid var(--border); overflow-y: auto;
    display: flex; flex-direction: column; padding: 6px;
  }
  .cs-hist-ver {
    display: flex; flex-direction: column; gap: 2px; text-align: left;
    padding: 8px 10px; border-radius: var(--rad-2); border: 1px solid transparent;
    background: none; color: var(--fg-muted); cursor: pointer;
  }
  .cs-hist-ver:hover:not(:disabled) { background: var(--surface-2); color: var(--fg-bright); }
  .cs-hist-ver.on { border-color: var(--accent); color: var(--fg-bright); background: color-mix(in oklab, var(--accent) 12%, transparent); }
  .cs-hist-ver:disabled { opacity: 0.6; cursor: default; }
  .cs-hist-when { font-size: var(--t-xs); }
  .cs-hist-note { font-size: var(--t-2xs); color: var(--fg-faint); }
  .cs-hist-diff { overflow: auto; padding: 6px 0; }

  /* ── floating jump index (TOC) ─────────────────────────────── */
  .cs-index {
    position: fixed;
    top: 96px;
    z-index: 28;
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .cs-index-tab {
    flex: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: color-mix(in oklab, var(--surface) 70%, transparent);
    color: var(--fg-muted);
    backdrop-filter: blur(8px);
    cursor: pointer;
    transition: color var(--dur), border-color var(--dur);
  }
  .cs-index-tab:hover { color: var(--fg-bright); border-color: var(--border-strong); }
  .cs-index.is-pinned .cs-index-tab { color: var(--accent); border-color: var(--accent); }
  .cs-index-panel {
    max-width: 0;
    max-height: 72vh;
    overflow: hidden auto;
    opacity: 0;
    padding: 0;
    border-radius: 10px;
    border: 1px solid transparent;
    background: color-mix(in oklab, var(--surface) 82%, transparent);
    backdrop-filter: blur(10px);
    transition: max-width var(--dur-slow) var(--ease), opacity var(--dur) var(--ease),
      padding var(--dur) var(--ease), border-color var(--dur) var(--ease);
  }
  .cs-index:hover .cs-index-panel,
  .cs-index.is-pinned .cs-index-panel {
    max-width: 250px;
    opacity: 1;
    padding: 7px;
    border-color: var(--border);
  }
  .cs-idx-secrow { display: flex; align-items: center; gap: 2px; }
  .cs-idx-caret {
    flex: none;
    width: 18px;
    padding: 6px 0 2px;
    font-size: 9px;
    color: var(--fg-faint);
    background: none;
    border: none;
    cursor: pointer;
  }
  .cs-idx-caret:hover { color: var(--fg-bright); }
  .cs-idx-sec {
    flex: 1;
    min-width: 0;
    display: block;
    text-align: left;
    font-family: var(--font-mono);
    font-size: 10.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
    padding: 7px 8px 2px 2px;
    cursor: pointer;
    background: none;
    border: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cs-idx-item {
    display: block;
    width: 100%;
    max-width: 236px;
    text-align: left;
    font-size: 12px;
    color: var(--fg-muted);
    padding: 3px 8px 3px 16px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    border-radius: 6px;
    cursor: pointer;
    background: none;
    border: none;
  }
  .cs-idx-item:hover,
  .cs-idx-sec:hover { color: var(--fg-bright); background: color-mix(in oklab, var(--accent) 13%, transparent); }
  @media print { .cs-index { display: none !important; } }

  /* faint divider between definition/concept items so they don't read as a wall */
  :global(.cs-item) + :global(.cs-item) {
    border-top: 1px solid color-mix(in oklab, var(--border) 60%, transparent);
    padding-top: 16px;
  }

  /* ── topic tab bar ─────────────────────────────────────────── */
  .cs-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: var(--sp-4, 16px);
    padding-bottom: var(--sp-3, 12px);
    border-bottom: 1px solid var(--border);
  }
  .cs-tab {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 5px 11px;
    border-radius: 999px;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    color: var(--fg-muted);
    font-size: var(--t-sm);
    cursor: pointer;
    white-space: nowrap;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
  }
  .cs-tab:hover {
    color: var(--fg-bright);
    border-color: var(--fg3);
  }
  .cs-tab.is-active {
    color: var(--fg-bright);
    border-color: color-mix(in oklab, var(--tab-accent) 60%, var(--border-strong));
    background: color-mix(in oklab, var(--tab-accent) 14%, var(--surface));
  }
  .cs-tab-glyph {
    font-size: var(--t-sm);
    line-height: 1;
  }
  .cs-tab-count {
    font-size: var(--t-xs);
    color: var(--fg-faint);
    padding: 0 5px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--fg3) 18%, transparent);
  }
  .cs-tab.is-active .cs-tab-count {
    color: var(--fg-bright);
    background: color-mix(in oklab, var(--tab-accent) 28%, transparent);
  }

  /* Centered empty / working states — mirrors GenerateMaterial's .genmat--working */
  .cs-empty-state,
  .cs-working {
    min-height: 50vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 12px;
  }
  .cs-working-ico {
    width: 54px;
    height: 54px;
    border-radius: 50%;
    border: 1px solid var(--border-strong);
    background: var(--surface);
    display: grid;
    place-items: center;
    margin-bottom: 4px;
  }
  .cs-working-title {
    font-size: var(--r-xl);
    color: var(--fg-bright);
    font-weight: 500;
  }
  .cs-working-sub,
  .cs-empty-state .ces-sub {
    max-width: 380px;
    font-size: var(--t-sm);
    line-height: 1.55;
  }
  .cs-empty-state .ces-title {
    font-size: var(--r-lg);
    color: var(--fg-bright);
    font-weight: 500;
  }
  .cs-working .btn {
    margin-top: 4px;
  }
  .cs-imgopt {
    display: inline-flex; align-items: center; gap: 7px;
    font-size: var(--t-xs); color: var(--fg-muted); cursor: pointer; margin-top: 4px;
  }
  .cs-imgopt input { accent-color: var(--accent); cursor: pointer; }
  .cs-sec-img {
    display: block; margin: 0 0 12px; max-width: 360px; border-radius: var(--rad-3);
    overflow: hidden; border: 1px solid var(--border);
  }
  .cs-sec-img img { width: 100%; height: auto; display: block; }

  /* export-all block is hidden on screen; print rules reveal it */
  .cs-export-all {
    display: none;
  }

  /* ── PRINT / EXPORT PDF ─────────────────────────────────────
     window.print() exports the cheatsheet. Hide all app chrome and
     show only the cheatsheet document, black-on-white, full width. */
  @media print {
    :global(.sidebar),
    :global(.statusbar),
    :global(.chatdock),
    :global(.chat-fab) {
      display: none !important;
    }
    :global(html),
    :global(body),
    :global(.app-shell),
    :global(.app-main),
    :global(.workspace) {
      display: block !important;
      height: auto !important;
      overflow: visible !important;
      background: #fff !important;
      color: #000 !important;
      margin: 0 !important;
    }
    :global(.app-shell) {
      grid-template-columns: 1fr !important;
    }
    .workspace-scroll {
      overflow: visible !important;
      height: auto !important;
      padding: 0 !important;
    }
    .cs-doc {
      max-width: none !important;
      width: 100% !important;
      margin: 0 !important;
      padding: 0 !important;
      color: #000 !important;
    }
    /* the tab bar and action buttons are screen-only chrome */
    .cs-tabs,
    .cs-doc-actions {
      display: none !important;
    }
    /* when an Export-all is in progress, reveal that block and hide the
       on-screen single-view doc so we don't print both. */
    .cs-export-all {
      display: block !important;
    }
    /* don't also print the single on-screen view during an Export-all */
    .cs-doc.is-exporting-all {
      display: none !important;
    }
    .cs-export-sheet {
      break-after: page;
    }
    .cs-export-sheet:last-child {
      break-after: auto;
    }
    /* keep each section from splitting awkwardly across pages */
    :global(.cs-section) {
      break-inside: avoid;
    }
    :global(.cs-item) {
      break-inside: avoid;
    }
    :global(.cs-title),
    :global(.cs-sec-title) {
      color: #000 !important;
    }
    /* readable tables/callouts on white paper */
    :global(.rt-table th) {
      background: #f0f0f0 !important;
      color: #000 !important;
    }
    :global(.rt-callout-label) {
      color: #000 !important;
    }
    :global(.rt-callout) {
      background: #f6f6f6 !important;
    }
    :global(.workspace-scroll),
    :global(.cs-doc),
    :global(.rt-callout-body),
    :global(.rt-table td),
    :global(.rt-p),
    :global(.cs-item dt),
    :global(.cs-item dd) {
      color: #000 !important;
    }
  }
</style>
