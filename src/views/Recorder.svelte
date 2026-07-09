<script lang="ts">
  import { app } from "../lib/store.svelte";
  import * as api from "../lib/api";
  import Icon from "../components/Icon.svelte";
  import Picker from "../components/Picker.svelte";
  import { isMobile } from "../lib/platform";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  // ---- state ----
  let recording = $state(false);
  let paused = $state(false);
  let secs = $state(0);
  // Waveform draws to ONE canvas inside rAF — the previous 72 DOM bars with
  // per-frame style updates forced layout 60×/s, which crawled (~2fps) on
  // WebKitGTK's software renderer.
  let waveCanvas: HTMLCanvasElement | null = $state(null);
  // "review" sits between stopping and committing: the user names the recording,
  // picks a topic, and confirms before we transcribe + save.
  let status = $state<"ready" | "recording" | "review" | "transcribing" | "done">("ready");
  let note = $state<string>("");
  let errorMsg = $state<string | null>(null);
  let tags = $state<{ at: string }[]>([]);

  // ---- review & save step ----
  // Captured between stop and save so the user can review before committing.
  // NOT $state: this is megabytes of audio — wrapping it in a deep reactive
  // proxy makes every later read (incl. IPC serialization) crawl, and no UI
  // ever renders the raw bytes.
  let reviewBytes: number[] = [];             // the assembled audio (number[] for the IPC contract)
  let reviewExt = "webm";                     // container of reviewBytes ("webm" | "wav")
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
  // One state drives both visibility AND the engine: closed panel = no live
  // transcription work, open panel = transcribing. Closed by default.
  let transcriptCollapsed = $state(true);
  const liveTranscriptOn = $derived(!transcriptCollapsed);
  let liveFinal = $state(""); // accumulated final results
  let liveInterim = $state(""); // current in-flight (unstable) chunk

  // ---- backend chunked fallback (WebKitGTK / Tauri Linux, no SpeechRecognition) ----
  // Periodically ships the audio recorded so far to backend Whisper and shows the text.
  let liveBackendText = $state(""); // latest backend partial transcript
  let liveUpdating = $state(false); // true while a transcribePartial call is in flight
  let whisperMissing = $state(false); // backend returned "" → no Whisper installed; stop polling

  // ---- transcript auto-scroll (anchored to newest text unless the user scrolls up) ----
  let rtBody: HTMLDivElement | null = $state(null);
  let rtPinned = $state(true); // stay glued to the bottom while true
  function onRtScroll() {
    const el = rtBody;
    if (!el) return;
    // Re-pin once the user returns to within a hair of the bottom.
    rtPinned = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
  }

  // ---- real recording machinery (not reactive) ----
  let mediaRecorder: MediaRecorder | null = null;
  let stream: MediaStream | null = null;
  let audioCtx: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let srcNode: MediaStreamAudioSourceNode | null = null;

  // ---- WAV capture fallback ----
  // WebKitGTK's MediaRecorder can run without error yet deliver ZERO data (its
  // GStreamer recorder is missing/broken on some systems) — every saved lecture
  // came out as an empty file. getUserMedia + Web Audio provably work here (the
  // live waveform uses them), so when a watchdog sees no data arrive we swap to
  // capturing raw PCM off the same graph and encode 16 kHz mono WAV ourselves.
  let captureMode: "media" | "wav" = "media";
  let wavProc: ScriptProcessorNode | null = null;
  let wavChunks: Int16Array[] = []; // 16 kHz mono PCM
  let wavSegStart = 0; // wavChunks index where the current live segment begins
  let watchdog: ReturnType<typeof setTimeout> | null = null;

  /** Linear-interpolation downsample of one Float32 block to 16 kHz Int16. */
  function downsampleTo16k(input: Float32Array, fromRate: number): Int16Array {
    const ratio = fromRate / 16000;
    const outLen = Math.max(1, Math.floor(input.length / ratio));
    const out = new Int16Array(outLen);
    for (let i = 0; i < outLen; i++) {
      const pos = i * ratio;
      const i0 = Math.floor(pos);
      const i1 = Math.min(i0 + 1, input.length - 1);
      const s = input[i0] + (input[i1] - input[i0]) * (pos - i0);
      out[i] = Math.max(-32768, Math.min(32767, Math.round(s * 32767)));
    }
    return out;
  }

  /** Assemble PCM chunks into a complete 16 kHz mono 16-bit WAV file. */
  function encodeWav(pcm: Int16Array[]): Uint8Array {
    const total = pcm.reduce((n, c) => n + c.length, 0);
    const buf = new ArrayBuffer(44 + total * 2);
    const dv = new DataView(buf);
    const str = (off: number, s: string) => {
      for (let i = 0; i < s.length; i++) dv.setUint8(off + i, s.charCodeAt(i));
    };
    str(0, "RIFF"); dv.setUint32(4, 36 + total * 2, true); str(8, "WAVE");
    str(12, "fmt "); dv.setUint32(16, 16, true);
    dv.setUint16(20, 1, true); dv.setUint16(22, 1, true); // PCM, mono
    dv.setUint32(24, 16000, true); dv.setUint32(28, 16000 * 2, true);
    dv.setUint16(32, 2, true); dv.setUint16(34, 16, true);
    str(36, "data"); dv.setUint32(40, total * 2, true);
    let off = 44;
    for (const c of pcm) for (let i = 0; i < c.length; i++) { dv.setInt16(off, c[i], true); off += 2; }
    return new Uint8Array(buf);
  }

  /** Kick a suspended/interrupted AudioContext back to "running".
   *  iOS suspends contexts created outside a direct user gesture (start() creates
   *  ours after the getUserMedia await) and again on audio-session interruptions
   *  (calls, Siri) — a suspended context silences the waveform AND the WAV engine. */
  function ensureAudioRunning() {
    const ctx = audioCtx;
    if (ctx && ctx.state !== "running" && ctx.state !== "closed") {
      ctx.resume().catch(() => { /* retried on the next statechange/visibility tick */ });
    }
  }

  /** Swap a silent MediaRecorder for the PCM/WAV engine mid-recording.
   *  Returns false when the PCM engine couldn't attach either. */
  function switchToWavCapture(): boolean {
    if (captureMode === "wav") return true;
    if (!audioCtx || !srcNode || !recording) return false;
    console.warn("[recorder] MediaRecorder produced no data — switching to WAV capture");
    try {
      if (mediaRecorder && mediaRecorder.state !== "inactive") {
        mediaRecorder.onstop = null;
        mediaRecorder.ondataavailable = null;
        mediaRecorder.stop();
      }
    } catch { /* noop */ }
    mediaRecorder = null;
    ensureAudioRunning();
    try {
      wavProc = audioCtx.createScriptProcessor(4096, 1, 1);
      srcNode.connect(wavProc);
      // The processor only runs while routed to the destination — mute it.
      const mute = audioCtx.createGain();
      mute.gain.value = 0;
      wavProc.connect(mute).connect(audioCtx.destination);
    } catch (err) {
      console.warn("[recorder] PCM capture unavailable", err);
      wavProc = null;
      return false;
    }
    captureMode = "wav";
    wavChunks = [];
    wavSegStart = 0;
    const rate = audioCtx.sampleRate;
    wavProc.onaudioprocess = (e) => {
      if (!recording || paused) return;
      wavChunks.push(downsampleTo16k(e.inputBuffer.getChannelData(0), rate));
    };
    // Restart the live-transcript segment loop on the new engine.
    if (liveTranscriptOn && !SR) {
      stopBackendPoll();
      startBackendPoll();
    }
    return true;
  }
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
  // ~7s cadence feels real-time and local faster-whisper (base.en) clears a 7s
  // clip in ~1-2s. The loop is ADAPTIVE: it never starts a second transcription
  // while one is in flight — the pending slice simply grows until the round-trip
  // returns, so slow whisper degrades cadence instead of queueing requests.
  const SEG_MS = 7000;
  // Below this mean |amplitude| (normalized 0-1) a WAV segment is treated as a
  // pause and skipped — no whisper call, no UI change. Tuned so room tone / mic
  // hiss stays under it but ordinary speech clears it comfortably.
  const SILENCE_RMS = 0.006;

  // ---- derived ----
  const mm = $derived(String(Math.floor(secs / 60)).padStart(2, "0"));
  const ss = $derived(String(secs % 60).padStart(2, "0"));
  const live = $derived(recording && !paused);
  const hasLiveTranscript = $derived(liveTranscriptSupported && (liveFinal.trim().length > 0 || liveInterim.trim().length > 0));

  // ---- subtle animated waveform from the mic (Web Audio analyser, rAF-driven) ----
  // Draws a small mirrored row of frequency bars onto the canvas. All work stays
  // inside the rAF — no Svelte state, no DOM diffing, no layout.
  const WAVE_N = 72;
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
    if (!live || !analyser) {
      drawWave(null); // idle baseline
      return;
    }
    const freq = new Uint8Array(analyser.frequencyBinCount);
    const levels = new Float32Array(WAVE_N);
    const band = Math.max(1, Math.floor(freq.length / WAVE_N));
    let rafId = 0;
    let skip = false;
    function tick() {
      rafId = requestAnimationFrame(tick);
      skip = !skip; // ~30fps is plenty for ambience and halves render cost
      if (skip) return;
      analyser!.getByteFrequencyData(freq);
      for (let i = 0; i < WAVE_N; i++) {
        let s = 0;
        for (let j = 0; j < band; j++) s += freq[i * band + j] ?? 0;
        // gentle curve so quiet rooms still show a soft baseline, loud peaks don't clip
        levels[i] = Math.min(1, (s / band / 255) * 1.7 + 0.04);
      }
      drawWave(levels);
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

  // ---- auto-scroll the transcript to the newest text (unless the user scrolled up) ----
  $effect(() => {
    // Touch the streams so this re-runs whenever new text lands.
    void liveFinal; void liveInterim; void liveBackendText;
    const el = rtBody;
    if (el && rtPinned) el.scrollTop = el.scrollHeight;
  });

  function cleanupStream() {
    stopRecognition();
    stopBackendPoll();
    if (watchdog) { clearTimeout(watchdog); watchdog = null; }
    if (wavProc) { try { wavProc.disconnect(); } catch { /* noop */ } wavProc = null; }
    stream?.getTracks().forEach((t) => t.stop());
    if (audioCtx) audioCtx.onstatechange = null;
    audioCtx?.close().catch(() => {});
    stream = null;
    audioCtx = null;
    analyser = null;
    srcNode = null;
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
    if (captureMode === "wav") {
      // PCM engine: a segment is just a slice of wavChunks. Anchor the start here
      // and cut at the timer; transcribeWavSegment re-anchors only when it
      // actually consumes the slice, so a slow round-trip just grows the window.
      wavSegStart = wavChunks.length;
      segTimer = setTimeout(() => void transcribeWavSegment(), SEG_MS);
      return;
    }
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

  /** Mean |amplitude| (0-1) across PCM blocks, for the silence gate. */
  function meanAmplitude(pcm: Int16Array[]): number {
    let sum = 0;
    let n = 0;
    for (const c of pcm) {
      for (let i = 0; i < c.length; i++) sum += Math.abs(c[i]);
      n += c.length;
    }
    return n === 0 ? 0 : sum / n / 32768;
  }

  async function transcribeWavSegment() {
    // ADAPTIVE GUARD: if the previous segment is still transcribing, don't queue
    // a second request — leave wavSegStart where it is so the pending slice grows
    // to cover this window, and re-check after another cadence tick.
    if (liveUpdating) {
      segTimer = setTimeout(() => void transcribeWavSegment(), SEG_MS);
      return;
    }
    const seg = wavChunks.slice(wavSegStart);
    // We're consuming this slice now: advance the anchor and re-arm the loop so
    // the next window starts from here (back-to-back, no overlap, no gap loss —
    // the saved transcript comes from the continuous WAV, re-transcribed on save).
    wavSegStart = wavChunks.length;
    segTimer = setTimeout(() => void transcribeWavSegment(), SEG_MS);
    if (seg.length === 0) return;
    // Silence gate: skip near-silent windows (pauses) — no whisper call, no UI
    // change. Saves cycles and avoids whisper hallucinating words from room tone.
    if (meanAmplitude(seg) < SILENCE_RMS) return;
    liveUpdating = true;
    try {
      const text = await api.transcribePartial(Array.from(encodeWav(seg)), "wav");
      if (text && text.trim()) {
        liveBackendText = (liveBackendText ? liveBackendText + " " : "") + text.trim();
        whisperMissing = false;
      } else if (!liveBackendText) {
        whisperMissing = true;
        stopBackendPoll();
      }
    } catch {
      // Backend hiccup — keep what we have; the next segment will continue.
    } finally {
      liveUpdating = false;
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
    // ADAPTIVE GUARD: never run two transcriptions at once. If the previous
    // segment is still in flight, drop this window from the live PREVIEW (the
    // saved transcript is the continuous recorder, re-transcribed precisely on
    // save) so requests can't pile up when whisper is slower than the cadence.
    if (liveUpdating) return;
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
    try {
      // analyser for the live waveform
      audioCtx = new AudioContext();
      srcNode = audioCtx.createMediaStreamSource(stream);
      analyser = audioCtx.createAnalyser();
      analyser.fftSize = 256;
      srcNode.connect(analyser);
    } catch (e) {
      // No Web Audio graph = no waveform and no WAV fallback, but MediaRecorder
      // below can still work — keep going with whatever attached.
      console.warn("[recorder] Web Audio graph unavailable", e);
    }
    // iOS: a context created after the getUserMedia await is outside the user
    // gesture and starts "suspended" — resume it now and after interruptions.
    ensureAudioRunning();
    if (audioCtx) audioCtx.onstatechange = () => { if (recording) ensureAudioRunning(); };

    chunks = [];
    wavChunks = [];
    captureMode = "media";
    // Ask for a container the engine claims to support; an unsupported default
    // is one way recordings end up empty. (iOS WebKit records audio/mp4 only.)
    const mime = ["audio/webm;codecs=opus", "audio/webm", "audio/ogg;codecs=opus", "audio/mp4"]
      .find((m) => typeof MediaRecorder !== "undefined" && MediaRecorder.isTypeSupported?.(m));
    try {
      mediaRecorder = mime ? new MediaRecorder(stream, { mimeType: mime }) : new MediaRecorder(stream);
      mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) chunks.push(e.data); };
      mediaRecorder.onstop = () => void finalize();
      // An async recorder failure (iOS audio-session hiccups) → PCM engine.
      mediaRecorder.onerror = (e: Event) => {
        console.warn("[recorder] MediaRecorder error", e);
        if (recording && !switchToWavCapture()) failStart();
      };
      mediaRecorder.start(1000);
    } catch (err) {
      console.warn("[recorder] MediaRecorder unavailable", err);
      mediaRecorder = null;
    }
    recording = true;
    paused = false;
    status = "recording";
    secs = 0;
    tags = [];
    liveFinal = "";
    liveInterim = "";
    liveBackendText = "";
    whisperMissing = false;
    // Watchdog: if MediaRecorder is silently broken (WebKitGTK), no chunk will
    // have arrived a few seconds in — swap engines without losing the session.
    // Engines that only deliver data on stop() would false-positive here, so
    // first poke requestData() and give the flush a moment to land.
    if (mediaRecorder) {
      watchdog = setTimeout(() => {
        if (!recording || chunks.length > 0) return;
        try { if (mediaRecorder?.state === "recording") mediaRecorder.requestData(); } catch { /* unsupported */ }
        watchdog = setTimeout(() => {
          if (recording && chunks.length === 0 && !switchToWavCapture()) failStart();
        }, 1200);
      }, 3500);
    } else if (!switchToWavCapture()) {
      failStart();
      return;
    }
    // Live transcript alongside the recording when the toggle is on (desktop only —
    // mobile has no live transcript; the saved audio is transcribed by homelab Whisper
    // on stop).
    if (liveTranscriptOn && !isMobile) {
      startRecognition();
      startBackendPoll();
    }
  }

  function togglePause() {
    if (!mediaRecorder && captureMode !== "wav") return;
    if (paused) {
      // WAV engine gates on `paused` inside onaudioprocess — nothing to resume.
      if (captureMode === "media") mediaRecorder!.resume();
      ensureAudioRunning();
      paused = false;
      if (liveTranscriptOn) {
        if (SR) startRecognition();
        else startSegment();
      }
    } else {
      if (captureMode === "media") mediaRecorder!.pause();
      paused = true;
      // keep accumulated final text, just halt the live engine(s) while paused
      recognitionWantsRun = false;
      if (recognition) { try { recognition.onend = null; recognition.stop(); } catch { /* noop */ } recognition = null; }
      stopBackendPoll();
      liveInterim = "";
    }
  }

  // Open/close the transcript panel; the live engines follow the panel state.
  function toggleTranscriptPanel() {
    transcriptCollapsed = !transcriptCollapsed;
    if (!recording) return;
    if (!transcriptCollapsed) {
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

  // Neither capture engine could start: tear the session down and say so,
  // leaving the "Upload an audio file" fallback in view.
  function failStart() {
    cleanupStream();
    recording = false;
    paused = false;
    secs = 0;
    status = "ready";
    errorMsg = "Couldn't start recording — the audio engine did not start. Try again, or upload an audio file instead.";
  }

  function stop() {
    if (captureMode === "wav") { void finalize(); return; }
    if (!mediaRecorder) return;
    // An errored recorder is already "inactive" — stop() would throw and leave
    // the session stuck, so finalize with whatever chunks arrived.
    if (mediaRecorder.state === "inactive") { void finalize(); return; }
    try {
      mediaRecorder.stop(); // triggers onstop → finalize()
    } catch {
      void finalize();
    }
  }

  async function finalize() {
    recording = false;
    paused = false;
    cleanupStream();
    const subj = app.activeSubject;
    if (!subj) { status = "ready"; return; }

    let bytes: number[];
    if (captureMode === "wav") {
      bytes = Array.from(encodeWav(wavChunks));
      reviewExt = "wav";
    } else {
      const blob = new Blob(chunks, { type: chunks[0]?.type || "audio/webm" });
      bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
      reviewExt = "webm";
    }
    if (bytes.length === 0) {
      errorMsg = "Nothing was captured — the microphone produced no audio. Check the input device in your system sound settings, then try again.";
      status = "ready";
      return;
    }
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
      const res = await api.saveRecording(subj.id, name, bytes, topicId, reviewExt);
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
      // Keep the real container so the backend stores a matching extension.
      reviewExt = file.name.split(".").pop()?.toLowerCase() || "webm";
      // Route uploads through the same review step so they can be named/topic-tagged too.
      enterReview(bytes, file.name, "—:—", "uploaded");
    } catch (err) {
      errorMsg = String(err);
      status = "ready";
    }
  }

  function cancel() {
    if (mediaRecorder && recording) {
      mediaRecorder.onstop = null;
      try { if (mediaRecorder.state !== "inactive") mediaRecorder.stop(); } catch { /* noop */ }
    }
    cleanupStream();
    recording = false; paused = false; secs = 0; tags = []; status = "ready";
    liveFinal = ""; liveInterim = ""; liveBackendText = ""; whisperMissing = false;
    app.setView("subject");
  }

  // Returning from the background mid-recording (iOS backgrounds the webview and
  // suspends the audio session) — nudge the context back to running.
  $effect(() => {
    function onVis() { if (recording && !document.hidden) ensureAudioRunning(); }
    document.addEventListener("visibilitychange", onVis);
    return () => document.removeEventListener("visibilitychange", onVis);
  });

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
      // "t" collapses / reopens the live transcript panel (recorder-local only).
      else if (e.key.toLowerCase() === "t") { e.preventDefault(); toggleTranscriptPanel(); }
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
<div class="recorder" class:transcript-collapsed={transcriptCollapsed}>
  <!-- Left: stage -->
  <div class="rec-stage">
    <div class="rec-status mono">
      <span class="rec-led{live ? ' live' : ''}"></span>
      {status === "transcribing" ? "TRANSCRIBING" : recording ? (paused ? "PAUSED" : "RECORDING") : status === "done" ? "DONE" : "READY"}
      <span class="grow"></span>
      <span class="rec-clock">{mm}:{ss}</span>
    </div>

    <!-- Subtle, secondary live waveform (mirrored frequency bars, rAF-driven,
         single canvas — no per-frame DOM work).
         Kept small — the compact mm:ss readout in .rec-status is the primary timer. -->
    <div class="waveform waveform--compact" class:is-live={live} aria-hidden="true">
      <canvas bind:this={waveCanvas} class="wave-canvas"></canvas>
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

    {#if isMobile}
      <button class="btn btn--ghost btn--sm" style:margin-top="18px" onclick={cancel}>Cancel</button>
    {/if}
  </div>

  <!-- Right: live transcript / status panel (desktop only — mobile transcribes the
       saved audio via homelab Whisper on stop; no live transcript). -->
  {#if !isMobile}
  <aside class="rec-transcript">
    <div class="rt-head">
      <span class="rt-eyebrow mono">LIVE TRANSCRIPT</span>
      <span class="grow"></span>
      {#if status === "transcribing"}
        <span class="status-pill status-pill--draft"><span class="dot dot--pulse"></span>processing</span>
      {:else if live && liveTranscriptOn && liveUpdating}
        <span class="status-pill status-pill--draft"><span class="dot dot--pulse"></span>transcribing…</span>
      {:else if live && liveTranscriptOn}
        <span class="status-pill status-pill--draft"><span class="dot dot--pulse"></span>listening</span>
      {/if}
      <!-- Closing the panel also stops live transcription; "t" toggles it. -->
      <button class="btn btn--icon btn--sm btn--ghost rt-collapse" title="Close transcript (t)" onclick={toggleTranscriptPanel}>
        <Icon name="chevron" size={13} />
      </button>
      <span class="kbd rt-kbd" title="Press t to toggle">t</span>
    </div>
    <div class="rt-body" bind:this={rtBody} onscroll={onRtScroll}>
      {#if status === "transcribing"}
        <div class="rt-empty mono faint">{note || "Running Whisper on your recording…"}</div>
        {#if liveFinal.trim()}
          <p class="rt-live read rt-live--dim">{liveFinal}</p>
        {:else if liveBackendText.trim()}
          <p class="rt-live read rt-live--dim">{liveBackendText}</p>
        {/if}
      {:else if recording && liveTranscriptSupported}
        <!-- Real-time path: browser SpeechRecognition (final + interim). -->
        {#if hasLiveTranscript}
          <p class="rt-live read">
            {liveFinal}<span class="rt-interim">{liveInterim}</span>
          </p>
        {:else}
          <div class="rt-listening mono faint"><span class="rt-shimmer">Listening</span><span class="rt-ell"></span></div>
        {/if}
      {:else if recording && whisperMissing}
        <!-- Backend fallback tried, came back empty: no Whisper installed. Be honest. -->
        <div class="rt-note rt-note--warn mono">
          <span class="rt-note-title">Live transcript needs Whisper</span>
          No Whisper backend answered. Configure a homelab Whisper server in Settings, or install
          faster-whisper locally — the recording is still saved and transcribed when you stop.
        </div>
      {:else if recording}
        <!-- Backend chunked fallback (WebKitGTK / Tauri Linux): refreshes every ~7s. -->
        {#if liveBackendText.trim()}
          <p class="rt-live read">{liveBackendText}</p>
        {:else}
          <div class="rt-listening mono faint"><span class="rt-shimmer">Listening</span><span class="rt-ell"></span></div>
        {/if}
      {:else}
        <div class="rt-empty mono faint">
          Hit record to capture a lecture. On stop, Cortex transcribes it with Whisper and saves it as a searchable source.
          A live transcript appears here while you record — close it with <span class="kbd">t</span>; closed means transcription is off until you reopen it.
        </div>
      {/if}
    </div>
    <button class="btn btn--ghost btn--sm rt-close" onclick={cancel}>Cancel</button>
  </aside>

  {#if transcriptCollapsed}
    <button class="rt-reopen mono" title="Open live transcript (t)" onclick={toggleTranscriptPanel}>
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
  .rev-hint {
    padding: 0 20px 16px;
    text-align: center;
    font-size: var(--t-xs);
  }
</style>
