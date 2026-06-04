<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // ---- state ----
  let recording = $state(false);
  let paused = $state(false);
  let secs = $state(0);
  let bars = $state<number[]>(Array.from({ length: 72 }, () => 0.06));
  // "review" sits between stopping and committing: the user names the recording,
  // picks a topic, and confirms before we transcribe + save.
  let status = $state<"ready" | "recording" | "review" | "transcribing" | "done">("ready");
  let note = $state<string>("");
  let errorMsg = $state<string | null>(null);
  let tags = $state<{ at: string }[]>([]);

  // ---- review & save step ----
  // Captured between stop and save so the user can review before committing.
  let reviewBytes = $state<number[]>([]);     // the assembled audio (number[] for the IPC contract)
  let reviewName = $state("");                // editable file/source name
  let reviewTopicId = $state("");             // chosen topic ("" → no topic)
  let reviewDuration = $state("00:00");       // captured length, mm:ss
  let reviewTranscript = $state("");          // live transcript preview (if any was produced)
  let reviewSourceLabel = $state("");         // "captured" vs "Uploaded audio" — for the success toast

  // Topic options for the review Picker: the active subject's topics + a "no topic" sentinel.
  const NO_TOPIC = "__none__";
  const topicOptions = $derived([
    { id: NO_TOPIC, label: "— no topic —" },
    ...(app.activeSubject?.topics ?? []).map((t) => ({ id: t.id, label: t.name })),
  ]);

  // ---- live (interim) transcription, where the platform supports SpeechRecognition ----
  // Feature-detected once at module init so the UI can be honest about availability.
  const SR: any =
    typeof window !== "undefined"
      ? (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition
      : undefined;
  const liveTranscriptSupported = !!SR;
  let liveTranscriptOn = $state(true); // user-facing toggle for the live transcript panel (default on)
  let liveFinal = $state(""); // accumulated final results
  let liveInterim = $state(""); // current in-flight (unstable) chunk

  // ---- backend chunked fallback (WebKitGTK / Tauri Linux, no SpeechRecognition) ----
  // Periodically ships the audio recorded so far to backend Whisper and shows the text.
  let liveBackendText = $state(""); // latest backend partial transcript
  let liveUpdating = $state(false); // true while a transcribePartial call is in flight
  let whisperMissing = $state(false); // backend returned "" → no Whisper installed; stop polling

  // ---- real recording machinery (not reactive) ----
  let mediaRecorder: MediaRecorder | null = null;
  let stream: MediaStream | null = null;
  let audioCtx: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let recognition: any = null; // SpeechRecognition instance (if supported)
  let recognitionWantsRun = false; // keep-alive flag so onend can restart it
  let chunks: Blob[] = [];
  let unlisten: UnlistenFn | null = null;
  // Live-transcript fallback uses a SECOND recorder on the same stream that emits
  // complete ~20s segments; each is transcribed and APPENDED (incremental), so the
  // cost stays flat no matter how long the lecture runs. The continuous recorder
  // above remains the authoritative audio that's precisely re-transcribed on save,
  // so small gaps between live segments don't affect the saved transcript.
  let segRecorder: MediaRecorder | null = null;
  let segChunks: Blob[] = [];
  let segTimer: ReturnType<typeof setTimeout> | null = null;
  const SEG_MS = 20000;

  // ---- derived ----
  const mm = $derived(String(Math.floor(secs / 60)).padStart(2, "0"));
  const ss = $derived(String(secs % 60).padStart(2, "0"));
  const live = $derived(recording && !paused);
  const hasLiveTranscript = $derived(liveTranscriptSupported && (liveFinal.trim().length > 0 || liveInterim.trim().length > 0));

  // ---- subtle animated waveform from the mic (Web Audio analyser, rAF-driven) ----
  // Renders a small mirrored row of frequency bars. Secondary to the compact clock.
  $effect(() => {
    if (!live || !analyser) return;
    const freq = new Uint8Array(analyser.frequencyBinCount);
    let rafId = 0;
    const N = 72;
    const band = Math.max(1, Math.floor(freq.length / N));
    function tick() {
      analyser!.getByteFrequencyData(freq);
      bars = Array.from({ length: N }, (_, i) => {
        let s = 0;
        for (let j = 0; j < band; j++) s += freq[i * band + j] ?? 0;
        const v = s / band / 255;
        // gentle curve so quiet rooms still show a soft baseline, loud peaks don't clip
        return Math.min(1, v * 1.7 + 0.04);
      });
      rafId = requestAnimationFrame(tick);
    }
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  });

  // ---- elapsed timer ----
  $effect(() => {
    if (!live) return;
    const iv = setInterval(() => (secs = secs + 1), 1000);
    return () => clearInterval(iv);
  });

  function cleanupStream() {
    stopRecognition();
    stopBackendPoll();
    stream?.getTracks().forEach((t) => t.stop());
    audioCtx?.close().catch(() => {});
    stream = null;
    audioCtx = null;
    analyser = null;
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

  // ---- backend chunked fallback (no SpeechRecognition): incremental segments ----
  function stopBackendPoll() {
    if (segTimer) { clearTimeout(segTimer); segTimer = null; }
    if (segRecorder) {
      try { segRecorder.onstop = null; segRecorder.stop(); } catch { /* noop */ }
      segRecorder = null;
    }
    segChunks = [];
    liveUpdating = false;
  }

  // Record one ~20s segment on the shared stream; onstop transcribes + appends it.
  function startSegment() {
    if (SR || !stream || !recording || paused || !liveTranscriptOn) return;
    try {
      segChunks = [];
      segRecorder = new MediaRecorder(stream);
      segRecorder.ondataavailable = (e) => { if (e.data.size > 0) segChunks.push(e.data); };
      segRecorder.onstop = () => void transcribeSegment();
      segRecorder.start();
      segTimer = setTimeout(() => { try { segRecorder?.stop(); } catch { /* noop */ } }, SEG_MS);
    } catch {
      segRecorder = null;
    }
  }

  async function transcribeSegment() {
    const localChunks = segChunks;
    segChunks = [];
    // Kick off the next segment immediately so we keep capturing while this one
    // transcribes (any tiny gap only affects the live PREVIEW — the saved audio is
    // the continuous recorder, re-transcribed precisely on save).
    startSegment();
    if (localChunks.length === 0) return;
    liveUpdating = true;
    try {
      const blob = new Blob(localChunks, { type: localChunks[0]?.type || "audio/webm" });
      const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
      const text = await api.transcribePartial(bytes);
      if (text && text.trim()) {
        liveBackendText = (liveBackendText ? liveBackendText + " " : "") + text.trim();
        whisperMissing = false;
      } else if (!liveBackendText) {
        // First segment came back empty → no Whisper installed. Be honest, stop.
        whisperMissing = true;
        stopBackendPoll();
      }
    } catch {
      // Backend hiccup — keep what we have; the next segment will continue.
    } finally {
      liveUpdating = false;
    }
  }

  function startBackendPoll() {
    if (SR) return; // SpeechRecognition path is authoritative where available
    stopBackendPoll();
    whisperMissing = false;
    startSegment();
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
    liveBackendText = "";
    whisperMissing = false;
    // Live transcript alongside the recording when the toggle is on:
    // SpeechRecognition where available, otherwise backend chunked Whisper.
    if (liveTranscriptOn) {
      startRecognition();
      startBackendPoll();
    }
  }

  function togglePause() {
    if (!mediaRecorder) return;
    if (paused) {
      mediaRecorder.resume();
      paused = false;
      if (liveTranscriptOn) {
        if (SR) startRecognition();
        else startSegment();
      }
    } else {
      mediaRecorder.pause();
      paused = true;
      // keep accumulated final text, just halt the live engine(s) while paused
      recognitionWantsRun = false;
      if (recognition) { try { recognition.onend = null; recognition.stop(); } catch { /* noop */ } recognition = null; }
      stopBackendPoll();
      liveInterim = "";
    }
  }

  // React to the user toggling the live transcript on/off while recording.
  function toggleLiveTranscript() {
    liveTranscriptOn = !liveTranscriptOn;
    if (!recording) return;
    if (liveTranscriptOn) {
      if (!paused) startRecognition();
      startBackendPoll();
    } else {
      stopRecognition();
      stopBackendPoll();
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
    // A friendly default name, e.g. "Lecture Jun 3, 2:07 PM".
    const stamp = new Date().toLocaleString(undefined, {
      month: "short", day: "numeric", hour: "numeric", minute: "2-digit",
    });
    // Whatever the live transcript captured (SpeechRecognition or backend fallback).
    const transcript = (liveFinal.trim() || liveBackendText.trim());
    enterReview(bytes, `Lecture ${stamp}`, `${mm}:${ss}`, "captured", transcript);
  }

  // Move into the review & save step: stash the audio + sensible defaults and let the user edit.
  function enterReview(bytes: number[], name: string, duration: string, sourceLabel: string, transcript = "") {
    const subj = app.activeSubject;
    if (!subj) { status = "ready"; return; }
    reviewBytes = bytes;
    reviewName = name;
    reviewDuration = duration;
    reviewSourceLabel = sourceLabel;
    reviewTranscript = transcript;
    // Default to the first topic when the subject has any, otherwise "no topic".
    reviewTopicId = subj.topics[0]?.id ?? NO_TOPIC;
    errorMsg = null;
    status = "review";
  }

  // Commit the reviewed recording: transcribe + save, then navigate as the old finalize did.
  async function confirmSave() {
    const subj = app.activeSubject;
    if (!subj) { status = "ready"; return; }
    const name = reviewName.trim() || "Untitled recording";
    const topicId = reviewTopicId && reviewTopicId !== NO_TOPIC ? reviewTopicId : undefined;
    const capturedLabel = `${reviewDuration} ${reviewSourceLabel}`;
    await saveAudio(reviewBytes, name, capturedLabel, topicId);
  }

  // Discard the reviewed audio and reset the recorder to idle.
  function discardReview() {
    reviewBytes = [];
    reviewName = "";
    reviewTopicId = "";
    reviewTranscript = "";
    reviewDuration = "00:00";
    secs = 0;
    tags = [];
    liveFinal = ""; liveInterim = ""; liveBackendText = ""; whisperMissing = false;
    bars = Array.from({ length: 72 }, () => 0.06);
    status = "ready";
  }

  // Shared save/transcribe pipeline for both live recordings and uploaded files.
  async function saveAudio(bytes: number[], name: string, capturedLabel: string, topicId?: string) {
    const subj = app.activeSubject;
    if (!subj) { status = "ready"; return; }

    status = "transcribing";
    errorMsg = null;
    unlisten = await api.onIngestProgress((p) => { note = p.detail; });
    try {
      const res = await api.saveRecording(subj.id, name, bytes, topicId);
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
      status = "review"; // back to review so the user can retry without losing the audio
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
      // Route uploads through the same review step so they can be named/topic-tagged too.
      enterReview(bytes, file.name, "—:—", "uploaded");
    } catch (err) {
      errorMsg = String(err);
      status = "ready";
    }
  }

  function cancel() {
    if (mediaRecorder && recording) { mediaRecorder.onstop = null; mediaRecorder.stop(); }
    cleanupStream();
    recording = false; paused = false; secs = 0; tags = []; status = "ready";
    liveFinal = ""; liveInterim = ""; liveBackendText = ""; whisperMissing = false;
    bars = Array.from({ length: 72 }, () => 0.06);
    app.setView("subject");
  }

  // keyboard: space toggles start/pause, enter stops, m tags
  $effect(() => {
    window.__cortexModalOpen = true;
    function onKey(e: KeyboardEvent) {
      const el = document.activeElement as HTMLElement | null;
      if (el && (el.tagName === "INPUT" || el.tagName === "TEXTAREA")) return;
      if (status === "review") {
        if (e.key === "Enter") { e.preventDefault(); void confirmSave(); }
        else if (e.key === "Escape") { e.preventDefault(); discardReview(); }
        return;
      }
      if (e.key === " ") { e.preventDefault(); recording ? togglePause() : start(); }
      else if (e.key === "Enter" && recording) { e.preventDefault(); stop(); }
      else if (e.key === "m") { e.preventDefault(); tagMoment(); }
      else if (e.key === "Escape") cancel();
    }
    window.addEventListener("keydown", onKey);
    return () => { window.removeEventListener("keydown", onKey); window.__cortexModalOpen = false; cleanupStream(); };
  });
</script>

{#if status === "review"}
  <!-- ─────────── REVIEW & SAVE ───────────
       Centered, monospace-chrome step between stopping and committing. -->
  <div class="rev-wrap">
    <div class="rev-card">
      <div class="rev-chrome mono">
        <span class="rev-led"></span>
        REVIEW &amp; SAVE
        <span class="grow"></span>
        <span class="rev-dur">{reviewSourceLabel === "uploaded" ? "FILE" : reviewDuration}</span>
      </div>

      <div class="rev-body">
        <div class="field">
          <span class="onb-label mono">NAME <span class="faint">how this source is titled</span></span>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="input"
            autofocus
            bind:value={reviewName}
            placeholder="Untitled recording"
          />
        </div>

        <div class="field" style:margin-top="16px">
          <span class="onb-label mono">TOPIC <span class="faint">where this recording lives</span></span>
          <Picker
            value={reviewTopicId}
            onChange={(id) => (reviewTopicId = id)}
            options={topicOptions}
            placeholder="— no topic —"
          />
        </div>

        <div class="rev-meta mono faint">
          <span class="rev-meta-item"><Icon name="bolt" size={11} color="var(--fg-faint)" />{reviewSourceLabel === "uploaded" ? "Uploaded audio file" : `Captured ${reviewDuration}`}</span>
          {#if reviewTranscript.trim()}
            <span class="rev-meta-item"><Icon name="doc" size={11} color="var(--fg-faint)" />Live transcript captured</span>
          {/if}
        </div>

        {#if reviewTranscript.trim()}
          <div class="field" style:margin-top="14px">
            <span class="onb-label mono">TRANSCRIPT PREVIEW <span class="faint">re-transcribed precisely on save</span></span>
            <div class="rev-transcript read">{reviewTranscript}</div>
          </div>
        {/if}

        {#if errorMsg}
          <div class="rev-error">{errorMsg}</div>
        {/if}
      </div>

      <div class="rev-actions">
        <button class="btn btn--ghost rev-discard" onclick={discardReview}>Discard</button>
        <span class="grow"></span>
        <button class="btn btn--primary" onclick={confirmSave}>Save recording</button>
      </div>

      <div class="rev-hint mono faint">
        <span class="kbd">⏎</span> save · <span class="kbd">esc</span> discard
      </div>
    </div>
  </div>
{:else}
<div class="recorder">
  <!-- Left: stage -->
  <div class="rec-stage">
    <div class="rec-status mono">
      <span class="rec-led{live ? ' live' : ''}"></span>
      {status === "transcribing" ? "TRANSCRIBING" : recording ? (paused ? "PAUSED" : "RECORDING") : status === "done" ? "DONE" : "READY"}
      <span class="grow"></span>
      <span class="rec-clock">{mm}:{ss}</span>
    </div>

    <!-- Subtle, secondary live waveform (mirrored frequency bars, rAF-driven).
         Kept small — the compact mm:ss readout in .rec-status is the primary timer. -->
    <div class="waveform waveform--compact" class:is-live={live} aria-hidden="true">
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
      <span class="label">Live transcript</span>
      <span class="grow"></span>
      {#if status === "transcribing"}
        <span class="status-pill status-pill--draft"><span class="dot"></span>processing</span>
      {:else if live && liveTranscriptOn && liveUpdating}
        <span class="status-pill status-pill--draft"><span class="dot"></span>updating…</span>
      {:else if live && liveTranscriptOn && (hasLiveTranscript || liveBackendText.trim())}
        <span class="status-pill status-pill--draft"><span class="dot"></span>live</span>
      {/if}
      <!-- Toggleable: turns the live transcript panel on/off. Default on. -->
      <button
        type="button"
        class="rt-toggle"
        class:is-on={liveTranscriptOn}
        role="switch"
        aria-checked={liveTranscriptOn}
        title="Toggle live transcript"
        onclick={toggleLiveTranscript}
      >
        <span class="rt-toggle-track"><span class="rt-toggle-knob"></span></span>
      </button>
    </div>
    <div class="rt-body">
      {#if status === "transcribing"}
        <div class="rt-empty mono faint">{note || "Running Whisper on your recording…"}</div>
        {#if liveFinal.trim()}
          <p class="rt-live read rt-live--dim">{liveFinal}</p>
        {:else if liveBackendText.trim()}
          <p class="rt-live read rt-live--dim">{liveBackendText}</p>
        {/if}
      {:else if recording && !liveTranscriptOn}
        <!-- User switched the live transcript off. -->
        <div class="rt-note mono faint">
          Live transcript is off. Flip the switch above to see your words as you speak — the full transcript is still saved when you stop.
        </div>
      {:else if recording && liveTranscriptSupported}
        <!-- Real-time path: browser SpeechRecognition (final + interim). -->
        {#if hasLiveTranscript}
          <p class="rt-live read">
            {liveFinal}<span class="rt-interim">{liveInterim}</span>
          </p>
        {:else}
          <div class="rt-empty mono faint">Listening… your words will appear here as you speak.</div>
        {/if}
      {:else if recording && whisperMissing}
        <!-- Backend fallback tried, came back empty: no Whisper installed. Be honest. -->
        <div class="rt-note mono faint">
          Live transcript needs Whisper — install openai-whisper (pip install openai-whisper) or whisper.cpp.
        </div>
      {:else if recording}
        <!-- Backend chunked fallback (WebKitGTK / Tauri Linux): refreshed ~every 15s. -->
        {#if liveBackendText.trim()}
          <p class="rt-live read">{liveBackendText}</p>
        {:else}
          <div class="rt-empty mono faint">Listening… the transcript refreshes every few seconds as Whisper catches up.</div>
        {/if}
      {:else}
        <div class="rt-empty mono faint">
          Hit record to capture a lecture. On stop, Cortex transcribes it with Whisper and saves it as a searchable source.
          A live transcript appears here while you record — toggle it with the switch above.
        </div>
      {/if}
    </div>
    <button class="btn btn--ghost btn--sm rt-close" onclick={cancel}>Cancel</button>
  </aside>
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

  /* ---- Live transcript is now the primary feature: give the panel more real estate ---- */
  .recorder { grid-template-columns: 1fr clamp(420px, 42vw, 560px); }

  /* ---- toggle switch in the transcript header ---- */
  .rt-toggle {
    flex: none;
    margin-left: 10px;
    padding: 0;
    border: 0;
    background: none;
    cursor: pointer;
    display: inline-flex;
    align-items: center;
  }
  .rt-toggle-track {
    width: 30px;
    height: 17px;
    border-radius: var(--rad-pill);
    background: var(--surface-2);
    border: 1px solid var(--border-strong);
    position: relative;
    transition: background var(--dur), border-color var(--dur);
  }
  .rt-toggle-knob {
    position: absolute;
    top: 1px;
    left: 1px;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background: var(--fg-muted);
    transition: transform var(--dur), background var(--dur);
  }
  .rt-toggle.is-on .rt-toggle-track {
    background: color-mix(in oklab, var(--accent) 45%, transparent);
    border-color: color-mix(in oklab, var(--accent) 60%, var(--border-strong));
  }
  .rt-toggle.is-on .rt-toggle-knob {
    transform: translateX(13px);
    background: var(--accent);
  }

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
  .rev-hint {
    padding: 0 20px 16px;
    text-align: center;
    font-size: var(--t-xs);
  }
</style>
