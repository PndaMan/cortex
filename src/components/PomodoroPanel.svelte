<script lang="ts">
  import { app } from "../lib/store.svelte";
  import Icon from "./Icon.svelte";

  // ─────────────────────────────────────────────────────────────────────────
  // POMODORO PANEL — focus timer with a generative bonsai that grows as the
  // current work session elapses. The tree is generated ONCE per session from a
  // seeded PRNG (so it never reshuffles between frames) and revealed
  // progressively; breaks show a calmer swaying / petal-fall state. A soft
  // Web-Audio bell plays on each work↔break transition.
  // ─────────────────────────────────────────────────────────────────────────

  type Phase = "work" | "short" | "long";

  // ── settings (minutes) ──
  let workMin = $state(25);
  let shortMin = $state(5);
  let longMin = $state(15);
  const CYCLES = 4; // long break after every 4th focus session
  let showSettings = $state(false);

  // ── timer state ──
  let phase = $state<Phase>("work");
  let cycle = $state(1); // 1..CYCLES — which focus session we're on
  let running = $state(false);
  let elapsedMs = $state(0); // accrued elapsed within the current phase
  let lastTickAt = 0; // wall-clock anchor for the running segment
  let sessionSeed = $state(seedFrom(Date.now())); // re-rolled each new work session

  const phaseMin = $derived(
    phase === "work" ? workMin : phase === "short" ? shortMin : longMin
  );
  const totalMs = $derived(Math.max(1, phaseMin) * 60_000);
  const remainMs = $derived(Math.max(0, totalMs - elapsedMs));
  const progress = $derived(Math.min(1, elapsedMs / totalMs)); // 0..1 of current phase

  const mmss = $derived.by(() => {
    const s = Math.ceil(remainMs / 1000);
    const m = Math.floor(s / 60);
    const r = s % 60;
    return `${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}`;
  });

  const phaseLabel = $derived(
    phase === "work" ? `Focus ${cycle} of ${CYCLES}` : phase === "long" ? "Long break" : "Short break"
  );

  // ── animation / canvas ──
  let canvas = $state<HTMLCanvasElement | null>(null);
  let raf = 0;

  // ── audio ──
  let audioCtx: AudioContext | null = null;

  // ───────────────────────── seeded PRNG (mulberry32) ─────────────────────────
  function seedFrom(n: number) {
    return (n ^ 0x9e3779b9) >>> 0;
  }
  function makeRng(seed: number) {
    let a = seed >>> 0;
    return () => {
      a |= 0;
      a = (a + 0x6d2b79f5) | 0;
      let t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  // ─────────────────────── procedural bonsai generation ───────────────────────
  // Each segment is a small straight stroke with curvature baked into a chain.
  // We precompute the whole tree as a flat list, each segment carrying an
  // `appearAt` fraction (when in the [0,1] reveal it begins) and `span` (how much
  // of the reveal it takes), so the SAME structure animates smoothly every frame.
  type Seg = {
    x1: number; y1: number; x2: number; y2: number;
    w: number; // line width
    depth: number;
    appearAt: number; // reveal fraction at which this seg starts drawing
    span: number; // reveal fraction width over which it completes
  };
  type Leaf = {
    x: number; y: number; r: number;
    appearAt: number;
    hueShift: number; // 0..1 picks between accent/ok foliage
    sway: number; // per-leaf phase offset for break sway
  };

  type Tree = { segs: Seg[]; leaves: Leaf[]; w: number; h: number; maxDepth: number };

  // Grow a tree into `segs`/`leaves`. Branches deeper => later appearAt, so the
  // reveal naturally goes trunk → branches → twigs → leaves over the session.
  function buildTree(seed: number, w: number, h: number): Tree {
    const rng = makeRng(seed);
    const segs: Seg[] = [];
    const leaves: Leaf[] = [];
    const maxDepth = 9;
    // appearAt is assigned per depth band so the trunk shows first; we reserve
    // the final ~22% of the session for foliage to bloom.
    const FOLIAGE_START = 0.78;

    function branch(
      x: number, y: number,
      angle: number, // radians, -PI/2 == straight up
      len: number,
      width: number,
      depth: number,
      tBase: number // reveal fraction this branch begins at
    ) {
      if (depth > maxDepth || len < 5 || width < 0.5) {
        // tip → spawn a small cluster of leaves that bloom late
        const cluster = 2 + Math.floor(rng() * 3);
        for (let i = 0; i < cluster; i++) {
          leaves.push({
            x: x + (rng() - 0.5) * 14,
            y: y + (rng() - 0.5) * 14,
            r: 2.2 + rng() * 3.2,
            appearAt: FOLIAGE_START + rng() * (1 - FOLIAGE_START),
            hueShift: rng(),
            sway: rng() * Math.PI * 2,
          });
        }
        return;
      }

      // Split this branch into N curving sub-segments so it bends organically.
      const SUB = 5;
      const depthFrac = depth / maxDepth;
      // depth band of the reveal: trunk (depth 0) occupies the first slice,
      // deeper branches fill progressively later, all before FOLIAGE_START.
      const bandStart = tBase;
      const bandLen = (FOLIAGE_START / maxDepth) * (1.1 - 0.4 * rng());

      let cx = x, cy = y;
      let curAngle = angle;
      const curveBias = (rng() - 0.5) * 0.5; // gentle consistent lean per branch
      for (let i = 0; i < SUB; i++) {
        const segLen = len / SUB;
        curAngle += curveBias / SUB + (rng() - 0.5) * 0.08;
        const nx = cx + Math.cos(curAngle) * segLen;
        const ny = cy + Math.sin(curAngle) * segLen;
        const tA = bandStart + (i / SUB) * bandLen;
        segs.push({
          x1: cx, y1: cy, x2: nx, y2: ny,
          w: width,
          depth,
          appearAt: Math.min(tA, FOLIAGE_START),
          span: Math.max(0.012, bandLen / SUB),
        });
        cx = nx; cy = ny;
      }

      const childBase = Math.min(bandStart + bandLen, FOLIAGE_START);
      // Number of children tapers with depth; randomized for variety.
      const kids = depth < 2 ? 2 : 1 + (rng() < 0.62 ? 1 : 0) + (rng() < 0.16 ? 1 : 0);
      for (let k = 0; k < kids; k++) {
        const dir = k === 0 ? -1 : 1;
        const spread = (0.42 + rng() * 0.5) * (kids === 1 ? (rng() < 0.5 ? -1 : 1) : dir);
        const childAngle = curAngle + spread + (rng() - 0.5) * 0.18;
        const childLen = len * (0.66 + rng() * 0.2);
        const childW = width * (0.62 + rng() * 0.16);
        branch(cx, cy, childAngle, childLen, childW, depth + 1, childBase);
      }
    }

    // Root: trunk rises from near the bottom-center with a slight natural lean.
    const rootX = w * (0.46 + rng() * 0.08);
    const rootY = h * 0.97;
    const trunkLen = h * (0.2 + rng() * 0.05);
    const trunkW = Math.max(7, w * 0.018);
    const lean = -Math.PI / 2 + (rng() - 0.5) * 0.22;
    branch(rootX, rootY, lean, trunkLen, trunkW, 0, 0);

    return { segs, leaves, w, h, maxDepth };
  }

  // Cache the tree for the active session; rebuild when seed or size changes.
  let tree: Tree | null = null;
  let treeKey = "";

  function ensureTree(w: number, h: number) {
    const key = `${sessionSeed}:${Math.round(w)}x${Math.round(h)}`;
    if (key !== treeKey || !tree) {
      tree = buildTree(sessionSeed, w, h);
      treeKey = key;
    }
    return tree;
  }

  // ─────────────────────────────── rendering ───────────────────────────────
  function cssVar(name: string, fallback: string) {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  }

  // Theme tokens only change on a theme switch, so reading them every frame via
  // getComputedStyle (a layout-flushing call) is wasted work on a 60fps path.
  // Cache the palette and refresh it sparingly.
  type Palette = { barkLo: string; barkHi: string; folA: string; folB: string };
  let palette: Palette | null = null;
  let paletteFrame = 0;
  function getPalette(): Palette {
    if (!palette || paletteFrame % 30 === 0) {
      palette = {
        barkLo: cssVar("--fg-faint", "#53685b"),
        barkHi: cssVar("--fg-muted", "#8a9a7e"),
        folA: cssVar("--accent", "#2dd5b7"),
        folB: cssVar("--ok", "#63b07a"),
      };
    }
    paletteFrame++;
    return palette;
  }

  function draw(now: number) {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = Math.min(2, window.devicePixelRatio || 1);
    const cw = canvas.clientWidth || 320;
    const ch = canvas.clientHeight || 360;
    if (canvas.width !== Math.round(cw * dpr) || canvas.height !== Math.round(ch * dpr)) {
      canvas.width = Math.round(cw * dpr);
      canvas.height = Math.round(ch * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, cw, ch);

    const { barkLo, barkHi, folA, folB } = getPalette();

    const t = ensureTree(cw, ch);

    // During work the reveal tracks the session progress; on break the tree is
    // fully grown (reveal = 1) and we layer a gentle sway / petal fall instead.
    const onBreak = phase !== "work";
    const reveal = onBreak ? 1 : progress;
    const tSec = now / 1000;

    ctx.lineCap = "round";
    ctx.lineJoin = "round";

    // Bark: draw segments whose appearAt has been reached. The frontier segment
    // (appearAt..appearAt+span straddling `reveal`) is drawn partially for
    // smooth growth rather than popping in.
    for (const s of t.segs) {
      if (reveal <= s.appearAt) continue;
      let f = 1;
      if (reveal < s.appearAt + s.span) {
        f = (reveal - s.appearAt) / s.span; // 0..1 partial
      }
      // optional break-time sway: deeper segments drift more
      let sx = 0, sy = 0;
      if (onBreak) {
        const amp = (s.depth / t.maxDepth) * 1.6;
        sx = Math.sin(tSec * 0.7 + s.y1 * 0.01) * amp;
      }
      const x2 = s.x1 + (s.x2 - s.x1) * f + sx;
      const y2 = s.y1 + (s.y2 - s.y1) * f + sy;
      // darker bark at the base, lighter toward the tips
      const mix = Math.min(1, s.depth / 5);
      ctx.strokeStyle = mix > 0.5 ? barkHi : barkLo;
      ctx.lineWidth = Math.max(0.6, s.w * (0.55 + 0.45 * f));
      ctx.beginPath();
      ctx.moveTo(s.x1 + (onBreak ? sx * 0.5 : 0), s.y1);
      ctx.lineTo(x2, y2);
      ctx.stroke();
    }

    // Foliage: leaves bloom in the final stretch (or are full on break).
    for (const lf of t.leaves) {
      if (reveal <= lf.appearAt) continue;
      const grow = Math.min(1, (reveal - lf.appearAt) / Math.max(0.001, 1 - lf.appearAt));
      let lx = lf.x, ly = lf.y;
      let alpha = 0.85;
      if (onBreak) {
        // petals drift: gentle bob + slow downward fall that loops
        const fall = ((tSec * 8 + lf.sway * 9) % 60);
        lx += Math.sin(tSec * 0.9 + lf.sway) * 3;
        ly += Math.sin(tSec * 0.6 + lf.sway) * 2 + fall * 0.15;
        alpha = 0.55 + 0.3 * Math.sin(tSec + lf.sway);
      }
      ctx.globalAlpha = alpha * Math.min(1, grow * 1.4);
      ctx.fillStyle = lf.hueShift < 0.5 ? folA : folB;
      ctx.beginPath();
      ctx.arc(lx, ly, lf.r * (0.5 + 0.5 * grow), 0, Math.PI * 2);
      ctx.fill();
    }
    ctx.globalAlpha = 1;

    // soft ground line
    ctx.strokeStyle = barkLo;
    ctx.globalAlpha = 0.35;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(cw * 0.18, ch * 0.965);
    ctx.lineTo(cw * 0.82, ch * 0.965);
    ctx.stroke();
    ctx.globalAlpha = 1;
  }

  // ─────────────────────────── timer / rAF loop ───────────────────────────
  function loop(now: number) {
    if (running) {
      if (lastTickAt) elapsedMs += now - lastTickAt;
      lastTickAt = now;
      if (elapsedMs >= totalMs) {
        elapsedMs = totalMs;
        advancePhase();
      }
    }
    draw(now);
    raf = requestAnimationFrame(loop);
  }

  // ─────────────────────────── phase transitions ───────────────────────────
  function advancePhase() {
    chime();
    if (phase === "work") {
      // work just ended → break
      app.pushToast({ kind: "success", title: "Break time", body: "Nice focus — take 5." });
      const isLong = cycle % CYCLES === 0;
      phase = isLong ? "long" : "short";
    } else {
      // break just ended → next focus session
      const wasLong = phase === "long";
      if (wasLong) cycle = 1;
      else cycle = cycle + 1;
      phase = "work";
      sessionSeed = seedFrom(Date.now() + cycle * 2654435761); // new tree each focus session
      tree = null; treeKey = "";
      app.pushToast({ kind: "info", title: "Back to focus", body: `Session ${cycle} starting.` });
    }
    elapsedMs = 0;
    lastTickAt = 0;
    running = true; // auto-continue into the next phase
  }

  // ─────────────────────────────── controls ───────────────────────────────
  function start() {
    if (running) return;
    running = true;
    lastTickAt = 0; // re-anchored on next rAF tick to avoid a jump
  }
  function pause() {
    running = false;
    lastTickAt = 0;
  }
  function reset() {
    running = false;
    elapsedMs = 0;
    lastTickAt = 0;
    if (phase === "work") {
      sessionSeed = seedFrom(Date.now()); // fresh tree on reset of a focus session
      tree = null; treeKey = "";
    }
  }

  // ─────────────────────────── Web Audio chime ───────────────────────────
  // Soft bell: two sine partials (fundamental + a quiet upper partial) through
  // an exponential decay envelope. Low volume, no external files.
  function chime() {
    try {
      if (!audioCtx) {
        const AC = window.AudioContext || (window as any).webkitAudioContext;
        if (!AC) return;
        audioCtx = new AC();
      }
      const ctx = audioCtx;
      if (ctx.state === "suspended") ctx.resume().catch(() => {});
      const t0 = ctx.currentTime;
      const master = ctx.createGain();
      master.gain.value = 0.0001;
      master.connect(ctx.destination);
      // gentle swell + long exponential decay
      master.gain.setValueAtTime(0.0001, t0);
      master.gain.exponentialRampToValueAtTime(0.16, t0 + 0.04);
      master.gain.exponentialRampToValueAtTime(0.0001, t0 + 2.4);

      const base = phase === "work" ? 528 : 440; // brighter going into a break
      const partials = [
        { f: base, g: 1.0 },
        { f: base * 2.01, g: 0.4 },
        { f: base * 3.0, g: 0.16 },
      ];
      for (const p of partials) {
        const osc = ctx.createOscillator();
        osc.type = "sine";
        osc.frequency.value = p.f;
        const g = ctx.createGain();
        g.gain.value = p.g;
        osc.connect(g);
        g.connect(master);
        osc.start(t0);
        osc.stop(t0 + 2.5);
      }
    } catch {
      /* audio is best-effort; never block the timer */
    }
  }

  // ─────────────────────────── lifecycle / cleanup ───────────────────────────
  function teardown() {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    running = false;
    lastTickAt = 0;
    palette = null; // re-read theme tokens on next open
    if (audioCtx) {
      audioCtx.close().catch(() => {});
      audioCtx = null;
    }
  }

  function close() {
    app.pomodoroOpen = false;
  }

  // Start the rAF loop only while the panel is open; tear everything down when it
  // closes or the component unmounts. Returning the teardown from $effect handles
  // both close (open flips false) and unmount.
  $effect(() => {
    if (!app.pomodoroOpen) return;
    raf = requestAnimationFrame(loop);
    return teardown;
  });

  function onKeyDown(e: KeyboardEvent) {
    if (!app.pomodoroOpen) return;
    if (e.key === "Escape") {
      if (showSettings) { showSettings = false; return; }
      close();
    }
    if (e.key === " " && (e.target as HTMLElement)?.tagName !== "INPUT") {
      e.preventDefault();
      running ? pause() : start();
    }
  }

  function clampMin(v: number) {
    return Math.max(1, Math.min(180, Math.round(v) || 1));
  }
</script>

<svelte:window onkeydown={onKeyDown} />

{#if app.pomodoroOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay pom-overlay" onmousedown={close}>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="pom-modal" onmousedown={(e) => e.stopPropagation()}>
      <header class="pom-head">
        <div>
          <div class="eyebrow">Focus ritual</div>
          <div class="pom-title">Pomodoro</div>
        </div>
        <div class="pom-head-actions">
          <button
            class="btn btn--icon btn--sm btn--ghost"
            class:on={showSettings}
            title="Settings"
            onclick={() => (showSettings = !showSettings)}
          >
            <Icon name="settings" size={13} />
          </button>
          <button class="btn btn--icon btn--sm btn--ghost" title="Close" onclick={close}>
            <Icon name="x" size={12} />
          </button>
        </div>
      </header>

      <!-- the bonsai -->
      <div class="pom-canvas-wrap" class:break={phase !== "work"}>
        <canvas bind:this={canvas} class="pom-canvas"></canvas>
        <div class="pom-phase-tag mono">{phaseLabel}</div>
      </div>

      <!-- countdown -->
      <div class="pom-clock">
        <div class="pom-time mono" class:rest={phase !== "work"}>{mmss}</div>
        <div class="pom-cycles">
          {#each Array(CYCLES) as _, i}
            <span
              class="pom-dot"
              class:done={phase === "work" ? i < cycle - 1 : i < cycle}
              class:active={phase === "work" && i === cycle - 1}
            ></span>
          {/each}
        </div>
      </div>

      <!-- controls -->
      <div class="pom-controls">
        {#if running}
          <button class="btn btn--sm" onclick={pause}>
            <Icon name="pause" size={12} /> Pause
          </button>
        {:else}
          <button class="btn btn--sm btn--primary" onclick={start}>
            <Icon name="play" size={12} /> {elapsedMs > 0 ? "Resume" : "Start"}
          </button>
        {/if}
        <button class="btn btn--sm btn--ghost" onclick={reset} title="Reset phase">
          <Icon name="x" size={12} /> Reset
        </button>
      </div>

      <!-- settings drawer -->
      {#if showSettings}
        <div class="pom-settings">
          <div class="pom-set-row">
            <label class="mono" for="pm-work">Focus</label>
            <input
              id="pm-work" type="number" min="1" max="180" value={workMin}
              onchange={(e) => (workMin = clampMin(+(e.target as HTMLInputElement).value))}
            />
            <span class="pom-set-unit mono">min</span>
          </div>
          <div class="pom-set-row">
            <label class="mono" for="pm-short">Short break</label>
            <input
              id="pm-short" type="number" min="1" max="180" value={shortMin}
              onchange={(e) => (shortMin = clampMin(+(e.target as HTMLInputElement).value))}
            />
            <span class="pom-set-unit mono">min</span>
          </div>
          <div class="pom-set-row">
            <label class="mono" for="pm-long">Long break</label>
            <input
              id="pm-long" type="number" min="1" max="180" value={longMin}
              onchange={(e) => (longMin = clampMin(+(e.target as HTMLInputElement).value))}
            />
            <span class="pom-set-unit mono">min</span>
          </div>
          <div class="pom-set-note mono">Long break every {CYCLES}th focus session.</div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .pom-overlay {
    justify-content: center;
    padding: 0;
  }
  .pom-modal {
    width: 100%;
    max-width: 420px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    box-shadow: var(--shadow-pop);
    overflow: hidden;
    display: flex;
    flex-direction: column;
    animation: popIn var(--dur) var(--ease);
  }

  .pom-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 13px 15px 11px;
    border-bottom: 1px solid var(--border);
  }
  .pom-title {
    font-family: var(--font-mono);
    font-size: var(--t-md);
    color: var(--fg-bright);
    font-weight: 600;
    margin-top: 3px;
  }
  .pom-head-actions {
    display: flex;
    gap: 6px;
  }
  .pom-head-actions .btn.on {
    color: var(--accent);
    border-color: var(--border);
    background: var(--surface-2);
  }

  .pom-canvas-wrap {
    position: relative;
    margin: 14px 15px 0;
    height: 280px;
    border-radius: var(--rad-3);
    border: 1px solid var(--border);
    background:
      radial-gradient(120% 90% at 50% 100%, var(--surface-2) 0%, var(--bg-sunken) 100%);
    overflow: hidden;
    transition: box-shadow var(--dur-slow) var(--ease);
  }
  .pom-canvas-wrap.break {
    box-shadow: inset 0 0 60px color-mix(in oklab, var(--accent) 12%, transparent);
  }
  .pom-canvas {
    width: 100%;
    height: 100%;
    display: block;
  }
  .pom-phase-tag {
    position: absolute;
    top: 9px;
    left: 10px;
    font-size: var(--t-2xs);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-faint);
    background: color-mix(in oklab, var(--bg-sunken) 60%, transparent);
    padding: 2px 7px;
    border-radius: var(--rad-pill);
  }

  .pom-clock {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 16px 0 4px;
  }
  .pom-time {
    font-size: 58px;
    line-height: 1;
    font-weight: 600;
    color: var(--fg-bright);
    letter-spacing: 0.02em;
    font-variant-numeric: tabular-nums;
  }
  .pom-time.rest {
    color: var(--accent);
  }
  .pom-cycles {
    display: flex;
    gap: 9px;
  }
  .pom-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: transparent;
    border: 1.5px solid var(--border-strong);
    transition: all var(--dur) var(--ease);
  }
  .pom-dot.done {
    background: var(--ok);
    border-color: var(--ok);
  }
  .pom-dot.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 22%, transparent);
  }

  .pom-controls {
    display: flex;
    justify-content: center;
    gap: 9px;
    padding: 12px 15px 16px;
  }
  .pom-controls .btn {
    gap: 6px;
  }

  .pom-settings {
    border-top: 1px solid var(--border);
    padding: 13px 15px 16px;
    background: var(--surface-2);
    display: flex;
    flex-direction: column;
    gap: 9px;
    animation: popIn var(--dur-fast) var(--ease);
  }
  .pom-set-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .pom-set-row label {
    flex: 1;
    font-size: var(--t-sm);
    color: var(--fg-muted);
  }
  .pom-set-row input {
    width: 64px;
    height: 26px;
    background: var(--surface);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-2);
    color: var(--fg-bright);
    font-family: var(--font-mono);
    font-size: var(--t-sm);
    text-align: right;
    padding: 0 8px;
    outline: none;
  }
  .pom-set-row input:focus {
    border-color: var(--accent);
  }
  .pom-set-unit {
    width: 26px;
    font-size: var(--t-2xs);
    color: var(--fg-faint);
  }
  .pom-set-note {
    font-size: var(--t-2xs);
    color: var(--fg-faint);
    padding-top: 2px;
  }
</style>
