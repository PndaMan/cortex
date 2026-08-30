<script lang="ts">
  // Themed standalone month/week/day calendar. Renders subjects' events, tasks &
  // deadlines as kind-colored chips (month) or positioned time-grid blocks
  // (week/day). No Google — pure local data via api.* calendar wrappers.
  //
  // Layout: a single shared time-grid engine (layoutDay → lanes) powers both the
  // week columns and the day column, so overlapping events split side-by-side
  // instead of stacking/squashing. Click an empty slot to create; click an
  // event to edit.
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import type { CalEvent } from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import EventModal from "../components/EventModal.svelte";
  import { t } from "../lib/i18n.svelte";

  const DOW = [t("Sun"), t("Mon"), t("Tue"), t("Wed"), t("Thu"), t("Fri"), t("Sat")];
  const MONTHS = [
    t("January"), t("February"), t("March"), t("April"), t("May"), t("June"),
    t("July"), t("August"), t("September"), t("October"), t("November"), t("December"),
  ];
  const DAYS_LONG = [t("Sunday"), t("Monday"), t("Tuesday"), t("Wednesday"), t("Thursday"), t("Friday"), t("Saturday")];

  // ---- visible month / view mode ----
  const today = new Date();
  let year = $state(today.getFullYear());
  let month = $state(today.getMonth()); // 0-11

  type Mode = "month" | "week" | "day";
  let mode = $state<Mode>("month");
  // The reference day for week/day views (anchors the grid + nav).
  let selectedDay = $state<Date>(new Date());

  function openDayView(d: Date) {
    selectedDay = startOfDay(d);
    year = d.getFullYear();
    month = d.getMonth();
    mode = "day";
  }
  function setMode(m: Mode) {
    mode = m;
    if (m !== "month") {
      // Keep the week/day anchor sensible when switching from month.
      const d = new Date(selectedDay);
      year = d.getFullYear();
      month = d.getMonth();
    }
  }

  // Arriving from the Assignments list ("open in calendar") — jump to that day.
  $effect(() => {
    const ms = app.calendarFocusMs;
    if (ms == null) return;
    app.calendarFocusMs = null;
    const d = new Date(ms);
    openDayView(d);
  });

  // ---- date helpers ----
  function startOfDay(d: Date): Date {
    const x = new Date(d);
    x.setHours(0, 0, 0, 0);
    return x;
  }
  function addDays(d: Date, n: number): Date {
    const x = new Date(d);
    x.setDate(x.getDate() + n);
    return x;
  }
  function startOfWeek(d: Date): Date {
    const x = startOfDay(d);
    return addDays(x, -x.getDay()); // Sunday-based
  }
  function sameYMD(a: Date, b: Date): boolean {
    return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
  }
  function dayKey(d: Date): string {
    return `${d.getFullYear()}-${d.getMonth()}-${d.getDate()}`;
  }

  // ---- unified navigation ----
  function prev() {
    if (mode === "month") prevMonth();
    else if (mode === "week") { selectedDay = addDays(selectedDay, -7); syncMonth(); }
    else { selectedDay = addDays(selectedDay, -1); syncMonth(); }
  }
  function next() {
    if (mode === "month") nextMonth();
    else if (mode === "week") { selectedDay = addDays(selectedDay, 7); syncMonth(); }
    else { selectedDay = addDays(selectedDay, 1); syncMonth(); }
  }
  function goToday() {
    const n = new Date();
    selectedDay = startOfDay(n);
    year = n.getFullYear();
    month = n.getMonth();
  }
  function syncMonth() {
    year = selectedDay.getFullYear();
    month = selectedDay.getMonth();
  }
  function prevMonth() {
    if (month === 0) { month = 11; year -= 1; } else month -= 1;
  }
  function nextMonth() {
    if (month === 11) { month = 0; year += 1; } else month += 1;
  }

  // ---- header title ----
  const weekDays = $derived.by<Date[]>(() => {
    const s = startOfWeek(selectedDay);
    return Array.from({ length: 7 }, (_, i) => addDays(s, i));
  });
  const headerTitle = $derived.by<string>(() => {
    if (mode === "month") return `${MONTHS[month]} ${year}`;
    if (mode === "day") {
      return `${DAYS_LONG[selectedDay.getDay()]}, ${MONTHS[selectedDay.getMonth()]} ${selectedDay.getDate()}`;
    }
    // week → "Jun 8 – 14, 2026" (compact, spanning months when needed)
    const a = weekDays[0], b = weekDays[6];
    const left = `${MONTHS[a.getMonth()].slice(0, 3)} ${a.getDate()}`;
    const right = a.getMonth() === b.getMonth()
      ? `${b.getDate()}`
      : `${MONTHS[b.getMonth()].slice(0, 3)} ${b.getDate()}`;
    return `${left} – ${right}, ${b.getFullYear()}`;
  });

  // ---- filter ----
  let filterSubjectId = $state<string>(""); // "" = all subjects
  const subjectOptions = $derived([
    { id: "", label: t("All subjects") },
    ...app.subjects.map((s) => ({ id: s.id, label: s.name })),
  ]);

  // ---- events + load window ----
  let events = $state<CalEvent[]>([]);
  let loading = $state(false);

  // Loads enough to cover whichever view is active (always the 6-week month grid,
  // which comfortably spans the week/day anchors too since they sync the month).
  function loadWindow(): { fromMs: number; toMs: number } {
    if (mode === "month") {
      const first = new Date(year, month, 1);
      const gridStart = addDays(first, -first.getDay());
      gridStart.setHours(0, 0, 0, 0);
      const gridEnd = addDays(gridStart, 42);
      return { fromMs: gridStart.getTime(), toMs: gridEnd.getTime() };
    }
    // week / day: load the whole surrounding week ± a day of slack.
    const s = addDays(startOfWeek(selectedDay), -1);
    const e = addDays(s, 9);
    return { fromMs: s.getTime(), toMs: e.getTime() };
  }

  $effect(() => {
    void app.eventsChangedNonce;
    void mode; void year; void month; void selectedDay;
    const sid = filterSubjectId || null;
    const { fromMs, toMs } = loadWindow();
    let cancelled = false;
    loading = true;
    api
      .listEvents(sid, fromMs, toMs)
      .then((evs) => { if (!cancelled) events = evs; })
      .catch((e) => {
        if (!cancelled) {
          events = [];
          app.pushToast({ kind: "error", title: t("Failed to load events"), body: String(e) });
        }
      })
      .finally(() => { if (!cancelled) loading = false; });
    return () => { cancelled = true; };
  });

  // ---- month grid model ----
  type Cell = { date: Date; key: string; inMonth: boolean; isToday: boolean; weekend: boolean };
  const cells = $derived.by<Cell[]>(() => {
    const first = new Date(year, month, 1);
    const start = addDays(first, -first.getDay());
    const now = new Date();
    return Array.from({ length: 42 }, (_, i) => {
      const d = addDays(start, i);
      return {
        date: d,
        key: dayKey(d),
        inMonth: d.getMonth() === month,
        isToday: sameYMD(d, now),
        weekend: d.getDay() === 0 || d.getDay() === 6,
      };
    });
  });

  // Bucket events by LOCAL start day — one key everywhere (cells + lookups).
  const byDay = $derived.by<Record<string, CalEvent[]>>(() => {
    const map: Record<string, CalEvent[]> = {};
    for (const e of events) {
      const k = dayKey(new Date(e.start_ms));
      (map[k] ??= []).push(e);
    }
    for (const k in map)
      map[k].sort((a, b) =>
        a.all_day === b.all_day ? a.start_ms - b.start_ms : a.all_day ? -1 : 1
      );
    return map;
  });

  // Month chips overflow cap.
  const MONTH_CHIP_CAP = 3;

  // ---- shared time-grid geometry ----
  const HOURS = Array.from({ length: 24 }, (_, h) => h);
  const HOUR_PX = 58; // tall enough to breathe
  const MIN_BLOCK_PX = 26; // always fits title + time

  function hourLabel(h: number): string {
    if (h === 0) return t("12 am");
    if (h === 12) return t("12 pm");
    const ap = h >= 12 ? t("pm") : t("am");
    return `${h % 12} ${ap}`;
  }

  // ---- time-grid layout engine (overlap → lanes) ----
  // For a given day's events, returns positioned blocks with left/width as
  // fractions of the column, splitting overlapping events into side-by-side
  // lanes via a greedy interval-graph colouring.
  type Block = { e: CalEvent; top: number; height: number; left: number; width: number };
  function layoutDay(day: Date, dayEvents: CalEvent[]): Block[] {
    const dayStartMs = startOfDay(day).getTime();
    const dayMs = 24 * 60 * 60 * 1000;
    // Build timed intervals (skip all-day), clamped to the visible day.
    type Iv = { e: CalEvent; start: number; end: number; top: number; height: number; lane: number; lanes: number };
    const ivs: Iv[] = [];
    for (const e of dayEvents) {
      if (e.all_day) continue;
      const startOff = Math.max(0, e.start_ms - dayStartMs);
      const endRaw = e.end_ms != null ? e.end_ms : e.start_ms + 60 * 60 * 1000;
      const endOff = Math.min(dayMs, endRaw - dayStartMs);
      const startMin = startOff / 60000;
      const top = (startMin / 60) * HOUR_PX;
      const height = Math.max(MIN_BLOCK_PX, ((endOff / 60000 - startMin) / 60) * HOUR_PX);
      ivs.push({ e, start: top, end: top + height, top, height, lane: 0, lanes: 1 });
    }
    ivs.sort((a, b) => a.start - b.start || a.end - b.end);

    // Greedy lane assignment within connected overlap clusters.
    const out: Block[] = [];
    let cluster: Iv[] = [];
    let clusterEnd = -1;
    const flush = () => {
      if (!cluster.length) return;
      // Assign each interval the first free lane (one that ended before it starts).
      const laneEnds: number[] = [];
      for (const iv of cluster) {
        let placed = -1;
        for (let l = 0; l < laneEnds.length; l++) {
          if (laneEnds[l] <= iv.start + 0.01) { placed = l; break; }
        }
        if (placed === -1) { placed = laneEnds.length; laneEnds.push(0); }
        iv.lane = placed;
        laneEnds[placed] = iv.end;
      }
      const lanes = laneEnds.length;
      for (const iv of cluster) {
        out.push({
          e: iv.e,
          top: iv.top,
          height: iv.height,
          left: iv.lane / lanes,
          width: 1 / lanes,
        });
      }
      cluster = [];
      clusterEnd = -1;
    };
    for (const iv of ivs) {
      if (cluster.length && iv.start >= clusterEnd) flush();
      cluster.push(iv);
      clusterEnd = Math.max(clusterEnd, iv.end);
    }
    flush();
    return out;
  }

  // Day-view: blocks + all-day for the single selected day.
  const dayEventsList = $derived<CalEvent[]>(byDay[dayKey(selectedDay)] ?? []);
  const dayAllDay = $derived<CalEvent[]>(dayEventsList.filter((e) => e.all_day));
  const dayBlocks = $derived.by<Block[]>(() => layoutDay(selectedDay, dayEventsList));

  // Week-view: per-column blocks + per-column all-day, plus whether any all-day
  // exists (so the pinned strip only takes space when needed).
  type WeekCol = { date: Date; isToday: boolean; weekend: boolean; allDay: CalEvent[]; blocks: Block[] };
  const weekCols = $derived.by<WeekCol[]>(() => {
    const now = new Date();
    return weekDays.map((d) => {
      const evs = byDay[dayKey(d)] ?? [];
      return {
        date: d,
        isToday: sameYMD(d, now),
        weekend: d.getDay() === 0 || d.getDay() === 6,
        allDay: evs.filter((e) => e.all_day),
        blocks: layoutDay(d, evs),
      };
    });
  });
  const weekHasAllDay = $derived(weekCols.some((c) => c.allDay.length > 0));

  // ---- now-line ----
  let nowMs = $state(Date.now());
  $effect(() => {
    const id = setInterval(() => { nowMs = Date.now(); }, 60000);
    return () => clearInterval(id);
  });
  const nowTop = $derived.by<number>(() => {
    const n = new Date(nowMs);
    return ((n.getHours() * 60 + n.getMinutes()) / 60) * HOUR_PX;
  });
  const dayIsToday = $derived(sameYMD(selectedDay, new Date(nowMs)));

  // Auto-scroll the active time-grid to the current hour (or 8am) on open / change.
  let gridEl = $state<HTMLDivElement | null>(null);
  $effect(() => {
    if (mode === "month" || !gridEl) return;
    void selectedDay; void mode;
    const el = gridEl;
    const anchorToday = mode === "day" ? dayIsToday : weekCols.some((c) => c.isToday);
    const hour = anchorToday ? new Date(nowMs).getHours() : 8;
    const target = Math.max(0, hour * HOUR_PX - HOUR_PX);
    queueMicrotask(() => el.scrollTo({ top: target }));
  });

  // ---- create at slot ----
  function openCreateAtHour(d: Date, h: number) {
    const x = new Date(d);
    x.setHours(h, 0, 0, 0);
    editEvent = null;
    modalDefaultMs = x.getTime();
    modalOpen = true;
  }

  // ---- colors / kinds ----
  const subjectColorMap = $derived(
    Object.fromEntries(app.subjects.map((s) => [s.id, app.subjectColor(s)]))
  );
  const DEADLINE_KINDS = ["exam", "assignment", "project", "deadline"];
  function isDeadline(e: CalEvent): boolean { return DEADLINE_KINDS.includes(e.kind); }
  function completable(e: CalEvent): boolean { return e.kind === "task" || isDeadline(e); }
  function kindColor(kind: string): string | null {
    switch (kind) {
      case "exam": return "var(--warn)";
      case "assignment": return "var(--accent)";
      case "project": return "var(--info)";
      case "deadline": return "var(--warn)";
      default: return null;
    }
  }
  function kindGlyph(kind: string): string {
    switch (kind) {
      case "exam": return "✎";
      case "assignment": return "▤";
      case "project": return "◆";
      case "deadline": return "⏰";
      default: return "";
    }
  }
  // Priority outranks subject color for assignments (so high-priority pops).
  function priorityColor(p: string | null): string | null {
    switch (p) {
      case "high": return "var(--err)";
      case "med": return "var(--warn)";
      case "low": return "var(--ok)";
      default: return null;
    }
  }
  function pillColor(e: CalEvent): string {
    if (e.color) return e.color;
    if (e.kind === "assignment") return priorityColor(e.priority) ?? "var(--accent)";
    if (isDeadline(e)) return kindColor(e.kind) ?? "var(--warn)";
    if (e.subject_id && subjectColorMap[e.subject_id]) return subjectColorMap[e.subject_id];
    return "var(--accent)";
  }

  function fmtMs(ms: number): string {
    const d = new Date(ms);
    let h = d.getHours();
    const m = d.getMinutes();
    const ap = h >= 12 ? t("pm") : t("am");
    h = h % 12 || 12;
    return m === 0 ? `${h}${ap}` : `${h}:${String(m).padStart(2, "0")}${ap}`;
  }
  function timeLabel(e: CalEvent): string { return e.all_day ? "" : fmtMs(e.start_ms); }
  function timeRange(e: CalEvent): string {
    if (e.all_day) return "";
    const s = fmtMs(e.start_ms);
    return e.end_ms != null ? `${s} – ${fmtMs(e.end_ms)}` : s;
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
    const sid = filterSubjectId || null;
    const { fromMs, toMs } = loadWindow();
    api
      .listEvents(sid, fromMs, toMs)
      .then((evs) => (events = evs))
      .catch((e) =>
        app.pushToast({ kind: "error", title: t("Failed to load events"), body: String(e) })
      );
  }

  async function toggleDone(e: CalEvent, ev: Event) {
    ev.stopPropagation();
    try {
      const updated = await api.setEventDone(e.id, !e.done);
      events = events.map((x) => (x.id === updated.id ? updated : x));
      app.notifyEventsChanged();
    } catch (err) {
      app.pushToast({ kind: "error", title: t("Update failed"), body: String(err) });
    }
  }

  // The "add" button target date for the current view.
  function addTarget(): Date {
    if (mode === "month") return new Date(year, month, 1);
    return new Date(selectedDay);
  }
</script>

<div class="cal">
  <!-- ===== HEADER ===== -->
  <div class="cal-head">
    <div class="cal-nav">
      <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label={t("Previous")} onclick={prev}>
        <Icon name="chevron" size={12} style="transform:rotate(180deg)" />
      </button>
      <button class="btn btn--sm cal-today" type="button" onclick={goToday}>{t("Today")}</button>
      <button class="btn btn--ghost btn--icon btn--sm" type="button" aria-label={t("Next")} onclick={next}>
        <Icon name="chevron" size={12} />
      </button>
      <div class="cal-title">{headerTitle}</div>
    </div>

    <div class="cal-head-r">
      <div class="seg cal-modeseg" role="group" aria-label={t("Calendar view")}>
        <button type="button" class={"seg-opt" + (mode === "month" ? " on" : "")} onclick={() => setMode("month")}>{t("Month")}</button>
        <button type="button" class={"seg-opt" + (mode === "week" ? " on" : "")} onclick={() => setMode("week")}>{t("Week")}</button>
        <button type="button" class={"seg-opt" + (mode === "day" ? " on" : "")} onclick={() => setMode("day")}>{t("Day")}</button>
      </div>
      <div class="picker-wrap">
        <Picker
          value={filterSubjectId}
          onChange={(id) => (filterSubjectId = id)}
          options={subjectOptions}
          icon="book"
          placeholder={t("All subjects")}
        />
      </div>
      <button class="btn btn--primary btn--sm" type="button" onclick={() => openCreate(addTarget())}>
        <Icon name="plus" size={12} />
        <span>{t("New")}</span>
      </button>
    </div>
  </div>

  {#if mode === "month"}
    <!-- ===== MONTH GRID ===== -->
    <div class="cal-dow">
      {#each DOW as d, i}
        <div class={"cal-dow-cell" + (i === 0 || i === 6 ? " weekend" : "")}>{d}</div>
      {/each}
    </div>

    <div class="cal-grid">
      {#each cells as c (c.key)}
        {@const dayEvents = byDay[c.key] ?? []}
        {@const overflow = dayEvents.length - MONTH_CHIP_CAP}
        <div
          class={"cal-cell" + (c.inMonth ? "" : " out") + (c.isToday ? " today" : "") + (c.weekend ? " weekend" : "")}
          role="button"
          tabindex="0"
          onclick={() => openDayView(c.date)}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); openDayView(c.date); } }}
        >
          <div class="cal-cell-head">
            <span class="cal-daynum">{c.date.getDate()}</span>
          </div>
          <div class="cal-chips">
            {#each dayEvents.slice(0, MONTH_CHIP_CAP) as e (e.id)}
              <button
                type="button"
                class={"chip" + (completable(e) && e.done ? " done" : "")}
                style:--chip-color={pillColor(e)}
                onclick={(ev) => openEdit(e, ev)}
                title={e.title}
              >
                {#if completable(e)}
                  <span
                    class={"chip-check" + (e.done ? " on" : "")}
                    role="checkbox"
                    aria-checked={e.done}
                    aria-label={t("Toggle done")}
                    tabindex="-1"
                    onclick={(ev) => toggleDone(e, ev)}
                    onkeydown={(ev) => { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); toggleDone(e, ev); } }}
                  >
                    {#if e.done}<Icon name="check" size={8} />{/if}
                  </span>
                {:else}
                  <span class="chip-dot"></span>
                {/if}
                {#if isDeadline(e)}<span class="chip-glyph" aria-hidden="true">{kindGlyph(e.kind)}</span>{/if}
                {#if !e.all_day}<span class="chip-time">{timeLabel(e)}</span>{/if}
                <span class="chip-title">{e.title}</span>
              </button>
            {/each}
            {#if overflow > 0}
              <button type="button" class="chip-more" onclick={() => openDayView(c.date)}>{t("+{n} more", { n: overflow })}</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    {#if !loading && events.length === 0}
      <div class="cal-empty">{t("No events — click a day to view or add one.")}</div>
    {/if}

  {:else if mode === "week"}
    <!-- ===== WEEK GRID (7-column time-grid) ===== -->
    <div class="cal-grid-wrap">
      <!-- weekday header strip -->
      <div class="tg-colhead">
        <div class="tg-gutter-head"></div>
        {#each weekCols as c (c.date.getTime())}
          <button
            type="button"
            class={"tg-day-head" + (c.isToday ? " today" : "") + (c.weekend ? " weekend" : "")}
            onclick={() => openDayView(c.date)}
          >
            <span class="tg-dow">{DOW[c.date.getDay()]}</span>
            <span class="tg-dnum">{c.date.getDate()}</span>
          </button>
        {/each}
      </div>

      <!-- all-day strip (only when present) -->
      {#if weekHasAllDay}
        <div class="tg-allday">
          <div class="tg-gutter-lbl">{t("All day")}</div>
          {#each weekCols as c (c.date.getTime())}
            <div class={"tg-allday-col" + (c.weekend ? " weekend" : "")}>
              {#each c.allDay as e (e.id)}
                <button
                  type="button"
                  class={"ad-pill" + (completable(e) && e.done ? " done" : "")}
                  style:--blk-color={pillColor(e)}
                  onclick={(ev) => openEdit(e, ev)}
                  title={e.title}
                >
                  {#if isDeadline(e)}<span class="ad-glyph" aria-hidden="true">{kindGlyph(e.kind)}</span>{/if}
                  <span class="ad-title">{e.title}</span>
                </button>
              {/each}
            </div>
          {/each}
        </div>
      {/if}

      <!-- scrollable hour grid -->
      <div class="tg-scroll" bind:this={gridEl}>
        <div class="tg-body" style:height={`${HOUR_PX * 24}px`}>
          <!-- hour rules + gutter -->
          {#each HOURS as h (h)}
            <div class="tg-hour" style:top={`${h * HOUR_PX}px`} style:height={`${HOUR_PX}px`}>
              <div class="tg-hour-lbl">{h === 0 ? "" : hourLabel(h)}</div>
            </div>
          {/each}

          <!-- 7 day columns -->
          <div class="tg-cols">
            {#each weekCols as c (c.date.getTime())}
              <div class={"tg-col" + (c.weekend ? " weekend" : "")}>
                <!-- click targets per hour -->
                {#each HOURS as h (h)}
                  <button
                    type="button"
                    class="tg-slot"
                    style:top={`${h * HOUR_PX}px`}
                    style:height={`${HOUR_PX}px`}
                    aria-label={t("Create event at {time}", { time: hourLabel(h) })}
                    onclick={() => openCreateAtHour(c.date, h)}
                  ></button>
                {/each}
                <!-- positioned blocks -->
                {#each c.blocks as b (b.e.id)}
                  {@const e = b.e}
                  <button
                    type="button"
                    class={"blk blk--wk" + (completable(e) && e.done ? " blk--done" : "")}
                    style:--blk-color={pillColor(e)}
                    style:top={`${b.top}px`}
                    style:height={`${b.height}px`}
                    style:left={`calc(${b.left * 100}% + 1px)`}
                    style:width={`calc(${b.width * 100}% - 2px)`}
                    onclick={(ev) => openEdit(e, ev)}
                  >
                    <span class="blk-title">{e.title}</span>
                    {#if b.height > 34}<span class="blk-time">{timeLabel(e)}</span>{/if}
                  </button>
                {/each}
                <!-- now-line within today's column -->
                {#if c.isToday}
                  <div class="now-line" style:top={`${nowTop}px`} aria-hidden="true"><span class="now-dot"></span></div>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      </div>
    </div>

  {:else}
    <!-- ===== DAY VIEW (single-column time-grid) ===== -->
    <div class="cal-grid-wrap">
      <!-- all-day strip -->
      {#if dayAllDay.length > 0}
        <div class="tg-allday day-allday">
          <div class="tg-gutter-lbl">{t("All day")}</div>
          <div class="day-allday-list">
            {#each dayAllDay as e (e.id)}
              <button
                type="button"
                class={"ad-pill" + (completable(e) && e.done ? " done" : "")}
                style:--blk-color={pillColor(e)}
                onclick={(ev) => openEdit(e, ev)}
                title={e.title}
              >
                {#if completable(e)}
                  <span
                    class={"ad-check" + (e.done ? " on" : "")}
                    role="checkbox"
                    aria-checked={e.done}
                    aria-label={t("Toggle done")}
                    tabindex="-1"
                    onclick={(ev) => toggleDone(e, ev)}
                    onkeydown={(ev) => { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); toggleDone(e, ev); } }}
                  >
                    {#if e.done}<Icon name="check" size={9} />{/if}
                  </span>
                {/if}
                {#if isDeadline(e)}<span class="ad-glyph" aria-hidden="true">{kindGlyph(e.kind)}</span>{/if}
                <span class="ad-title">{e.title}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- scrollable hour grid -->
      <div class="tg-scroll" bind:this={gridEl}>
        <div class="tg-body" style:height={`${HOUR_PX * 24}px`}>
          {#each HOURS as h (h)}
            <div class="tg-hour" style:top={`${h * HOUR_PX}px`} style:height={`${HOUR_PX}px`}>
              <div class="tg-hour-lbl">{h === 0 ? "" : hourLabel(h)}</div>
            </div>
          {/each}

          <div class="tg-cols tg-cols--day">
            <div class="tg-col">
              {#each HOURS as h (h)}
                <button
                  type="button"
                  class="tg-slot"
                  style:top={`${h * HOUR_PX}px`}
                  style:height={`${HOUR_PX}px`}
                  aria-label={t("Create event at {time}", { time: hourLabel(h) })}
                  onclick={() => openCreateAtHour(selectedDay, h)}
                ></button>
              {/each}

              {#each dayBlocks as b (b.e.id)}
                {@const e = b.e}
                <button
                  type="button"
                  class={"blk blk--day" + (completable(e) && e.done ? " blk--done" : "")}
                  style:--blk-color={pillColor(e)}
                  style:top={`${b.top}px`}
                  style:height={`${b.height}px`}
                  style:left={`calc(${b.left * 100}% + 1px)`}
                  style:width={`calc(${b.width * 100}% - 2px)`}
                  onclick={(ev) => openEdit(e, ev)}
                >
                  <div class="blk-head">
                    {#if completable(e)}
                      <span
                        class={"blk-check" + (e.done ? " on" : "")}
                        role="checkbox"
                        aria-checked={e.done}
                        aria-label={t("Toggle done")}
                        tabindex="-1"
                        onclick={(ev) => toggleDone(e, ev)}
                        onkeydown={(ev) => { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); toggleDone(e, ev); } }}
                      >
                        {#if e.done}<Icon name="check" size={10} />{/if}
                      </span>
                    {/if}
                    {#if isDeadline(e)}<span class="blk-glyph" aria-hidden="true">{kindGlyph(e.kind)}</span>{/if}
                    <span class="blk-title">{e.title}</span>
                  </div>
                  {#if b.height > 32}<div class="blk-time">{timeRange(e)}</div>{/if}
                  {#if e.location && b.height > 56}<div class="blk-loc">{e.location}</div>{/if}
                </button>
              {/each}

              {#if dayIsToday}
                <div class="now-line" style:top={`${nowTop}px`} aria-hidden="true"><span class="now-dot"></span></div>
              {/if}
            </div>
          </div>
        </div>
      </div>
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

  /* ===== HEADER ===== */
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
    gap: 6px;
  }
  .cal-today { margin: 0 2px; }
  .cal-title {
    font-family: var(--font-mono);
    font-size: var(--t-md);
    font-weight: 600;
    color: var(--fg-bright);
    margin-left: 8px;
    white-space: nowrap;
  }
  .cal-head-r {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .cal-modeseg { flex: none; }
  .picker-wrap {
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-3);
    background: var(--surface-2);
    /* No overflow:hidden — it clipped the Picker's dropdown menu (the "All
       subjects dropdown broken" bug). The menu is absolutely positioned and must
       escape this wrapper. */
    min-width: 170px;
  }

  /* ===== MONTH ===== */
  .cal-dow {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    border: 1px solid var(--border);
    border-bottom: none;
    border-radius: var(--rad-3) var(--rad-3) 0 0;
    overflow: hidden;
    background: var(--surface-2);
  }
  .cal-dow-cell {
    padding: 7px 9px;
    font-size: var(--t-2xs);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--fg-muted);
    text-align: left;
  }
  .cal-dow-cell.weekend { color: var(--fg-faint); }

  .cal-grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    grid-template-rows: repeat(6, minmax(0, 1fr));
    border: 1px solid var(--border);
    border-radius: 0 0 var(--rad-3) var(--rad-3);
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
  .cal-cell:nth-child(7n) { border-right: none; }
  .cal-cell:nth-child(n + 36) { border-bottom: none; }
  .cal-cell:hover { background: var(--surface-2); }
  .cal-cell.weekend { background: color-mix(in oklab, var(--bg) 35%, var(--surface)); }
  .cal-cell.weekend:hover { background: var(--surface-2); }
  .cal-cell.out { background: var(--bg); color: var(--fg-faint); }
  .cal-cell.out:hover { background: var(--surface); }
  /* today: accent ring inside the cell */
  .cal-cell.today {
    background: color-mix(in oklab, var(--accent) 7%, var(--surface));
    box-shadow: inset 0 0 0 1.5px color-mix(in oklab, var(--accent) 55%, transparent);
  }

  .cal-cell-head {
    display: flex;
    justify-content: flex-end;
    flex: none;
  }
  .cal-daynum {
    font-family: var(--font-mono);
    font-size: var(--t-xs);
    color: var(--fg-muted);
    min-width: 19px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--rad-2);
    padding: 0 2px;
  }
  .cal-cell.out .cal-daynum { color: var(--fg-faint); opacity: 0.6; }
  .cal-cell.today .cal-daynum {
    background: var(--accent);
    color: var(--accent-fg);
    font-weight: 600;
  }

  .cal-chips {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    min-height: 0;
  }
  /* compact single-line chip */
  .chip {
    appearance: none;
    border: none;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 6px 2px 5px;
    border-radius: var(--rad-2);
    cursor: pointer;
    text-align: left;
    font-family: var(--font-mono);
    font-size: var(--t-2xs);
    line-height: 1.45;
    color: var(--fg-bright);
    background: color-mix(in oklab, var(--chip-color) 16%, var(--surface));
    border-left: 2px solid var(--chip-color);
    transition: background 0.1s ease;
    min-width: 0;
  }
  .chip:hover { background: color-mix(in oklab, var(--chip-color) 30%, var(--surface)); }
  .chip.done { opacity: 0.5; }
  .chip.done .chip-title { text-decoration: line-through; }
  .chip-dot {
    width: 5px; height: 5px; border-radius: 50%;
    background: var(--chip-color); flex: none;
  }
  .chip-glyph { flex: none; font-size: 9.5px; line-height: 1; color: var(--chip-color); }
  .chip-check {
    width: 11px; height: 11px; border-radius: 3px;
    border: 1.5px solid var(--chip-color);
    display: flex; align-items: center; justify-content: center;
    flex: none; color: var(--accent-fg); cursor: pointer;
  }
  .chip-check.on { background: var(--chip-color); }
  .chip-time { color: var(--fg-muted); flex: none; }
  .chip-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .chip-more {
    appearance: none; border: none; background: transparent;
    font-family: var(--font-mono); font-size: var(--t-2xs);
    color: var(--fg-muted); cursor: pointer; text-align: left;
    padding: 1px 5px; border-radius: var(--rad-2);
  }
  .chip-more:hover { color: var(--fg-bright); background: var(--surface-2); }

  .cal-empty {
    margin-top: 12px;
    text-align: center;
    font-size: var(--t-xs);
    color: var(--fg-faint);
  }

  /* ===== SHARED TIME-GRID (week + day) ===== */
  .cal-grid-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--rad-3);
    overflow: hidden;
    background: var(--surface);
    --gutter: 58px;
  }

  /* weekday column headers (week view) */
  .tg-colhead {
    display: grid;
    grid-template-columns: var(--gutter) repeat(7, 1fr);
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex: none;
  }
  .tg-gutter-head { border-right: 1px solid var(--border); }
  .tg-day-head {
    appearance: none;
    border: none;
    border-left: 1px solid var(--border);
    background: transparent;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1px;
    padding: 7px 4px;
    transition: background 0.1s ease;
  }
  .tg-day-head:hover { background: var(--surface-3); }
  .tg-day-head.weekend { background: color-mix(in oklab, var(--bg) 30%, transparent); }
  .tg-dow {
    font-family: var(--font-mono);
    font-size: var(--t-2xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .tg-day-head.weekend .tg-dow { color: color-mix(in oklab, var(--fg-faint) 75%, transparent); }
  .tg-dnum {
    font-family: var(--font-mono);
    font-size: var(--t-md);
    font-weight: 600;
    color: var(--fg-muted);
    width: 24px; height: 24px;
    display: inline-flex; align-items: center; justify-content: center;
    border-radius: var(--rad-pill);
  }
  .tg-day-head.today .tg-dow { color: var(--accent); }
  .tg-day-head.today .tg-dnum {
    background: var(--accent);
    color: var(--accent-fg);
  }

  /* all-day strip */
  .tg-allday {
    display: grid;
    grid-template-columns: var(--gutter) repeat(7, 1fr);
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    max-height: 92px;
    overflow-y: auto;
    flex: none;
  }
  .day-allday { grid-template-columns: var(--gutter) 1fr; align-items: flex-start; padding: 6px 0; }
  .tg-gutter-lbl {
    grid-column: 1;
    padding: 5px 8px 0 0;
    text-align: right;
    font-family: var(--font-mono);
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    border-right: 1px solid var(--border);
  }
  .day-allday .tg-gutter-lbl { border-right: none; padding-top: 2px; }
  .tg-allday-col {
    border-left: 1px solid var(--border);
    padding: 5px 4px;
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .tg-allday-col.weekend { background: color-mix(in oklab, var(--bg) 30%, transparent); }
  .day-allday-list {
    display: flex; flex-wrap: wrap; gap: 4px;
    padding: 0 10px; min-width: 0;
  }
  .ad-pill {
    appearance: none; border: none;
    display: inline-flex; align-items: center; gap: 5px;
    max-width: 100%;
    padding: 3px 8px;
    border-radius: var(--rad-2);
    cursor: pointer;
    font-family: var(--font-mono);
    font-size: var(--t-2xs);
    color: var(--fg-bright);
    background: color-mix(in oklab, var(--blk-color) 22%, var(--surface));
    border-left: 2px solid var(--blk-color);
    transition: background 0.1s ease;
    min-width: 0;
  }
  .ad-pill:hover { background: color-mix(in oklab, var(--blk-color) 34%, var(--surface)); }
  .ad-pill.done { opacity: 0.5; }
  .ad-pill.done .ad-title { text-decoration: line-through; }
  .ad-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .ad-glyph { flex: none; font-size: 10px; line-height: 1; color: var(--blk-color); }
  .ad-check {
    width: 12px; height: 12px; border-radius: 3px;
    border: 1.5px solid var(--blk-color);
    display: flex; align-items: center; justify-content: center;
    flex: none; color: var(--accent-fg); cursor: pointer;
  }
  .ad-check.on { background: var(--blk-color); }

  /* scroll body */
  .tg-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    position: relative;
  }
  .tg-body { position: relative; width: 100%; }

  /* hour rule + gutter label */
  .tg-hour {
    position: absolute;
    left: 0;
    right: 0;
    border-top: 1px solid var(--border);
  }
  .tg-hour-lbl {
    position: absolute;
    left: 0;
    top: -7px;
    width: var(--gutter);
    padding-right: 8px;
    text-align: right;
    font-family: var(--font-mono);
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    user-select: none;
  }

  /* day columns layer (sits to the right of the gutter) */
  .tg-cols {
    position: absolute;
    left: var(--gutter);
    right: 0;
    top: 0;
    bottom: 0;
    display: grid;
    grid-template-columns: repeat(7, 1fr);
  }
  .tg-cols--day { grid-template-columns: 1fr; }
  .tg-col {
    position: relative;
    border-left: 1px solid var(--border);
    min-width: 0;
  }
  .tg-col.weekend { background: color-mix(in oklab, var(--bg) 22%, transparent); }

  /* empty hour slot (click to create) */
  .tg-slot {
    appearance: none;
    border: none;
    background: transparent;
    position: absolute;
    left: 0;
    right: 0;
    cursor: pointer;
    transition: background 0.1s ease;
  }
  .tg-slot:hover { background: color-mix(in oklab, var(--accent) 7%, transparent); }

  /* positioned event block */
  .blk {
    appearance: none;
    position: absolute;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 3px 6px;
    border-radius: var(--rad-2);
    overflow: hidden;
    cursor: pointer;
    text-align: left;
    z-index: 2;
    background: color-mix(in oklab, var(--blk-color) 26%, var(--surface));
    border-left: 3px solid var(--blk-color);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
    transition: background 0.1s ease;
  }
  .blk:hover { background: color-mix(in oklab, var(--blk-color) 38%, var(--surface)); z-index: 3; }
  .blk--done { opacity: 0.55; }
  .blk--done .blk-title { text-decoration: line-through; }
  .blk--wk { padding: 2px 5px; }
  .blk-head { display: flex; align-items: center; gap: 5px; min-width: 0; }
  .blk-glyph { flex: none; font-size: 10px; line-height: 1; color: var(--blk-color); }
  .blk-title {
    font-family: var(--font-mono);
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--fg-bright);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .blk--wk .blk-title { font-size: var(--t-2xs); }
  .blk-check {
    width: 13px; height: 13px; border-radius: 3px;
    border: 1.5px solid var(--blk-color);
    display: flex; align-items: center; justify-content: center;
    flex: none; color: var(--accent-fg); cursor: pointer;
  }
  .blk-check.on { background: var(--blk-color); }
  .blk-time {
    font-family: var(--font-mono);
    font-size: var(--t-2xs);
    color: var(--fg-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .blk-loc {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* now-line */
  .now-line {
    position: absolute;
    left: -1px;
    right: 0;
    height: 0;
    border-top: 2px solid var(--err);
    z-index: 5;
    pointer-events: none;
  }
  .now-dot {
    position: absolute;
    left: -4px;
    top: -5px;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--err);
  }

  /* responsive: tighten gutter + header on narrow shells */
  @media (max-width: 860px) {
    .cal { padding: 16px; }
    .cal-grid-wrap { --gutter: 48px; }
    .cal-title { margin-left: 4px; font-size: var(--t-sm); }
  }
</style>
