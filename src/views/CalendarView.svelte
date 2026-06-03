<script lang="ts">
  // Themed standalone month calendar. Renders subjects' events & tasks as
  // colored day-cell pills. No Google — pure local data via api.* calendar
  // wrappers. Click a day to add, click a pill to edit.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { CalEvent } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import EventModal from "../components/EventModal.svelte";

  const DOW = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
  const MONTHS = [
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December",
  ];

  // ---- visible month ----
  const today = new Date();
  let year = $state(today.getFullYear());
  let month = $state(today.getMonth()); // 0-11

  // ---- filter ----
  let filterSubjectId = $state<string>(""); // "" = all subjects

  const subjectOptions = $derived([
    { id: "", label: "All subjects" },
    ...app.subjects.map((s) => ({ id: s.id, label: s.name })),
  ]);

  // ---- events ----
  let events = $state<CalEvent[]>([]);
  let loading = $state(false);

  // Inclusive-of-grid window: from the first visible (possibly trailing) day to
  // the last, so events from adjacent months that show in the grid load too.
  function monthWindow(): { fromMs: number; toMs: number } {
    const first = new Date(year, month, 1);
    const startOffset = first.getDay(); // days of previous month shown
    const gridStart = new Date(year, month, 1 - startOffset);
    gridStart.setHours(0, 0, 0, 0);
    const gridEnd = new Date(gridStart);
    gridEnd.setDate(gridEnd.getDate() + 42); // 6 weeks
    gridEnd.setHours(0, 0, 0, 0);
    return { fromMs: gridStart.getTime(), toMs: gridEnd.getTime() };
  }

  // Reload whenever the visible month or subject filter changes.
  $effect(() => {
    const sid = filterSubjectId || null;
    const { fromMs, toMs } = monthWindow();
    let cancelled = false;
    loading = true;
    api
      .listEvents(sid, fromMs, toMs)
      .then((evs) => {
        if (!cancelled) events = evs;
      })
      .catch((e) => {
        if (!cancelled) {
          events = [];
          app.pushToast({ kind: "error", title: "Failed to load events", body: String(e) });
        }
      })
      .finally(() => {
        if (!cancelled) loading = false;
      });
    return () => {
      cancelled = true;
    };
  });

  // ---- grid model ----
  type Cell = { date: Date; key: string; inMonth: boolean; isToday: boolean };

  const cells = $derived.by<Cell[]>(() => {
    const first = new Date(year, month, 1);
    const startOffset = first.getDay();
    const start = new Date(year, month, 1 - startOffset);
    const now = new Date();
    const out: Cell[] = [];
    for (let i = 0; i < 42; i++) {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      out.push({
        date: d,
        key: dayKey(d),
        inMonth: d.getMonth() === month,
        isToday:
          d.getFullYear() === now.getFullYear() &&
          d.getMonth() === now.getMonth() &&
          d.getDate() === now.getDate(),
      });
    }
    return out;
  });

  function dayKey(d: Date): string {
    return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
  }

  // Bucket events by their start day (local).
  const byDay = $derived.by<Record<string, CalEvent[]>>(() => {
    const map: Record<string, CalEvent[]> = {};
    for (const e of events) {
      const k = dayKey(new Date(e.start_ms));
      (map[k] ??= []).push(e);
    }
    for (const k in map) map[k].sort((a, b) => a.start_ms - b.start_ms);
    return map;
  });

  // Precomputed subject→color map so pill rendering is a constant-time lookup
  // (rather than a linear find per pill on every re-render).
  const subjectColorMap = $derived(
    Object.fromEntries(app.subjects.map((s) => [s.id, app.subjectColor(s)]))
  );

  // Color for an event: explicit color, else its subject color, else accent.
  function pillColor(e: CalEvent): string {
    if (e.color) return e.color;
    if (e.subject_id && subjectColorMap[e.subject_id]) return subjectColorMap[e.subject_id];
    return "var(--accent)";
  }

  function timeLabel(e: CalEvent): string {
    if (e.all_day) return "";
    const d = new Date(e.start_ms);
    let h = d.getHours();
    const m = d.getMinutes();
    const ap = h >= 12 ? "p" : "a";
    h = h % 12 || 12;
    return m === 0 ? `${h}${ap}` : `${h}:${String(m).padStart(2, "0")}${ap}`;
  }

  // ---- navigation ----
  function prevMonth() {
    if (month === 0) {
      month = 11;
      year -= 1;
    } else month -= 1;
  }
  function nextMonth() {
    if (month === 11) {
      month = 0;
      year += 1;
    } else month += 1;
  }
  function goToday() {
    year = today.getFullYear();
    month = today.getMonth();
  }

  // ---- modal ----
  let modalOpen = $state(false);
  let editEvent = $state<CalEvent | null>(null);
  let modalDefaultMs = $state<number | undefined>(undefined);

  function openCreate(d: Date) {
    editEvent = null;
    modalDefaultMs = d.getTime();
    modalOpen = true;
  }
  function openEdit(e: CalEvent, ev: MouseEvent) {
    ev.stopPropagation();
    editEvent = e;
    modalDefaultMs = undefined;
    modalOpen = true;
  }
  function closeModal() {
    modalOpen = false;
    editEvent = null;
  }
  function reload() {
    // Refetch the current window directly after a save/delete (the load $effect
    // only re-runs on month/filter changes, not on event mutations).
    const sid = filterSubjectId || null;
    const { fromMs, toMs } = monthWindow();
    api
      .listEvents(sid, fromMs, toMs)
      .then((evs) => (events = evs))
      .catch((e) =>
        app.pushToast({ kind: "error", title: "Failed to load events", body: String(e) })
      );
  }

  async function toggleDone(e: CalEvent, ev: Event) {
    ev.stopPropagation();
    try {
      const updated = await api.setEventDone(e.id, !e.done);
      events = events.map((x) => (x.id === updated.id ? updated : x));
    } catch (err) {
      app.pushToast({ kind: "error", title: "Update failed", body: String(err) });
    }
  }
</script>

<div class="cal">
  <div class="cal-head">
    <div class="cal-nav">
      <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label="Previous month" onclick={prevMonth}>
        <Icon name="chevron" size={12} style="transform:rotate(180deg)" />
      </button>
      <div class="cal-monthyear">{MONTHS[month]} {year}</div>
      <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label="Next month" onclick={nextMonth}>
        <Icon name="chevron" size={12} />
      </button>
      <button class="btn btn--sm cal-today" type="button" onclick={goToday}>Today</button>
    </div>

    <div class="cal-head-r">
      <div class="cal-filter">
        <Picker
          value={filterSubjectId}
          onChange={(id) => (filterSubjectId = id)}
          options={subjectOptions}
          icon="book"
          placeholder="All subjects"
        />
      </div>
      <button class="btn btn--primary btn--sm" type="button" onclick={() => openCreate(new Date(year, month, 1))}>
        <Icon name="plus" size={12} />
        <span>New</span>
      </button>
    </div>
  </div>

  <div class="cal-dow">
    {#each DOW as d}
      <div class="cal-dow-cell">{d}</div>
    {/each}
  </div>

  <div class="cal-grid">
    {#each cells as c (c.key)}
      {@const dayEvents = byDay[c.key] ?? []}
      <div
        class={"cal-cell" + (c.inMonth ? "" : " out") + (c.isToday ? " today" : "")}
        role="button"
        tabindex="0"
        onclick={() => openCreate(c.date)}
        onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openCreate(c.date); } }}
      >
        <div class="cal-daynum">{c.date.getDate()}</div>
        <div class="cal-pills">
          {#each dayEvents as e (e.id)}
            <button
              type="button"
              class={"pill" + (e.kind === "task" && e.done ? " done" : "")}
              style:--pill-color={pillColor(e)}
              onclick={(ev) => openEdit(e, ev)}
              title={e.title}
            >
              {#if e.kind === "task"}
                <span
                  class={"pill-check" + (e.done ? " on" : "")}
                  role="checkbox"
                  aria-checked={e.done}
                  aria-label="Toggle done"
                  tabindex="-1"
                  onclick={(ev) => toggleDone(e, ev)}
                  onkeydown={(ev) => { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); toggleDone(e, ev); } }}
                >
                  {#if e.done}<Icon name="check" size={9} />{/if}
                </span>
              {:else}
                <span class="pill-dot"></span>
              {/if}
              {#if !e.all_day}<span class="pill-time">{timeLabel(e)}</span>{/if}
              <span class="pill-title">{e.title}</span>
            </button>
          {/each}
        </div>
      </div>
    {/each}
  </div>

  {#if !loading && events.length === 0}
    <div class="cal-empty">No events — click a day to add one.</div>
  {/if}
</div>

{#if modalOpen}
  <EventModal
    event={editEvent}
    defaultDateMs={modalDefaultMs}
    onClose={closeModal}
    onSaved={reload}
  />
{/if}

<style>
  .cal {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    padding: 20px 24px;
    box-sizing: border-box;
  }
  .cal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 14px;
    flex-wrap: wrap;
  }
  .cal-nav {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .cal-monthyear {
    font-family: var(--font-mono);
    font-size: var(--t-md, 14px);
    font-weight: 600;
    color: var(--fg-bright);
    min-width: 168px;
    text-align: center;
  }
  .cal-today {
    margin-left: 6px;
  }
  .cal-head-r {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .cal-filter {
    min-width: 180px;
  }

  /* day-of-week header */
  .cal-dow {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    border: 1px solid var(--border);
    border-bottom: none;
    border-radius: var(--r-lg, 12px) var(--r-lg, 12px) 0 0;
    overflow: hidden;
    background: var(--surface-2);
  }
  .cal-dow-cell {
    padding: 7px 8px;
    font-size: var(--t-2xs, 10.5px);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-faint);
    text-align: left;
  }

  /* month grid */
  .cal-grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    grid-template-rows: repeat(6, 1fr);
    border: 1px solid var(--border);
    border-radius: 0 0 var(--r-lg, 12px) var(--r-lg, 12px);
    overflow: hidden;
  }
  .cal-cell {
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    padding: 4px 5px 5px;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 3px;
    background: var(--surface);
    cursor: pointer;
    transition: background 0.1s ease;
    text-align: left;
  }
  /* remove far-right / bottom doubled borders */
  .cal-cell:nth-child(7n) {
    border-right: none;
  }
  .cal-cell:nth-child(n + 36) {
    border-bottom: none;
  }
  .cal-cell:hover {
    background: var(--surface-2);
  }
  .cal-cell.out {
    background: var(--bg);
    color: var(--fg-faint);
  }
  .cal-cell.out:hover {
    background: var(--surface);
  }
  .cal-cell.today {
    background: color-mix(in oklab, var(--accent) 8%, var(--surface));
  }
  .cal-daynum {
    font-family: var(--font-mono);
    font-size: var(--t-xs, 11.5px);
    color: var(--fg-muted);
    align-self: flex-start;
    width: 20px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    flex: none;
  }
  .cal-cell.out .cal-daynum {
    color: var(--fg-faint);
    opacity: 0.6;
  }
  .cal-cell.today .cal-daynum {
    background: var(--accent);
    color: var(--accent-fg);
    font-weight: 600;
  }

  /* pills */
  .cal-pills {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    min-height: 0;
  }
  .pill {
    appearance: none;
    border: none;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 5px;
    border-radius: 5px;
    cursor: pointer;
    text-align: left;
    font-family: var(--font-mono);
    font-size: var(--t-2xs, 10.5px);
    line-height: 1.3;
    color: var(--fg-bright);
    background: color-mix(in oklab, var(--pill-color) 18%, var(--surface));
    border-left: 2px solid var(--pill-color);
    transition: background 0.1s ease;
  }
  .pill:hover {
    background: color-mix(in oklab, var(--pill-color) 30%, var(--surface));
  }
  .pill.done {
    opacity: 0.5;
  }
  .pill.done .pill-title {
    text-decoration: line-through;
  }
  .pill-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--pill-color);
    flex: none;
  }
  .pill-check {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    border: 1.5px solid var(--pill-color);
    display: flex;
    align-items: center;
    justify-content: center;
    flex: none;
    color: var(--accent-fg);
    cursor: pointer;
  }
  .pill-check.on {
    background: var(--pill-color);
    color: var(--accent-fg);
  }
  .pill-time {
    color: var(--fg-muted);
    flex: none;
  }
  .pill-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .cal-empty {
    margin-top: 12px;
    text-align: center;
    font-size: var(--t-xs, 11.5px);
    color: var(--fg-faint);
  }
</style>
