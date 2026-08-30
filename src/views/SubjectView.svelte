<script lang="ts">
  import { app, topicGlyph } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { Source } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Cheatsheet from "./Cheatsheet.svelte";
  import Materials from "./Materials.svelte";
  import Citations from "./Citations.svelte";
  import ChatPanel from "../components/ChatPanel.svelte";
  import GeneratingCard from "../components/GeneratingCard.svelte";
  import Picker from "../components/Picker.svelte";
  import { jobs } from "../lib/jobs.svelte";
  import { t } from "../lib/i18n.svelte";

  const TABS = [
    { id: "cheatsheet", label: t("Cheatsheet"), icon: "book" },
    { id: "sources", label: t("Sources"), icon: "doc" },
    { id: "chats", label: t("Chats"), icon: "chat" },
    { id: "materials", label: t("Materials"), icon: "grid" },
    { id: "citations", label: t("Planner"), icon: "cards" },
  ] as const;

  // Collapsed tab menu (narrow widths / touch): one trigger opens a popover of all
  // sections instead of the inline row, which clips on a phone. Same look as Picker.
  let tabMenuOpen = $state(false);
  const activeTab = $derived(TABS.find((t) => t.id === app.subjectTab) ?? TABS[0]);

  function kindLabel(kind: string): string {
    const map: Record<string, string> = {
      pdf: "PDF", pptx: "PPTX", docx: "DOCX", txt: "TXT",
      md: "MD", web: "WEB", yt: "YT", audio: "AUD", image: "IMG",
    };
    return map[kind] ?? kind.toUpperCase().slice(0, 3);
  }
  const kindBadge = (kind: string) => (kind === "audio" ? "audio" : kind);

  const subj = $derived(app.activeSubject);

  // The cheatsheet scope currently in view: the active topic's name, or
  // "Whole subject" when no topic is selected. Tracks app.cheatTopicId, which the
  // Cheatsheet view keeps in sync with its tab selection.
  const scopeLabel = $derived(
    app.cheatTopicId
      ? (subj?.topics.find((t) => t.id === app.cheatTopicId)?.name ?? t("Whole subject"))
      : t("Whole subject")
  );

  // Load ALL sources for the subject (including ones with no topic, which the
  // subject tree omits) and group them by topic for the Sources tab.
  let srcList = $state<Source[]>([]);
  function loadSources() {
    const id = subj?.id;
    if (!id) { srcList = []; return; }
    api.listSources(id).then((s) => (srcList = s)).catch(() => (srcList = []));
  }
  $effect(() => {
    // loadSources reads subj?.id, so this effect re-runs whenever it changes
    loadSources();
  });

  function editSubject() {
    if (!subj) return;
    app.openEdit({
      kind: "subject",
      id: subj.id,
      name: subj.name,
      code: subj.code ?? "",
      glyph: subj.glyph,
      color: app.subjectColor(subj),
    });
  }

  function editSrc(e: MouseEvent, src: Source) {
    e.stopPropagation();
    app.openEdit({
      kind: "source",
      id: src.id,
      name: src.name,
      subjectId: src.subject_id,
      topicId: src.topic_id,
      tags: src.tags ?? [],
      topicOptions: (subj?.topics ?? []).map((t) => ({ id: t.id, label: t.name })),
    });
  }

  async function deleteSrc(e: MouseEvent, src: Source) {
    e.stopPropagation();
    if (!(await app.confirm({ title: t("Delete this source?"), danger: true, okLabel: t("Delete") }))) return;
    await app.deleteSource(src.id); // toasts + refreshes the store internally
    loadSources(); // reload the local list this tab renders from
  }

  function editTopicGroup(topicId: string, name: string) {
    const t = subj?.topics.find((x) => x.id === topicId);
    app.openEdit({
      kind: "topic",
      id: topicId,
      name,
      subjectId: subj!.id,
      glyph: t?.glyph || topicGlyph(topicId),
      tags: t?.tags ?? [],
    });
  }

  async function deleteTopicGroup(topicId: string, name: string) {
    if (!(await app.confirm({ title: t("Delete topic \"{n}\"?", { n: name }), body: t("This empty topic will be removed."), danger: true, okLabel: t("Delete") }))) return;
    await app.deleteTopic(topicId); // toasts + refreshes store internally
    loadSources();
  }

  async function addTopic() {
    const name = await app.prompt({ title: t("Add topic"), label: t("Topic name"), placeholder: t("e.g. Determinism") });
    if (name) {
      await app.createTopic(name); // adds to active subject + toasts + refreshes
      loadSources();
    }
  }
  const groups = $derived.by(() => {
    const m = new Map<string, Source[]>();
    for (const s of srcList) {
      const key = s.topic_id ?? "__none__";
      (m.get(key) ?? m.set(key, []).get(key)!).push(s);
    }
    return [...m.entries()].map(([k, items]) => ({
      key: k,
      name: k === "__none__" ? t("Ungrouped") : (subj?.topics.find((t) => t.id === k)?.name ?? t("Ungrouped")),
      items,
    }));
  });

  // Source ingestion jobs for this subject: running cards + errors until dismissed.
  const sourceJobs = $derived(
    jobs.forSubject(app.activeSubjectId).filter(
      (j) => j.kind === "source" && (j.status === "running" || j.status === "error")
    )
  );

  // ── bulk source selection ───────────────────────────────────────
  let sel = $state<Record<string, boolean>>({});
  const selIds = $derived(Object.keys(sel).filter((k) => sel[k]));
  // Selection mode: once anything is selected, clicking a tile (not just its
  // checkbox) toggles selection instead of opening the source.
  const selecting = $derived(selIds.length > 0);
  function toggleSel(id: string, e: Event) {
    e.stopPropagation();
    sel = { ...sel, [id]: !sel[id] };
  }
  // Tile click while in selection mode → toggle; otherwise open.
  function tileClick(src: Source) {
    if (selecting) sel = { ...sel, [src.id]: !sel[src.id] };
    else app.openSource(src);
  }
  function clearSel() { sel = {}; }
  const allSelected = $derived(srcList.length > 0 && selIds.length === srcList.length);
  function selectAll() {
    sel = allSelected ? {} : Object.fromEntries(srcList.map((s) => [s.id, true]));
  }
  const moveTargets = $derived([
    ...(subj?.topics ?? []).map((t) => ({ id: t.id, label: "→ " + t.name })),
    { id: "__none__", label: t("→ no topic") },
  ]);
  async function bulkDelete() {
    const n = selIds.length;
    if (n === 0) return;
    if (!(await app.confirm({ title: t("Delete {n} source{c}?", { n, c: n === 1 ? "" : "s" }), danger: true, okLabel: t("Delete") }))) return;
    for (const id of selIds) { try { await api.deleteSource(id); } catch (e) { /* keep going */ } }
    app.pushToast({ kind: "success", title: t("Deleted {n} source{c}", { n, c: n === 1 ? "" : "s" }) });
    clearSel();
    await app.refresh();
    loadSources();
  }
  async function bulkMove(target: string) {
    if (!subj || selIds.length === 0 || !target) return;
    const tid = target === "__none__" ? null : target;
    const n = selIds.length;
    for (const id of selIds) { try { await api.moveSource(id, subj.id, tid); } catch (e) { /* keep going */ } }
    app.pushToast({ kind: "success", title: t("Moved {n} source{c}", { n, c: n === 1 ? "" : "s" }) });
    clearSel();
    await app.refresh();
    loadSources();
  }
  async function bulkReingest() {
    const sid = subj?.id;
    const ids = selIds;
    if (!sid || ids.length === 0) return;
    app.pushToast({ kind: "info", title: t("Re-ingesting {n} source{c}…", { n: ids.length, c: ids.length === 1 ? "" : "s" }), body: t("Re-OCR / re-chunk in progress.") });
    clearSel();
    for (const id of ids) {
      // Surface each as a running job card so progress is visible.
      jobs.start({
        kind: "source",
        label: srcList.find((s) => s.id === id)?.name ?? t("source"),
        subjectId: sid,
        topicId: null,
        run: () => api.reingestSource(id),
        onDone: () => { app.refresh(); loadSources(); },
      });
    }
  }
</script>

{#if subj}
  <div class="subject-view">
    <!-- Tab bar -->
    <div class="subj-tabs">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div
        class="st-id st-id--clickable"
        role="button"
        tabindex="0"
        title={t("Open subject details")}
        onclick={() => app.openSubjectPanel()}
        onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); app.openSubjectPanel(); } }}
      >
        <span class="subj-glyph sm" style="color:{app.subjectColor(subj)};font-size:13px;line-height:1">{subj.glyph || "◆"}</span>
        <div>
          <div class="st-name">{subj.name}</div>
          <div class="st-code mono">{subj.code ? subj.code + " · " : ""}{scopeLabel}</div>
        </div>
        <button
          class="btn btn--icon btn--sm btn--ghost"
          title={t("Edit subject")}
          onclick={(e) => { e.stopPropagation(); editSubject(); }}
        >
          <Icon name="pencil" size={12} />
        </button>
      </div>
      <div class="grow"></div>

      <!-- Wide desktop: the full inline tab row (unchanged). -->
      <div class="subj-tabs-row">
        {#each TABS as tab (tab.id)}
          <button
            class="subj-tab{app.subjectTab === tab.id ? ' on' : ''}"
            onclick={() => (app.subjectTab = tab.id)}
          >
            <Icon name={tab.icon} size={13} />{tab.label}
          </button>
        {/each}
      </div>

      <!-- Narrow / touch: collapse the row into one trigger + popover menu. -->
      <div class="subj-tabmenu picker">
        <button
          type="button"
          class={"picker-btn subj-tabmenu-btn" + (tabMenuOpen ? " open" : "")}
          onclick={() => (tabMenuOpen = !tabMenuOpen)}
          aria-haspopup="menu"
          aria-expanded={tabMenuOpen}
        >
          <Icon name={activeTab.icon} size={13} color="var(--accent)" />
          <span class="picker-val">{activeTab.label}</span>
          <Icon name="chevron" size={11} style="transform:rotate(90deg);color:var(--fg-faint)" />
        </button>
        {#if tabMenuOpen}
          <div class="picker-back" role="presentation" onclick={() => (tabMenuOpen = false)}></div>
          <div class="picker-menu subj-tabmenu-list" role="menu">
            {#each TABS as tab (tab.id)}
              <button
                type="button"
                role="menuitemradio"
                aria-checked={app.subjectTab === tab.id}
                class={"picker-item" + (app.subjectTab === tab.id ? " on" : "")}
                onclick={() => { app.subjectTab = tab.id; tabMenuOpen = false; }}
              >
                <Icon name={tab.icon} size={13} color={app.subjectTab === tab.id ? "var(--accent)" : "var(--fg-muted)"} />
                <span class="grow" style="text-align:left">{tab.label}</span>
                {#if app.subjectTab === tab.id}
                  <Icon name="check" size={12} color="var(--accent)" />
                {/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <!-- Tab body -->
    <div class="subj-body">
      {#if app.subjectTab === "cheatsheet"}
        <Cheatsheet />

      {:else if app.subjectTab === "sources"}
        <div class="workspace-scroll">
          <div class="sources-page">
            <div class="sources-toolbar">
              <span class="label">{srcList.length} {srcList.length === 1 ? t("source") : t("sources")} · {groups.length} {groups.length === 1 ? t("group") : t("groups")}</span>
              <div class="grow"></div>
              {#if srcList.length > 0}
                <button class="btn btn--sm btn--ghost" onclick={selectAll} title={t("Select all sources")}>
                  <Icon name="check" size={12} /> {allSelected ? t("Deselect all") : t("Select all")}
                </button>
              {/if}
              <button class="btn btn--sm btn--ghost" onclick={addTopic}>
                <Icon name="plus" size={12} /> {t("Add topic")}
              </button>
              <button class="btn btn--sm btn--primary" onclick={() => app.setView("add-source")}>
                <Icon name="plus" size={12} /> {t("Add source")}
              </button>
            </div>

            {#if selIds.length > 0}
              <div class="src-bulkbar">
                <span class="mono">{t("{n} selected", { n: selIds.length })}</span>
                <div class="grow"></div>
                <div style:width="170px">
                  <Picker
                    value=""
                    onChange={(id) => bulkMove(id)}
                    options={moveTargets}
                    placeholder={t("Move to…")}
                  />
                </div>
                <button class="btn btn--sm" onclick={bulkReingest} title={t("Re-OCR / re-chunk selected sources")}>
                  <Icon name="refresh" size={12} /> {t("Re-ingest")}
                </button>
                <button class="btn btn--sm sv-delete" onclick={bulkDelete}>
                  <Icon name="x" size={12} /> {t("Delete")}
                </button>
                <button class="btn btn--sm btn--ghost" onclick={clearSel}>{t("Clear")}</button>
              </div>
            {/if}

            {#each sourceJobs as job (job.id)}
              <GeneratingCard {job} />
            {/each}

            {#each groups as g (g.key)}
              <div class="src-topic">
                <div class="src-topic-h mono">
                  <Icon name="chevron" size={11} /> {g.name}
                  <span class="faint">· {g.items.length}</span>
                  {#if g.key !== "__none__"}
                    <div class="grow"></div>
                    <button
                      class="btn btn--icon btn--sm btn--ghost"
                      title={t("Edit topic")}
                      onclick={() => editTopicGroup(g.key, g.name)}
                    >
                      <Icon name="pencil" size={12} />
                    </button>
                    {#if g.items.length === 0}
                      <button
                        class="btn btn--icon btn--sm btn--ghost"
                        title={t("Delete empty topic")}
                        onclick={() => deleteTopicGroup(g.key, g.name)}
                      >
                        <Icon name="x" size={12} />
                      </button>
                    {/if}
                  {/if}
                </div>
                <div class="src-grid">
                  {#each g.items as src (src.id)}
                    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
                    <div
                      class="source-tile"
                      class:is-sel={!!sel[src.id]}
                      role="button"
                      tabindex="0"
                      onclick={() => tileClick(src)}
                      onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); tileClick(src); } }}
                      title={selecting ? t("Toggle selection") : t("Open source")}
                    >
                      <div class="stl-top">
                        <button
                          class="src-check{sel[src.id] ? ' on' : ''}"
                          onclick={(e) => toggleSel(src.id, e)}
                          title={t("Select source")}
                          aria-pressed={!!sel[src.id]}
                        >
                          {#if sel[src.id]}<Icon name="check" size={10} />{/if}
                        </button>
                        <span class="badge badge--{kindBadge(src.kind)}">
                          <span class="dot"></span>{kindLabel(src.kind)}
                        </span>
                        <span
                          class="status-pill status-pill--{src.status === 'ready' ? 'ready' : src.status === 'error' ? 'error' : 'pending'}"
                          title={src.status === 'ready' ? t("Ready") : src.status === 'error' ? t("Failed — re-ingest") : t("Not generated yet")}
                        >
                          <span class="dot"></span>
                        </span>
                        <div class="grow"></div>
                        <button
                          class="btn btn--icon btn--sm btn--ghost"
                          title={t("Edit source")}
                          onclick={(e) => editSrc(e, src)}
                        >
                          <Icon name="pencil" size={12} />
                        </button>
                        <button
                          class="btn btn--icon btn--sm btn--ghost"
                          title={t("Delete source")}
                          onclick={(e) => deleteSrc(e, src)}
                        >
                          <Icon name="x" size={12} />
                        </button>
                      </div>
                      <div class="stl-name mono">{src.name}</div>
                      {#if src.meta}
                        <div class="stl-meta mono">{src.meta} · {src.status === "ready" ? t("embedded") : src.status === "error" ? t("error") : (app.ingestProgress[src.id]?.detail ?? t("ingesting…"))}</div>
                      {/if}
                      {#if src.tags && src.tags.length > 0}
                        <div class="stl-tags">
                          {#each src.tags as tag (tag)}<span class="src-tag">{tag}</span>{/each}
                        </div>
                      {/if}
                    </div>
                  {/each}
                </div>
              </div>
            {/each}

            {#if srcList.length === 0}
              <div
                style:display="flex"
                style:flex-direction="column"
                style:align-items="center"
                style:justify-content="center"
                style:text-align="center"
                style:gap="12px"
                style:height="100%"
                style:min-height="50vh"
                style:color="var(--fg-faint)"
              >
                <Icon name="doc" size={26} color="var(--fg-faint)" />
                <h1 class="read" style:font-size="var(--r-xl)" style:color="var(--fg-bright)" style:font-weight="500">{t("No sources yet")}</h1>
                <p class="mono muted">{t("Add a lecture, PDF, link, recording or photo to start building this subject.")}</p>
                <button class="btn btn--primary" onclick={() => app.setView("add-source")}>
                  <Icon name="plus" size={13} /> {t("Add source")}
                </button>
              </div>
            {/if}
          </div>
        </div>

      {:else if app.subjectTab === "chats"}
        <div class="chats-tab" style:height="100%">
          <ChatPanel />
        </div>

      {:else if app.subjectTab === "materials"}
        <Materials />

      {:else if app.subjectTab === "citations"}
        <Citations />
      {/if}
    </div>
  </div>
{:else}
  <div class="workspace-scroll">
    <div style="display:flex;flex-direction:column;align-items:center;justify-content:center;gap:12px;min-height:62vh;padding:48px 32px;text-align:center;color:var(--fg-faint)">
      <Icon name="diamond" size={30} color="var(--fg-faint)" />
      {#if app.subjects.length === 0}
        <p class="read" style="font-size:var(--r-lg);color:var(--fg-bright);margin:4px 0 0">{t("No subjects yet")}</p>
        <p style="max-width:360px;margin:0">{t("Add your first subject to start building cheatsheets, flashcards and more.")}</p>
        <button class="btn btn--primary btn--sm" style="margin-top:6px" onclick={() => app.setView("add-subject")}>
          <Icon name="plus" size={13} /> {t("New subject")}
        </button>
      {:else}
        <p style="margin:0">{t("Select a subject from the sidebar to get started.")}</p>
      {/if}
    </div>
  </div>
{/if}
