<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { Reference, CalEvent } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import DatePicker from "../components/DatePicker.svelte";
  import BoardView from "./BoardView.svelte";

  // Assignments display: kanban board (default) or the detailed list.
  let aView = $state<"board" | "list">("board");

  const subjectId = $derived(app.activeSubject?.id ?? null);

  // ── references ────────────────────────────────────────────────
  let refs = $state<Reference[]>([]);
  let style = $state<"harvard" | "apa" | "mla">("harvard"); // Harvard is the default
  let editing = $state<string | null>(null); // ref id being edited, or "new"

  const CTYPES = [
    { id: "article", label: "Article" },
    { id: "book", label: "Book" },
    { id: "web", label: "Website" },
    { id: "other", label: "Other" },
  ] as const;

  // form state
  let f = $state({ ctype: "article", title: "", authors: "", year: "", container: "", url: "", doi: "", notes: "" });
  function resetForm() {
    f = { ctype: "article", title: "", authors: "", year: "", container: "", url: "", doi: "", notes: "" };
  }

  async function load() {
    if (!subjectId) { refs = []; return; }
    try { refs = await api.listCitations(subjectId); } catch (e) { app.pushToast({ kind: "error", title: "Load failed", body: String(e) }); }
  }
  $effect(() => { void subjectId; load(); });

  function startNew() { resetForm(); editing = "new"; }
  function startEdit(r: Reference) {
    f = {
      ctype: r.ctype, title: r.title, authors: r.authors ?? "", year: r.year ?? "",
      container: r.container ?? "", url: r.url ?? "", doi: r.doi ?? "", notes: r.notes ?? "",
    };
    editing = r.id;
  }
  function cancel() { editing = null; resetForm(); }

  async function saveForm() {
    if (!subjectId) return;
    if (!f.title.trim()) { app.pushToast({ kind: "warning", title: "Title required" }); return; }
    const fields = {
      ctype: f.ctype, title: f.title.trim(),
      authors: f.authors.trim() || null, year: f.year.trim() || null,
      container: f.container.trim() || null, url: f.url.trim() || null,
      doi: f.doi.trim() || null, notes: f.notes.trim() || null,
    };
    try {
      if (editing === "new") await api.addCitation(subjectId, fields);
      else if (editing) await api.updateCitation(editing, fields);
      cancel();
      await load();
    } catch (e) {
      app.pushToast({ kind: "error", title: "Save failed", body: String(e) });
    }
  }

  async function remove(r: Reference) {
    if (!(await app.confirm({ title: `Delete "${r.title}"?`, danger: true, okLabel: "Delete" }))) return;
    try { await api.deleteCitation(r.id); await load(); }
    catch (e) { app.pushToast({ kind: "error", title: "Delete failed", body: String(e) }); }
  }

  // ── formatting (lightweight APA / MLA) ────────────────────────
  function dotted(s: string) { return /[.!?]$/.test(s.trim()) ? s.trim() : s.trim() + "."; }
  function formatApa(r: Reference): string {
    const parts: string[] = [];
    if (r.authors) parts.push(dotted(r.authors));
    parts.push(`(${r.year?.trim() || "n.d."}).`);
    parts.push(dotted(r.title));
    if (r.container) parts.push(dotted(r.container));
    if (r.doi) parts.push(`https://doi.org/${r.doi.replace(/^https?:\/\/doi\.org\//, "").trim()}`);
    else if (r.url) parts.push(r.url.trim());
    return parts.join(" ").replace(/\s+/g, " ").trim();
  }
  function formatMla(r: Reference): string {
    const parts: string[] = [];
    if (r.authors) parts.push(dotted(r.authors));
    parts.push(`"${r.title.trim()}."`);
    if (r.container) parts.push(`${r.container.trim()},`);
    if (r.year) parts.push(`${r.year.trim()},`);
    if (r.url) parts.push(r.url.trim() + ".");
    else if (r.doi) parts.push(`https://doi.org/${r.doi.trim()}.`);
    return parts.join(" ").replace(/\s+/g, " ").replace(/,\s*$/, ".").trim();
  }
  // Harvard (author–date): Authors (Year) Title. Container. Available at: URL.
  function formatHarvard(r: Reference): string {
    const parts: string[] = [];
    if (r.authors) parts.push(r.authors.trim());
    parts.push(`(${r.year?.trim() || "no date"})`);
    parts.push(dotted(r.title));
    if (r.container) parts.push(dotted(r.container));
    if (r.doi) parts.push(`Available at: https://doi.org/${r.doi.replace(/^https?:\/\/doi\.org\//, "").trim()}.`);
    else if (r.url) parts.push(`Available at: ${r.url.trim()}.`);
    return parts.join(" ").replace(/\s+/g, " ").trim();
  }
  const fmt = $derived(style === "harvard" ? formatHarvard : style === "apa" ? formatApa : formatMla);

  async function copyOne(r: Reference) {
    try { await navigator.clipboard.writeText(fmt(r)); app.pushToast({ kind: "success", title: "Citation copied" }); }
    catch { app.pushToast({ kind: "error", title: "Copy failed" }); }
  }
  async function copyAll() {
    if (refs.length === 0) return;
    const list = [...refs].sort((a, b) => (a.authors ?? a.title).localeCompare(b.authors ?? b.title)).map(fmt).join("\n");
    try { await navigator.clipboard.writeText(list); app.pushToast({ kind: "success", title: `Copied ${refs.length} references` }); }
    catch { app.pushToast({ kind: "error", title: "Copy failed" }); }
  }

  // ── assignments (calendar events: assignment | project | exam | deadline) ──
  const ASSIGNMENT_KINDS = ["exam", "assignment", "project", "deadline"];
  let assignments = $state<CalEvent[]>([]);

  // Priority is a real event field now (migration 0015); the colour is only a
  // calendar display hint derived from it — never parsed back.
  const PRIORITIES = [
    { id: "none", label: "None", color: null as string | null },
    { id: "low", label: "Low", color: "#3b9eff" },
    { id: "med", label: "Med", color: "#f5a623" },
    { id: "high", label: "High", color: "#e5484d" },
  ] as const;
  function colorForPriority(id: string): string | null {
    return PRIORITIES.find((p) => p.id === id)?.color ?? null;
  }
  function priorityOf(e: CalEvent): (typeof PRIORITIES)[number] {
    return PRIORITIES.find((p) => p.id === (e.priority ?? "none")) ?? PRIORITIES[0];
  }

  // create form
  let aTitle = $state("");
  let aDate = $state("");
  let aKind = $state("assignment"); // assignment | project | exam
  let aPriority = $state("none"); // none | low | med | high
  let aNotes = $state("");
  let aTopics = $state<string[]>([]); // selected covered-topic ids
  let expanded = $state<Record<string, boolean>>({}); // event id → checklist open
  function resetAssignmentForm() {
    aTitle = ""; aDate = ""; aKind = "assignment"; aPriority = "none"; aNotes = ""; aTopics = [];
  }
  function toggleFormTopic(id: string) {
    aTopics = aTopics.includes(id) ? aTopics.filter((t) => t !== id) : [...aTopics, id];
  }
  function toggleExpanded(id: string) {
    expanded = { ...expanded, [id]: !expanded[id] };
  }

  async function loadAssignments() {
    if (!subjectId) { assignments = []; return; }
    try {
      const all = await api.listEvents(subjectId);
      const now = Date.now();
      assignments = all
        .filter((e) => ASSIGNMENT_KINDS.includes(e.kind) && !e.done && e.start_ms >= now - 86_400_000)
        .sort((a, b) => a.start_ms - b.start_ms);
    } catch { /* non-fatal */ }
  }
  // Reload when the subject changes OR any event changes anywhere (two-way sync
  // with the Calendar tab).
  $effect(() => { void subjectId; void app.eventsChangedNonce; loadAssignments(); });

  // Covered topics = the subject topics whose ids are in the event's topic_ids
  // (real column; older rows were migrated off the shared tags field).
  function coveredTopics(e: CalEvent) {
    const ids = new Set(e.topic_ids ?? []);
    if (ids.size === 0) return [];
    return (app.activeSubject?.topics ?? []).filter((t) => ids.has(t.id));
  }
  function doneCount(e: CalEvent, topics: { id: string }[]): number {
    return topics.filter((t) => (e.checklist ?? []).includes(t.id)).length;
  }

  async function toggleTopicDone(e: CalEvent, topicId: string) {
    const done = new Set(e.checklist ?? []);
    if (done.has(topicId)) done.delete(topicId);
    else done.add(topicId);
    try {
      const updated = await api.setEventChecklist(e.id, [...done]);
      assignments = assignments.map((d) => (d.id === updated.id ? updated : d));
      app.notifyEventsChanged();
    } catch (err) {
      app.pushToast({ kind: "error", title: "Update failed", body: String(err) });
    }
  }

  async function addAssignment() {
    if (!subjectId || !aTitle.trim() || !aDate) return;
    const startMs = new Date(aDate + "T23:59:00").getTime();
    try {
      await api.createEvent({
        title: aTitle.trim(), startMs, subjectId, kind: aKind, allDay: true,
        reminderMs: startMs - 86_400_000, // remind 1 day before
        topicIds: [...aTopics], // covered topic ids (real field)
        priority: aPriority,
        description: aNotes.trim() || null,
        color: colorForPriority(aPriority), // display hint for the calendar only
      });
      resetAssignmentForm();
      app.notifyEventsChanged();
      await loadAssignments();
      app.pushToast({ kind: "success", title: "Added" });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Add failed", body: String(e) });
    }
  }
  async function completeAssignment(e: CalEvent) {
    try { await api.setEventDone(e.id, true); app.notifyEventsChanged(); await loadAssignments(); }
    catch (err) { app.pushToast({ kind: "error", title: "Update failed", body: String(err) }); }
  }
  function openInCalendar(e: CalEvent) {
    // Jump to the Calendar and focus the assignment's day.
    app.openCalendarAt(e.start_ms);
  }

  const overdueCount = $derived(assignments.filter((e) => e.start_ms < Date.now()).length);

  function daysLeft(ms: number): string {
    const d = Math.ceil((ms - Date.now()) / 86_400_000);
    if (d < 0) return "overdue";
    if (d === 0) return "today";
    if (d === 1) return "tomorrow";
    return `${d} days`;
  }
  function fmtDate(ms: number): string {
    return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  }

  // progress ring geometry (r = 9 → circumference ≈ 56.55)
  const RING_R = 9;
  const RING_C = 2 * Math.PI * RING_R;
</script>

<div class="workspace-scroll">
  <div class="cit-page">
    <!-- Assignments -->
    <section class="cit-section">
      <div class="cit-head">
        <h2 class="cit-h read"><Icon name="calendar" size={15} /> Assignments</h2>
        {#if assignments.length}
          <span class="faint mono cit-summary">
            {assignments.length} {assignments.length === 1 ? "assignment" : "assignments"}{#if overdueCount} · {overdueCount} overdue{/if}
          </span>
        {/if}
        <div class="cit-view-seg mono" role="group" aria-label="Assignments view">
          <button class={aView === "board" ? "on" : ""} onclick={() => (aView = "board")}>Board</button>
          <button class={aView === "list" ? "on" : ""} onclick={() => (aView = "list")}>List</button>
        </div>
      </div>

      <div class="cit-assign-form">
        <div class="cit-assign-row1">
          <div class="cit-kind-seg mono">
            {#each ["assignment", "project", "exam"] as k (k)}
              <button class={aKind === k ? "on" : ""} onclick={() => (aKind = k)}>{k}</button>
            {/each}
          </div>
          <input class="input" placeholder="e.g. Essay 2, Capstone, Midterm…" bind:value={aTitle} />
          <div class="cit-date"><DatePicker value={aDate} onChange={(v) => (aDate = v)} placeholder="Due date" /></div>
        </div>
        <div class="cit-assign-row2">
          <div class="cit-prio-seg mono" role="group" aria-label="Priority">
            {#each PRIORITIES as p (p.id)}
              <button
                class={aPriority === p.id ? "on" : ""}
                title="Priority: {p.label}"
                onclick={() => (aPriority = p.id)}
              >
                <span class="cit-prio-dot" style={p.color ? `background:${p.color}` : ""}></span>{p.label}
              </button>
            {/each}
          </div>
          <input class="input" placeholder="Notes (optional)" bind:value={aNotes} />
          <button class="btn btn--sm btn--primary" disabled={!aTitle.trim() || !aDate} onclick={addAssignment}>
            <Icon name="plus" size={12} /> Add
          </button>
        </div>
        {#if (app.activeSubject?.topics ?? []).length}
          <div class="cit-topic-pick">
            <span class="onb-label mono">TOPICS COVERED</span>
            <div class="cit-topic-chips">
              {#each app.activeSubject?.topics ?? [] as t (t.id)}
                {@const on = aTopics.includes(t.id)}
                <button class="cit-chip{on ? ' on' : ''}" onclick={() => toggleFormTopic(t.id)}>
                  <span class="cit-chip-box">{#if on}<Icon name="check" size={9} />{/if}</span>
                  {t.glyph ?? ""} {t.name}
                </button>
              {/each}
            </div>
          </div>
        {/if}
      </div>

      {#if aView === "board"}
        <div class="cit-board-wrap"><BoardView /></div>
      {:else if assignments.length === 0}
        <p class="mono faint cit-empty">No upcoming assignments.</p>
      {:else}
        <ul class="cit-deadlines">
          {#each assignments as e (e.id)}
            {@const overdue = e.start_ms < Date.now()}
            {@const topics = coveredTopics(e)}
            {@const doneN = doneCount(e, topics)}
            {@const pct = topics.length ? doneN / topics.length : 0}
            {@const prio = priorityOf(e)}
            {@const open = expanded[e.id] ?? false}
            <li class="cit-deadline{overdue ? ' overdue' : ''}">
              {#if topics.length}
                <div class="cit-dl-bar" style="width:{pct * 100}%"></div>
              {/if}
              <div class="cit-dl-main">
                <button class="cit-dl-check" title="Mark done" aria-label="Mark done" onclick={(ev) => { ev.stopPropagation(); completeAssignment(e); }}>
                  <Icon name="check" size={11} />
                </button>
                {#if prio.color}
                  <span class="cit-prio-dot" style="background:{prio.color}" title="Priority: {prio.label}"></span>
                {/if}
                <span class="cit-dl-kind mono cit-dl-kind--{e.kind}">{e.kind === "deadline" ? "due" : e.kind}</span>
                <button class="cit-dl-title" title="Open in calendar" onclick={() => openInCalendar(e)}>
                  {e.title}<Icon name="external" size={11} />
                </button>
                {#if topics.length}
                  <span class="cit-dl-prog mono" class:complete={doneN === topics.length}>{doneN}/{topics.length}</span>
                  <svg class="cit-ring" width="24" height="24" viewBox="0 0 24 24" aria-label="{Math.round(pct * 100)} percent complete">
                    <circle class="cit-ring-track" cx="12" cy="12" r={RING_R} />
                    <circle
                      class="cit-ring-fill"
                      class:complete={doneN === topics.length}
                      cx="12" cy="12" r={RING_R}
                      stroke-dasharray={RING_C}
                      stroke-dashoffset={RING_C * (1 - pct)}
                    />
                  </svg>
                {/if}
                <span class="cit-dl-date mono">{fmtDate(e.start_ms)}</span>
                <span class="cit-dl-left mono{overdue ? ' overdue' : ''}">{daysLeft(e.start_ms)}</span>
                {#if topics.length}
                  <button class="cit-dl-expand" class:open title="Topic checklist" aria-label="Toggle checklist" onclick={(ev) => { ev.stopPropagation(); toggleExpanded(e.id); }}>
                    <Icon name="chevron" size={13} />
                  </button>
                {/if}
              </div>
              {#if e.description}
                <div class="cit-dl-notes mono faint">{e.description}</div>
              {/if}
              {#if topics.length && open}
                <ul class="cit-checklist">
                  {#each topics as t (t.id)}
                    {@const checked = (e.checklist ?? []).includes(t.id)}
                    <li>
                      <button class="cit-ck{checked ? ' on' : ''}" onclick={(ev) => { ev.stopPropagation(); toggleTopicDone(e, t.id); }}>
                        <span class="cit-ck-box">{#if checked}<Icon name="check" size={10} />{/if}</span>
                        <span class="cit-ck-label{checked ? ' done' : ''}">{t.glyph ?? ""} {t.name}</span>
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    <!-- References -->
    <section class="cit-section">
      <div class="cit-head">
        <h2 class="cit-h read"><Icon name="book" size={15} /> References <span class="faint mono">{refs.length}</span></h2>
        <div class="grow"></div>
        <div class="cit-style-toggle mono">
          <button class={style === "harvard" ? "on" : ""} onclick={() => (style = "harvard")}>Harvard</button>
          <button class={style === "apa" ? "on" : ""} onclick={() => (style = "apa")}>APA</button>
          <button class={style === "mla" ? "on" : ""} onclick={() => (style = "mla")}>MLA</button>
        </div>
        <button class="btn btn--sm" disabled={refs.length === 0} onclick={copyAll} title="Copy the full bibliography">
          <Icon name="doc" size={12} /> Copy all
        </button>
        <button class="btn btn--sm btn--primary" onclick={startNew}><Icon name="plus" size={12} /> Add</button>
      </div>

      {#if editing === "new"}
        {@render form()}
      {/if}

      {#if refs.length === 0 && editing !== "new"}
        <p class="mono faint cit-empty">No references yet. Add your sources to build a bibliography.</p>
      {:else}
        <ul class="cit-list">
          {#each refs as r (r.id)}
            <li class="cit-item">
              {#if editing === r.id}
                {@render form()}
              {:else}
                <div class="cit-row">
                  <span class="cit-badge mono">{r.ctype}</span>
                  <div class="cit-formatted read">{fmt(r)}</div>
                  <div class="cit-acts">
                    {#if r.url}
                      <a class="btn btn--icon btn--sm btn--ghost" href={r.url} target="_blank" rel="noreferrer" title="Open link"><Icon name="external" size={12} /></a>
                    {/if}
                    <button class="btn btn--icon btn--sm btn--ghost" title="Copy" aria-label="Copy citation" onclick={() => copyOne(r)}><Icon name="doc" size={12} /></button>
                    <button class="btn btn--icon btn--sm btn--ghost" title="Edit" aria-label="Edit" onclick={() => startEdit(r)}><Icon name="pencil" size={12} /></button>
                    <button class="btn btn--icon btn--sm btn--ghost" title="Delete" aria-label="Delete" onclick={() => remove(r)}><Icon name="x" size={12} /></button>
                  </div>
                </div>
                {#if r.notes}<div class="cit-notes mono faint">{r.notes}</div>{/if}
              {/if}
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  </div>
</div>

{#snippet form()}
  <div class="cit-form">
    <div class="cit-form-grid">
      <div class="cit-field cit-field--type">
        <span class="onb-label mono">TYPE</span>
        <Picker
          value={f.ctype}
          onChange={(id) => (f.ctype = id)}
          options={CTYPES.map((t) => ({ id: t.id, label: t.label }))}
          placeholder="Type"
        />
      </div>
      <label class="cit-field cit-field--wide">
        <span class="onb-label mono">TITLE</span>
        <input class="input" bind:value={f.title} placeholder="Title of the work" />
      </label>
      <label class="cit-field">
        <span class="onb-label mono">AUTHORS</span>
        <input class="input" bind:value={f.authors} placeholder="Last, F.; Last, F." />
      </label>
      <label class="cit-field">
        <span class="onb-label mono">YEAR</span>
        <input class="input" bind:value={f.year} placeholder="2024" />
      </label>
      <label class="cit-field">
        <span class="onb-label mono">CONTAINER</span>
        <input class="input" bind:value={f.container} placeholder="Journal / publisher / site" />
      </label>
      <label class="cit-field">
        <span class="onb-label mono">DOI</span>
        <input class="input" bind:value={f.doi} placeholder="10.xxxx/…" />
      </label>
      <label class="cit-field cit-field--wide">
        <span class="onb-label mono">URL</span>
        <input class="input" bind:value={f.url} placeholder="https://…" />
      </label>
      <label class="cit-field cit-field--wide">
        <span class="onb-label mono">NOTES</span>
        <input class="input" bind:value={f.notes} placeholder="Optional note" />
      </label>
    </div>
    <div class="cit-form-foot">
      <button class="btn btn--ghost btn--sm" onclick={cancel}>Cancel</button>
      <button class="btn btn--primary btn--sm" onclick={saveForm}><Icon name="check" size={12} /> Save</button>
    </div>
  </div>
{/snippet}

<style>
  .cit-page { max-width: 880px; margin: 0 auto; width: 100%; padding: var(--sp-6) var(--sp-7); display: flex; flex-direction: column; gap: var(--sp-7); }
  .cit-section { display: flex; flex-direction: column; gap: var(--sp-3); }
  .cit-head { display: flex; align-items: center; gap: 10px; }
  .cit-h { display: flex; align-items: center; gap: 8px; font-size: var(--r-md); margin: 0; }
  .grow { flex: 1; }
  .cit-empty { padding: var(--sp-4) 0; }

  /* assignments */
  .cit-summary { font-size: var(--t-xs); margin-left: 2px; }
  .cit-view-seg { margin-left: auto; display: inline-flex; border: 1px solid var(--border); border-radius: var(--rad-2); overflow: hidden; flex: none; }
  .cit-view-seg button { padding: 5px 11px; font-size: var(--t-xs); background: transparent; border: none; color: var(--fg-muted); cursor: pointer; border-right: 1px solid var(--border); }
  .cit-view-seg button:last-child { border-right: none; }
  .cit-view-seg button.on { background: var(--accent); color: var(--bg); }
  /* The embedded board needs an explicit height (BoardView fills its parent). */
  .cit-board-wrap { height: 60vh; min-height: 420px; margin-top: 4px; }
  .cit-assign-form { display: flex; flex-direction: column; gap: 8px; border: 1px solid var(--border); border-radius: var(--rad-3); background: var(--surface); padding: 11px 12px; }
  .cit-assign-row1, .cit-assign-row2 { display: flex; gap: 8px; align-items: center; }
  .cit-assign-row1 .input { flex: 1; }
  .cit-assign-row2 .input { flex: 1; }
  .cit-kind-seg { display: inline-flex; border: 1px solid var(--border); border-radius: var(--rad-2); overflow: hidden; flex: none; }
  .cit-kind-seg button { padding: 5px 9px; font-size: var(--t-xs); text-transform: capitalize; background: transparent; border: none; color: var(--fg-muted); cursor: pointer; border-right: 1px solid var(--border); }
  .cit-kind-seg button:last-child { border-right: none; }
  .cit-kind-seg button.on { background: var(--accent); color: var(--bg); }

  /* priority selector */
  .cit-prio-seg { display: inline-flex; border: 1px solid var(--border); border-radius: var(--rad-2); overflow: hidden; flex: none; }
  .cit-prio-seg button { display: inline-flex; align-items: center; gap: 5px; padding: 5px 9px; font-size: var(--t-xs); background: transparent; border: none; color: var(--fg-muted); cursor: pointer; border-right: 1px solid var(--border); }
  .cit-prio-seg button:last-child { border-right: none; }
  .cit-prio-seg button.on { background: var(--surface-3); color: var(--fg-bright); }
  .cit-prio-dot { flex: none; width: 8px; height: 8px; border-radius: 50%; background: var(--border-strong); }

  /* topic multi-select chips */
  .cit-topic-pick { display: flex; flex-direction: column; gap: 6px; }
  .cit-topic-chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .cit-chip { display: inline-flex; align-items: center; gap: 6px; padding: 4px 9px 4px 6px; font-size: var(--t-sm); border: 1px solid var(--border); border-radius: var(--rad-pill); background: var(--surface-2); color: var(--fg-muted); cursor: pointer; transition: background var(--dur-fast) var(--ease), border-color var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease); }
  .cit-chip:hover { border-color: var(--border-strong); color: var(--fg); }
  .cit-chip.on { background: color-mix(in oklab, var(--accent) 14%, var(--surface)); border-color: var(--accent-dim); color: var(--fg-bright); }
  .cit-chip-box { flex: none; width: 15px; height: 15px; border-radius: 4px; border: 1.5px solid var(--border-strong); display: inline-flex; align-items: center; justify-content: center; color: var(--bg); }
  .cit-chip.on .cit-chip-box { background: var(--accent); border-color: var(--accent); }
  .cit-dl-kind { flex: none; font-size: 9.5px; text-transform: uppercase; letter-spacing: 0.05em; padding: 2px 6px; border-radius: 999px; background: color-mix(in oklab, var(--accent) 16%, var(--surface)); color: var(--accent); }
  .cit-dl-kind--exam { background: color-mix(in oklab, var(--warn) 18%, var(--surface)); color: var(--warn); }
  .cit-dl-kind--project { background: color-mix(in oklab, var(--info) 18%, var(--surface)); color: var(--info); }
  .cit-date { flex: none !important; width: 170px; }
  .cit-date :global(.dp) { width: 100%; }
  .cit-deadlines { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .cit-deadline { position: relative; padding: 9px 12px; border: 1px solid var(--border); border-radius: var(--rad-3); background: var(--surface); overflow: hidden; }
  .cit-deadline.overdue { border-color: color-mix(in oklab, var(--warn) 55%, var(--border)); }
  /* subtle whole-row progress fill behind content */
  .cit-dl-bar { position: absolute; inset: 0 auto 0 0; background: color-mix(in oklab, var(--ok) 9%, transparent); transition: width var(--dur, 0.3s) var(--ease, ease); pointer-events: none; z-index: 0; }
  .cit-dl-main { position: relative; z-index: 1; display: flex; align-items: center; gap: 10px; }
  .cit-dl-prog { font-size: var(--t-xs); color: var(--fg-muted); padding: 1px 7px; border-radius: 999px; background: var(--surface-2); transition: color 0.2s, background 0.2s; }
  .cit-dl-prog.complete { color: var(--ok); background: color-mix(in oklab, var(--ok) 16%, var(--surface)); }

  /* circular progress ring */
  .cit-ring { flex: none; transform: rotate(-90deg); }
  .cit-ring-track { fill: none; stroke: var(--surface-3); stroke-width: 3; }
  .cit-ring-fill { fill: none; stroke: var(--accent); stroke-width: 3; stroke-linecap: round; transition: stroke-dashoffset var(--dur, 0.3s) var(--ease, ease), stroke 0.2s; }
  .cit-ring-fill.complete { stroke: var(--ok); }

  .cit-dl-expand { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 22px; height: 22px; border-radius: var(--rad-2); border: none; background: transparent; color: var(--fg-faint); cursor: pointer; transition: transform var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease); }
  .cit-dl-expand:hover { color: var(--fg); }
  .cit-dl-expand.open { transform: rotate(180deg); }
  .cit-dl-notes { position: relative; z-index: 1; margin: 6px 0 0 30px; font-size: var(--t-xs); }

  /* per-topic study checklist (cool, design-system animated) */
  .cit-checklist { position: relative; z-index: 1; list-style: none; margin: 9px 0 2px; padding: 9px 0 0 30px; display: flex; flex-direction: column; gap: 3px; border-top: 1px dashed var(--border); }
  .cit-ck { display: flex; align-items: center; gap: 9px; width: 100%; background: transparent; border: none; cursor: pointer; padding: 4px 6px; border-radius: var(--rad-2); text-align: left; }
  .cit-ck:hover { background: var(--surface-2); }
  .cit-ck-box {
    flex: none; width: 17px; height: 17px; border-radius: 5px; border: 1.5px solid var(--border-strong);
    display: inline-flex; align-items: center; justify-content: center; color: var(--bg);
    transition: background 0.18s var(--ease, ease), border-color 0.18s, transform 0.12s;
  }
  .cit-ck.on .cit-ck-box { background: var(--ok); border-color: var(--ok); transform: scale(1.06); animation: ck-pop 0.24s var(--ease, ease); }
  @keyframes ck-pop { 0% { transform: scale(0.7); } 55% { transform: scale(1.18); } 100% { transform: scale(1.06); } }
  .cit-ck-label { font-size: var(--t-sm); color: var(--fg); position: relative; transition: color 0.2s; }
  .cit-ck-label::after {
    content: ""; position: absolute; left: 0; top: 50%; width: 0; height: 1.5px; background: var(--fg-faint);
    transition: width 0.24s var(--ease, ease);
  }
  .cit-ck-label.done { color: var(--fg-faint); }
  .cit-ck-label.done::after { width: 100%; }
  .cit-dl-check { display: inline-flex; align-items: center; justify-content: center; width: 20px; height: 20px; border-radius: 50%; border: 1px solid var(--border); background: transparent; color: var(--fg-faint); cursor: pointer; flex: none; }
  .cit-dl-check:hover { color: var(--ok); border-color: var(--ok); }
  .cit-dl-title { flex: 1; min-width: 0; display: inline-flex; align-items: center; gap: 5px; font-size: var(--t-sm); text-align: left; background: transparent; border: none; padding: 0; color: var(--fg); cursor: pointer; }
  .cit-dl-title :global(svg) { flex: none; opacity: 0; color: var(--fg-faint); transition: opacity var(--dur-fast) var(--ease); }
  .cit-dl-title:hover { color: var(--accent); }
  .cit-dl-title:hover :global(svg) { opacity: 1; }
  .cit-dl-date { font-size: var(--t-xs); color: var(--fg-muted); }
  .cit-dl-left { font-size: var(--t-xs); color: var(--accent); min-width: 64px; text-align: right; }
  .cit-dl-left.overdue { color: var(--warn); }

  /* references */
  .cit-style-toggle { display: inline-flex; border: 1px solid var(--border); border-radius: var(--rad-2); overflow: hidden; }
  .cit-style-toggle button { padding: 3px 10px; font-size: var(--t-xs); background: transparent; border: none; color: var(--fg-muted); cursor: pointer; }
  .cit-style-toggle button.on { background: var(--accent); color: var(--bg); }
  .cit-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 8px; }
  .cit-item { border: 1px solid var(--border); border-radius: var(--rad-3); background: var(--surface); padding: 11px 12px; }
  .cit-row { display: flex; align-items: flex-start; gap: 10px; }
  .cit-badge { flex: none; font-size: 10px; text-transform: uppercase; letter-spacing: 0.05em; padding: 2px 7px; border-radius: 999px; background: color-mix(in oklab, var(--accent) 14%, var(--surface)); color: var(--accent); margin-top: 2px; }
  .cit-formatted { flex: 1; font-size: 13.5px; line-height: 1.5; }
  .cit-acts { display: flex; gap: 2px; flex: none; }
  .cit-notes { margin-top: 6px; font-size: var(--t-xs); padding-left: 2px; }

  /* form */
  .cit-form { border: 1px solid color-mix(in oklab, var(--accent) 30%, var(--border)); border-radius: var(--rad-3); background: color-mix(in oklab, var(--accent) 4%, var(--surface)); padding: 14px; }
  .cit-form-grid { display: grid; grid-template-columns: repeat(2, 1fr); gap: 10px; }
  .cit-field { display: flex; flex-direction: column; gap: 4px; }
  .cit-field--wide { grid-column: 1 / -1; }
  .cit-form-foot { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }

  @media (max-width: 600px) {
    .cit-page { padding: var(--sp-5) var(--sp-4); }
    .cit-form-grid { grid-template-columns: 1fr; }
    .cit-head { flex-wrap: wrap; }
    .cit-assign-row1, .cit-assign-row2 { flex-wrap: wrap; }
    .cit-assign-row1 .input, .cit-assign-row2 .input { flex: 1 1 100%; }
    .cit-date { width: 100%; flex: 1 1 100% !important; }
    .cit-row { flex-wrap: wrap; }
    .cit-acts { margin-left: auto; }
  }
</style>
