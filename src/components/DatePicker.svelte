<script lang="ts">
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n.svelte";

  // Themed date (and optional time) picker — a trigger button + a month-grid
  // popover. Replaces the native <input type="date">/<datetime-local> whose
  // dropdowns are OS-rendered and unstyleable. `value` is an ISO string:
  //   withTime=false → "YYYY-MM-DD"
  //   withTime=true  → "YYYY-MM-DDTHH:MM"
  let {
    value,
    onChange,
    placeholder = t("Pick a date"),
    withTime = false,
  }: {
    value: string;
    onChange: (v: string) => void;
    placeholder?: string;
    withTime?: boolean;
  } = $props();

  const DOW = [t("Su"), t("Mo"), t("Tu"), t("We"), t("Th"), t("Fr"), t("Sa")];
  const MONTHS = [t("January"),t("February"),t("March"),t("April"),t("May"),t("June"),t("July"),t("August"),t("September"),t("October"),t("November"),t("December")];

  let open = $state(false);

  function parse(v: string): Date | null {
    const m = /^(\d{4})-(\d{2})-(\d{2})(?:T(\d{2}):(\d{2}))?/.exec(v);
    if (!m) return null;
    return new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]), Number(m[4] ?? 0), Number(m[5] ?? 0));
  }
  function emit(y: number, mo: number, d: number, h: number, mi: number) {
    const date = `${y}-${String(mo + 1).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
    onChange(withTime ? `${date}T${String(h).padStart(2, "0")}:${String(mi).padStart(2, "0")}` : date);
  }
  function sameDay(a: Date, b: Date): boolean {
    return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
  }

  let viewY = $state(0);
  let viewM = $state(0);
  let hh = $state(9);
  let mi = $state(0);
  function openPicker() {
    const s = parse(value) ?? new Date();
    viewY = s.getFullYear();
    viewM = s.getMonth();
    if (withTime) { hh = s.getHours(); mi = s.getMinutes(); }
    open = true;
  }
  function prevMonth() { if (viewM === 0) { viewM = 11; viewY -= 1; } else viewM -= 1; }
  function nextMonth() { if (viewM === 11) { viewM = 0; viewY += 1; } else viewM += 1; }

  const today = new Date();
  const selected = $derived(parse(value));

  const cells = $derived.by(() => {
    const first = new Date(viewY, viewM, 1);
    const start = new Date(viewY, viewM, 1 - first.getDay());
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
      return { d, inMonth: d.getMonth() === viewM };
    });
  });

  const label = $derived.by(() => {
    const d = parse(value);
    if (!d) return placeholder;
    const date = d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
    if (!withTime) return date;
    const time = d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
    return `${date} · ${time}`;
  });

  // The day currently selected in the grid (so picking a day keeps the chosen time).
  let chosenDay = $state<Date | null>(null);
  function pickDay(d: Date) {
    if (withTime) {
      chosenDay = d;
      emit(d.getFullYear(), d.getMonth(), d.getDate(), hh, mi);
      // keep open so the user can adjust the time
    } else {
      emit(d.getFullYear(), d.getMonth(), d.getDate(), 0, 0);
      open = false;
    }
  }
  function commitTime() {
    const base = chosenDay ?? parse(value) ?? new Date();
    hh = Math.max(0, Math.min(23, Math.round(hh) || 0));
    mi = Math.max(0, Math.min(59, Math.round(mi) || 0));
    emit(base.getFullYear(), base.getMonth(), base.getDate(), hh, mi);
  }
</script>

<div class="dp">
  <button type="button" class={"dp-btn input" + (value ? "" : " is-empty")} onclick={openPicker}>
    <Icon name="calendar" size={13} color="var(--fg-faint)" />
    <span class="dp-label">{label}</span>
    <span class="dp-caret"><Icon name="chevron" size={11} /></span>
  </button>

  {#if open}
    <div class="dp-pop" role="dialog" aria-label={t("Choose a date")}>
      <div class="dp-head">
        <button type="button" class="dp-nav" aria-label={t("Previous month")} onclick={prevMonth}>
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={12} /></span>
        </button>
        <span class="dp-title">{MONTHS[viewM]} {viewY}</span>
        <button type="button" class="dp-nav" aria-label={t("Next month")} onclick={nextMonth}>
          <Icon name="chevron" size={12} />
        </button>
      </div>
      <div class="dp-dow">
        {#each DOW as d (d)}<span>{d}</span>{/each}
      </div>
      <div class="dp-grid">
        {#each cells as c (c.d.getTime())}
          <button
            type="button"
            class={"dp-cell" + (c.inMonth ? "" : " dim") + (selected && sameDay(c.d, selected) ? " on" : "") + (sameDay(c.d, today) ? " today" : "")}
            onclick={() => pickDay(c.d)}
          >{c.d.getDate()}</button>
        {/each}
      </div>
      {#if withTime}
        <div class="dp-time">
          <span class="dp-time-lbl">{t("Time")}</span>
          <input class="input dp-tnum" type="number" min="0" max="23" bind:value={hh} onchange={commitTime} aria-label={t("Hour")} />
          <span class="dp-colon">:</span>
          <input class="input dp-tnum" type="number" min="0" max="59" bind:value={mi} onchange={commitTime} aria-label={t("Minute")} />
          <button type="button" class="dp-done" onclick={() => (open = false)}>{t("Done")}</button>
        </div>
      {:else}
        <div class="dp-foot">
          <button type="button" class="dp-quick" onclick={() => pickDay(new Date())}>{t("Today")}</button>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .dp { position: relative; display: inline-flex; }
  .dp-btn {
    display: inline-flex; align-items: center; gap: 8px; cursor: pointer; text-align: left;
    width: 100%; min-width: 0;
  }
  .dp-label { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dp-btn.is-empty .dp-label { color: var(--fg-faint); }
  .dp-caret { color: var(--fg-faint); transform: rotate(90deg); display: inline-flex; }

  .dp-back { position: fixed; inset: 0; z-index: 220; }
  .dp-pop {
    position: absolute; z-index: 221; top: calc(100% + 6px); left: 0;
    width: 260px; padding: 10px;
    background: var(--surface-2); border: 1px solid var(--border-strong);
    border-radius: var(--rad-3); box-shadow: var(--shadow-pop);
    animation: dp-pop 0.12s ease;
  }
  @keyframes dp-pop { from { opacity: 0; transform: translateY(-4px) scale(0.98); } to { opacity: 1; transform: none; } }
  .dp-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
  .dp-title { font-size: var(--t-sm); font-weight: 600; color: var(--fg-bright); }
  .dp-nav {
    width: 24px; height: 24px; display: inline-flex; align-items: center; justify-content: center;
    border: 1px solid var(--border); border-radius: var(--rad-2); background: transparent;
    color: var(--fg-muted); cursor: pointer;
  }
  .dp-nav:hover { color: var(--fg-bright); border-color: var(--border-strong); }
  .dp-dow { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; margin-bottom: 4px; }
  .dp-dow span { text-align: center; font-size: 10px; color: var(--fg-faint); font-family: var(--font-mono); }
  .dp-grid { display: grid; grid-template-columns: repeat(7, 1fr); gap: 2px; }
  .dp-cell {
    aspect-ratio: 1; display: inline-flex; align-items: center; justify-content: center;
    border: none; background: transparent; color: var(--fg); font-size: 12.5px;
    border-radius: var(--rad-2); cursor: pointer; transition: background 0.1s ease, transform 0.08s ease;
  }
  .dp-cell:hover { background: var(--surface-3); transform: scale(1.06); }
  .dp-cell.dim { color: var(--fg-faint); }
  .dp-cell.today { box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent) 45%, var(--border)); }
  .dp-cell.on { background: var(--accent); color: var(--bg); font-weight: 600; }
  .dp-foot { display: flex; justify-content: flex-end; margin-top: 8px; }
  .dp-quick {
    font-size: var(--t-xs); color: var(--accent); background: transparent; border: none; cursor: pointer;
    padding: 3px 6px; border-radius: var(--rad-2);
  }
  .dp-quick:hover { background: color-mix(in oklab, var(--accent) 12%, transparent); }
  .dp-time { display: flex; align-items: center; gap: 6px; margin-top: 10px; padding-top: 8px; border-top: 1px solid var(--border); }
  .dp-time-lbl { font-size: var(--t-2xs); text-transform: uppercase; letter-spacing: 0.1em; color: var(--fg-faint); margin-right: auto; }
  .dp-tnum { width: 46px; text-align: center; padding: 4px 4px; }
  .dp-colon { color: var(--fg-muted); }
  .dp-done {
    font-size: var(--t-xs); color: var(--bg); background: var(--accent); border: none; cursor: pointer;
    padding: 4px 10px; border-radius: var(--rad-2); margin-left: 4px;
  }
</style>
