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
  function dayNum(iso: string): string {
    return String(Number(iso.split("-")[2]));
  }

  // ── study-minutes bar chart geometry (pure SVG, no chart lib) ──
  // A fixed 0..N viewBox the bars are laid out in; CSS scales it responsively.
  const CH_W = 720;
  const CH_H = 160;
  const PAD_B = 22; // room for the baseline date labels
  const days = $derived(data?.minutes_per_day ?? []);
  const maxMinutes = $derived(Math.max(1, ...days.map((d) => d.minutes)));
  const barGap = 3;
  const barW = $derived(days.length ? (CH_W - barGap * (days.length - 1)) / days.length : 0);
  function barH(min: number): number {
    return ((CH_H - PAD_B) * min) / maxMinutes;
  }
  // Label only a few x-ticks (first, ~middle, last) to avoid a crowded axis.
  function showTick(i: number): boolean {
    const n = days.length;
    return n > 0 && (i === 0 || i === n - 1 || i === Math.floor(n / 2));
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

  // Total minutes studied across the whole window (chart caption).
  const windowMinutes = $derived(days.reduce((a, d) => a + d.minutes, 0));
</script>

<div class="workspace-scroll">
  <div class="dash an">
    <header class="dash-head">
      <div>
        <div class="eyebrow">Insights</div>
        <h1 class="dash-title read">Study analytics</h1>
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

      <!-- ── daily study minutes (last 30 days) ── -->
      <section class="an-card">
        <div class="an-card-h">
          <h3 class="an-card-t mono">Study minutes · last 30 days</h3>
          <span class="an-card-sub mono">{fmtMins(windowMinutes)} total</span>
        </div>
        <svg
          class="an-chart"
          viewBox="0 0 {CH_W} {CH_H}"
          preserveAspectRatio="none"
          role="img"
          aria-label="Daily study minutes for the last 30 days"
        >
          {#each days as d, i (d.day)}
            {@const h = barH(d.minutes)}
            {@const x = i * (barW + barGap)}
            <rect
              class="an-bar"
              x={x}
              y={CH_H - PAD_B - h}
              width={barW}
              height={Math.max(0, h)}
              rx="1.5"
            >
              <title>{d.day}: {fmtMins(d.minutes)}</title>
            </rect>
            {#if showTick(i)}
              <text class="an-axis" x={x + barW / 2} y={CH_H - 6} text-anchor="middle">
                {dayNum(d.day)}
              </text>
            {/if}
          {/each}
        </svg>
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
  @media (max-width: 760px) {
    .an-stats {
      grid-template-columns: repeat(2, 1fr);
    }
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

  /* ── study-minutes bar chart ── */
  .an-chart {
    display: block;
    width: 100%;
    height: 160px;
  }
  .an-bar {
    fill: var(--accent);
    opacity: 0.78;
    transition: opacity var(--dur-fast) var(--ease);
  }
  .an-bar:hover {
    opacity: 1;
  }
  .an-axis {
    fill: var(--fg-faint);
    font-family: var(--font-mono);
    font-size: 11px;
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
</style>
