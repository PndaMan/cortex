<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { Reference, CalEvent } from "../lib/api";
  import Icon from "../components/Icon.svelte";

  const subjectId = $derived(app.activeSubject?.id ?? null);

  // ── references ────────────────────────────────────────────────
  let refs = $state<Reference[]>([]);
  let style = $state<"apa" | "mla">("apa");
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
  const fmt = $derived(style === "apa" ? formatApa : formatMla);

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

  // ── deadlines (events with kind = 'deadline') ─────────────────
  let deadlines = $state<CalEvent[]>([]);
  let dTitle = $state("");
  let dDate = $state("");
  async function loadDeadlines() {
    if (!subjectId) { deadlines = []; return; }
    try {
      const all = await api.listEvents(subjectId);
      const now = Date.now();
      deadlines = all
        .filter((e) => e.kind === "deadline" && !e.done && e.start_ms >= now - 86_400_000)
        .sort((a, b) => a.start_ms - b.start_ms);
    } catch { /* non-fatal */ }
  }
  $effect(() => { void subjectId; loadDeadlines(); });

  async function addDeadline() {
    if (!subjectId || !dTitle.trim() || !dDate) return;
    const startMs = new Date(dDate + "T23:59:00").getTime();
    try {
      await api.createEvent({
        title: dTitle.trim(), startMs, subjectId, kind: "deadline", allDay: true,
        reminderMs: startMs - 86_400_000, // remind 1 day before
        color: "var(--warn)",
      });
      dTitle = ""; dDate = "";
      await loadDeadlines();
      app.pushToast({ kind: "success", title: "Deadline added" });
    } catch (e) {
      app.pushToast({ kind: "error", title: "Add failed", body: String(e) });
    }
  }
  async function completeDeadline(e: CalEvent) {
    try { await api.setEventDone(e.id, true); await loadDeadlines(); }
    catch (err) { app.pushToast({ kind: "error", title: "Update failed", body: String(err) }); }
  }
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
</script>

<div class="workspace-scroll">
  <div class="cit-page">
    <!-- Deadlines -->
    <section class="cit-section">
      <div class="cit-head">
        <h2 class="cit-h read"><Icon name="calendar" size={15} /> Upcoming deadlines</h2>
      </div>
      <div class="cit-deadline-add">
        <input class="input" placeholder="Assignment / exam…" bind:value={dTitle} />
        <input class="input cit-date" type="date" bind:value={dDate} />
        <button class="btn btn--sm btn--primary" disabled={!dTitle.trim() || !dDate} onclick={addDeadline}>
          <Icon name="plus" size={12} /> Add
        </button>
      </div>
      {#if deadlines.length === 0}
        <p class="mono faint cit-empty">No upcoming deadlines.</p>
      {:else}
        <ul class="cit-deadlines">
          {#each deadlines as e (e.id)}
            {@const overdue = e.start_ms < Date.now()}
            <li class="cit-deadline{overdue ? ' overdue' : ''}">
              <button class="cit-dl-check" title="Mark done" aria-label="Mark done" onclick={() => completeDeadline(e)}>
                <Icon name="check" size={11} />
              </button>
              <span class="cit-dl-title">{e.title}</span>
              <span class="cit-dl-date mono">{fmtDate(e.start_ms)}</span>
              <span class="cit-dl-left mono{overdue ? ' overdue' : ''}">{daysLeft(e.start_ms)}</span>
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
      <label class="cit-field cit-field--type">
        <span class="onb-label mono">TYPE</span>
        <select class="input" bind:value={f.ctype}>
          {#each CTYPES as t (t.id)}<option value={t.id}>{t.label}</option>{/each}
        </select>
      </label>
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

  /* deadlines */
  .cit-deadline-add { display: flex; gap: 8px; align-items: center; }
  .cit-deadline-add .input { flex: 1; }
  .cit-date { flex: none !important; width: 160px; }
  .cit-deadlines { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .cit-deadline { display: flex; align-items: center; gap: 10px; padding: 9px 12px; border: 1px solid var(--border); border-radius: var(--rad-3); background: var(--surface); }
  .cit-deadline.overdue { border-color: color-mix(in oklab, var(--warn) 55%, var(--border)); }
  .cit-dl-check { display: inline-flex; align-items: center; justify-content: center; width: 20px; height: 20px; border-radius: 50%; border: 1px solid var(--border); background: transparent; color: var(--fg-faint); cursor: pointer; flex: none; }
  .cit-dl-check:hover { color: var(--ok); border-color: var(--ok); }
  .cit-dl-title { flex: 1; font-size: var(--t-sm); }
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
    .cit-deadline-add { flex-wrap: wrap; }
    .cit-deadline-add .input { flex: 1 1 100%; }
    .cit-date { width: 100%; flex: 1 1 100% !important; }
    .cit-row { flex-wrap: wrap; }
    .cit-acts { margin-left: auto; }
  }
</style>
