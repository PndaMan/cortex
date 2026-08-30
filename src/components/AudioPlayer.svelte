<script lang="ts">
  import Icon from "./Icon.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import * as api from "../lib/api";
  import { app } from "../lib/store.svelte";
  import { t } from "../lib/i18n.svelte";

  export interface ScriptSegment {
    speaker: string;
    text: string;
  }

  let {
    title = t("Audio overview"),
    script: scriptProp = [],
    materialId,
    onExit,
  }: { title?: string; script?: ScriptSegment[]; materialId?: string; onExit?: () => void } = $props();

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

  // Real-audio state (declared early so the timeline deriveds below can read it).
  let audioSrc = $state<string | null>(null);
  let realTotal = $state(0); // real audio duration (s), set on loadedmetadata
  const realMode = $derived(!!audioSrc);

  // When playing the real audio file, sync the transcript to its TRUE duration:
  // scale the per-segment estimates so the timeline matches the actual length.
  const effTotal = $derived(realMode && realTotal > 0 ? realTotal : total);
  const effStarts = $derived.by(() => {
    if (realMode && realTotal > 0 && total > 0) {
      const k = realTotal / total;
      return starts.map((s) => s * k);
    }
    return starts;
  });

  // ── Playback state ──────────────────────────────────────────
  let playing = $state(false);
  let pos     = $state(0);
  let speed   = $state<number>(1);
  const speeds = [1, 1.25, 1.5, 2] as const;

  // ── REAL audio (cloud TTS) ──────────────────────────────────
  // When the user generates a real audio file we play it through an <audio>
  // element and sync the transcript to its true duration. Until then (or when
  // offline / no key) we fall back to on-device speech synthesis / the visual
  // timeline below. (audioSrc / realTotal / realMode are declared above.)
  let audioEl  = $state<HTMLAudioElement | null>(null);
  let generating = $state(false);

  async function generateRealAudio() {
    if (!materialId || generating || scriptProp.length === 0) return;
    generating = true;
    try {
      const path = await api.synthesizeOverview(materialId, scriptProp);
      audioSrc = convertFileSrc(path);
      playing = false;
      pos = 0;
      app.pushToast({ kind: "success", title: t("Audio ready"), body: t("Generated a real audio overview.") });
    } catch (e) {
      app.pushToast({
        kind: "error",
        title: t("Couldn't generate audio"),
        body: String(e instanceof Error ? e.message : e),
      });
    } finally {
      generating = false;
    }
  }

  // Keep the <audio> element's rate in sync with the speed control.
  $effect(() => {
    if (audioEl) audioEl.playbackRate = speed;
  });

  // ── real voice playback via the Web Speech API (free, no key). Needs a
  // system TTS engine (speech-dispatcher / espeak-ng); if none is present we
  // fall back to the silent visual timeline and show a hint.
  const synth = typeof window !== "undefined" ? window.speechSynthesis : undefined;
  let voicesAvailable = $state(false);
  $effect(() => {
    if (!synth) return;
    const load = () => (voicesAvailable = synth.getVoices().length > 0);
    load();
    synth.addEventListener?.("voiceschanged", load);
    return () => synth.removeEventListener?.("voiceschanged", load);
  });
  function voiceFor(speaker: string): SpeechSynthesisVoice | null {
    const vs = synth?.getVoices() ?? [];
    if (vs.length === 0) return null;
    const idx = Math.max(0, speakers.findIndex((s) => s.name === speaker));
    return vs[idx % vs.length];
  }

  // Transcript body ref for auto-scroll
  let bodyEl = $state<HTMLElement | null>(null);

  const curIdx = $derived.by(() => {
    if (segments.length === 0) return 0;
    let idx = 0;
    for (let n = 0; n < effStarts.length; n++) { if (pos >= effStarts[n]) idx = n; }
    return idx;
  });

  // rAF playback loop — drives the SILENT visual timeline only when no TTS
  // voices are available (otherwise the speech effect below drives playback).
  $effect(() => {
    if (!playing || total === 0 || voicesAvailable || realMode) return;
    let last: number | null = null;
    let rafId: number;
    const curSpeed = speed;

    function tick(now: number) {
      if (last == null) last = now;
      const dt = (now - last) / 1000 * curSpeed;
      last = now;
      pos = Math.min(total, pos + dt);
      if (pos >= total) { playing = false; return; }
      rafId = requestAnimationFrame(tick);
    }

    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  });

  // Speech-driven playback: read each segment aloud with its host's voice,
  // advancing the timeline as each utterance ends.
  $effect(() => {
    if (!playing || !synth || !voicesAvailable || segments.length === 0 || realMode) return;
    let cancelled = false;
    const startIdx = pos >= total ? 0 : curIdx;
    function speakFrom(n: number) {
      if (cancelled) return;
      if (n >= segments.length) { pos = total; playing = false; return; }
      pos = starts[n];
      const u = new SpeechSynthesisUtterance(segments[n].text);
      u.rate = speed;
      const v = voiceFor(segments[n].speaker);
      if (v) u.voice = v;
      u.onend = () => { if (!cancelled) speakFrom(n + 1); };
      synth!.speak(u);
    }
    synth.cancel();
    speakFrom(startIdx);
    return () => { cancelled = true; synth.cancel(); };
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

  function seekTo(nt: number) {
    const clamped = Math.max(0, Math.min(effTotal, nt));
    pos = clamped;
    // In real mode the bound currentTime follows `pos`, but set it explicitly too
    // so a seek while paused takes effect immediately.
    if (realMode && audioEl) audioEl.currentTime = clamped;
  }

  function scrub(e: MouseEvent) {
    if (effTotal === 0) return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    seekTo((e.clientX - r.left) / r.width * effTotal);
  }

  function togglePlay() {
    if (realMode && audioEl) {
      if (audioEl.paused) audioEl.play().catch(() => {});
      else audioEl.pause();
    } else {
      playing = !playing;
    }
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
      <button class="btn btn--icon btn--sm btn--ghost ao-back" onclick={onExit} title={t("Back to materials")}>
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
                ? 20 + Math.abs(Math.sin((pos * 3 + idx) * 0.6)) * 70
                : 16 + (idx % 5) * 8}%"
              style:animation-delay="{idx * 40}ms"
            ></span>
          {/each}
        </div>
      </div>
    </div>

    <!-- Title / hosts -->
    <div class="ao-meta">
      <div class="eyebrow">{t("Audio overview")}</div>
      <h1 class="ao-title read">{title}</h1>
      {#if speakers.length > 0}
        <div class="ao-hosts">
          {#each speakers as h, hidx (h.name)}
            {@const isSpeaking = segments.length > 0 && segments[curIdx]?.speaker === h.name && playing}
            <span class="ao-host{isSpeaking ? ' speaking' : ''}">
              <span class="ao-avatar" style:background={h.color}>{h.name[0]}</span>
              {h.name} <span class="faint">· {hidx === 0 ? t("host") : t("co-host")}</span>
            </span>
          {/each}
        </div>
      {/if}
    </div>

    {#if segments.length === 0}
      <p class="mono faint" style="font-size: var(--t-sm);">{t("No audio script.")}</p>
    {:else}
      {#if realMode}
        <!-- The real generated audio file; drives the timeline + transcript sync. -->
        <!-- svelte-ignore a11y_media_has_caption -->
        <audio
          bind:this={audioEl}
          src={audioSrc}
          bind:currentTime={pos}
          bind:duration={realTotal}
          onplay={() => (playing = true)}
          onpause={() => (playing = false)}
          onended={() => (playing = false)}
        ></audio>
        <p class="mono faint" style="font-size: var(--t-xs); margin: 0 0 6px; color: var(--ok);">
          {t("● Real audio — narrated by your cloud voices.")}
        </p>
      {:else}
        <!-- Offline / not-yet-generated: real audio button + on-device fallback. -->
        {#if materialId}
          <button class="btn btn--sm btn--primary ao-gen" onclick={generateRealAudio} disabled={generating}>
            <Icon name="bolt" size={13} /> {generating ? t("Generating audio…") : t("Generate real audio")}
          </button>
        {/if}
        {#if !voicesAvailable}
          <p class="mono faint" style="font-size: var(--t-xs); margin: 6px 0 6px;">
            {t("Offline preview uses on-device voices — install")} <span class="kbd">espeak-ng</span> {t("or")}
            <span class="kbd">speech-dispatcher</span> {t("for spoken playback, or generate real audio above.")}
          </p>
        {/if}
      {/if}
      <!-- Scrubber -->
      <div class="ao-scrubber">
        <span class="ao-time mono">{fmt(pos)}</span>
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="ao-track" onclick={scrub} role="slider" aria-valuenow={pos} aria-valuemin={0} aria-valuemax={effTotal} tabindex="0">
          <div class="ao-fill" style:width="{effTotal > 0 ? (pos / effTotal * 100) : 0}%">
            <span class="ao-knob"></span>
          </div>
        </div>
        <span class="ao-time mono">{fmt(effTotal)}</span>
      </div>

      <!-- Controls -->
      <div class="ao-controls">
        <button class="btn btn--icon btn--ghost" onclick={() => seekTo(pos - 15)} title={t("Back 15s")}>
          <span style="display:inline-flex;transform:scaleX(-1)"><Icon name="refresh" size={15} /></span>
        </button>
        <button class="ao-play" onclick={togglePlay}>
          {#if playing}
            <Icon name="pause" size={22} color="var(--accent-fg)" />
          {:else}
            <Icon name="play"  size={22} color="var(--accent-fg)" />
          {/if}
        </button>
        <button class="btn btn--icon btn--ghost" onclick={() => seekTo(pos + 15)} title={t("Forward 15s")}>
          <Icon name="refresh" size={15} />
        </button>
        <button class="ao-speed mono" onclick={nextSpeed}>{speed}×</button>
      </div>
    {/if}
  </div>

  <!-- ── Right: transcript ── -->
  <aside class="ao-transcript">
    <div class="ao-tr-head">
      <span class="label">{t("Transcript")}</span>
      {#if segments.length > 0}
        <span class="mono faint">{t("{n} turns · synced", { n: segments.length })}</span>
      {:else}
        <span class="mono faint">{t("no script")}</span>
      {/if}
    </div>
    <div class="ao-tr-body" bind:this={bodyEl}>
      {#if segments.length === 0}
        <p class="mono faint" style="padding: 16px; font-size: var(--t-sm);">{t("No audio script.")}</p>
      {:else}
        {#each segments as seg, idx (idx)}
          {@const col = speakerColor(seg.speaker)}
          <div
            class="ao-line{idx === curIdx ? ' cur' : ''}{idx < curIdx ? ' past' : ''}"
            onclick={() => { pos = starts[idx]; }}
            role="button"
            tabindex="0"
            onkeydown={(e) => { if (e.key === "Enter") pos = starts[idx]; }}
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
