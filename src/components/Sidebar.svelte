<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { topicGlyph } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import { stations } from "../lib/mock";
  import { moveItem, reorderable } from "../lib/dnd";
  import logo from "../assets/cortex-logo.png";

  // ── pointer-based reordering (subjects + a subject's topics). No native
  //    HTML5 drag — it crashes WebKitGTK; see lib/dnd.ts. ──
  function reorderSubject(from: number, to: number) {
    app.reorderSubjects(moveItem(app.subjects.map((s) => s.id), from, to));
  }
  function reorderTopic(subjectId: string, ids: string[], from: number, to: number) {
    app.reorderTopics(subjectId, moveItem(ids, from, to));
  }

  const stationName = $derived(
    stations.find((s) => s.id === app.music.current)?.name ?? "Study sound"
  );

  // Local state: which subject is expanded in the tree
  let expanded = $state<string | null>(null);

  // Local state: which topics are expanded. Keyed by topic id, independent per
  // topic, persists for the session. A topic id present in the set is open.
  let openTopics = $state<Set<string>>(new Set());

  function toggleExpand(id: string, e: MouseEvent) {
    e.stopPropagation();
    expanded = expanded === id ? null : id;
  }

  function openSubject(id: string) {
    expanded = id;
    app.openSubject(id);
  }

  // Click a subject: go to its whole-subject cheatsheet + expand the tree. Click
  // again while already there → collapse the tree (stay on the cheatsheet).
  function clickSubject(s: { id: string }) {
    const here =
      app.view === "subject" &&
      app.activeSubjectId === s.id &&
      app.subjectTab === "cheatsheet" &&
      app.cheatTopicId === null;
    if (here && expanded === s.id) {
      expanded = null;
      return;
    }
    expanded = s.id;
    app.openTopicSheet(s.id, null);
  }

  // Click a topic: go to THAT topic's cheatsheet + expand its sources. Click
  // again while already there → collapse its sources (stay on the cheatsheet).
  function clickTopic(s: { id: string }, t: { id: string }) {
    const here =
      app.view === "subject" &&
      app.activeSubjectId === s.id &&
      app.subjectTab === "cheatsheet" &&
      app.cheatTopicId === t.id;
    expanded = s.id;
    app.openTopicSheet(s.id, t.id);
    const next = new Set(openTopics);
    if (here && next.has(t.id)) next.delete(t.id);
    else next.add(t.id);
    openTopics = next;
  }

  function toggleTopic(id: string, e: Event) {
    e.stopPropagation();
    const next = new Set(openTopics);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    openTopics = next;
  }

  function editSubject(s: { id: string; name: string; code?: string | null; glyph: string }, e: Event) {
    e.stopPropagation();
    app.openEdit({
      kind: "subject",
      id: s.id,
      name: s.name,
      code: s.code ?? "",
      glyph: s.glyph,
      color: app.subjectColor(s),
    });
  }

  function editTopic(t: { id: string; name: string; glyph?: string | null; tags?: string[] }, subjectId: string, e: Event) {
    e.stopPropagation();
    app.openEdit({ kind: "topic", id: t.id, name: t.name, subjectId, glyph: t.glyph || topicGlyph(t.id), tags: t.tags ?? [] });
  }

  // Sources directly under a subject that belong to no topic (topic_id null).
  function ungroupedSources(s: { topics: { sources: import("../lib/api").Source[] }[] }) {
    return s.topics.flatMap((t) => t.sources).filter((src) => src.topic_id == null);
  }
</script>

<div class="sidebar">
  <!-- Brand row -->
  <div class="sb-brand">
    <img class="glyph" src={logo} alt="Cortex" />
    <span class="b-name">Cortex</span>
    <span class="b-spacer"></span>
    <button class="b-cmd" type="button" title="Minimize sidebar (b)" style="background:none;border:none;padding:0;cursor:pointer;color:inherit;display:inline-flex;align-items:center;transform:rotate(180deg)" onclick={() => app.toggleSidebar()}>
      <Icon name="chevron" size={15} />
    </button>
  </div>

  <div class="sb-scroll">
    <!-- Nav items -->
    <div
      class="sb-nav-item{app.view === 'dashboard' ? ' on' : ''}"
      onclick={() => app.setView("dashboard")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("dashboard")}
    >
      <Icon name="home" size={14} /> Dashboard <span class="nav-k">␣ g</span>
    </div>
    <div
      class="sb-nav-item{app.view === 'recorder' ? ' on' : ''}"
      onclick={() => app.setView("recorder")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("recorder")}
    >
      <Icon name="record" size={13} /> Record lecture <span class="nav-k">␣ r</span>
    </div>
    <div
      class="sb-nav-item{app.view === 'add-source' ? ' on' : ''}"
      onclick={() => app.setView("add-source")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("add-source")}
    >
      <Icon name="plus" size={14} /> Add source <span class="nav-k">␣ s</span>
    </div>
    <div
      class="sb-nav-item{app.view === 'notes' ? ' on' : ''}"
      onclick={() => app.setView("notes")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("notes")}
    >
      <Icon name="reader" size={14} /> Notes <span class="nav-k">␣ o</span>
    </div>
    <div
      class="sb-nav-item{app.view === 'calendar' ? ' on' : ''}"
      onclick={() => app.setView("calendar")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("calendar")}
    >
      <Icon name="calendar" size={14} /> Calendar <span class="nav-k">␣ a</span>
    </div>
    <div
      class="sb-nav-item{app.view === 'analytics' ? ' on' : ''}"
      onclick={() => app.setView("analytics")}
      role="button"
      tabindex="0"
      onkeydown={(e) => e.key === "Enter" && app.setView("analytics")}
    >
      <Icon name="chart" size={14} /> Insights <span class="nav-k">␣ i</span>
    </div>

    <!-- Subjects section header -->
    <div class="sb-section-l">
      <span class="label">Subjects</span>
      <span
        class="add"
        title="New subject"
        role="button"
        tabindex="0"
        onclick={() => app.setView("add-subject")}
        onkeydown={(e) => e.key === "Enter" && app.setView("add-subject")}
      >
        <Icon name="plus" size={13} />
      </span>
    </div>

    <!-- Subjects tree -->
    {#each app.subjects as s, si (s.id)}
      <div class="sb-subj">
        <div
          class="sb-subj-row{app.activeSubjectId === s.id && app.view !== 'dashboard' ? ' on' : ''}"
          onclick={() => clickSubject(s)}
          role="button"
          tabindex="0"
          onkeydown={(e) => e.key === "Enter" && clickSubject(s)}
          use:reorderable={{ index: si, group: "subjects", onReorder: reorderSubject }}
        >
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <span
            class="twisty{expanded === s.id ? ' open' : ''}"
            onclick={(e) => toggleExpand(s.id, e)}
            role="button"
            tabindex="-1"
            aria-label="Expand {s.name}"
          >
            <Icon name="chevron" size={11} />
          </span>
          <span class="s-glyph" style:color={app.subjectColor(s)} style:font-size="12px">
            {s.glyph || "◆"}
          </span>
          <span class="s-name">{s.name}</span>
          <span class="s-actions">
            <button
              class="s-act"
              type="button"
              title="Edit subject"
              aria-label="Edit {s.name}"
              onclick={(e) => editSubject(s, e)}
            >
              <Icon name="pencil" size={12} />
            </button>
            <button
              class="s-act"
              type="button"
              title="Add a topic to {s.name}"
              aria-label="Add topic to {s.name}"
              onclick={async (e) => {
                e.stopPropagation();
                app.activeSubjectId = s.id;
                const n = await app.prompt({ title: "Add topic to " + s.name, label: "Topic name", placeholder: "e.g. Determinism" });
                if (n) app.createTopic(n);
              }}
            >
              <Icon name="plus" size={12} />
            </button>
          </span>
          <span class="s-count">{s.sourceCount}</span>
          <span
            class="s-dot"
            style:background={s.status === "ready" ? "var(--ok)" : s.status === "error" ? "var(--err, #e5484d)" : "var(--warn)"}
          ></span>
        </div>

        {#if expanded === s.id}
          <div class="sb-children">
            {#each s.topics as t, ti (t.id)}
              {@const tOpen = openTopics.has(t.id)}
              <div
                class="sb-topic"
                onclick={() => clickTopic(s, t)}
                role="button"
                tabindex="0"
                onkeydown={(e) => (e.key === "Enter" || e.key === " ") && clickTopic(s, t)}
                use:reorderable={{ index: ti, group: "topics:" + s.id, onReorder: (from, to) => reorderTopic(s.id, s.topics.map((x) => x.id), from, to) }}
              >
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <span class="t-tw{tOpen ? ' open' : ''}" role="button" tabindex="-1" aria-label="Toggle sources" onclick={(e) => toggleTopic(t.id, e)}><Icon name="chevron" size={10} /></span>
                <span class="t-glyph" style="flex:none;font-size:11px;margin-right:5px">{t.glyph || topicGlyph(t.id)}</span>
                <span class="t-name">{t.name}</span>
                <span class="t-actions">
                  <button
                    class="s-act"
                    type="button"
                    title="Rename topic"
                    aria-label="Rename {t.name}"
                    onclick={(e) => editTopic(t, s.id, e)}
                  >
                    <Icon name="pencil" size={11} />
                  </button>
                  <button
                    class="s-act"
                    type="button"
                    title="Add a source to {t.name}"
                    aria-label="Add source to {t.name}"
                    onclick={(e) => { e.stopPropagation(); app.newSourceInTopic(t.id); }}
                  >
                    <Icon name="plus" size={11} />
                  </button>
                  {#if t.sources.length === 0}
                    <button
                      class="s-act"
                      type="button"
                      title="Delete empty topic"
                      aria-label="Delete topic {t.name}"
                      onclick={async (e) => {
                        e.stopPropagation();
                        const ok = await app.confirm({ title: `Delete topic "${t.name}"?`, body: "This empty topic will be removed.", danger: true, okLabel: "Delete" });
                        if (ok) app.deleteTopic(t.id, s.id);
                      }}
                    >
                      <Icon name="x" size={11} />
                    </button>
                  {/if}
                </span>
              </div>
              {#if tOpen}
                {#each t.sources as src (src.id)}
                  <div
                    class="sb-src sb-src--nested"
                    role="button"
                    tabindex="0"
                    onclick={() => app.openSource(src)}
                    onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && app.openSource(src)}
                  >
                    <span
                      class="badge badge--{src.kind === 'audio' ? 'audio' : src.kind}"
                      style:height="14px"
                      style:padding="0 4px"
                      style:font-size="9px"
                    >{src.kind.toUpperCase().slice(0, 3)}</span>
                    <span class="src-name">{src.name}</span>
                  </div>
                {/each}
              {/if}
            {/each}

            {#if ungroupedSources(s).length}
              {@const ugId = `__ungrouped__${s.id}`}
              {@const ugOpen = openTopics.has(ugId)}
              <div
                class="sb-topic sb-topic--ungrouped"
                onclick={(e) => toggleTopic(ugId, e)}
                role="button"
                tabindex="0"
                onkeydown={(e) => (e.key === "Enter" || e.key === " ") && toggleTopic(ugId, e)}
              >
                <span class="t-tw{ugOpen ? ' open' : ''}"><Icon name="chevron" size={10} /></span>
                <span class="t-name">Ungrouped</span>
              </div>
              {#if ugOpen}
                {#each ungroupedSources(s) as src (src.id)}
                  <div
                    class="sb-src sb-src--nested"
                    role="button"
                    tabindex="0"
                    onclick={() => app.openSource(src)}
                    onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && app.openSource(src)}
                  >
                    <span
                      class="badge badge--{src.kind === 'audio' ? 'audio' : src.kind}"
                      style:height="14px"
                      style:padding="0 4px"
                      style:font-size="9px"
                    >{src.kind.toUpperCase().slice(0, 3)}</span>
                    <span class="src-name">{src.name}</span>
                  </div>
                {/each}
              {/if}
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </div>

  <button
    class="sb-settings{app.view === 'settings' ? ' on' : ''}"
    onclick={() => app.setView("settings")}
    title="Settings"
  >
    <Icon name="settings" size={14} /> Settings
  </button>

  <!-- Music mini row — opens the study-sound panel -->
  <div
    class="sb-music"
    role="button"
    tabindex="0"
    title="Study sound (m)"
    onclick={() => (app.musicOpen = true)}
    onkeydown={(e) => e.key === "Enter" && (app.musicOpen = true)}
  >
    <div class="m-art">
      {#if app.music.playing}
        <span class="eq"><i></i><i></i><i></i><i></i></span>
      {:else}
        <Icon name="music" size={14} color="var(--accent-fg)" />
      {/if}
    </div>
    <div style:flex="1" style:min-width="0">
      <div class="m-name">{stationName}</div>
      <div class="m-sub">lo-fi · ad-free study</div>
    </div>
    <button
      class="m-play"
      title={app.music.playing ? "Pause" : "Play"}
      onclick={(e) => { e.stopPropagation(); app.toggleMusic(); }}
    >
      <Icon name={app.music.playing ? "pause" : "play"} size={14} />
    </button>
  </div>
</div>

<style>
  /* Per-subject inline rename / delete controls — faint, brighten on row hover */
  .sb-subj-row .s-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: auto;
    opacity: 0;
    transition: opacity var(--dur-fast);
  }
  .sb-subj-row:hover .s-actions,
  .sb-subj-row:focus-within .s-actions {
    opacity: 1;
  }
  .sb-subj-row .s-act {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    background: none;
    border-radius: var(--rad-2);
    color: var(--fg-faint);
    cursor: pointer;
    transition: color var(--dur-fast), background var(--dur-fast);
  }
  .sb-subj-row .s-act:hover {
    color: var(--fg-bright);
    background: var(--surface-3);
  }
  /* Count still hugs the dot; cancel the row's default margin-auto on name */
  .sb-subj-row .s-actions + .s-count {
    margin-left: 0;
  }

  /* ── Overflow safety: long names truncate instead of widening the 248px sidebar ── */
  .sb-subj-row {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  .sb-subj-row :global(.twisty),
  .sb-subj-row .s-glyph,
  .sb-subj-row .s-actions,
  .sb-subj-row .s-count,
  .sb-subj-row .s-dot {
    flex: none;
  }
  .sb-subj-row .s-glyph {
    display: inline-flex;
    align-items: center;
    margin-right: 4px;
  }
  .sb-subj-row .s-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Pointer-based reordering feedback (classes toggled by the reorderable
     action; see src/lib/dnd.ts + global .reorder-* rules in app.css). */
  .sb-subj-row.reorder-over, .sb-topic.reorder-over { outline: none; box-shadow: inset 0 2px 0 0 var(--accent); }

  /* Topic rows: chevron stays put, name flexes + truncates, like subjects */
  .sb-topic {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  .sb-topic .t-tw {
    flex: none;
    display: inline-flex;
    align-items: center;
    transition: transform var(--dur-fast);
  }
  .sb-topic .t-tw.open {
    transform: rotate(90deg);
  }
  .sb-topic .t-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* Per-topic inline rename / delete — faint, brighten on row hover */
  .sb-topic .t-actions {
    flex: none;
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: auto;
    opacity: 0;
    transition: opacity var(--dur-fast);
  }
  .sb-topic:hover .t-actions,
  .sb-topic:focus-within .t-actions {
    opacity: 1;
  }
  .sb-topic .t-actions .s-act {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    background: none;
    border-radius: var(--rad-2);
    color: var(--fg-faint);
    cursor: pointer;
    transition: color var(--dur-fast), background var(--dur-fast);
  }
  .sb-topic .t-actions .s-act:hover {
    color: var(--fg-bright);
    background: var(--surface-3);
  }

  /* Source rows: badge stays put, name flexes + truncates */
  .sb-src {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  /* Sources now nest one level under their topic */
  .sb-src--nested {
    padding-left: 14px;
  }
  .sb-src :global(.badge) {
    flex: none;
  }
  .sb-src .src-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
