<script lang="ts">
  import Icon from "./Icon.svelte";

  // Themed date picker — a trigger button + a month-grid popover. Replaces the
  // native <input type="date"> whose dropdown is OS-rendered and unstyleable.
  // `value` is an ISO date string "YYYY-MM-DD" (or "" for unset).
  let {
    value,
    onChange,
    placeholder = "Pick a date",
  }: { value: string; onChange: (v: string) => void; placeholder?: string } = $props();

  const DOW = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
  const MONTHS = ["January","February","March","April","May","June","July","August","September","October","November","December"];

  let open = $state(false);

  function parse(v: string): Date | null {
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(v);
    if (!m) return null;
    return new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  }
  function iso(y: number, mo: number, d: number): string {
    return `${y}-${String(mo + 1).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
  }
  function sameDay(a: Date, b: Date): boolean {
    return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
  }

  // Month being displayed in the popover (seeds from value, else today on open).
  let viewY = $state(0);
  let viewM = $state(0);
  // Re-seed the view each time the popover opens.
  function openPicker() {
    const s = parse(value) ?? new Date();
    viewY = s.getFullYear();
    viewM = s.getMonth();
    open = true;
  }
  function prevMonth() { if (viewM === 0) { viewM = 11; viewY -= 1; } else viewM -= 1; }
  function nextMonth() { if (viewM === 11) { viewM = 0; viewY += 1; } else viewM += 1; }

  const today = new Date();
  const selected = $derived(parse(value));

  // 42-cell grid (6 weeks) starting on the Sunday of the week containing the 1st.
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
    return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });
  });

  function pick(d: Date) {
    onChange(iso(d.getFullYear(), d.getMonth(), d.getDate()));
    open = false;
  }
</script>

<div class="dp">
  <button type="button" class={"dp-btn input" + (value ? "" : " is-empty")} onclick={openPicker}>
    <Icon name="calendar" size={13} color="var(--fg-faint)" />
    <span class="dp-label">{label}</span>
    <span class="dp-caret"><Icon name="chevron" size={11} /></span>
  </button>

  {#if open}
    <div class="dp-back" role="presentation" onclick={() => (open = false)}></div>
    <div class="dp-pop" role="dialog" aria-label="Choose a date">
      <div class="dp-head">
        <button type="button" class="dp-nav" aria-label="Previous month" onclick={prevMonth}>
          <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={12} /></span>
        </button>
        <span class="dp-title">{MONTHS[viewM]} {viewY}</span>
        <button type="button" class="dp-nav" aria-label="Next month" onclick={nextMonth}>
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
            onclick={() => pick(c.d)}
          >{c.d.getDate()}</button>
        {/each}
      </div>
      <div class="dp-foot">
        <button type="button" class="dp-quick" onclick={() => pick(new Date())}>Today</button>
      </div>
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

  .dp-back { position: fixed; inset: 0; z-index: 80; }
  .dp-pop {
    position: absolute; z-index: 81; top: calc(100% + 6px); left: 0;
    width: 252px; padding: 10px;
    background: var(--surface-2); border: 1px solid var(--border-strong);
    border-radius: var(--rad-3); box-shadow: var(--shadow-pop);
  }
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
    border-radius: var(--rad-2); cursor: pointer;
  }
  .dp-cell:hover { background: var(--surface-3); }
  .dp-cell.dim { color: var(--fg-faint); }
  .dp-cell.today { box-shadow: inset 0 0 0 1px color-mix(in oklab, var(--accent) 45%, var(--border)); }
  .dp-cell.on { background: var(--accent); color: var(--bg); font-weight: 600; }
  .dp-foot { display: flex; justify-content: flex-end; margin-top: 8px; }
  .dp-quick {
    font-size: var(--t-xs); color: var(--accent); background: transparent; border: none; cursor: pointer;
    padding: 3px 6px; border-radius: var(--rad-2);
  }
  .dp-quick:hover { background: color-mix(in oklab, var(--accent) 12%, transparent); }
</style>
