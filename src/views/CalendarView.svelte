<script lang="ts">
  // Themed standalone month/day calendar. Renders subjects' events & tasks as
  // colored day-cell pills. No Google — pure local data via api.* calendar
  // wrappers. Click a day cell to enter day view; click a pill to edit.
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
  const DAYS_LONG = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

  // ---- visible month ----
  const today = new Date();
  let year = $state(today.getFullYear());
  let month = $state(today.getMonth()); // 0-11

  // ---- view mode + day selection ----
  let mode = $state<"month" | "day">("month");
  let selectedDay = $state<Date | null>(null);

  function openDayView(d: Date) {
    selectedDay = d;
    mode = "day";
  }
  function backToMonth() {
    mode = "month";
  }
  function prevDay() {
    if (!selectedDay) return;
    const d = new Date(selectedDay);
    d.setDate(d.getDate() - 1);
    selectedDay = d;
    // Keep month grid in sync so "Month" returns to the right month.
    year = d.getFullYear();
    month = d.getMonth();
  }
  function nextDay() {
    if (!selectedDay) return;
    const d = new Date(selectedDay);
    d.setDate(d.getDate() + 1);
    selectedDay = d;
    year = d.getFullYear();
    month = d.getMonth();
  }

  const selectedDayLabel = $derived.by<string>(() => {
    if (!selectedDay) return "";
    return `${DAYS_LONG[selectedDay.getDay()]}, ${MONTHS[selectedDay.getMonth()]} ${selectedDay.getDate()} ${selectedDay.getFullYear()}`;
  });

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

  // Events for the day-view (byDay buckets are already sorted by start_ms).
  const selectedDayEvents = $derived<CalEvent[]>(
    selectedDay ? (byDay[dayKey(selectedDay)] ?? []) : []
  );

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

  function fmtMs(ms: number): string {
    const d = new Date(ms);
    let h = d.getHours();
    const m = d.getMinutes();
    const ap = h >= 12 ? "pm" : "am";
    h = h % 12 || 12;
    return m === 0 ? `${h}${ap}` : `${h}:${String(m).padStart(2, "0")}${ap}`;
  }

  function timeLabel(e: CalEvent): string {
    return e.all_day ? "" : fmtMs(e.start_ms);
  }

  function timeLabelEnd(e: CalEvent): string {
    return e.all_day || e.end_ms == null ? "" : fmtMs(e.end_ms);
  }

  // ---- navigation (month) ----
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
  <!-- ===== SHARED HEADER ===== -->
  <div class="cal-head">
    <div class="cal-nav">
      {#if mode === "month"}
        <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label="Previous month" onclick={prevMonth}>
          <Icon name="chevron" size={12} style="transform:rotate(180deg)" />
        </button>
        <div class="cal-monthyear">{MONTHS[month]} {year}</div>
        <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label="Next month" onclick={nextMonth}>
          <Icon name="chevron" size={12} />
        </button>
        <button class="btn btn--sm cal-today" type="button" onclick={goToday}>Today</button>
      {:else}
        <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label="Previous day" onclick={prevDay}>
          <Icon name="chevron" size={12} style="transform:rotate(180deg)" />
        </button>
        <div class="cal-monthyear cal-daydate">{selectedDayLabel}</div>
        <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label="Next day" onclick={nextDay}>
          <Icon name="chevron" size={12} />
        </button>
        <button class="btn btn--sm cal-today" type="button" onclick={backToMonth}>Month</button>
      {/if}
    </div>

    <div class="cal-head-r">
      <div class="picker-wrap">
        <Picker
          value={filterSubjectId}
          onChange={(id) => (filterSubjectId = id)}
          options={subjectOptions}
          icon="book"
          placeholder="All subjects"
        />
      </div>
      {#if mode === "month"}
        <button class="btn btn--primary btn--sm" type="button" onclick={() => openCreate(new Date(year, month, 1))}>
          <Icon name="plus" size={12} />
          <span>New</span>
        </button>
      {:else}
        <button class="btn btn--primary btn--sm" type="button" onclick={() => selectedDay && openCreate(selectedDay)}>
          <Icon name="plus" size={12} />
          <span>Add</span>
        </button>
      {/if}
    </div>
  </div>

  {#if mode === "month"}
    <!-- ===== MONTH GRID ===== -->
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
          onclick={() => openDayView(c.date)}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openDayView(c.date); } }}
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
                <div class="pill-row">
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
                </div>
                {#if e.location}
                  <div class="pill-loc">{e.location}</div>
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </div>

    {#if !loading && events.length === 0}
      <div class="cal-empty">No events — click a day to view or add one.</div>
    {/if}

  {:else}
    <!-- ===== DAY VIEW ===== -->
    <div class="cal-day">
      {#if selectedDayEvents.length === 0}
        <div class="cal-day-empty">
          <div class="cal-day-empty-msg">No events on this day.</div>
          <button
            class="btn btn--primary btn--sm"
            type="button"
            onclick={() => selectedDay && openCreate(selectedDay)}
          >
            <Icon name="plus" size={12} />
            <span>Add event</span>
          </button>
        </div>
      {:else}
        <div class="cal-day-events">
          {#each selectedDayEvents as e (e.id)}
            {@const startLbl = timeLabel(e)}
            {@const endLbl = timeLabelEnd(e)}
            <button
              type="button"
              class={"dey" + (e.kind === "task" && e.done ? " dey--done" : "")}
              style:--dey-color={pillColor(e)}
              onclick={(ev) => openEdit(e, ev)}
            >
              <div class="dey-accent"></div>
              <div class="dey-body">
                <div class="dey-header">
                  <span class="dey-time">
                    {e.all_day ? "All day" : startLbl}{endLbl ? ` – ${endLbl}` : ""}
                  </span>
                  {#if e.kind === "task"}
                    <span
                      class={"pill-check dey-check" + (e.done ? " on" : "")}
                      role="checkbox"
                      aria-checked={e.done}
                      aria-label="Toggle done"
                      tabindex="-1"
                      onclick={(ev) => toggleDone(e, ev)}
                      onkeydown={(ev) => { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); toggleDone(e, ev); } }}
                    >
                      {#if e.done}<Icon name="check" size={10} />{/if}
                    </span>
                  {/if}
                </div>
                <div class="dey-title">{e.title}</div>
                {#if e.location}
                  <div class="dey-loc">{e.location}</div>
                {/if}
                {#if e.description}
                  <div class="dey-desc">{e.description}</div>
                {/if}
              </div>
            </button>
          {/each}
        </div>
      {/if}
    </div>
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
  .cal-daydate {
    min-width: 240px;
    font-size: var(--t-sm, 12.5px);
  }
  .cal-today {
    margin-left: 6px;
  }
  .cal-head-r {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  /* subject filter picker wrapper — gives Picker a styled border container */
  .picker-wrap {
    border: 1px solid var(--border-strong);
    border-radius: var(--r-lg, 12px);
    background: var(--surface-2);
    overflow: hidden;
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
    flex-direction: column;
    align-items: flex-start;
    gap: 1px;
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
  /* inner row: dot/check + time + title on one line */
  .pill-row {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    min-width: 0;
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
  /* location line beneath the main row */
  .pill-loc {
    font-size: calc(var(--t-2xs, 10.5px) - 0.5px);
    color: var(--fg-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    width: 100%;
    padding-left: 9px; /* indent past dot/check */
  }

  .cal-empty {
    margin-top: 12px;
    text-align: center;
    font-size: var(--t-xs, 11.5px);
    color: var(--fg-faint);
  }

  /* ===== DAY VIEW ===== */
  .cal-day {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--r-lg, 12px);
    overflow: hidden;
    background: var(--surface);
  }

  .cal-day-events {
    flex: 1;
    overflow-y: auto;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  /* Day event row */
  .dey {
    appearance: none;
    display: flex;
    align-items: stretch;
    background: color-mix(in oklab, var(--dey-color) 10%, var(--surface));
    border: 1px solid color-mix(in oklab, var(--dey-color) 20%, var(--border));
    border-radius: 8px;
    cursor: pointer;
    text-align: left;
    width: 100%;
    transition: background 0.1s ease;
    overflow: hidden;
  }
  .dey:hover {
    background: color-mix(in oklab, var(--dey-color) 20%, var(--surface));
  }
  .dey--done {
    opacity: 0.55;
  }
  .dey--done .dey-title {
    text-decoration: line-through;
  }
  .dey-accent {
    width: 4px;
    flex: none;
    background: var(--dey-color);
  }
  .dey-body {
    flex: 1;
    min-width: 0;
    padding: 8px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .dey-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 1px;
  }
  .dey-time {
    font-family: var(--font-mono);
    font-size: var(--t-xs, 11.5px);
    color: var(--fg-muted);
    flex: none;
  }
  .dey-check {
    border-color: var(--dey-color);
  }
  .dey-check.on {
    background: var(--dey-color);
  }
  .dey-title {
    font-family: var(--font-mono);
    font-size: var(--t-sm, 12.5px);
    font-weight: 600;
    color: var(--fg-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dey-loc {
    font-size: var(--t-xs, 11.5px);
    color: var(--fg-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dey-desc {
    font-size: var(--t-xs, 11.5px);
    color: var(--fg-muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    line-height: 1.4;
  }

  .cal-day-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
  }
  .cal-day-empty-msg {
    font-size: var(--t-sm, 12.5px);
    color: var(--fg-faint);
  }
</style>
