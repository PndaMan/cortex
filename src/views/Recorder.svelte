<script lang="ts">
  import { app } from "../lib/store.svelte";
  import { rec, liveTranscriptSupported } from "../lib/recorder.svelte";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import { isMobile } from "../lib/platform";

  // ─────────────────────────────────────────────────────────────────────────
  // The capture engine lives in src/lib/recorder.svelte.ts (global) so a
  // recording is a BACKGROUND ACTIVITY: leaving this view keeps recording, and
  // the floating RecordingActivity widget mirrors it from any screen. This
  // component is only the full-size recorder UI.
  // ─────────────────────────────────────────────────────────────────────────

  // Waveform draws to ONE canvas inside rAF — per-frame DOM work crawls on
  // WebKitGTK's software renderer.
  let waveCanvas: HTMLCanvasElement | null = $state(null);

  // Save-screen pickers: any subject in the library, and the CHOSEN subject's
  // topics (not just the active one — a lecture can be filed anywhere).
  const subjectOptions = $derived(
    app.subjects.map((s) => ({ id: s.id, label: s.code ? `${s.name} · ${s.code}` : s.name })),
  );
  const topicOptions = $derived([
    { id: "", label: "— no topic —" },
    ...(app.subjects.find((s) => s.id === rec.reviewSubjectId)?.topics ?? []).map((t) => ({ id: t.id, label: t.name })),
  ]);

  // ---- transcript auto-scroll (anchored to newest text unless the user scrolls up) ----
  let rtBody: HTMLDivElement | null = $state(null);
  let rtPinned = $state(true); // stay glued to the bottom while true
  function onRtScroll() {
    const el = rtBody;
    if (!el) return;
    // Re-pin once the user returns to within a hair of the bottom.
    rtPinned = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
  }

  // ---- subtle animated waveform (Web Audio analyser on desktop, native level
  //      metering on iOS), rAF-driven ----
  const WAVE_N = 48;
  function drawWave(levels: Float32Array | null) {
    const cv = waveCanvas;
    if (!cv) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const w = cv.clientWidth * dpr;
    const h = cv.clientHeight * dpr;
    if (cv.width !== w || cv.height !== h) { cv.width = w; cv.height = h; }
    const ctx2d = cv.getContext("2d");
    if (!ctx2d || w === 0) return;
    ctx2d.clearRect(0, 0, w, h);
    ctx2d.fillStyle = getComputedStyle(cv).color || "#7aa2f7";
    const slot = w / WAVE_N;
    const barW = Math.max(1, slot * 0.55);
    for (let i = 0; i < WAVE_N; i++) {
      const v = levels ? Math.max(0.06, levels[i]) : 0.06;
      ctx2d.globalAlpha = levels ? 0.5 + v * 0.5 : 0.22;
      const bh = Math.max(1, v * h);
      ctx2d.fillRect(i * slot + (slot - barW) / 2, (h - bh) / 2, barW, bh);
    }
    ctx2d.globalAlpha = 1;
  }
  $effect(() => {
    const analyser = rec.analyser;
    if (!rec.live || (!analyser && !rec.native)) {
      drawWave(null); // idle baseline
      return;
    }
    const freq = analyser ? new Uint8Array(analyser.frequencyBinCount) : null;
    const levels = new Float32Array(WAVE_N);
    // Log-spaced bin edges over the voice band (~100 Hz–8 kHz). A linear mapping
    // put all of speech's energy (low frequencies) into the left-most bars and drew
    // ultrasonic silence on the right — the strip looked heavily weighted left.
    // Log spacing gives each bar a perceptually similar slice, so talking lights
    // the whole strip. Bars can round onto the same low bin; the max(a+1, b) guard
    // below keeps every bar at least one bin wide.
    const edges: number[] = [];
    if (freq && analyser) {
      const nyquist = (analyser.context?.sampleRate ?? 48000) / 2;
      const binHz = nyquist / freq.length;
      const lo = 100, hi = Math.min(8000, nyquist);
      for (let i = 0; i <= WAVE_N; i++) {
        edges.push(Math.min(freq.length - 1, Math.round((lo * (hi / lo) ** (i / WAVE_N)) / binHz)));
      }
    }
    let rafId = 0;
    let skip = false;
    function tick() {
      rafId = requestAnimationFrame(tick);
      skip = !skip; // ~30fps is plenty for ambience and halves render cost
      if (skip) return;
      if (analyser && freq) {
        analyser.getByteFrequencyData(freq);
        for (let i = 0; i < WAVE_N; i++) {
          const a = edges[i];
          const b = Math.max(a + 1, edges[i + 1]);
          let s = 0;
          for (let j = a; j < b; j++) s += freq[j] ?? 0;
          // gentle curve so quiet rooms still show a soft baseline, loud peaks don't clip
          levels[i] = Math.min(1, (s / (b - a) / 255) * 1.7 + 0.04);
        }
      } else {
        // Native engine: one metering level — synthesize a soft symmetric hump.
        const base = Math.min(1, rec.nativeLevel * 1.6 + 0.04);
        for (let i = 0; i < WAVE_N; i++) {
          const centered = 1 - Math.abs(i - WAVE_N / 2) / (WAVE_N / 2);
          levels[i] = base * (0.35 + 0.65 * centered) * (0.85 + 0.3 * Math.sin(i * 1.7 + performance.now() / 180));
        }
      }
      drawWave(levels);
    }
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  });

  // ---- auto-scroll the transcript to the newest text (unless the user scrolled up) ----
  $effect(() => {
    // Touch the streams so this re-runs whenever new text lands.
    void rec.liveFinal; void rec.liveInterim; void rec.liveBackendText;
    const el = rtBody;
    if (el && rtPinned) el.scrollTop = el.scrollHeight;
  });

  // Leave the recorder view. The recording (if any) KEEPS RUNNING as a
  // background activity — the floating widget takes over.
  function leave() {
    app.setView("subject");
  }

  // Explicitly abort the take and throw the audio away.
  function discard() {
    rec.discardRecording();
    app.setView("subject");
  }

  // Fallback: pick a pre-recorded audio file and run it through the same pipeline.
  async function uploadAudioFile(e: Event) {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    input.value = ""; // allow re-picking the same file
    if (!file) return;
    if (!app.activeSubject) {
      app.pushToast({ kind: "error", title: "Open a subject first", body: "Select a subject before adding audio." });
      return;
    }
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      // Keep the real container so the backend stores a matching extension.
      const ext = file.name.split(".").pop()?.toLowerCase() || "webm";
      // Route uploads through the same review step so they can be named/topic-tagged too.
      rec.enterReviewFromUpload(bytes, file.name, ext);
    } catch (err) {
      rec.errorMsg = String(err);
    }
  }

  // keyboard: space toggles start/pause, enter stops, m tags
  $effect(() => {
    window.__cortexModalOpen = true;
    function onKey(e: KeyboardEvent) {
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
      if (rec.status === "review") {
        if (e.key === "Enter") { e.preventDefault(); void rec.confirmSave(); }
        else if (e.key === "Escape") { e.preventDefault(); rec.discardReview(); }
        return;
      }
      // While the previous take is saving/transcribing, only Esc (leave) works —
      // space must not start a fresh session over the in-flight one.
      if (rec.status === "transcribing") {
        if (e.key === "Escape") leave();
        return;
      }
      if (e.key === " ") { e.preventDefault(); rec.recording ? rec.togglePause() : void rec.start(); }
      else if (e.key === "Enter" && rec.recording) { e.preventDefault(); rec.stop(); }
      else if (e.key === "m") { e.preventDefault(); rec.tagMoment(); }
      // "t" collapses / reopens the live transcript panel (recorder-local only).
      else if (e.key.toLowerCase() === "t") { e.preventDefault(); rec.toggleTranscriptPanel(); }
      // Esc leaves the view — the recording continues in the background.
      else if (e.key === "Escape") leave();
    }
    window.addEventListener("keydown", onKey);
    return () => { window.removeEventListener("keydown", onKey); window.__cortexModalOpen = false; };
  });
</script>

{#if rec.status === "review"}
  <!-- ─────────── REVIEW & SAVE ───────────
       Centered, monospace-chrome step between stopping and committing. -->
  <div class="rev-wrap">
    <div class="rev-card">
      <div class="rev-chrome mono">
        <span class="rev-led"></span>
        REVIEW &amp; SAVE
        <span class="grow"></span>
        <span class="rev-dur">{rec.reviewSourceLabel === "uploaded" ? "FILE" : rec.reviewDuration}</span>
      </div>

      <div class="rev-body">
        <div class="field">
          <span class="onb-label mono">NAME <span class="faint">how this source is titled</span></span>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="input"
            autofocus
            bind:value={rec.reviewName}
            placeholder="Untitled recording"
          />
        </div>

        <div class="field" style:margin-top="16px">
          <span class="onb-label mono">SUBJECT <span class="faint">which course this lecture belongs to</span></span>
          <Picker
            value={rec.reviewSubjectId}
            onChange={(id) => rec.setReviewSubject(id)}
            options={subjectOptions}
            placeholder="Pick a subject"
          />
        </div>

        <div class="field" style:margin-top="16px">
          <span class="onb-label mono">TOPIC <span class="faint">where this recording lives</span></span>
          <Picker
            value={rec.reviewTopicId}
            onChange={(id) => (rec.reviewTopicId = id)}
            options={topicOptions}
            placeholder="— no topic —"
          />
        </div>

        <div class="rev-speakers" style:margin-top="16px">
          <div class="rev-speakers-l">
            <span class="onb-label mono">MULTIPLE PEOPLE SPEAKING</span>
            <span class="rev-speakers-d">Label the transcript by voice — “Speaker 1 / Speaker 2” (homelab WhisperX or a diarizing cloud model).</span>
          </div>
          <button
            type="button"
            class={"st-toggle" + (rec.reviewDiarize ? " on" : "")}
            onclick={() => (rec.reviewDiarize = !rec.reviewDiarize)}
            role="switch"
            aria-checked={rec.reviewDiarize}
            aria-label="multiple people speaking"
          ><span class="st-knob"></span></button>
        </div>

        <div class="rev-meta mono faint">
          <span class="rev-meta-item"><Icon name="bolt" size={11} color="var(--fg-faint)" />{rec.reviewSourceLabel === "uploaded" ? "Uploaded audio file" : `Captured ${rec.reviewDuration}`}</span>
          {#if rec.reviewTranscript.trim()}
            <span class="rev-meta-item"><Icon name="doc" size={11} color="var(--fg-faint)" />Live transcript captured</span>
          {/if}
        </div>

        {#if rec.reviewTranscript.trim()}
          <div class="field" style:margin-top="14px">
            <span class="onb-label mono">TRANSCRIPT PREVIEW <span class="faint">re-transcribed precisely on save</span></span>
            <div class="rev-transcript read">{rec.reviewTranscript}</div>
          </div>
        {/if}

        {#if rec.errorMsg}
          <div class="rev-error">{rec.errorMsg}</div>
        {/if}
      </div>

      <div class="rev-actions">
        <button class="btn btn--ghost rev-discard" onclick={() => rec.discardReview()}>Discard</button>
        <span class="grow"></span>
        <button class="btn btn--primary" onclick={() => rec.confirmSave()}>Save recording</button>
      </div>

      <div class="rev-hint mono faint">
        <span class="kbd">⏎</span> save · <span class="kbd">esc</span> discard
      </div>
    </div>
  </div>
{:else}
<div class="recorder" class:transcript-collapsed={rec.transcriptCollapsed}>
  <!-- Left: stage -->
  <div class="rec-stage">
    <div class="rec-status mono">
      <span class="rec-led{rec.live ? ' live' : ''}"></span>
      {rec.status === "transcribing" ? "TRANSCRIBING" : rec.recording ? (rec.paused ? "PAUSED" : "RECORDING") : rec.status === "done" ? "DONE" : "READY"}
      <span class="grow"></span>
      <span class="rec-clock">{rec.mm}:{rec.ss}</span>
    </div>

    <!-- Subtle, secondary live waveform (mirrored frequency bars, rAF-driven,
         single canvas — no per-frame DOM work). -->
    <div class="waveform waveform--compact" class:is-live={rec.live} aria-hidden="true">
      <canvas bind:this={waveCanvas} class="wave-canvas"></canvas>
      <div class="wf-center"></div>
    </div>

    <div class="rec-controls">
      {#if rec.status === "transcribing"}
        <span class="is-spin" style:width="22px" style:height="22px"></span>
      {:else if !rec.recording}
        <button class="rec-btn rec-btn--go" onclick={() => rec.start()} title="Start recording">
          <span class="rec-btn-dot"></span>
        </button>
      {:else}
        <button class="btn btn--icon" onclick={() => rec.togglePause()} title={rec.paused ? "Resume" : "Pause"}>
          {#if rec.paused}<Icon name="play" size={15} />{:else}<Icon name="pause" size={15} />{/if}
        </button>
        <button class="rec-btn rec-btn--stop" onclick={() => rec.stop()} title="Stop & save"><span class="rec-stop-sq"></span></button>
        <button class="btn btn--icon" onclick={() => rec.tagMoment()} title="Tag moment (m)"><Icon name="bolt" size={15} color="var(--warn)" /></button>
      {/if}
    </div>

    <div class="rec-hint mono faint">
      {#if rec.status === "transcribing"}
        Transcribing with Whisper…
      {:else if !rec.recording}
        Press <span class="kbd">␣</span> or click to start · output becomes a transcribed source
      {:else}
        <span class="kbd">m</span> tag moment · <span class="kbd">space</span> pause · <span class="kbd">⏎</span> stop &amp; save · leaving this screen keeps recording
      {/if}
    </div>

    {#if rec.errorMsg}
      <div style:color="var(--err)" style:margin-top="14px" style:font-size="var(--t-sm)" style:max-width="420px" style:text-align="center">{rec.errorMsg}</div>
    {/if}

    <!-- Fallback: upload a pre-recorded audio file (always available, emphasised on error) -->
    {#if rec.status !== "transcribing"}
      <div class="rec-upload mono faint" style:margin-top={rec.errorMsg ? "12px" : "18px"}>
        {#if rec.errorMsg}Can't use the mic? {/if}
        <label class="btn btn--ghost btn--sm" style:cursor="pointer">
          <Icon name="doc" size={13} />
          Upload an audio file
          <input type="file" accept="audio/*" onchange={uploadAudioFile} style:display="none" />
        </label>
      </div>
    {/if}

    {#if rec.tags.length > 0}
      <div class="rec-tags">
        {#each rec.tags as tag, i (i)}
          <span class="rec-tag mono"><Icon name="bolt" size={10} color="var(--warn)" />{tag.at}</span>
        {/each}
      </div>
    {/if}

    {#if isMobile}
      <div style:display="flex" style:gap="10px" style:margin-top="18px">
        {#if rec.recording}
          <button class="btn btn--ghost btn--sm" onclick={discard}>Discard</button>
          <button class="btn btn--ghost btn--sm" onclick={leave}>Hide (keeps recording)</button>
        {:else}
          <button class="btn btn--ghost btn--sm" onclick={leave}>Close</button>
        {/if}
      </div>
    {/if}
  </div>

  <!-- Right: live transcript / status panel (desktop only — mobile transcribes the
       saved audio via homelab Whisper on stop; no live transcript). -->
  {#if !isMobile}
  <aside class="rec-transcript">
    <div class="rt-head">
      <span class="rt-eyebrow mono">LIVE TRANSCRIPT</span>
      <span class="grow"></span>
      {#if rec.status === "transcribing"}
        <span class="status-pill status-pill--draft"><span class="dot dot--pulse"></span>processing</span>
      {:else if rec.live && rec.liveTranscriptOn && rec.liveUpdating}
        <span class="status-pill status-pill--draft"><span class="dot dot--pulse"></span>transcribing…</span>
      {:else if rec.live && rec.liveTranscriptOn}
        <span class="status-pill status-pill--draft"><span class="dot dot--pulse"></span>listening</span>
      {/if}
      <!-- Closing the panel also stops live transcription; "t" toggles it. -->
      <button class="btn btn--icon btn--sm btn--ghost rt-collapse" title="Close transcript (t)" onclick={() => rec.toggleTranscriptPanel()}>
        <Icon name="chevron" size={13} />
      </button>
      <span class="kbd rt-kbd" title="Press t to toggle">t</span>
    </div>
    <div class="rt-body" bind:this={rtBody} onscroll={onRtScroll}>
      {#if rec.status === "transcribing"}
        <div class="rt-empty mono faint">{rec.note || "Running Whisper on your recording…"}</div>
        {#if rec.liveFinal.trim()}
          <p class="rt-live read rt-live--dim">{rec.liveFinal}</p>
        {:else if rec.liveBackendText.trim()}
          <p class="rt-live read rt-live--dim">{rec.liveBackendText}</p>
        {/if}
      {:else if rec.recording && liveTranscriptSupported}
        <!-- Real-time path: browser SpeechRecognition (final + interim). -->
        {#if rec.hasLiveTranscript}
          <p class="rt-live read">
            {rec.liveFinal}<span class="rt-interim">{rec.liveInterim}</span>
          </p>
        {:else}
          <div class="rt-listening mono faint"><span class="rt-shimmer">Listening</span><span class="rt-ell"></span></div>
        {/if}
      {:else if rec.recording && rec.whisperMissing}
        <!-- Backend fallback tried, came back empty: no Whisper installed. Be honest. -->
        <div class="rt-note rt-note--warn mono">
          <span class="rt-note-title">Live transcript needs Whisper</span>
          No Whisper backend answered. Configure a homelab Whisper server in Settings, or install
          faster-whisper locally — the recording is still saved and transcribed when you stop.
        </div>
      {:else if rec.recording}
        <!-- Backend chunked fallback (WebKitGTK / Tauri Linux): refreshes every ~7s. -->
        {#if rec.liveBackendText.trim()}
          <p class="rt-live read">{rec.liveBackendText}</p>
        {:else}
          <div class="rt-listening mono faint"><span class="rt-shimmer">Listening</span><span class="rt-ell"></span></div>
        {/if}
      {:else}
        <div class="rt-empty mono faint">
          Hit record to capture a lecture. On stop, Cortex transcribes it with Whisper and saves it as a searchable source.
          A live transcript appears here while you record — close it with <span class="kbd">t</span>; closed means transcription is off until you reopen it.
          Leaving this screen mid-recording keeps capturing — a small floating widget follows you around the app.
        </div>
      {/if}
    </div>
    {#if rec.recording}
      <button class="btn btn--ghost btn--sm rt-close" onclick={discard}>Discard recording</button>
    {:else}
      <button class="btn btn--ghost btn--sm rt-close" onclick={leave}>Close</button>
    {/if}
  </aside>

  {#if rec.transcriptCollapsed}
    <button class="rt-reopen mono" title="Open live transcript (t)" onclick={() => rec.toggleTranscriptPanel()}>
      <span style="display:inline-flex;transform:rotate(180deg)"><Icon name="chevron" size={13} /></span>
      <span class="kbd rt-kbd">t</span>
    </button>
  {/if}
  {/if}
</div>
{/if}

<style>
  /* ---- compact, secondary waveform (the small mm:ss in .rec-status is the timer) ---- */
  .waveform--compact { height: 64px; max-width: 460px; opacity: 0.9; }

  /* ---- waveform: subtle idle breathing so it never looks dead ---- */
  .waveform :global(.wbar) {
    transition: height 80ms linear, opacity 80ms linear;
  }
  .waveform:not(.is-live) :global(.wbar) {
    animation: rec-idle 2.6s ease-in-out infinite;
  }
  .waveform:not(.is-live) :global(.wbar):nth-child(odd) { animation-delay: -1.3s; }
  @keyframes rec-idle {
    0%, 100% { transform: scaleY(0.7); }
    50% { transform: scaleY(1.6); }
  }

  /* ---- live transcript text ---- */
  .rt-live {
    margin: 0;
    font-size: var(--t-md);
    line-height: 1.7;
    color: var(--fg);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .rt-live--dim { color: var(--fg-muted); }
  .rt-interim {
    color: var(--fg-muted);
    font-style: italic;
    opacity: 0.85;
  }
  .rt-note {
    line-height: 1.7;
    padding: 12px 14px;
    border: 1px dashed var(--border-strong);
    border-radius: var(--rad-3);
    background: color-mix(in oklab, var(--surface-2) 60%, transparent);
    font-size: var(--t-xs);
  }
  /* Honest "no Whisper" callout — solid warn-tinted card, not a faint dashed note. */
  .rt-note--warn {
    border: 1px solid color-mix(in oklab, var(--warn) 45%, var(--border-strong));
    background: color-mix(in oklab, var(--warn) 10%, transparent);
    color: var(--fg-muted);
  }
  .rt-note-title {
    display: block;
    margin-bottom: 5px;
    color: var(--warn);
    letter-spacing: 0.04em;
    text-transform: uppercase;
    font-size: var(--t-2xs);
  }

  /* ---- header eyebrow + keybind chip ---- */
  .rt-eyebrow {
    font-size: var(--t-2xs);
    letter-spacing: 0.12em;
    color: var(--fg-muted);
  }
  .rt-kbd { flex: none; margin-left: 6px; text-transform: lowercase; }

  /* ---- pulsing status dot ---- */
  .status-pill .dot--pulse { animation: rt-pulse 1.4s ease-in-out infinite; }
  @keyframes rt-pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50% { opacity: 0.35; transform: scale(0.78); }
  }

  /* ---- "Listening…" shimmer while waiting for the first words ---- */
  .rt-listening {
    display: inline-flex;
    align-items: baseline;
    padding: 14px 4px;
    font-size: var(--t-sm);
    letter-spacing: 0.03em;
  }
  .rt-shimmer {
    background: linear-gradient(
      90deg,
      var(--fg-faint) 0%, var(--fg-faint) 38%,
      var(--fg-bright) 50%,
      var(--fg-faint) 62%, var(--fg-faint) 100%
    );
    background-size: 220% 100%;
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
    animation: rt-shimmer 2.1s linear infinite;
  }
  @keyframes rt-shimmer { 0% { background-position: 120% 0; } 100% { background-position: -120% 0; } }
  /* animated ellipsis dots */
  .rt-ell::after {
    content: "";
    animation: rt-ell 1.4s steps(4, end) infinite;
  }
  @keyframes rt-ell {
    0% { content: ""; } 25% { content: "."; } 50% { content: ".."; } 75%, 100% { content: "..."; }
  }

  /* ---- Live transcript is now the primary feature: give the panel more real estate ---- */
  .recorder { grid-template-columns: 1fr clamp(420px, 42vw, 560px); position: relative; }
  /* Collapsed: hand the whole width to the recorder; show a reopen tab on the edge. */
  .recorder.transcript-collapsed { grid-template-columns: 1fr; }
  .recorder.transcript-collapsed .rec-transcript { display: none; }
  .rt-collapse { flex: none; margin-left: 6px; }
  .rt-reopen {
    position: absolute; top: 14px; right: 0;
    display: flex; align-items: center; gap: 8px;
    padding: 8px 12px;
    background: var(--surface-2); border: 1px solid var(--border-strong); border-right: none;
    border-radius: var(--rad-2) 0 0 var(--rad-2);
    color: var(--fg-muted); cursor: pointer; font-size: var(--t-xs); letter-spacing: 0.08em;
  }
  .rt-reopen:hover { color: var(--fg-bright); border-color: var(--accent-dim); }

  /* ─────────── review & save step ─────────── */
  .rev-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100%;
    width: 100%;
    padding: 32px 20px;
  }
  .rev-card {
    width: 100%;
    max-width: 480px;
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--rad-4);
    box-shadow: var(--shadow-pop);
    overflow: hidden;
    animation: popIn var(--dur) var(--ease);
  }
  /* monospace chrome bar */
  .rev-chrome {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 16px;
    font-size: var(--t-xs);
    letter-spacing: 0.08em;
    color: var(--fg-muted);
    background: var(--surface-3);
    border-bottom: 1px solid var(--border-strong);
  }
  .rev-led {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--accent) 25%, transparent);
    flex: none;
  }
  .rev-dur { color: var(--fg-faint); letter-spacing: 0.06em; }
  .rev-body { padding: 22px 20px 18px; }
  .rev-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 16px;
    font-size: var(--t-xs);
  }
  .rev-meta-item { display: inline-flex; align-items: center; gap: 6px; }
  .rev-transcript {
    margin-top: 8px;
    max-height: 160px;
    overflow-y: auto;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--rad-3);
    background: color-mix(in oklab, var(--surface-2) 60%, transparent);
    font-size: var(--t-sm);
    line-height: 1.7;
    color: var(--fg-muted);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .rev-error {
    margin-top: 16px;
    padding: 10px 12px;
    border: 1px solid color-mix(in oklab, var(--err) 50%, var(--border));
    border-radius: var(--rad-3);
    background: color-mix(in oklab, var(--err) 12%, transparent);
    color: var(--err);
    font-size: var(--t-sm);
  }
  .rev-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 20px;
    border-top: 1px solid var(--border);
  }
  .rev-discard { color: var(--err); }
  /* "multiple people speaking" row: label block left, switch right */
  .rev-speakers {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .rev-speakers-l { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .rev-speakers-d { font-size: var(--t-xs); color: var(--fg-muted); line-height: 1.5; }
  .rev-speakers :global(.st-toggle) { flex: none; margin-left: auto; }
  .rev-hint {
    padding: 0 20px 16px;
    text-align: center;
    font-size: var(--t-xs);
  }
</style>
