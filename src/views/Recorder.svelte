<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // ---- state ----
  let recording = $state(false);
  let paused = $state(false);
  let secs = $state(0);
  let bars = $state<number[]>(Array.from({ length: 72 }, () => 0.06));
  let level = $state(0); // smoothed overall mic level 0..1 (drives the glow ring)
  let status = $state<"ready" | "recording" | "transcribing" | "done">("ready");
  let note = $state<string>("");
  let errorMsg = $state<string | null>(null);
  let tags = $state<{ at: string }[]>([]);

  // ---- live (interim) transcription, where the platform supports SpeechRecognition ----
  // Feature-detected once at module init so the UI can be honest about availability.
  const SR: any =
    typeof window !== "undefined"
      ? (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition
      : undefined;
  const liveTranscriptSupported = !!SR;
  let liveFinal = $state(""); // accumulated final results
  let liveInterim = $state(""); // current in-flight (unstable) chunk

  // ---- real recording machinery (not reactive) ----
  let mediaRecorder: MediaRecorder | null = null;
  let stream: MediaStream | null = null;
  let audioCtx: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let recognition: any = null; // SpeechRecognition instance (if supported)
  let recognitionWantsRun = false; // keep-alive flag so onend can restart it
  let chunks: Blob[] = [];
  let unlisten: UnlistenFn | null = null;

  // ---- derived ----
  const mm = $derived(String(Math.floor(secs / 60)).padStart(2, "0"));
  const ss = $derived(String(secs % 60).padStart(2, "0"));
  const live = $derived(recording && !paused);
  const hasLiveTranscript = $derived(liveTranscriptSupported && (liveFinal.trim().length > 0 || liveInterim.trim().length > 0));

  // ---- real animated waveform from the mic (Web Audio analyser, rAF-driven) ----
  // Renders a mirrored row of frequency bars + a smoothed overall level that
  // drives the glow ring. Always works regardless of transcription support.
  $effect(() => {
    if (!live || !analyser) return;
    const freq = new Uint8Array(analyser.frequencyBinCount);
    let rafId = 0;
    let lvl = 0;
    const N = 72;
    const band = Math.max(1, Math.floor(freq.length / N));
    function tick() {
      analyser!.getByteFrequencyData(freq);
      let peak = 0;
      bars = Array.from({ length: N }, (_, i) => {
        let s = 0;
        for (let j = 0; j < band; j++) s += freq[i * band + j] ?? 0;
        const v = s / band / 255;
        if (v > peak) peak = v;
        // gentle curve so quiet rooms still show a soft baseline, loud peaks don't clip
        return Math.min(1, v * 1.7 + 0.04);
      });
      // exponential smoothing on the overall level → calm, low-glare glow
      lvl += (Math.min(1, peak * 1.5) - lvl) * 0.18;
      level = lvl;
      rafId = requestAnimationFrame(tick);
    }
    rafId = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(rafId);
      level = 0;
    };
  });

  // ---- elapsed timer ----
  $effect(() => {
    if (!live) return;
    const iv = setInterval(() => (secs = secs + 1), 1000);
    return () => clearInterval(iv);
  });

  function cleanupStream() {
    stopRecognition();
    stream?.getTracks().forEach((t) => t.stop());
    audioCtx?.close().catch(() => {});
    stream = null;
    audioCtx = null;
    analyser = null;
    level = 0;
  }

  // ---- live transcription (feature-detected) ----
  function startRecognition() {
    if (!SR) return; // platform (e.g. WebKitGTK / Tauri Linux) has no SpeechRecognition
    try {
      recognition = new SR();
      recognition.continuous = true;
      recognition.interimResults = true;
      recognition.lang = navigator.language || "en-US";
      recognition.onresult = (ev: any) => {
        let interim = "";
        let finalAdd = "";
        for (let i = ev.resultIndex; i < ev.results.length; i++) {
          const r = ev.results[i];
          if (r.isFinal) finalAdd += r[0].transcript;
          else interim += r[0].transcript;
        }
        if (finalAdd) liveFinal = (liveFinal + finalAdd).replace(/\s+/g, " ").trimStart();
        liveInterim = interim;
      };
      // Recognition engines auto-stop periodically; restart while we still want it.
      recognition.onend = () => {
        if (recognitionWantsRun && !paused) {
          try { recognition.start(); } catch { /* already running */ }
        }
      };
      recognition.onerror = () => { /* swallow; backend Whisper remains authoritative */ };
      recognitionWantsRun = true;
      recognition.start();
    } catch {
      recognition = null;
      recognitionWantsRun = false;
    }
  }

  function stopRecognition() {
    recognitionWantsRun = false;
    if (recognition) {
      try { recognition.onend = null; recognition.stop(); } catch { /* noop */ }
      recognition = null;
    }
    liveInterim = "";
  }

  async function start() {
    if (!app.activeSubject) {
      app.pushToast({ kind: "error", title: "Open a subject first", body: "Select a subject before recording." });
      return;
    }
    errorMsg = null;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch (e) {
      errorMsg = "Microphone access was denied or unavailable: " + String(e);
      return;
    }
    // analyser for the live waveform
    audioCtx = new AudioContext();
    const src = audioCtx.createMediaStreamSource(stream);
    analyser = audioCtx.createAnalyser();
    analyser.fftSize = 256;
    src.connect(analyser);

    chunks = [];
    mediaRecorder = new MediaRecorder(stream);
    mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) chunks.push(e.data); };
    mediaRecorder.onstop = () => void finalize();
    mediaRecorder.start(1000);
    recording = true;
    paused = false;
    status = "recording";
    secs = 0;
    tags = [];
    liveFinal = "";
    liveInterim = "";
    // Live interim transcript alongside the recording, where supported.
    startRecognition();
  }

  function togglePause() {
    if (!mediaRecorder) return;
    if (paused) {
      mediaRecorder.resume();
      paused = false;
      if (SR) startRecognition();
    } else {
      mediaRecorder.pause();
      paused = true;
      // keep accumulated final text, just halt the live engine while paused
      recognitionWantsRun = false;
      if (recognition) { try { recognition.onend = null; recognition.stop(); } catch { /* noop */ } recognition = null; }
      liveInterim = "";
    }
  }

  function tagMoment() {
    if (recording) tags = [...tags, { at: `${mm}:${ss}` }];
  }

  function stop() {
    if (!mediaRecorder) return;
    mediaRecorder.stop(); // triggers onstop → finalize()
  }

  async function finalize() {
    recording = false;
    paused = false;
    cleanupStream();
    const subj = app.activeSubject;
    if (!subj) { status = "ready"; return; }

    const blob = new Blob(chunks, { type: chunks[0]?.type || "audio/webm" });
    const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
    const name = `lecture-${new Date().toISOString().slice(0, 16).replace("T", "-").replace(":", "")}.webm`;
    await saveAudio(bytes, name, `${mm}:${ss} captured`);
  }

  // Shared save/transcribe pipeline for both live recordings and uploaded files.
  async function saveAudio(bytes: number[], name: string, capturedLabel: string) {
    const subj = app.activeSubject;
    if (!subj) { status = "ready"; return; }

    status = "transcribing";
    errorMsg = null;
    unlisten = await api.onIngestProgress((p) => { note = p.detail; });
    try {
      const res = await api.saveRecording(subj.id, name, bytes, subj.topics[0]?.id);
      await app.refresh();
      status = "done";
      if (res.warning) {
        app.pushToast({ kind: "warning", title: "Recording saved", body: res.warning });
      } else {
        app.pushToast({
          kind: "success",
          title: "Recording transcribed",
          body: `${capturedLabel} · ${res.chunk_count} chunks embedded.`,
        });
      }
      app.setView("subject");
      app.setTab("sources");
    } catch (e) {
      errorMsg = String(e);
      status = "ready";
    } finally {
      if (unlisten) { unlisten(); unlisten = null; }
    }
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
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      await saveAudio(bytes, file.name, "Uploaded audio");
    } catch (err) {
      errorMsg = String(err);
      status = "ready";
    }
  }

  function cancel() {
    if (mediaRecorder && recording) { mediaRecorder.onstop = null; mediaRecorder.stop(); }
    cleanupStream();
    recording = false; paused = false; secs = 0; tags = []; status = "ready";
    liveFinal = ""; liveInterim = "";
    bars = Array.from({ length: 72 }, () => 0.06);
    app.setView("subject");
  }

  // keyboard: space toggles start/pause, enter stops, m tags
  $effect(() => {
    window.__cortexModalOpen = true;
    function onKey(e: KeyboardEvent) {
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
      if (e.key === " ") { e.preventDefault(); recording ? togglePause() : start(); }
      else if (e.key === "Enter" && recording) { e.preventDefault(); stop(); }
      else if (e.key === "m") { e.preventDefault(); tagMoment(); }
      else if (e.key === "Escape") cancel();
    }
    window.addEventListener("keydown", onKey);
    return () => { window.removeEventListener("keydown", onKey); window.__cortexModalOpen = false; cleanupStream(); };
  });
</script>

<div class="recorder">
  <!-- Left: stage -->
  <div class="rec-stage">
    <div class="rec-status mono">
      <span class="rec-led{live ? ' live' : ''}"></span>
      {status === "transcribing" ? "TRANSCRIBING" : recording ? (paused ? "PAUSED" : "RECORDING") : status === "done" ? "DONE" : "READY"}
      <span class="grow"></span>
      <span class="rec-clock">{mm}:{ss}</span>
    </div>

    <!-- Big elapsed timer with a soft level-reactive glow ring -->
    <div
      class="rec-timer mono"
      class:is-live={live}
      style:--lvl={live ? level : 0}
    >
      <span class="rec-timer-val">{mm}<span class="rec-timer-colon">:</span>{ss}</span>
      <span class="rec-timer-cap faint">
        {#if status === "transcribing"}transcribing{:else if paused}paused{:else if recording}elapsed{:else}ready{/if}
      </span>
    </div>

    <!-- Animated live waveform (mirrored frequency bars, rAF-driven) -->
    <div class="waveform" class:is-live={live} aria-hidden="true">
      {#each bars as bar, i (i)}
        <span
          class="wbar"
          style:height="{(live ? Math.max(0.06, bar) : 0.06) * 100}%"
          style:opacity={live ? 0.5 + bar * 0.5 : 0.22}
        ></span>
      {/each}
      <div class="wf-center"></div>
    </div>

    <div class="rec-controls">
      {#if status === "transcribing"}
        <span class="is-spin" style:width="22px" style:height="22px"></span>
      {:else if !recording}
        <button class="rec-btn rec-btn--go" onclick={start} title="Start recording">
          <span class="rec-btn-dot"></span>
        </button>
      {:else}
        <button class="btn btn--icon" onclick={togglePause} title={paused ? "Resume" : "Pause"}>
          {#if paused}<Icon name="play" size={15} />{:else}<Icon name="pause" size={15} />{/if}
        </button>
        <button class="rec-btn rec-btn--stop" onclick={stop} title="Stop & save"><span class="rec-stop-sq"></span></button>
        <button class="btn btn--icon" onclick={tagMoment} title="Tag moment (m)"><Icon name="bolt" size={15} color="var(--warn)" /></button>
      {/if}
    </div>

    <div class="rec-hint mono faint">
      {#if status === "transcribing"}
        Transcribing with Whisper…
      {:else if !recording}
        Press <span class="kbd">␣</span> or click to start · output becomes a transcribed source
      {:else}
        <span class="kbd">m</span> tag moment · <span class="kbd">space</span> pause · <span class="kbd">⏎</span> stop &amp; save
      {/if}
    </div>

    {#if errorMsg}
      <div style:color="var(--err)" style:margin-top="14px" style:font-size="var(--t-sm)" style:max-width="420px" style:text-align="center">{errorMsg}</div>
    {/if}

    <!-- Fallback: upload a pre-recorded audio file (always available, emphasised on error) -->
    {#if status !== "transcribing"}
      <div class="rec-upload mono faint" style:margin-top={errorMsg ? "12px" : "18px"}>
        {#if errorMsg}Can't use the mic? {/if}
        <label class="btn btn--ghost btn--sm" style:cursor="pointer">
          <Icon name="doc" size={13} />
          Upload an audio file
          <input type="file" accept="audio/*" onchange={uploadAudioFile} style:display="none" />
        </label>
      </div>
    {/if}

    {#if tags.length > 0}
      <div class="rec-tags">
        {#each tags as tag, i (i)}
          <span class="rec-tag mono"><Icon name="bolt" size={10} color="var(--warn)" />{tag.at}</span>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Right: live transcript / status panel -->
  <aside class="rec-transcript">
    <div class="rt-head">
      <span class="label">Transcript</span>
      {#if status === "transcribing"}
        <span class="status-pill status-pill--draft"><span class="dot"></span>processing</span>
      {:else if live && hasLiveTranscript}
        <span class="status-pill status-pill--draft"><span class="dot"></span>live preview</span>
      {/if}
    </div>
    <div class="rt-body">
      {#if status === "transcribing"}
        <div class="rt-empty mono faint">{note || "Running Whisper on your recording…"}</div>
        {#if liveFinal.trim()}
          <p class="rt-live read rt-live--dim">{liveFinal}</p>
        {/if}
      {:else if recording && liveTranscriptSupported}
        {#if hasLiveTranscript}
          <p class="rt-live read">
            {liveFinal}<span class="rt-interim">{liveInterim}</span>
          </p>
        {:else}
          <div class="rt-empty mono faint">Listening… your words will appear here as you speak.</div>
        {/if}
      {:else if recording}
        <!-- Honest fallback: no SpeechRecognition on this platform (e.g. WebKitGTK / Tauri Linux) -->
        <div class="rt-note mono faint">
          Live transcription unavailable on this platform — the full transcript is generated after you stop (needs Whisper).
        </div>
      {:else}
        <div class="rt-empty mono faint">
          Hit record to capture a lecture. On stop, Cortex transcribes it with Whisper and saves it as a searchable source.
          {#if liveTranscriptSupported}
            A live preview appears here while you record.
          {/if}
        </div>
      {/if}
    </div>
    <button class="btn btn--ghost btn--sm rt-close" onclick={cancel}>Cancel</button>
  </aside>
</div>

<style>
  /* ---- Big elapsed timer + level-reactive glow ring ---- */
  .rec-timer {
    --lvl: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 22px 34px 18px;
    border-radius: var(--rad-4);
    border: 1px solid var(--border);
    background:
      radial-gradient(
        120% 120% at 50% 30%,
        color-mix(in oklab, var(--accent) calc(7% + var(--lvl) * 16%), transparent),
        transparent 70%
      ),
      var(--surface);
    /* glow intensity tracks the smoothed mic level — calm, low-glare */
    box-shadow:
      0 0 0 1px color-mix(in oklab, var(--accent) calc(var(--lvl) * 40%), transparent),
      0 0 calc(8px + var(--lvl) * 44px)
        color-mix(in oklab, var(--accent) calc(var(--lvl) * 38%), transparent);
    transition: box-shadow 120ms linear, background 120ms linear, border-color var(--dur);
  }
  .rec-timer.is-live {
    border-color: color-mix(in oklab, var(--accent) 45%, var(--border));
  }
  .rec-timer-val {
    font-size: clamp(40px, 8vw, 58px);
    line-height: 1;
    font-weight: 500;
    letter-spacing: 0.02em;
    color: var(--fg-bright);
    font-variant-numeric: tabular-nums;
  }
  .rec-timer.is-live .rec-timer-val { color: var(--accent); }
  .rec-timer-colon { opacity: 0.55; }
  .rec-timer.is-live .rec-timer-colon { animation: rec-colon 1s steps(1) infinite; }
  .rec-timer-cap {
    font-size: var(--t-2xs);
    letter-spacing: 0.22em;
    text-transform: uppercase;
  }
  @keyframes rec-colon { 50% { opacity: 0.18; } }

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
</style>
