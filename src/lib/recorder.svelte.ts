// ─────────────────────────────────────────────────────────────────────────────
// GLOBAL LECTURE-RECORDING ENGINE
//
// Lives OUTSIDE the Recorder view so a recording is a background activity:
// navigating anywhere in the app (or closing the Recorder view) keeps the
// capture running, and the floating RecordingActivity widget mirrors it from
// any screen — the same pattern as the Pomodoro LiveActivity.
//
// Two capture engines, picked automatically:
//   • web    — getUserMedia + MediaRecorder (with the WebKitGTK WAV fallback).
//              Desktop and Android.
//   • native — Tauri commands backed by AVAudioRecorder on iOS. WKWebView's
//              custom-scheme pages aren't a secure context, so
//              navigator.mediaDevices never exists there — getUserMedia can NOT
//              work no matter what permissions are granted. The native recorder
//              also keeps recording with the phone locked / app backgrounded
//              (AVAudioSession + UIBackgroundModes audio), which a webview
//              capture never survives.
// ─────────────────────────────────────────────────────────────────────────────
import * as api from "./api";
import { app } from "./store.svelte";
import { isIOS, isMobile } from "./platform";

// Live (interim) transcription via the platform SpeechRecognition, where it exists.
const SR: any =
  typeof window !== "undefined"
    ? (window as any).SpeechRecognition || (window as any).webkitSpeechRecognition
    : undefined;
export const liveTranscriptSupported = !!SR;

// ~7s cadence feels real-time and local faster-whisper clears a 7s clip in ~1-2s.
// The loop is ADAPTIVE: it never starts a second transcription while one is in
// flight — the pending slice simply grows until the round-trip returns.
const SEG_MS = 7000;
// Below this mean |amplitude| (normalized 0-1) a WAV segment is treated as a
// pause and skipped — no whisper call, no UI change.
const SILENCE_RMS = 0.006;

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

class RecorderStore {
  // ---- reactive state (drives the Recorder view AND the floating widget) ----
  recording = $state(false);
  paused = $state(false);
  secs = $state(0);
  status = $state<"ready" | "recording" | "review" | "transcribing" | "done">("ready");
  errorMsg = $state<string | null>(null);
  note = $state("");
  tags = $state<{ at: string }[]>([]);

  // live transcript
  transcriptCollapsed = $state(true);
  liveFinal = $state("");
  liveInterim = $state("");
  liveBackendText = $state("");
  liveUpdating = $state(false);
  whisperMissing = $state(false);

  // review & save step
  // reviewBytes/reviewPath are NOT $state: megabytes of audio must never be
  // wrapped in a deep reactive proxy (it makes IPC serialization crawl).
  reviewBytes: number[] = [];
  reviewPath = "";      // native (iOS) recordings stay a backend file — no bytes over IPC
  reviewExt = "webm";
  reviewName = $state("");
  reviewTopicId = $state("");
  reviewDuration = $state("00:00");
  reviewTranscript = $state("");
  reviewSourceLabel = $state("");

  // waveform inputs: web engine exposes the analyser; native exposes a 0-1 level
  analyser = $state<AnalyserNode | null>(null);
  nativeLevel = $state(0);
  /** True while the native (iOS) engine is the active capture path. */
  native = $state(false);

  // ---- non-reactive machinery ----
  private mediaRecorder: MediaRecorder | null = null;
  private stream: MediaStream | null = null;
  private audioCtx: AudioContext | null = null;
  private srcNode: MediaStreamAudioSourceNode | null = null;
  private chunks: Blob[] = [];
  private captureMode: "media" | "wav" = "media";
  private wavProc: ScriptProcessorNode | null = null;
  private wavChunks: Int16Array[] = [];
  private wavSegStart = 0;
  private watchdog: ReturnType<typeof setTimeout> | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;
  private levelTimer: ReturnType<typeof setInterval> | null = null;
  private recognition: any = null;
  private recognitionWantsRun = false;
  private segRecorder: MediaRecorder | null = null;
  private segChunks: Blob[] = [];
  private segTimer: ReturnType<typeof setTimeout> | null = null;

  // ---- derived helpers ----
  get mm(): string { return String(Math.floor(this.secs / 60)).padStart(2, "0"); }
  get ss(): string { return String(this.secs % 60).padStart(2, "0"); }
  get live(): boolean { return this.recording && !this.paused; }
  get liveTranscriptOn(): boolean { return !this.transcriptCollapsed; }
  get hasLiveTranscript(): boolean {
    return liveTranscriptSupported && (this.liveFinal.trim().length > 0 || this.liveInterim.trim().length > 0);
  }

  // ════════════════════════════ lifecycle ════════════════════════════

  async start(): Promise<void> {
    if (this.recording) return;
    if (!app.activeSubject) {
      app.pushToast({ kind: "error", title: "Open a subject first", body: "Select a subject before recording." });
      return;
    }
    this.errorMsg = null;
    if (isIOS) return this.startNative();
    return this.startWeb();
  }

  /** Native iOS capture via AVAudioRecorder (see src-tauri recorder commands). */
  private async startNative(): Promise<void> {
    try {
      await api.nativeRecStart();
    } catch (e) {
      this.errorMsg = "Couldn't start the microphone: " + String(e);
      return;
    }
    this.native = true;
    this.beginSession();
    // Poll the recorder's metering for the waveform ambience (~8 fps is plenty).
    // The sample also carries the recorder's own elapsed time — the webview's JS
    // timers freeze while the phone is locked, so the UI clock resyncs from it.
    this.levelTimer = setInterval(() => {
      if (!this.live) return;
      api.nativeRecLevel()
        .then((m) => {
          this.nativeLevel = m.level;
          const authoritative = Math.floor(m.secs);
          if (Math.abs(authoritative - this.secs) > 1) this.secs = authoritative;
        })
        .catch(() => {});
    }, 120);
  }

  private async startWeb(): Promise<void> {
    if (!navigator.mediaDevices?.getUserMedia) {
      this.errorMsg =
        "Microphone capture isn't available in this webview. You can still add a lecture with “Upload an audio file”.";
      return;
    }
    try {
      this.stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch (e) {
      this.errorMsg = "Microphone access was denied or unavailable: " + String(e);
      return;
    }
    this.native = false;
    // analyser for the live waveform
    this.audioCtx = new AudioContext();
    this.srcNode = this.audioCtx.createMediaStreamSource(this.stream);
    const analyser = this.audioCtx.createAnalyser();
    analyser.fftSize = 256;
    this.srcNode.connect(analyser);
    this.analyser = analyser;

    this.chunks = [];
    this.wavChunks = [];
    this.captureMode = "media";
    // Ask for a container WebKitGTK claims to support; an unsupported default
    // is one way recordings end up empty.
    const mime = ["audio/webm;codecs=opus", "audio/webm", "audio/ogg;codecs=opus", "audio/mp4"]
      .find((m) => typeof MediaRecorder !== "undefined" && MediaRecorder.isTypeSupported?.(m));
    try {
      this.mediaRecorder = mime ? new MediaRecorder(this.stream, { mimeType: mime }) : new MediaRecorder(this.stream);
      this.mediaRecorder.ondataavailable = (e) => { if (e.data.size > 0) this.chunks.push(e.data); };
      this.mediaRecorder.onstop = () => void this.finalizeWeb();
      this.mediaRecorder.start(1000);
    } catch (err) {
      console.warn("[recorder] MediaRecorder unavailable", err);
      this.mediaRecorder = null;
    }
    this.beginSession();
    // Watchdog: if MediaRecorder is silently broken (WebKitGTK), no chunk will
    // have arrived a few seconds in — swap engines without losing the session.
    if (this.mediaRecorder) {
      this.watchdog = setTimeout(() => {
        if (this.recording && this.chunks.length === 0) this.switchToWavCapture();
      }, 3500);
    } else {
      this.switchToWavCapture();
    }
    // Live transcript alongside the recording when the panel is open (desktop
    // only — mobile transcribes the saved audio on stop).
    if (this.liveTranscriptOn && !isMobile) {
      this.startRecognition();
      this.startBackendPoll();
    }
  }

  /** Shared session bootstrap once an engine is capturing. */
  private beginSession(): void {
    this.recording = true;
    this.paused = false;
    this.status = "recording";
    this.secs = 0;
    this.tags = [];
    this.liveFinal = "";
    this.liveInterim = "";
    this.liveBackendText = "";
    this.whisperMissing = false;
    if (this.timer) clearInterval(this.timer);
    this.timer = setInterval(() => { if (this.live) this.secs += 1; }, 1000);
  }

  togglePause(): void {
    if (!this.recording) return;
    if (this.native) {
      if (this.paused) { api.nativeRecResume().catch(() => {}); this.paused = false; }
      else { api.nativeRecPause().catch(() => {}); this.paused = true; }
      return;
    }
    if (!this.mediaRecorder && this.captureMode !== "wav") return;
    if (this.paused) {
      // WAV engine gates on `paused` inside onaudioprocess — nothing to resume.
      if (this.captureMode === "media") this.mediaRecorder!.resume();
      this.paused = false;
      if (this.liveTranscriptOn) {
        if (SR) this.startRecognition();
        else this.startSegment();
      }
    } else {
      if (this.captureMode === "media") this.mediaRecorder!.pause();
      this.paused = true;
      // keep accumulated final text, just halt the live engine(s) while paused
      this.recognitionWantsRun = false;
      if (this.recognition) { try { this.recognition.onend = null; this.recognition.stop(); } catch { /* noop */ } this.recognition = null; }
      this.stopBackendPoll();
      this.liveInterim = "";
    }
  }

  tagMoment(): void {
    if (this.recording) this.tags = [...this.tags, { at: `${this.mm}:${this.ss}` }];
  }

  stop(): void {
    if (!this.recording) return;
    if (this.native) { void this.finalizeNative(); return; }
    if (this.captureMode === "wav") { void this.finalizeWeb(); return; }
    if (!this.mediaRecorder) return;
    this.mediaRecorder.stop(); // triggers onstop → finalizeWeb()
  }

  /** Abort the in-flight recording and throw the audio away. */
  discardRecording(): void {
    if (this.native) {
      api.nativeRecCancel().catch(() => {});
    } else if (this.mediaRecorder && this.recording) {
      this.mediaRecorder.onstop = null;
      try { this.mediaRecorder.stop(); } catch { /* noop */ }
    }
    this.cleanupStream();
    this.endTimers();
    this.native = false;
    this.recording = false;
    this.paused = false;
    this.secs = 0;
    this.tags = [];
    this.status = "ready";
    this.liveFinal = ""; this.liveInterim = ""; this.liveBackendText = ""; this.whisperMissing = false;
  }

  private endTimers(): void {
    if (this.timer) { clearInterval(this.timer); this.timer = null; }
    if (this.levelTimer) { clearInterval(this.levelTimer); this.levelTimer = null; }
    this.nativeLevel = 0;
  }

  private cleanupStream(): void {
    this.stopRecognition();
    this.stopBackendPoll();
    if (this.watchdog) { clearTimeout(this.watchdog); this.watchdog = null; }
    if (this.wavProc) { try { this.wavProc.disconnect(); } catch { /* noop */ } this.wavProc = null; }
    this.stream?.getTracks().forEach((t) => t.stop());
    this.audioCtx?.close().catch(() => {});
    this.stream = null;
    this.audioCtx = null;
    this.analyser = null;
    this.srcNode = null;
    this.mediaRecorder = null;
  }

  private async finalizeNative(): Promise<void> {
    this.recording = false;
    this.paused = false;
    this.endTimers();
    const dur = `${this.mm}:${this.ss}`;
    let res: { path: string; secs: number };
    try {
      res = await api.nativeRecStop();
    } catch (e) {
      this.native = false;
      this.errorMsg = "Couldn't finish the recording: " + String(e);
      this.status = "ready";
      return;
    }
    if (!res.path) {
      this.native = false;
      this.errorMsg = "Nothing was captured — the microphone produced no audio.";
      this.status = "ready";
      return;
    }
    this.reviewBytes = [];
    this.reviewPath = res.path;
    this.reviewExt = "m4a";
    this.enterReview(`Lecture ${this.stamp()}`, dur, "captured", "");
  }

  private async finalizeWeb(): Promise<void> {
    this.recording = false;
    this.paused = false;
    this.cleanupStream();
    this.endTimers();
    if (!app.activeSubject) { this.status = "ready"; return; }

    let bytes: number[];
    if (this.captureMode === "wav") {
      bytes = Array.from(encodeWav(this.wavChunks));
      this.reviewExt = "wav";
    } else {
      const blob = new Blob(this.chunks, { type: this.chunks[0]?.type || "audio/webm" });
      bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
      this.reviewExt = "webm";
    }
    if (bytes.length === 0) {
      this.errorMsg = "Nothing was captured — the microphone produced no audio. Check the input device in your system sound settings, then try again.";
      this.status = "ready";
      return;
    }
    // Whatever the live transcript captured (SpeechRecognition or backend fallback).
    const transcript = (this.liveFinal.trim() || this.liveBackendText.trim());
    this.reviewBytes = bytes;
    this.reviewPath = "";
    this.enterReview(`Lecture ${this.stamp()}`, `${this.mm}:${this.ss}`, "captured", transcript);
  }

  private stamp(): string {
    // A friendly default name, e.g. "Lecture Jun 3, 2:07 PM".
    return new Date().toLocaleString(undefined, {
      month: "short", day: "numeric", hour: "numeric", minute: "2-digit",
    });
  }

  // ════════════════════════════ review & save ════════════════════════════

  /** Move into the review step (used by both engines and file uploads). */
  enterReview(name: string, duration: string, sourceLabel: string, transcript = ""): void {
    const subj = app.activeSubject;
    if (!subj) { this.status = "ready"; return; }
    this.reviewName = name;
    this.reviewDuration = duration;
    this.reviewSourceLabel = sourceLabel;
    this.reviewTranscript = transcript;
    // Default to the first topic when the subject has any, otherwise "no topic".
    this.reviewTopicId = subj.topics[0]?.id ?? "";
    this.errorMsg = null;
    this.status = "review";
  }

  /** Stash an uploaded audio file and enter review. */
  enterReviewFromUpload(bytes: number[], name: string, ext: string): void {
    this.reviewBytes = bytes;
    this.reviewPath = "";
    this.reviewExt = ext;
    this.enterReview(name, "—:—", "uploaded");
  }

  async confirmSave(): Promise<void> {
    const subj = app.activeSubject;
    if (!subj) { this.status = "ready"; return; }
    const name = this.reviewName.trim() || "Untitled recording";
    const topicId = this.reviewTopicId || undefined;
    const capturedLabel = `${this.reviewDuration} ${this.reviewSourceLabel}`;

    this.status = "transcribing";
    this.errorMsg = null;
    const unlisten = await api.onIngestProgress((p) => { this.note = p.detail; });
    try {
      const res = this.reviewPath
        ? await api.saveRecordingPath(subj.id, name, this.reviewPath, topicId)
        : await api.saveRecording(subj.id, name, this.reviewBytes, topicId, this.reviewExt);
      await app.refresh();
      this.status = "done";
      this.native = false;
      if (res.warning) {
        app.pushToast({ kind: "warning", title: "Recording saved", body: res.warning });
      } else {
        app.pushToast({
          kind: "success",
          title: "Recording transcribed",
          body: `${capturedLabel} · ${res.chunk_count} chunks embedded.`,
        });
      }
      this.reviewBytes = [];
      this.reviewPath = "";
      app.setView("subject");
      app.setTab("sources");
    } catch (e) {
      this.errorMsg = String(e);
      this.status = "review"; // back to review so the user can retry without losing the audio
    } finally {
      unlisten();
    }
  }

  discardReview(): void {
    if (this.reviewPath) api.nativeRecDiscardFile(this.reviewPath).catch(() => {});
    this.reviewBytes = [];
    this.reviewPath = "";
    this.reviewName = "";
    this.reviewTopicId = "";
    this.reviewTranscript = "";
    this.reviewDuration = "00:00";
    this.secs = 0;
    this.tags = [];
    this.liveFinal = ""; this.liveInterim = ""; this.liveBackendText = ""; this.whisperMissing = false;
    this.native = false;
    this.status = "ready";
  }

  // ════════════════════ live transcription (web engine) ════════════════════

  /** Open/close the transcript panel; the live engines follow the panel state. */
  toggleTranscriptPanel(): void {
    this.transcriptCollapsed = !this.transcriptCollapsed;
    if (!this.recording || this.native) return;
    if (!this.transcriptCollapsed) {
      if (!this.paused) this.startRecognition();
      this.startBackendPoll();
    } else {
      this.stopRecognition();
      this.stopBackendPoll();
    }
  }

  private startRecognition(): void {
    if (!SR) return; // platform (e.g. WebKitGTK / Tauri Linux) has no SpeechRecognition
    try {
      this.recognition = new SR();
      this.recognition.continuous = true;
      this.recognition.interimResults = true;
      this.recognition.lang = navigator.language || "en-US";
      this.recognition.onresult = (ev: any) => {
        let interim = "";
        let finalAdd = "";
        for (let i = ev.resultIndex; i < ev.results.length; i++) {
          const r = ev.results[i];
          if (r.isFinal) finalAdd += r[0].transcript;
          else interim += r[0].transcript;
        }
        if (finalAdd) this.liveFinal = (this.liveFinal + finalAdd).replace(/\s+/g, " ").trimStart();
        this.liveInterim = interim;
      };
      // Recognition engines auto-stop periodically; restart while we still want it.
      this.recognition.onend = () => {
        if (this.recognitionWantsRun && !this.paused) {
          try { this.recognition.start(); } catch { /* already running */ }
        }
      };
      this.recognition.onerror = () => { /* swallow; backend Whisper remains authoritative */ };
      this.recognitionWantsRun = true;
      this.recognition.start();
    } catch {
      this.recognition = null;
      this.recognitionWantsRun = false;
    }
  }

  private stopRecognition(): void {
    this.recognitionWantsRun = false;
    if (this.recognition) {
      try { this.recognition.onend = null; this.recognition.stop(); } catch { /* noop */ }
      this.recognition = null;
    }
    this.liveInterim = "";
  }

  private startBackendPoll(): void {
    if (SR) return; // SpeechRecognition path is authoritative where available
    this.stopBackendPoll();
    this.whisperMissing = false;
    this.startSegment();
  }

  private stopBackendPoll(): void {
    if (this.segTimer) { clearTimeout(this.segTimer); this.segTimer = null; }
    if (this.segRecorder) {
      try { this.segRecorder.onstop = null; this.segRecorder.stop(); } catch { /* noop */ }
      this.segRecorder = null;
    }
    this.segChunks = [];
    this.liveUpdating = false;
  }

  // Record one ~7s segment on the shared stream; onstop transcribes + appends it.
  private startSegment(): void {
    if (SR || !this.stream || !this.recording || this.paused || !this.liveTranscriptOn) return;
    if (this.captureMode === "wav") {
      // PCM engine: a segment is just a slice of wavChunks. Anchor the start here
      // and cut at the timer; transcribeWavSegment re-anchors only when it
      // actually consumes the slice, so a slow round-trip just grows the window.
      this.wavSegStart = this.wavChunks.length;
      this.segTimer = setTimeout(() => void this.transcribeWavSegment(), SEG_MS);
      return;
    }
    try {
      this.segChunks = [];
      this.segRecorder = new MediaRecorder(this.stream);
      this.segRecorder.ondataavailable = (e) => { if (e.data.size > 0) this.segChunks.push(e.data); };
      this.segRecorder.onstop = () => void this.transcribeSegment();
      this.segRecorder.start();
      this.segTimer = setTimeout(() => { try { this.segRecorder?.stop(); } catch { /* noop */ } }, SEG_MS);
    } catch {
      this.segRecorder = null;
    }
  }

  private async transcribeWavSegment(): Promise<void> {
    // ADAPTIVE GUARD: if the previous segment is still transcribing, don't queue
    // a second request — leave wavSegStart where it is so the pending slice grows
    // to cover this window, and re-check after another cadence tick.
    if (this.liveUpdating) {
      this.segTimer = setTimeout(() => void this.transcribeWavSegment(), SEG_MS);
      return;
    }
    const seg = this.wavChunks.slice(this.wavSegStart);
    // We're consuming this slice now: advance the anchor and re-arm the loop so
    // the next window starts from here (back-to-back, no overlap, no gap loss —
    // the saved transcript comes from the continuous WAV, re-transcribed on save).
    this.wavSegStart = this.wavChunks.length;
    this.segTimer = setTimeout(() => void this.transcribeWavSegment(), SEG_MS);
    if (seg.length === 0) return;
    // Silence gate: skip near-silent windows (pauses) — no whisper call, no UI
    // change. Saves cycles and avoids whisper hallucinating words from room tone.
    if (meanAmplitude(seg) < SILENCE_RMS) return;
    this.liveUpdating = true;
    try {
      const text = await api.transcribePartial(Array.from(encodeWav(seg)), "wav");
      if (text && text.trim()) {
        this.liveBackendText = (this.liveBackendText ? this.liveBackendText + " " : "") + text.trim();
        this.whisperMissing = false;
      } else if (!this.liveBackendText) {
        this.whisperMissing = true;
        this.stopBackendPoll();
      }
    } catch {
      // Backend hiccup — keep what we have; the next segment will continue.
    } finally {
      this.liveUpdating = false;
    }
  }

  private async transcribeSegment(): Promise<void> {
    const localChunks = this.segChunks;
    this.segChunks = [];
    // Kick off the next segment immediately so we keep capturing while this one
    // transcribes (any tiny gap only affects the live PREVIEW — the saved audio is
    // the continuous recorder, re-transcribed precisely on save).
    this.startSegment();
    if (localChunks.length === 0) return;
    // ADAPTIVE GUARD: never run two transcriptions at once.
    if (this.liveUpdating) return;
    this.liveUpdating = true;
    try {
      const blob = new Blob(localChunks, { type: localChunks[0]?.type || "audio/webm" });
      const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
      const text = await api.transcribePartial(bytes);
      if (text && text.trim()) {
        this.liveBackendText = (this.liveBackendText ? this.liveBackendText + " " : "") + text.trim();
        this.whisperMissing = false;
      } else if (!this.liveBackendText) {
        // First segment came back empty → no Whisper installed. Be honest, stop.
        this.whisperMissing = true;
        this.stopBackendPoll();
      }
    } catch {
      // Backend hiccup — keep what we have; the next segment will continue.
    } finally {
      this.liveUpdating = false;
    }
  }

  // ════════════════════ WAV capture fallback (WebKitGTK) ════════════════════
  // WebKitGTK's MediaRecorder can run without error yet deliver ZERO data —
  // every saved lecture came out empty. getUserMedia + Web Audio provably work
  // (the live waveform uses them), so when the watchdog sees no data arrive we
  // capture raw PCM off the same graph and encode 16 kHz mono WAV ourselves.
  private switchToWavCapture(): void {
    if (this.captureMode === "wav" || !this.audioCtx || !this.srcNode || !this.recording) return;
    console.warn("[recorder] MediaRecorder produced no data — switching to WAV capture");
    try {
      if (this.mediaRecorder && this.mediaRecorder.state !== "inactive") {
        this.mediaRecorder.onstop = null;
        this.mediaRecorder.ondataavailable = null;
        this.mediaRecorder.stop();
      }
    } catch { /* noop */ }
    this.mediaRecorder = null;
    this.captureMode = "wav";
    this.wavChunks = [];
    this.wavSegStart = 0;
    this.wavProc = this.audioCtx.createScriptProcessor(4096, 1, 1);
    this.srcNode.connect(this.wavProc);
    // The processor only runs while routed to the destination — mute it.
    const mute = this.audioCtx.createGain();
    mute.gain.value = 0;
    this.wavProc.connect(mute).connect(this.audioCtx.destination);
    const rate = this.audioCtx.sampleRate;
    this.wavProc.onaudioprocess = (e) => {
      if (!this.recording || this.paused) return;
      this.wavChunks.push(downsampleTo16k(e.inputBuffer.getChannelData(0), rate));
    };
    // Restart the live-transcript segment loop on the new engine.
    if (this.liveTranscriptOn && !SR) {
      this.stopBackendPoll();
      this.startBackendPoll();
    }
  }
}

export const rec = new RecorderStore();
