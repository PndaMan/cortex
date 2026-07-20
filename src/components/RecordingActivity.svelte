<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { rec } from "../lib/recorder.svelte";
  import Icon from "./Icon.svelte";

  // ─────────────────────────────────────────────────────────────────────────
  // RECORDING LIVE ACTIVITY — the lecture-recorder twin of the Pomodoro
  // LiveActivity widget. While a recording runs (or waits in review /
  // transcribes) and the user is anywhere OTHER than the Recorder view, this
  // small floating card keeps the session visible and controllable:
  //   • red pulsing dot + elapsed time while capturing,
  //   • pause/resume + stop without leaving the current screen,
  //   • click the card body to jump back to the full Recorder view,
  //   • draggable, snaps to the nearest corner (same feel as the Pomodoro one).
  // ─────────────────────────────────────────────────────────────────────────

  const MARGIN = 16;
  const BOTTOM_MARGIN = 44; // clear the status bar

  const visible = $derived(
    app.view !== "recorder" &&
      (rec.recording || rec.status === "review" || rec.status === "transcribing"),
  );

  // Corner state is widget-local (defaults opposite the Pomodoro widget so the
  // two don't stack when both are up).
  let corner = $state<"tl" | "tr" | "bl" | "br">("tr");
  let dragging = $state(false);
  let dragX = $state(0);
  let dragY = $state(0);
  let pointerOff = { x: 0, y: 0 };
  let downPt = { x: 0, y: 0 };
  let moved = false;
  let el = $state<HTMLElement | null>(null);

  const cornerStyle = $derived.by(() => {
    switch (corner) {
      case "tl": return `top:${MARGIN}px;left:${MARGIN}px;`;
      case "tr": return `top:${MARGIN}px;right:${MARGIN}px;`;
      case "bl": return `bottom:${BOTTOM_MARGIN}px;left:${MARGIN}px;`;
      default: return `bottom:${BOTTOM_MARGIN}px;right:${MARGIN}px;`;
    }
  });
  const positionStyle = $derived(dragging ? `top:${dragY}px;left:${dragX}px;` : cornerStyle);

  function clamp(v: number, lo: number, hi: number) {
    return Math.max(lo, Math.min(hi, v));
  }
  function onPointerDown(e: PointerEvent) {
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
    dragX = clamp(e.clientX - pointerOff.x, MARGIN, window.innerWidth - el.offsetWidth - MARGIN);
    dragY = clamp(e.clientY - pointerOff.y, MARGIN, window.innerHeight - el.offsetHeight - MARGIN);
  }
  function onPointerUp(e: PointerEvent) {
    if (!dragging || !el) return;
    dragging = false;
    (e.currentTarget as HTMLElement).releasePointerCapture?.(e.pointerId);
    const cx = dragX + el.offsetWidth / 2;
    const cy = dragY + el.offsetHeight / 2;
    corner = ((cy > window.innerHeight / 2 ? "b" : "t") + (cx > window.innerWidth / 2 ? "r" : "l")) as typeof corner;
  }

  function openRecorder() {
    app.setView("recorder");
  }
  function togglePause(e: Event) {
    e.stopPropagation();
    rec.togglePause();
  }
  function stop(e: Event) {
    e.stopPropagation();
    rec.stop();
    // The review step needs the full view — jump straight to it.
    app.setView("recorder");
  }
  function onCardKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); openRecorder(); }
  }
</script>

{#if visible}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    bind:this={el}
    class="ra-card"
    class:dragging
    class:paused={rec.paused}
    style={positionStyle}
    role="button"
    tabindex="0"
    aria-label="Lecture recording, {rec.mm}:{rec.ss} elapsed. Click to open the recorder."
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onclick={() => { if (!moved) openRecorder(); }}
    onkeydown={onCardKey}
  >
    {#if rec.status === "transcribing"}
      <span class="ra-dot ra-dot--busy"></span>
      <div class="ra-body">
        <div class="ra-phase mono">TRANSCRIBING</div>
        <div class="ra-time mono">{rec.note || "Whisper…"}</div>
      </div>
    {:else if rec.status === "review"}
      <span class="ra-dot ra-dot--review"></span>
      <div class="ra-body">
        <div class="ra-phase mono">UNSAVED TAKE</div>
        <div class="ra-time mono">{rec.reviewDuration} · tap to save</div>
      </div>
    {:else}
      <span class="ra-dot" class:live={!rec.paused}></span>
      <div class="ra-body">
        <div class="ra-phase mono">{rec.paused ? "PAUSED" : "RECORDING"}</div>
        <div class="ra-time mono">{rec.mm}:{rec.ss}</div>
      </div>
      <div class="ra-actions">
        <button class="ra-btn" title={rec.paused ? "Resume" : "Pause"} onclick={togglePause}>
          <Icon name={rec.paused ? "play" : "pause"} size={11} />
        </button>
        <button class="ra-btn ra-btn--stop" title="Stop & review" onclick={stop}>
          <span class="ra-stop-sq"></span>
        </button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .ra-card {
    position: fixed;
    z-index: 56; /* just above the Pomodoro widget (55), below modals (75+) */
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    min-width: 168px;
    font-family: var(--font-mono);
    background: color-mix(in oklab, var(--surface) 90%, transparent);
    backdrop-filter: blur(8px);
    border: 1px solid color-mix(in oklab, var(--err) 40%, var(--border-strong));
    border-radius: var(--rad-4);
    box-shadow: var(--shadow-2, 0 8px 24px rgba(0, 0, 0, 0.4));
    user-select: none;
    cursor: grab;
    opacity: 0.72;
    transition:
      opacity var(--dur) var(--ease),
      top var(--dur-slow) var(--ease),
      left var(--dur-slow) var(--ease),
      right var(--dur-slow) var(--ease),
      bottom var(--dur-slow) var(--ease),
      box-shadow var(--dur) var(--ease);
  }
  .ra-card:hover,
  .ra-card:focus-visible {
    opacity: 1;
    outline: none;
  }
  .ra-card.dragging {
    transition: opacity var(--dur) var(--ease), box-shadow var(--dur) var(--ease);
    cursor: grabbing;
    opacity: 1;
    box-shadow: var(--shadow-pop);
  }
  .ra-card.paused { border-color: var(--border-strong); }
  .ra-card:focus-visible {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--accent) 35%, transparent);
  }

  .ra-dot {
    width: 10px;
    height: 10px;
    flex: none;
    border-radius: 50%;
    background: var(--err);
    opacity: 0.6;
  }
  .ra-dot.live {
    opacity: 1;
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--err) 25%, transparent);
    animation: ra-pulse 1.4s ease-in-out infinite;
  }
  .ra-dot--busy { background: var(--warn); animation: ra-pulse 1.4s ease-in-out infinite; }
  .ra-dot--review { background: var(--accent); }
  @keyframes ra-pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.4; transform: scale(0.8); }
  }

  .ra-body {
    display: flex;
    flex-direction: column;
    gap: 2px;
    line-height: 1.1;
    min-width: 0;
  }
  .ra-phase {
    font-size: var(--t-2xs);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--fg-faint);
    white-space: nowrap;
  }
  .ra-time {
    font-size: var(--t-lg);
    font-weight: 600;
    color: var(--fg-bright);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 180px;
  }

  .ra-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
  }
  .ra-btn {
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
  .ra-btn:hover {
    background: var(--surface-3);
    color: var(--fg-bright);
  }
  .ra-stop-sq {
    width: 9px;
    height: 9px;
    border-radius: 2px;
    background: var(--err);
  }
</style>
