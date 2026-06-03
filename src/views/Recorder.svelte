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
  let status = $state<"ready" | "recording" | "transcribing" | "done">("ready");
  let note = $state<string>("");
  let errorMsg = $state<string | null>(null);
  let tags = $state<{ at: string }[]>([]);

  // ---- real recording machinery (not reactive) ----
  let mediaRecorder: MediaRecorder | null = null;
  let stream: MediaStream | null = null;
  let audioCtx: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let chunks: Blob[] = [];
  let unlisten: UnlistenFn | null = null;

  // ---- derived ----
  const mm = $derived(String(Math.floor(secs / 60)).padStart(2, "0"));
  const ss = $derived(String(secs % 60).padStart(2, "0"));
  const live = $derived(recording && !paused);

  // ---- real waveform from the mic (Web Audio analyser RMS per band) ----
  $effect(() => {
    if (!live || !analyser) return;
    const buf = new Uint8Array(analyser.frequencyBinCount);
    let rafId = 0;
    const band = Math.max(1, Math.floor(buf.length / 72));
    function tick() {
      analyser!.getByteFrequencyData(buf);
      bars = Array.from({ length: 72 }, (_, i) => {
        let s = 0;
        for (let j = 0; j < band; j++) s += buf[i * band + j] ?? 0;
        return Math.min(1, (s / band / 255) * 1.8 + 0.04);
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
    stream?.getTracks().forEach((t) => t.stop());
    audioCtx?.close().catch(() => {});
    stream = null;
    audioCtx = null;
    analyser = null;
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
  }

  function togglePause() {
    if (!mediaRecorder) return;
    if (paused) { mediaRecorder.resume(); paused = false; }
    else { mediaRecorder.pause(); paused = true; }
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

    <div class="waveform" aria-hidden="true">
      {#each bars as bar, i (i)}
        <span class="wbar" style:height="{(live ? Math.max(0.06, bar) : 0.06) * 100}%" style:opacity={live ? 0.55 + bar * 0.45 : 0.25}></span>
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

  <!-- Right: status panel -->
  <aside class="rec-transcript">
    <div class="rt-head">
      <span class="label">Transcript</span>
      {#if status === "transcribing"}
        <span class="status-pill status-pill--draft"><span class="dot"></span>processing</span>
      {/if}
    </div>
    <div class="rt-body">
      <div class="rt-empty mono faint">
        {#if status === "transcribing"}
          {note || "Running Whisper on your recording…"}
        {:else if recording}
          Recording… the transcript is generated when you stop. Whisper runs locally — or on your homelab if configured.
        {:else}
          Hit record to capture a lecture. On stop, Cortex transcribes it with Whisper and saves it as a searchable source.
        {/if}
      </div>
    </div>
    <button class="btn btn--ghost btn--sm rt-close" onclick={cancel}>Cancel</button>
  </aside>
</div>
