<script lang="ts">
  import Icon from "./Icon.svelte";

  export interface ScriptSegment {
    speaker: string;
    text: string;
  }

  let {
    title = "Audio overview",
    script: scriptProp = [],
    onExit,
  }: { title?: string; script?: ScriptSegment[]; onExit?: () => void } = $props();

  // Build a normalized internal representation.
  // Each segment gets an estimated duration based on word count (~130 wpm).
  const segments = $derived.by(() => {
    if (!scriptProp || scriptProp.length === 0) return [];
    return scriptProp.map((seg) => ({
      speaker: seg.speaker,
      text: seg.text,
      // estimate ~130 words/min read-aloud speed
      d: Math.max(2, Math.round((seg.text.split(/\s+/).length / 130) * 60)),
    }));
  });

  // Derive unique speakers (in appearance order) for the hosts display.
  const speakers = $derived.by(() => {
    const seen = new Set<string>();
    const out: { name: string; color: string }[] = [];
    const palette = ["var(--accent)", "var(--mode-select)", "var(--info)", "var(--warn)"];
    let idx = 0;
    for (const seg of segments) {
      if (!seen.has(seg.speaker)) {
        seen.add(seg.speaker);
        out.push({ name: seg.speaker, color: palette[idx % palette.length] });
        idx++;
      }
    }
    return out;
  });

  function speakerColor(name: string): string {
    return speakers.find(s => s.name === name)?.color ?? "var(--fg-muted)";
  }

  // Compute cumulative start times and total duration
  const starts = $derived.by(() => {
    const arr: number[] = [];
    let acc = 0;
    for (const seg of segments) { arr.push(acc); acc += seg.d; }
    return arr;
  });
  const total = $derived(segments.reduce((a, s) => a + s.d, 0));

  // ── Playback state ──────────────────────────────────────────
  let playing = $state(false);
  let t       = $state(0);
  let speed   = $state<number>(1);
  const speeds = [1, 1.25, 1.5, 2] as const;

  // Transcript body ref for auto-scroll
  let bodyEl = $state<HTMLElement | null>(null);

  const curIdx = $derived.by(() => {
    if (segments.length === 0) return 0;
    let idx = 0;
    for (let n = 0; n < starts.length; n++) { if (t >= starts[n]) idx = n; }
    return idx;
  });

  // rAF playback loop
  $effect(() => {
    if (!playing || total === 0) return;
    let last: number | null = null;
    let rafId: number;
    const curSpeed = speed;

    function tick(now: number) {
      if (last == null) last = now;
      const dt = (now - last) / 1000 * curSpeed;
      last = now;
      t = Math.min(total, t + dt);
      if (t >= total) { playing = false; return; }
      rafId = requestAnimationFrame(tick);
    }

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  });

  // Auto-scroll active transcript line
  $effect(() => {
    const idx = curIdx;
    if (!bodyEl) return;
    const lines = bodyEl.querySelectorAll<HTMLElement>(".ao-line");
    const node = lines[idx];
    if (!node) return;
    const b = bodyEl.getBoundingClientRect();
    const n = node.getBoundingClientRect();
    if (n.top < b.top + 40 || n.bottom > b.bottom - 20) {
      node.scrollIntoView({ block: "center", behavior: "smooth" });
    }
  });

  function fmt(x: number) {
    return `${Math.floor(x / 60)}:${String(Math.floor(x % 60)).padStart(2, "0")}`;
  }

  function scrub(e: MouseEvent) {
    if (total === 0) return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    t = Math.max(0, Math.min(total, (e.clientX - r.left) / r.width * total));
  }

  function nextSpeed() {
    const idx = speeds.indexOf(speed as typeof speeds[number]);
    speed = speeds[(idx + 1) % speeds.length];
  }
</script>

<div class="ao">
  <!-- ── Left: player chrome ── -->
  <div class="ao-main">
    {#if onExit}
      <button class="btn btn--icon btn--sm btn--ghost ao-back" onclick={onExit} title="Back to materials">
        <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={14} /></span>
      </button>
    {/if}

    <!-- Artwork / visualizer -->
    <div class="ao-cover">
      <div class="ao-cover-art">
        <div class="ao-bars">
          {#each Array.from({ length: 28 }, (_, i) => i) as idx (idx)}
            <span
              class={playing ? "ao-bar live" : "ao-bar"}
              style:height="{playing
                ? 20 + Math.abs(Math.sin((t * 3 + idx) * 0.6)) * 70
                : 16 + (idx % 5) * 8}%"
              style:animation-delay="{idx * 40}ms"
            ></span>
          {/each}
        </div>
      </div>
    </div>

    <!-- Title / hosts -->
    <div class="ao-meta">
      <div class="eyebrow">Audio overview</div>
      <h1 class="ao-title read">{title}</h1>
      {#if speakers.length > 0}
        <div class="ao-hosts">
          {#each speakers as h, hidx (h.name)}
            {@const isSpeaking = segments.length > 0 && segments[curIdx]?.speaker === h.name && playing}
            <span class="ao-host{isSpeaking ? ' speaking' : ''}">
              <span class="ao-avatar" style:background={h.color}>{h.name[0]}</span>
              {h.name} <span class="faint">· {hidx === 0 ? "host" : "co-host"}</span>
            </span>
          {/each}
        </div>
      {/if}
    </div>

    {#if segments.length === 0}
      <p class="mono faint" style="font-size: var(--t-sm);">No audio script.</p>
    {:else}
      <!-- Scrubber -->
      <div class="ao-scrubber">
        <span class="ao-time mono">{fmt(t)}</span>
        <div class="ao-track" onclick={scrub} role="slider" aria-valuenow={t} aria-valuemin={0} aria-valuemax={total} tabindex="0">
          <div class="ao-fill" style:width="{total > 0 ? (t / total * 100) : 0}%">
            <span class="ao-knob"></span>
          </div>
        </div>
        <span class="ao-time mono">{fmt(total)}</span>
      </div>

      <!-- Controls -->
      <div class="ao-controls">
        <button class="btn btn--icon btn--ghost" onclick={() => { t = Math.max(0, t - 15); }} title="Back 15s">
          <span style="display:inline-flex;transform:scaleX(-1)"><Icon name="refresh" size={15} /></span>
        </button>
        <button class="ao-play" onclick={() => { playing = !playing; }}>
          {#if playing}
            <Icon name="pause" size={22} color="var(--accent-fg)" />
          {:else}
            <Icon name="play"  size={22} color="var(--accent-fg)" />
          {/if}
        </button>
        <button class="btn btn--icon btn--ghost" onclick={() => { t = Math.min(total, t + 15); }} title="Forward 15s">
          <Icon name="refresh" size={15} />
        </button>
        <button class="ao-speed mono" onclick={nextSpeed}>{speed}×</button>
      </div>
    {/if}
  </div>

  <!-- ── Right: transcript ── -->
  <aside class="ao-transcript">
    <div class="ao-tr-head">
      <span class="label">Transcript</span>
      {#if segments.length > 0}
        <span class="mono faint">{segments.length} turns · synced</span>
      {:else}
        <span class="mono faint">no script</span>
      {/if}
    </div>
    <div class="ao-tr-body" bind:this={bodyEl}>
      {#if segments.length === 0}
        <p class="mono faint" style="padding: 16px; font-size: var(--t-sm);">No audio script.</p>
      {:else}
        {#each segments as seg, idx (idx)}
          {@const col = speakerColor(seg.speaker)}
          <div
            class="ao-line{idx === curIdx ? ' cur' : ''}{idx < curIdx ? ' past' : ''}"
            onclick={() => { t = starts[idx]; }}
            role="button"
            tabindex="0"
            onkeydown={(e) => { if (e.key === "Enter") t = starts[idx]; }}
          >
            <div class="ao-line-head">
              <span class="ao-avatar sm" style:background={col}>{seg.speaker[0]}</span>
              <span class="ao-spk mono">{seg.speaker}</span>
              <span class="ao-ts mono">{fmt(starts[idx])}</span>
            </div>
            <p class="ao-text read">{seg.text}</p>
          </div>
        {/each}
      {/if}
    </div>
  </aside>
</div>
