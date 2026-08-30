<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";
  import { t } from "../lib/i18n.svelte";

  // ─────────────────────────────────────────────────────────────────────────
  // LIVE ACTIVITY — a small floating widget that mirrors the app-wide Pomodoro
  // timer (`app.pomo`). It is:
  //   • DRAGGABLE and SNAPS to the nearest screen corner on release,
  //   • MINIMISABLE to a tiny progress-ring pill,
  //   • non-intrusive (semi-transparent until hovered), and
  //   • a shortcut: clicking the card body opens the full PomodoroPanel.
  // Visible only while the timer is active (or explicitly forced on).
  // ─────────────────────────────────────────────────────────────────────────

  const pomo = app.pomo;
  const MARGIN = 16;
  const BOTTOM_MARGIN = 44; // clear the status bar so the pill never overlaps it

  // Live drag position (px from top-left). Null until first measured / dragged;
  // we render at the snapped corner via derived offsets when not dragging.
  let dragging = $state(false);
  let dragX = $state(0);
  let dragY = $state(0);
  let pointerOff = { x: 0, y: 0 };
  let downPt = { x: 0, y: 0 };
  // True once the pointer moved past a small threshold — used to tell a real
  // DRAG from a CLICK so releasing a drag doesn't also open the full panel.
  let moved = false;
  let el = $state<HTMLElement | null>(null);

  const visible = $derived(pomo.active || app.pomoLiveForce);

  // corner → fixed-position style (used when not dragging)
  const cornerStyle = $derived.by(() => {
    switch (app.pomoCorner) {
      case "tl": return `top:${MARGIN}px;left:${MARGIN}px;`;
      case "tr": return `top:${MARGIN}px;right:${MARGIN}px;`;
      case "bl": return `bottom:${BOTTOM_MARGIN}px;left:${MARGIN}px;`;
      default: return `bottom:${BOTTOM_MARGIN}px;right:${MARGIN}px;`;
    }
  });

  const positionStyle = $derived(
    dragging ? `top:${dragY}px;left:${dragX}px;` : cornerStyle,
  );

  // progress ring geometry
  const R = 11;
  const C = 2 * Math.PI * R;
  const dash = $derived(C * (1 - pomo.progress));
  const isBreak = $derived(pomo.phase !== "work");

  // ── dragging ──
  function onPointerDown(e: PointerEvent) {
    // don't start a drag from the inner buttons
    if ((e.target as HTMLElement)?.closest("button")) return;
    if (!el) return;
    const r = el.getBoundingClientRect();
    pointerOff = { x: e.clientX - r.left, y: e.clientY - r.top };
    downPt = { x: e.clientX, y: e.clientY };
    moved = false;
    dragX = r.left;
    dragY = r.top;
    dragging = true;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    e.preventDefault();
  }
  function onPointerMove(e: PointerEvent) {
    if (!dragging || !el) return;
    if (Math.hypot(e.clientX - downPt.x, e.clientY - downPt.y) > 5) moved = true;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    dragX = clamp(e.clientX - pointerOff.x, MARGIN, window.innerWidth - w - MARGIN);
    dragY = clamp(e.clientY - pointerOff.y, MARGIN, window.innerHeight - h - MARGIN);
  }
  function onPointerUp(e: PointerEvent) {
    if (!dragging || !el) return;
    dragging = false;
    (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
    // snap to nearest corner by the widget's center
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    const cx = dragX + w / 2;
    const cy = dragY + h / 2;
    const right = cx > window.innerWidth / 2;
    const bottom = cy > window.innerHeight / 2;
    app.pomoCorner = (bottom ? "b" : "t") + (right ? "r" : "l") as typeof app.pomoCorner;
  }
  function clamp(v: number, lo: number, hi: number) {
    return Math.max(lo, Math.min(hi, v));
  }

  // ── interactions ──
  function openPanel() {
    app.pomodoroOpen = true;
  }
  function toggleMin(e: Event) {
    e.stopPropagation();
    app.pomoLiveMin = !app.pomoLiveMin;
  }
  function togglePlay(e: Event) {
    e.stopPropagation();
    pomo.pomoToggle();
  }
  function onCardKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); openPanel(); }
    else if (e.key === "Escape") { e.preventDefault(); app.pomoLiveMin = true; }
  }
</script>

{#if visible}
  {#if app.pomoLiveMin}
    <!-- minimised: a tiny progress-ring pill -->
    <button
      bind:this={el}
      class="la-pill"
      class:dragging
      class:break={isBreak}
      style={positionStyle}
      title={t("Expand focus timer")}
      aria-label={t("Pomodoro timer, {n} remaining. Expand.", { n: pomo.mmss })}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onclick={() => { if (!moved) app.pomoLiveMin = false; }}
    >
      <svg class="la-ring" viewBox="0 0 28 28" aria-hidden="true">
        <circle class="la-ring-bg" cx="14" cy="14" r={R} />
        <circle
          class="la-ring-fg"
          cx="14" cy="14" r={R}
          stroke-dasharray={C}
          stroke-dashoffset={dash}
        />
      </svg>
      <span class="la-pill-leaf" aria-hidden="true">🌱</span>
    </button>
  {:else}
    <!-- expanded: the small live card -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      bind:this={el}
      class="la-card"
      class:dragging
      class:break={isBreak}
      style={positionStyle}
      role="button"
      tabindex="0"
      aria-label={t("Pomodoro live activity. {phase}, {n} remaining. Click to open the full timer.", { phase: pomo.phaseLabel, n: pomo.mmss })}
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onclick={() => { if (!moved) openPanel(); }}
      onkeydown={onCardKey}
    >
      <!-- progress ring + tiny bonsai glyph -->
      <div class="la-glyph">
        <svg class="la-ring" viewBox="0 0 28 28" aria-hidden="true">
          <circle class="la-ring-bg" cx="14" cy="14" r={R} />
          <circle
            class="la-ring-fg"
            cx="14" cy="14" r={R}
            stroke-dasharray={C}
            stroke-dashoffset={dash}
          />
        </svg>
        <span class="la-leaf" aria-hidden="true">🌱</span>
      </div>

      <div class="la-body">
        <div class="la-phase mono">{pomo.phaseLabel}</div>
        <div class="la-time mono">{pomo.mmss}</div>
      </div>

      <div class="la-actions">
        <button class="la-btn" title={pomo.running ? t("Pause") : t("Start")} onclick={togglePlay}>
          <Icon name={pomo.running ? "pause" : "play"} size={11} />
        </button>
        <button class="la-btn" title={t("Minimise")} aria-label={t("Minimise")} onclick={toggleMin}>
          <Icon name="x" size={10} />
        </button>
      </div>
    </div>
  {/if}
{/if}

<style>
  /* shared positioning + non-intrusive transparency */
  .la-card,
  .la-pill {
    position: fixed;
    z-index: 55; /* above content, below modals/overlays (75+) */
    font-family: var(--font-mono);
    cursor: grab;
    opacity: 0.62;
    transition:
      opacity var(--dur) var(--ease),
      top var(--dur-slow) var(--ease),
      left var(--dur-slow) var(--ease),
      right var(--dur-slow) var(--ease),
      bottom var(--dur-slow) var(--ease),
      box-shadow var(--dur) var(--ease),
      transform var(--dur-fast) var(--ease);
  }
  .la-card:hover,
  .la-pill:hover,
  .la-card:focus-visible {
    opacity: 1;
    outline: none;
  }
  /* while dragging, the live top/left are authoritative — kill the snap transition */
  .la-card.dragging,
  .la-pill.dragging {
    transition: opacity var(--dur) var(--ease), box-shadow var(--dur) var(--ease);
    cursor: grabbing;
    opacity: 1;
    box-shadow: var(--shadow-pop);
  }

  /* ── expanded card ── */
  .la-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    min-width: 168px;
    background: color-mix(in oklab, var(--surface) 90%, transparent);
    backdrop-filter: blur(8px);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    box-shadow: var(--shadow-2, 0 8px 24px rgba(0, 0, 0, 0.4));
    user-select: none;
  }
  .la-card:focus-visible {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 35%, transparent);
  }

  .la-glyph {
    position: relative;
    width: 28px;
    height: 28px;
    flex: none;
  }
  .la-leaf {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    line-height: 1;
    filter: saturate(0.85);
  }

  .la-ring {
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }
  .la-ring-bg {
    fill: none;
    stroke: var(--border-strong);
    stroke-width: 2.4;
  }
  .la-ring-fg {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2.4;
    stroke-linecap: round;
    transition: stroke-dashoffset 0.5s linear;
  }
  .break .la-ring-fg {
    stroke: var(--ok);
  }

  .la-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    line-height: 1.1;
  }
  .la-phase {
    font-size: var(--t-2xs);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .la-time {
    font-size: var(--t-lg);
    font-weight: 600;
    color: var(--fg-bright);
    font-variant-numeric: tabular-nums;
  }
  .break .la-time {
    color: var(--accent);
  }

  .la-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
  }
  .la-btn {
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: var(--rad-2);
    background: transparent;
    color: var(--fg-muted);
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease), color var(--dur-fast) var(--ease);
  }
  .la-btn:hover {
    background: var(--surface-3);
    color: var(--fg-bright);
  }

  /* ── minimised pill ── */
  .la-pill {
    width: 40px;
    height: 40px;
    padding: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-pill);
    background: color-mix(in oklab, var(--surface) 88%, transparent);
    backdrop-filter: blur(8px);
    box-shadow: var(--shadow-2, 0 8px 24px rgba(0, 0, 0, 0.4));
  }
  .la-pill .la-ring {
    position: absolute;
    inset: 0;            /* concentric with the pill (and the leaf) */
    width: 100%;
    height: 100%;
  }
  /* Leaf absolutely centered (inset:0) so it sits exactly on the ring's centre. */
  .la-pill-leaf {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 14px;
    line-height: 1;
    filter: saturate(0.85);
    pointer-events: none;
  }
</style>
