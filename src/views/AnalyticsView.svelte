<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";

  // The whole dashboard arrives in ONE call (see repo::analytics_summary) so a
  // single DB lock backs every chart below. Reloaded when the view mounts and
  // whenever the active subject changes (cheap; keeps figures fresh after study).
  let data = $state<api.AnalyticsSummary | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      data = await api.analyticsSummary(30);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    load();
  });

  // Subject lookups come from the already-loaded subject list, not extra queries.
  function subjectName(id: string): string {
    return app.subjects.find((s) => s.id === id)?.name ?? "Unknown subject";
  }
  function subjectColor(id: string): string {
    return app.subjectColor(app.subjects.find((s) => s.id === id) ?? null);
  }

  // ── headline figures ──
  const streak = $derived(data?.streak ?? 0);
  const minutesWeek = $derived(data?.minutes_week ?? 0);
  const reviewsWeek = $derived(data?.reviews_week ?? 0);
  const accuracyWeek = $derived(data?.accuracy_week ?? 0);

  // Format minutes as "Hh Mm" / "Mm" so the cards read naturally.
  function fmtMins(m: number): string {
    if (m <= 0) return "0m";
    const h = Math.floor(m / 60);
    const min = m % 60;
    return h > 0 ? (min > 0 ? `${h}h ${min}m` : `${h}h`) : `${min}m`;
  }
  const pct = (x: number) => `${Math.round(x * 100)}%`;

  // Short weekday/day label for an ISO "YYYY-MM-DD" (local date string).
  function dayLabel(iso: string): string {
    const [y, m, d] = iso.split("-").map(Number);
    return new Date(y, m - 1, d).toLocaleDateString(undefined, { weekday: "short" });
  }

  // ── focus-hours contributions heatmap (GitHub-style; pure CSS grid) ──
  // The backend hands us a full rolling year (366 days, oldest → newest, gaps
  // already filled with 0). We bucket it into week-columns of 7 day-rows
  // (Mon–Sun), today in the last column, the first column padded so each row is
  // a fixed weekday. No chart library — just a CSS grid of cells.
  type Cell = { date: string; minutes: number; level: number } | null;
  const yearDays = $derived(data?.year_minutes ?? []);

  // Weekday index with Monday = 0 … Sunday = 6 (JS getDay() has Sunday = 0).
  function mondayIdx(iso: string): number {
    const [y, m, d] = iso.split("-").map(Number);
    return (new Date(y, m - 1, d).getDay() + 6) % 7;
  }

  // Intensity thresholds from the user's OWN nonzero distribution (quartiles of
  // the max), so a light user still sees 4 shades rather than one faint band.
  const yearMax = $derived(Math.max(0, ...yearDays.map((d) => d.minutes)));
  function levelFor(min: number): number {
    if (min <= 0 || yearMax <= 0) return 0;
    const q = min / yearMax;
    if (q <= 0.25) return 1;
    if (q <= 0.5) return 2;
    if (q <= 0.75) return 3;
    return 4;
  }

  // Build week-columns. Pad the FIRST column with leading nulls so the oldest
  // day lands on its true weekday row (keeps Mon/Wed/Fri labels aligned).
  const weeks = $derived.by<Cell[][]>(() => {
    if (!yearDays.length) return [];
    const cols: Cell[][] = [];
    let col: Cell[] = [];
    const firstRow = mondayIdx(yearDays[0].day);
    for (let i = 0; i < firstRow; i++) col.push(null); // pad partial first week
    for (const d of yearDays) {
      col.push({ date: d.day, minutes: d.minutes, level: levelFor(d.minutes) });
      if (col.length === 7) {
        cols.push(col);
        col = [];
      }
    }
    if (col.length) {
      while (col.length < 7) col.push(null); // pad trailing partial week
      cols.push(col);
    }
    return cols;
  });

  // Month labels along the top: the column index where each month first appears
  // (placed above that week so labels read like GitHub's).
  const monthLabels = $derived.by<{ col: number; label: string }[]>(() => {
    const out: { col: number; label: string }[] = [];
    let lastMonth = -1;
    weeks.forEach((wk, ci) => {
      // Use the first real cell in the column to date the week.
      const cell = wk.find((c) => c !== null);
      if (!cell) return;
      const month = Number(cell.date.split("-")[1]) - 1;
      if (month !== lastMonth) {
        // Skip a label crammed into the very first partial column (GitHub does too).
        if (ci > 0 || mondayIdx(cell.date) === 0) {
          out.push({ col: ci, label: MONTHS[month] });
        }
        lastMonth = month;
      }
    });
    return out;
  });
  const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

  const yearMinutesTotal = $derived(yearDays.reduce((a, d) => a + d.minutes, 0));
  const yearHours = $derived(yearMinutesTotal / 60);
  const heatmapEmpty = $derived(yearDays.length > 0 && yearMinutesTotal === 0);

  // Tooltip — a positioned div (not a title attr), shown on cell hover.
  let tip = $state<{ x: number; y: number; text: string } | null>(null);
  // "2.4h · Tue 14 May" — hours with one decimal when ≥60m, else "45m".
  function tipLabel(c: NonNullable<Cell>): string {
    const [y, m, d] = c.date.split("-").map(Number);
    const dt = new Date(y, m - 1, d);
    const when = dt.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
    const amount = c.minutes >= 60 ? `${(c.minutes / 60).toFixed(1)}h` : `${c.minutes}m`;
    return `${amount} · ${when}`;
  }
  function showTip(e: MouseEvent, c: Cell) {
    if (!c) return;
    const cell = e.currentTarget as HTMLElement;
    const wrap = cell.closest(".hm-card") as HTMLElement | null;
    if (!wrap) return;
    const cr = cell.getBoundingClientRect();
    const wr = wrap.getBoundingClientRect();
    // Position relative to the card so the tooltip rides horizontal scroll.
    tip = {
      x: cr.left - wr.left + cr.width / 2 + wrap.scrollLeft,
      y: cr.top - wr.top - 6,
      text: tipLabel(c),
    };
  }
  function hideTip() {
    tip = null;
  }

  // ── due-forecast mini bars (next 7 days) ──
  const forecast = $derived(data?.due_forecast ?? []);
  const maxDue = $derived(Math.max(1, ...forecast.map((d) => d.due)));

  // Whole-dashboard empty state: nothing studied AND nothing scheduled.
  const isEmpty = $derived(
    !!data &&
      data.minutes_week === 0 &&
      data.reviews_week === 0 &&
      data.fsrs.cards === 0 &&
      data.per_subject.length === 0 &&
      data.streak === 0
  );

  // ── topics needing the most work ──
  const weakTopics = $derived(data?.weak_topics ?? []);
  function weakSubjectName(t: api.WeakTopic): string {
    return subjectName(t.subject_id);
  }
</script>

<div class="workspace-scroll">
  <div class="dash an">
    <header class="dash-head">
      <div>
        <div class="eyebrow">Insights</div>
        <h1 class="dash-title">Study analytics</h1>
      </div>
    </header>

    {#if loading}
      <div class="an-note">Loading your study data…</div>
    {:else if error}
      <div class="an-note an-note--err">Couldn't load analytics: {error}</div>
    {:else if isEmpty}
      <div class="an-empty">
        <div class="an-empty-glyph">📊</div>
        <div class="an-empty-t read">No study data yet</div>
        <div class="an-empty-d">
          Start a pomodoro or review some cards — your focus minutes, review
          accuracy and upcoming due cards will show up here.
        </div>
      </div>
    {:else if data}
      <!-- ── headline stat cards ── -->
      <div class="an-stats">
        <div class="an-stat">
          <div class="an-stat-k mono">Current streak</div>
          <div class="an-stat-v">{streak}<span class="an-stat-u"> day{streak === 1 ? "" : "s"}</span></div>
        </div>
        <div class="an-stat">
          <div class="an-stat-k mono">Focus this week</div>
          <div class="an-stat-v">{fmtMins(minutesWeek)}</div>
        </div>
        <div class="an-stat">
          <div class="an-stat-k mono">Reviews this week</div>
          <div class="an-stat-v">{reviewsWeek}</div>
        </div>
        <div class="an-stat">
          <div class="an-stat-k mono">Accuracy this week</div>
          <div class="an-stat-v">{reviewsWeek > 0 ? pct(accuracyWeek) : "—"}</div>
        </div>
      </div>

      <!-- ── focus-hours contributions heatmap (last year) ── -->
      <section class="an-card hm-card">
        <div class="an-card-h">
          <h3 class="an-card-t mono">Focus hours · last year</h3>
          <span class="an-card-sub mono">{yearHours.toFixed(1)}h total</span>
        </div>

        <!-- Scroll wrapper: the full year is ~53 weeks wide; if it can't fit the
             panel it scrolls horizontally inside the card rather than the page. -->
        <div class="hm-scroll">
          <div class="hm-grid-wrap">
            <!-- month labels along the top edge -->
            <div class="hm-months">
              {#each monthLabels as ml (ml.col)}
                <span class="hm-month mono" style:grid-column={ml.col + 2}>{ml.label}</span>
              {/each}
            </div>

            <div class="hm-body">
              <!-- weekday labels (Mon/Wed/Fri) down the left, faint mono -->
              <div class="hm-wd">
                <span class="hm-wd-l mono" style:grid-row="2">Mon</span>
                <span class="hm-wd-l mono" style:grid-row="4">Wed</span>
                <span class="hm-wd-l mono" style:grid-row="6">Fri</span>
              </div>

              <div class="hm-grid" role="img" aria-label="Focus hours over the last year">
                {#each weeks as wk, ci (ci)}
                  {#each wk as cell, ri (ri)}
                    {#if cell}
                      <!-- svelte-ignore a11y_no_static_element_interactions -->
                      <div
                        class="hm-cell"
                        data-lvl={cell.level}
                        style:grid-column={ci + 1}
                        style:grid-row={ri + 1}
                        onmouseenter={(e) => showTip(e, cell)}
                        onmouseleave={hideTip}
                      ></div>
                    {:else}
                      <div class="hm-cell hm-cell--pad" style:grid-column={ci + 1} style:grid-row={ri + 1}></div>
                    {/if}
                  {/each}
                {/each}
              </div>
            </div>
          </div>

          {#if tip}
            <div class="hm-tip mono" style:left="{tip.x}px" style:top="{tip.y}px">{tip.text}</div>
          {/if}
        </div>

        <!-- legend bottom-right + (when empty) the tracking hint bottom-left -->
        <div class="hm-foot">
          {#if heatmapEmpty}
            <span class="hm-hint">No study time tracked yet — focused app time counts automatically now.</span>
          {:else}
            <span></span>
          {/if}
          <div class="hm-legend mono">
            less
            <span class="hm-cell hm-leg" data-lvl="0"></span>
            <span class="hm-cell hm-leg" data-lvl="1"></span>
            <span class="hm-cell hm-leg" data-lvl="2"></span>
            <span class="hm-cell hm-leg" data-lvl="3"></span>
            <span class="hm-cell hm-leg" data-lvl="4"></span>
            more
          </div>
        </div>
      </section>

      <!-- ── due forecast (next 7 days) ── -->
      <section class="an-card">
        <div class="an-card-h">
          <h3 class="an-card-t mono">Cards due · next 7 days</h3>
          <span class="an-card-sub mono">
            {forecast.reduce((a, d) => a + d.due, 0)} scheduled
          </span>
        </div>
        <div class="an-forecast">
          {#each forecast as d, i (d.day)}
            <div class="an-fc">
              <div class="an-fc-track">
                <div
                  class="an-fc-fill"
                  style:height={pct(d.due / maxDue)}
                  class:zero={d.due === 0}
                ></div>
              </div>
              <div class="an-fc-n mono">{d.due}</div>
              <div class="an-fc-d mono">{i === 0 ? "Today" : dayLabel(d.day)}</div>
            </div>
          {/each}
        </div>
      </section>

      <!-- ── per-subject breakdown ── -->
      {#if data.per_subject.length}
        <section class="an-card">
          <div class="an-card-h">
            <h3 class="an-card-t mono">By subject</h3>
          </div>
          <div class="an-table">
            <div class="an-tr an-tr--head mono">
              <span class="an-th an-th--name">Subject</span>
              <span class="an-th">Focus</span>
              <span class="an-th">Reviews</span>
              <span class="an-th">Accuracy</span>
            </div>
            {#each data.per_subject as s (s.subject_id)}
              <div class="an-tr">
                <span class="an-td an-td--name">
                  <span class="an-dot" style:background={subjectColor(s.subject_id)}></span>
                  <span class="an-name read">{subjectName(s.subject_id)}</span>
                </span>
                <span class="an-td mono">{fmtMins(s.minutes)}</span>
                <span class="an-td mono">{s.reviews}</span>
                <span class="an-td mono">{s.reviews > 0 ? pct(s.accuracy) : "—"}</span>
              </div>
            {/each}
          </div>
        </section>
      {/if}

      <!-- ── topics needing the most work ── -->
      {#if weakTopics.length}
        <section class="an-card">
          <div class="an-card-h">
            <h3 class="an-card-t mono">Topics needing work</h3>
            <span class="an-card-sub mono">weakest first</span>
          </div>
          <div class="an-weak">
            {#each weakTopics as t (t.topic_id)}
              <div class="an-weak-row">
                <span class="an-dot" style:background={subjectColor(t.subject_id)}></span>
                <div class="an-weak-main">
                  <div class="an-weak-name read">
                    <span class="an-weak-subj">{weakSubjectName(t)}</span>
                    <span class="an-weak-sep">·</span>
                    {t.topic_name}
                  </div>
                  <div class="an-weak-reason mono">{t.reason}</div>
                </div>
                <div class="an-weak-stats mono">
                  {#if t.reviews > 0}<span class="an-weak-stat">{pct(t.accuracy)} acc</span>{/if}
                  {#if t.lapses > 0}<span class="an-weak-stat">{t.lapses} lapse{t.lapses === 1 ? "" : "s"}</span>{/if}
                </div>
              </div>
            {/each}
          </div>
        </section>
      {/if}

      <!-- ── FSRS memory state ── -->
      <section class="an-card">
        <div class="an-card-h">
          <h3 class="an-card-t mono">Spaced-repetition memory</h3>
        </div>
        <div class="an-fsrs">
          <div class="an-fsrs-stat">
            <div class="an-fsrs-v">{data.fsrs.cards}</div>
            <div class="an-fsrs-k mono">cards scheduled</div>
          </div>
          <div class="an-fsrs-stat">
            <div class="an-fsrs-v">
              {data.fsrs.cards > 0 ? data.fsrs.avg_stability.toFixed(1) : "—"}<span class="an-stat-u">{data.fsrs.cards > 0 ? " d" : ""}</span>
            </div>
            <div class="an-fsrs-k mono">avg stability</div>
          </div>
          <div class="an-fsrs-stat">
            <div class="an-fsrs-v">{data.fsrs.lapses}</div>
            <div class="an-fsrs-k mono">lapses</div>
          </div>
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  /* The view reuses .workspace-scroll / .dash / .dash-head / .eyebrow /
     .dash-title from the design system (see Dashboard.svelte); everything below
     is analytics-specific, styled purely with the shared tokens. */
  .an {
    display: flex;
    flex-direction: column;
    gap: var(--sp-6);
  }

  .an-note {
    padding: 40px 0;
    text-align: center;
    color: var(--fg-faint);
    font-size: var(--t-sm);
  }
  .an-note--err {
    color: var(--err);
  }

  /* ── empty state ── */
  .an-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 56px 24px;
    text-align: center;
  }
  .an-empty-glyph {
    font-size: 40px;
    opacity: 0.85;
  }
  .an-empty-t {
    font-size: var(--r-lg);
    color: var(--fg-bright);
  }
  .an-empty-d {
    max-width: 420px;
    font-size: var(--t-sm);
    color: var(--fg-muted);
    line-height: 1.5;
  }

  /* ── headline stat cards ── */
  .an-stats {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: var(--sp-4);
  }
  /* Reflow 4→2 under ~1000px, 2→1 under ~560px — same cards, never crushed. */
  @media (max-width: 1000px) {
    .an-stats { grid-template-columns: repeat(2, 1fr); }
  }
  @media (max-width: 560px) {
    .an-stats { grid-template-columns: 1fr; }
  }
  .an-stat {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--rad-3);
    padding: 14px 16px;
  }
  .an-stat-k {
    font-size: var(--t-2xs);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }
  .an-stat-v {
    margin-top: 8px;
    font-size: var(--r-xl);
    font-weight: 600;
    color: var(--fg-bright);
    font-variant-numeric: tabular-nums;
  }
  .an-stat-u {
    font-size: var(--t-sm);
    font-weight: 400;
    color: var(--fg-faint);
  }

  /* ── card frame shared by chart / forecast / table / fsrs ── */
  .an-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--rad-3);
    padding: 16px;
  }
  .an-card-h {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 14px;
  }
  .an-card-t {
    font-size: var(--t-xs);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .an-card-sub {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
  }

  /* ── focus-hours contributions heatmap ── */
  /* Cell sizing — kept as custom props so cells, gaps and label tracks stay in
     lockstep (the weekday-label rows must match the grid rows exactly). */
  .hm-card {
    --hm-cell: 11px;
    --hm-gap: 3px;
    --hm-wd: 26px; /* width of the weekday-label gutter */
  }
  /* The grid can be ~53 weeks wide; scroll inside the card, never the page. */
  .hm-scroll {
    position: relative;
    overflow-x: auto;
    overflow-y: hidden;
    padding-bottom: 2px;
    scrollbar-width: thin;
    scrollbar-color: var(--border-strong) transparent;
  }
  .hm-scroll::-webkit-scrollbar {
    height: 7px;
  }
  .hm-scroll::-webkit-scrollbar-thumb {
    background: var(--border-strong);
    border-radius: var(--rad-pill);
  }
  .hm-grid-wrap {
    /* Just wide enough for the whole year so the scroll wrapper can pan it. */
    width: max-content;
  }

  /* month labels: a single-row grid whose columns mirror the heatmap columns,
     offset by 1 to clear the weekday gutter. */
  .hm-months {
    display: grid;
    grid-auto-columns: calc(var(--hm-cell) + var(--hm-gap));
    grid-auto-flow: column;
    margin-left: var(--hm-wd);
    height: 15px;
  }
  .hm-month {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    white-space: nowrap;
  }

  .hm-body {
    display: flex;
    gap: 0;
  }
  /* weekday labels aligned to grid rows via a matching 7-row grid */
  .hm-wd {
    display: grid;
    grid-template-rows: repeat(7, var(--hm-cell));
    grid-auto-flow: row;
    gap: var(--hm-gap);
    width: var(--hm-wd);
    padding-right: 4px;
  }
  .hm-wd-l {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    line-height: var(--hm-cell);
    align-self: center;
  }

  .hm-grid {
    display: grid;
    grid-auto-flow: column;
    grid-template-rows: repeat(7, var(--hm-cell));
    grid-auto-columns: var(--hm-cell);
    gap: var(--hm-gap);
  }
  .hm-cell {
    width: var(--hm-cell);
    height: var(--hm-cell);
    border-radius: 2px;
    background: var(--surface-2);
    transition: outline-color var(--dur-fast) var(--ease);
    outline: 1px solid transparent;
  }
  .hm-cell[data-lvl="1"] { background: color-mix(in oklab, var(--accent) 25%, var(--surface-2)); }
  .hm-cell[data-lvl="2"] { background: color-mix(in oklab, var(--accent) 50%, var(--surface-2)); }
  .hm-cell[data-lvl="3"] { background: color-mix(in oklab, var(--accent) 75%, var(--surface-2)); }
  .hm-cell[data-lvl="4"] { background: var(--accent); }
  .hm-cell:not(.hm-cell--pad):hover {
    outline-color: var(--fg-muted);
  }
  .hm-cell--pad {
    background: transparent;
  }

  /* tooltip: positioned div anchored to the card (rides horizontal scroll) */
  .hm-tip {
    position: absolute;
    transform: translate(-50%, -100%);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-2);
    padding: 4px 8px;
    font-size: var(--t-2xs);
    color: var(--fg-bright);
    white-space: nowrap;
    pointer-events: none;
    box-shadow: var(--shadow-pop, 0 4px 14px rgba(0, 0, 0, 0.3));
    z-index: 5;
  }

  .hm-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-top: 12px;
  }
  .hm-hint {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
  }
  .hm-legend {
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    margin-left: auto;
  }
  .hm-leg {
    /* legend swatches are independent of the grid sizing vars */
    width: 11px;
    height: 11px;
  }

  /* ── due forecast (7 mini columns) ── */
  .an-forecast {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 8px;
    align-items: end;
  }
  .an-fc {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }
  .an-fc-track {
    width: 100%;
    height: 84px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    background: var(--bg-sunken);
    border-radius: var(--rad-2);
    overflow: hidden;
  }
  .an-fc-fill {
    width: 100%;
    min-height: 2px;
    background: var(--accent);
    opacity: 0.78;
    border-radius: var(--rad-2) var(--rad-2) 0 0;
    transition: height var(--dur) var(--ease);
  }
  .an-fc-fill.zero {
    background: var(--border-strong);
    opacity: 0.5;
    min-height: 2px;
  }
  .an-fc-n {
    font-size: var(--t-sm);
    color: var(--fg-bright);
    font-variant-numeric: tabular-nums;
  }
  .an-fc-d {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
  }

  /* ── per-subject table ── */
  .an-table {
    display: flex;
    flex-direction: column;
  }
  .an-tr {
    display: grid;
    grid-template-columns: 2fr 1fr 1fr 1fr;
    align-items: center;
    gap: 10px;
    padding: 10px 4px;
    border-top: 1px solid var(--border);
  }
  .an-tr--head {
    border-top: none;
    padding-top: 0;
  }
  .an-th {
    font-size: var(--t-2xs);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-faint);
    text-align: right;
  }
  .an-th--name {
    text-align: left;
  }
  .an-td {
    font-size: var(--t-sm);
    color: var(--fg-muted);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .an-td--name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    text-align: left;
  }
  .an-dot {
    flex: none;
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
  .an-name {
    color: var(--fg-bright);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ── FSRS memory state ── */
  .an-fsrs {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--sp-4);
  }
  .an-fsrs-stat {
    text-align: center;
    padding: 8px 0;
  }
  .an-fsrs-v {
    font-size: var(--r-xl);
    font-weight: 600;
    color: var(--fg-bright);
    font-variant-numeric: tabular-nums;
  }
  .an-fsrs-k {
    margin-top: 6px;
    font-size: var(--t-2xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-faint);
  }

  /* ── topics needing work ── */
  .an-weak {
    display: flex;
    flex-direction: column;
  }
  .an-weak-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 4px;
    border-top: 1px solid var(--border);
  }
  .an-weak-row:first-child {
    border-top: none;
  }
  .an-weak-main {
    flex: 1;
    min-width: 0;
  }
  .an-weak-name {
    font-size: var(--t-md);
    color: var(--fg-bright);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .an-weak-subj {
    color: var(--fg-muted);
  }
  .an-weak-sep {
    color: var(--fg-faint);
    margin: 0 2px;
  }
  .an-weak-reason {
    margin-top: 3px;
    font-size: var(--t-2xs);
    color: var(--fg-faint);
  }
  .an-weak-stats {
    flex: none;
    display: flex;
    gap: 8px;
  }
  .an-weak-stat {
    font-size: var(--t-2xs);
    color: var(--warn);
    background: color-mix(in oklab, var(--warn) 12%, transparent);
    border-radius: var(--rad-pill);
    padding: 2px 8px;
    white-space: nowrap;
  }
</style>
