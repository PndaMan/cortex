<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  // ─────────────────────────────────────────────────────────────────────────
  // POMODORO PANEL — a calming focus timer with a generative bonsai that grows
  // as the current work session elapses. The tree structure is generated ONCE
  // per session from a seeded PRNG (so it never reshuffles between frames) and
  // revealed progressively along smooth bezier branches; foliage blooms in the
  // final stretch. Breaks show the full tree in a gentle sway with drifting
  // petals. A soft Web-Audio bell plays on each work↔break transition.
  //
  // The timer itself lives in the store (`app.pomo`) so it keeps running while
  // the panel is closed and the floating LiveActivity widget mirrors it.
  // ─────────────────────────────────────────────────────────────────────────

  const pomo = app.pomo;

  // local UI-only state
  let showSettings = $state(false);

  // The bonsai seed is derived from the timer's completed-session count so each
  // focus session grows a distinct tree, stable for that session.
  const sessionSeed = $derived(seedFrom(pomo.completedSessions + pomo.cycle * 97 + 1));

  // ── animation / canvas ──
  let canvas = $state<HTMLCanvasElement | null>(null);
  let raf = 0;

  // ── audio ──
  let audioCtx: AudioContext | null = null;
  let lastChimePhase: string = pomo.phase;

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
  // A branch is stored as a smooth quadratic curve (start, control, end) so it
  // bends organically and can be partially revealed by sampling the curve. Each
  // branch carries an `appearAt` reveal fraction (deeper → later) and a `span`
  // over which it draws in, giving an unhurried trunk → twig → leaf growth.
  type Branch = {
    x0: number; y0: number; cx: number; cy: number; x1: number; y1: number;
    w0: number; w1: number; // tapered width
    depth: number;
    appearAt: number;
    span: number;
  };
  type Leaf = {
    x: number; y: number; r: number;
    appearAt: number;
    tone: number; // 0..1 blends accent → ok foliage
    sway: number; // per-leaf phase offset
  };
  type Tree = { branches: Branch[]; leaves: Leaf[]; w: number; h: number; maxDepth: number };

  function buildTree(seed: number, w: number, h: number): Tree {
    const rng = makeRng(seed);
    const branches: Branch[] = [];
    const leaves: Leaf[] = [];
    const maxDepth = 8;
    const FOLIAGE_START = 0.74; // last ~26% of the session is leaf bloom

    function grow(
      x: number, y: number,
      angle: number, // radians, -PI/2 == straight up
      len: number,
      width: number,
      depth: number,
      tBase: number,
    ) {
      if (depth > maxDepth || len < 7 || width < 0.6) {
        // tip → a soft cluster of leaves that bloom late
        const cluster = 3 + Math.floor(rng() * 3);
        for (let i = 0; i < cluster; i++) {
          const a = rng() * Math.PI * 2;
          const d = rng() * 11;
          leaves.push({
            x: x + Math.cos(a) * d,
            y: y + Math.sin(a) * d,
            r: 2.6 + rng() * 3.4,
            appearAt: FOLIAGE_START + rng() * (1 - FOLIAGE_START),
            tone: rng(),
            sway: rng() * Math.PI * 2,
          });
        }
        return;
      }

      const bandLen = (FOLIAGE_START / maxDepth) * (1.05 - 0.35 * rng());
      // Gentle organic curvature: bow the branch sideways via a control point.
      const lean = (rng() - 0.5) * 0.55; // overall bend of this branch
      const ex = x + Math.cos(angle) * len;
      const ey = y + Math.sin(angle) * len;
      // perpendicular offset for the control point → smooth arc
      const perp = angle + Math.PI / 2;
      const bow = lean * len * 0.5;
      const cx = (x + ex) / 2 + Math.cos(perp) * bow;
      const cy = (y + ey) / 2 + Math.sin(perp) * bow;

      const w1 = width * 0.64;
      branches.push({
        x0: x, y0: y, cx, cy, x1: ex, y1: ey,
        w0: width, w1,
        depth,
        appearAt: Math.min(tBase, FOLIAGE_START),
        span: Math.max(0.02, bandLen),
      });

      const childBase = Math.min(tBase + bandLen, FOLIAGE_START);
      const endAngle = Math.atan2(ey - cy, ex - cx); // tangent at the tip
      const kids = depth < 2 ? 2 : 1 + (rng() < 0.66 ? 1 : 0) + (rng() < 0.14 ? 1 : 0);
      for (let k = 0; k < kids; k++) {
        const dir = kids === 1 ? (rng() < 0.5 ? -1 : 1) : k === 0 ? -1 : 1;
        const spread = (0.36 + rng() * 0.46) * dir;
        const childAngle = endAngle + spread + (rng() - 0.5) * 0.16;
        const childLen = len * (0.68 + rng() * 0.16);
        const childW = w1 * (0.92 + rng() * 0.12);
        grow(ex, ey, childAngle, childLen, childW, depth + 1, childBase);
      }
    }

    const rootX = w * (0.47 + rng() * 0.06);
    const rootY = h * 0.96;
    const trunkLen = h * (0.21 + rng() * 0.05);
    const trunkW = Math.max(8, w * 0.02);
    const lean = -Math.PI / 2 + (rng() - 0.5) * 0.18;
    grow(rootX, rootY, lean, trunkLen, trunkW, 0, 0);

    return { branches, leaves, w, h, maxDepth };
  }

  let tree: Tree | null = null;
  let treeKey = "";
  function ensureTree(w: number, h: number): Tree {
    const key = `${sessionSeed}:${Math.round(w)}x${Math.round(h)}`;
    if (key !== treeKey || !tree) {
      tree = buildTree(sessionSeed, w, h);
      treeKey = key;
    }
    return tree;
  }

  // ─────────────────────────────── palette ───────────────────────────────
  function cssVar(name: string, fallback: string) {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  }
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

  // sample a point on a quadratic bezier at t∈[0,1]
  function qbez(p0: number, p1: number, p2: number, t: number) {
    const mt = 1 - t;
    return mt * mt * p0 + 2 * mt * t * p1 + t * t * p2;
  }

  // ─────────────────────────────── rendering ───────────────────────────────
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

    const onBreak = pomo.phase !== "work";
    // ease the reveal so growth feels gentle near the start and end
    const raw = onBreak ? 1 : pomo.progress;
    const reveal = raw * raw * (3 - 2 * raw); // smoothstep
    const tSec = now / 1000;
    // calm global breathing of the whole canopy
    const breath = Math.sin(tSec * 0.5) * (onBreak ? 1.4 : 0.6);

    ctx.lineCap = "round";
    ctx.lineJoin = "round";

    // ── soft halo behind the foliage (calming glow) ──
    const haloR = Math.min(cw, ch) * (0.18 + 0.16 * reveal);
    const halo = ctx.createRadialGradient(
      cw * 0.5, ch * 0.42, haloR * 0.1,
      cw * 0.5, ch * 0.42, haloR,
    );
    halo.addColorStop(0, withAlpha(folA, 0.1 * reveal));
    halo.addColorStop(1, withAlpha(folA, 0));
    ctx.fillStyle = halo;
    ctx.fillRect(0, 0, cw, ch);

    // ── bark: draw each curve up to the revealed fraction ──
    for (const b of t.branches) {
      if (reveal <= b.appearAt) continue;
      let f = 1;
      if (reveal < b.appearAt + b.span) f = (reveal - b.appearAt) / b.span;
      const steps = 10;
      const swayAmp = (b.depth / t.maxDepth) * (onBreak ? 2.2 : 1) + breath * (b.depth / t.maxDepth);
      const mix = Math.min(1, b.depth / 4.5);
      ctx.strokeStyle = mix > 0.55 ? barkHi : barkLo;
      ctx.beginPath();
      for (let i = 0; i <= steps; i++) {
        const u = (i / steps) * f;
        let px = qbez(b.x0, b.cx, b.x1, u);
        let py = qbez(b.y0, b.cy, b.y1, u);
        // sway grows toward the tip of the branch and with depth
        const s = Math.sin(tSec * 0.6 + b.y0 * 0.012 + b.depth) * swayAmp * u;
        px += s;
        if (i === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      // taper the stroke along reveal
      ctx.lineWidth = Math.max(0.7, ((b.w0 + b.w1) / 2) * (0.55 + 0.45 * f));
      ctx.stroke();
    }

    // ── foliage: leaves bloom late, drawn as soft glowing dots ──
    for (const lf of t.leaves) {
      if (reveal <= lf.appearAt) continue;
      const grow = Math.min(1, (reveal - lf.appearAt) / Math.max(0.001, 1 - lf.appearAt));
      let lx = lf.x, ly = lf.y;
      let alpha = 0.8;
      if (onBreak) {
        const fall = (tSec * 7 + lf.sway * 9) % 70;
        lx += Math.sin(tSec * 0.8 + lf.sway) * 3 + breath;
        ly += Math.sin(tSec * 0.5 + lf.sway) * 2 + fall * 0.12;
        alpha = 0.5 + 0.32 * Math.sin(tSec * 0.9 + lf.sway);
      } else {
        lx += breath * 0.6;
      }
      const col = blend(folA, folB, lf.tone);
      const r = lf.r * (0.45 + 0.55 * grow);
      // soft radial leaf
      const g = ctx.createRadialGradient(lx, ly, 0, lx, ly, r * 1.8);
      g.addColorStop(0, withAlpha(col, alpha * Math.min(1, grow * 1.5)));
      g.addColorStop(1, withAlpha(col, 0));
      ctx.fillStyle = g;
      ctx.beginPath();
      ctx.arc(lx, ly, r * 1.8, 0, Math.PI * 2);
      ctx.fill();
    }

    // ── soft ground line + reflection-ish glow ──
    ctx.strokeStyle = barkLo;
    ctx.globalAlpha = 0.3;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(cw * 0.16, ch * 0.955);
    ctx.lineTo(cw * 0.84, ch * 0.955);
    ctx.stroke();
    ctx.globalAlpha = 1;
  }

  // colour helpers — accept hex or rgb()/oklab vars; fall back gracefully
  function withAlpha(c: string, a: number): string {
    return `color-mix(in oklab, ${c} ${Math.round(Math.max(0, Math.min(1, a)) * 100)}%, transparent)`;
  }
  function blend(a: string, b: string, t: number): string {
    return `color-mix(in oklab, ${a} ${Math.round((1 - t) * 100)}%, ${b})`;
  }

  // ─────────────────────────── rAF loop (panel-only) ───────────────────────────
  function loop(now: number) {
    // chime on phase change (the store advances phases on its own interval)
    if (pomo.phase !== lastChimePhase) {
      lastChimePhase = pomo.phase;
      chime();
    }
    draw(now);
    raf = requestAnimationFrame(loop);
  }

  // ─────────────────────────── Web Audio chime ───────────────────────────
  function chime() {
    try {
      if (!audioCtx) {
        const AC = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
        if (!AC) return;
        audioCtx = new AC();
      }
      const ctx = audioCtx;
      if (ctx.state === "suspended") ctx.resume().catch(() => {});
      const t0 = ctx.currentTime;
      const master = ctx.createGain();
      master.gain.value = 0.0001;
      master.connect(ctx.destination);
      master.gain.setValueAtTime(0.0001, t0);
      master.gain.exponentialRampToValueAtTime(0.16, t0 + 0.05);
      master.gain.exponentialRampToValueAtTime(0.0001, t0 + 2.6);

      const base = pomo.phase === "work" ? 528 : 440;
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
        osc.stop(t0 + 2.7);
      }
    } catch {
      /* audio is best-effort; never block the timer */
    }
  }

  // ─────────────────────────── lifecycle / cleanup ───────────────────────────
  function teardown() {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    palette = null; // re-read theme tokens on next open
    if (audioCtx) {
      audioCtx.close().catch(() => {});
      audioCtx = null;
    }
  }

  function close() {
    app.pomodoroOpen = false;
  }

  // Run the rAF draw loop only while the panel is open. The timer keeps ticking
  // in the store regardless, so closing the panel never pauses the session.
  $effect(() => {
    if (!app.pomodoroOpen) return;
    lastChimePhase = pomo.phase;
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
      pomo.pomoToggle();
    }
  }

  // settings inputs feed back through the store so durations apply globally
  function commitSettings(work: number, brk: number, long: number) {
    pomo.setDurations(work, brk, long);
  }
</script>

<svelte:window onkeydown={onKeyDown} />

{#if app.pomodoroOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay pom-overlay" role="presentation" onmousedown={close}>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="pom-modal" role="presentation" onmousedown={(e) => e.stopPropagation()}>
      <header class="pom-head">
        <div>
          <div class="eyebrow">{t("Focus ritual")}</div>
          <div class="pom-title">{t("Pomodoro")}</div>
        </div>
        <div class="pom-head-actions">
          <button
            class="btn btn--icon btn--sm btn--ghost"
            class:on={showSettings}
            title={t("Settings")}
            onclick={() => (showSettings = !showSettings)}
          >
            <Icon name="settings" size={13} />
          </button>
          <button class="btn btn--icon btn--sm btn--ghost" title={t("Close")} onclick={close}>
            <Icon name="x" size={12} />
          </button>
        </div>
      </header>

      <!-- the bonsai -->
      <div class="pom-canvas-wrap" class:break={pomo.phase !== "work"}>
        <canvas bind:this={canvas} class="pom-canvas"></canvas>
        <div class="pom-phase-tag mono">{pomo.phaseLabel}</div>
      </div>

      <!-- countdown -->
      <div class="pom-clock">
        <div class="pom-time mono" class:rest={pomo.phase !== "work"}>{pomo.mmss}</div>
        <div class="pom-cycles">
          {#each Array(pomo.sessionsBeforeLong) as _, i (i)}
            <span
              class="pom-dot"
              class:done={pomo.phase === "work" ? i < pomo.cycle - 1 : i < pomo.cycle}
              class:active={pomo.phase === "work" && i === pomo.cycle - 1}
            ></span>
          {/each}
        </div>
      </div>

      <!-- controls -->
      <div class="pom-controls">
        {#if pomo.running}
          <button class="btn btn--sm" onclick={() => pomo.pomoPause()}>
            <Icon name="pause" size={12} /> {t("Pause")}
          </button>
        {:else}
          <button class="btn btn--sm btn--primary" onclick={() => pomo.pomoStart()}>
            <Icon name="play" size={12} /> {pomo.progress > 0 ? t("Resume") : t("Start")}
          </button>
        {/if}
        <button class="btn btn--sm btn--ghost" onclick={() => pomo.pomoReset()} title={t("Reset phase")}>
          <Icon name="refresh" size={12} /> {t("Reset")}
        </button>
        <button class="btn btn--sm btn--ghost" onclick={() => pomo.pomoSkip()} title={t("Skip to next phase")}>
          <Icon name="arrowR" size={12} /> {t("Skip")}
        </button>
      </div>

      <!-- settings drawer -->
      {#if showSettings}
        <div class="pom-settings">
          <div class="pom-set-row">
            <label class="mono" for="pm-work">{t("Focus")}</label>
            <input
              id="pm-work" type="number" min="1" max="180" value={pomo.workMin}
              onchange={(e) => commitSettings(+(e.target as HTMLInputElement).value, pomo.breakMin, pomo.longBreakMin)}
            />
            <span class="pom-set-unit mono">{t("min")}</span>
          </div>
          <div class="pom-set-row">
            <label class="mono" for="pm-short">{t("Short break")}</label>
            <input
              id="pm-short" type="number" min="1" max="180" value={pomo.breakMin}
              onchange={(e) => commitSettings(pomo.workMin, +(e.target as HTMLInputElement).value, pomo.longBreakMin)}
            />
            <span class="pom-set-unit mono">{t("min")}</span>
          </div>
          <div class="pom-set-row">
            <label class="mono" for="pm-long">{t("Long break")}</label>
            <input
              id="pm-long" type="number" min="1" max="180" value={pomo.longBreakMin}
              onchange={(e) => commitSettings(pomo.workMin, pomo.breakMin, +(e.target as HTMLInputElement).value)}
            />
            <span class="pom-set-unit mono">{t("min")}</span>
          </div>
          <div class="pom-set-note mono">{t("Long break every {n}th focus session · {m} done today.", { n: pomo.sessionsBeforeLong, m: pomo.completedSessions })}</div>
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
    box-shadow: inset 0 0 70px color-mix(in oklab, var(--accent) 14%, transparent);
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
