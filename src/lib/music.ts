// Continuous study-music controller. A single module-level instance that
// outlives every view, so playback never stops/restarts on navigation.
//
// Three engines:
//  - Streaming stations use one persistent <audio> pointed at an ad-free,
//    continuous SomaFM stream (changing station swaps the src).
//  - Noise/binaural stations are generated locally with the Web Audio API, so
//    they loop seamlessly and work fully offline.
//  - YouTube stations (user-added, by URL — video or livestream) are streamed
//    ad-free by the Rust mpv sidecar (see src-tauri/src/mpv.rs); the controller
//    just drives play/pause/volume over Tauri commands.

import * as api from "./api";

type Station =
  | { kind: "stream"; url: string }
  | { kind: "noise"; tone: "brown" | "rain" | "cafe" }
  | { kind: "binaural"; base: number; beat: number };

// Maps the station ids from src/lib/mock.ts → an engine.
const STATIONS: Record<string, Station> = {
  lofi: { kind: "stream", url: "https://ice1.somafm.com/groovesalad-128-mp3" },
  jazz: { kind: "stream", url: "https://ice1.somafm.com/sonicuniverse-128-mp3" },
  classical: { kind: "stream", url: "https://ice1.somafm.com/dronezone-128-mp3" },
  piano: { kind: "stream", url: "https://ice1.somafm.com/deepspaceone-128-mp3" },
  focus: { kind: "stream", url: "https://ice1.somafm.com/spacestation-128-mp3" },
  brown: { kind: "noise", tone: "brown" },
  rain: { kind: "noise", tone: "rain" },
  cafe: { kind: "noise", tone: "cafe" },
  binaural: { kind: "binaural", base: 200, beat: 40 },
};

class MusicController {
  private audio: HTMLAudioElement | null = null;
  private ctx: AudioContext | null = null;
  private gain: GainNode | null = null;
  private genNodes: AudioScheduledSourceNode[] = [];
  private volume = 0.6;
  current: string | null = null;
  // When a YouTube/URL station is active, playback lives in the mpv sidecar, so
  // pause/resume/volume route to Tauri commands instead of the local engines.
  private isYoutube = false;
  // Bumped on every play/switch so callbacks from a superseded station (late
  // errors, stale promises) can't clobber the state of the one now playing.
  private token = 0;
  // Set by the store so playback failures surface to the user instead of being
  // swallowed (SomaFM streams need internet; a dead network shows a toast).
  onError: ((msg: string) => void) | null = null;
  // Authoritative engine state, pushed to the store: the UI reflects what is
  // actually happening (buffering spinner, failed start → paused icon).
  onState: ((s: { playing: boolean; buffering: boolean }) => void) | null = null;

  private emit(playing: boolean, buffering = false) {
    this.onState?.({ playing, buffering });
  }

  private ensureAudio(): HTMLAudioElement {
    if (!this.audio) {
      this.audio = new Audio();
      this.audio.loop = true; // streams are endless, but loop covers any EOF
      this.audio.preload = "none";
      // NOTE: do NOT set crossOrigin — SomaFM Icecast streams send no CORS
      // headers, so "anonymous" makes the WebView block the load entirely.
      // We never feed the stream through Web Audio, so opaque playback is fine.
      const a = this.audio;
      a.onwaiting = () => { if (!this.isYoutube) this.emit(true, true); };
      a.onplaying = () => { if (!this.isYoutube) this.emit(true, false); };
      a.onerror = () => {
        if (this.isYoutube) return; // stale event from an abandoned stream
        const msg = "Stream failed to load — check your internet connection.";
        console.warn("[music]", msg, this.audio?.error);
        this.emit(false);
        this.onError?.(msg);
      };
    }
    return this.audio;
  }

  private ensureCtx(): AudioContext {
    if (!this.ctx || this.ctx.state === "closed") {
      this.ctx = new AudioContext();
    }
    return this.ctx;
  }

  private stopGenerated() {
    this.genNodes.forEach((n) => {
      try { n.stop(); } catch { /* already stopped */ }
    });
    this.genNodes = [];
    this.gain?.disconnect();
    this.gain = null;
  }

  /** Start (or switch to) a user-added YouTube/URL station via the mpv sidecar. */
  playYoutube(stationId: string, url: string) {
    // Quiet the local engines so nothing double-plays.
    this.audio?.pause();
    this.stopGenerated();
    this.isYoutube = true;
    this.current = stationId;
    const tok = ++this.token;
    this.emit(true, true); // buffering until mpv accepts the load
    api.youtubePlay(url, Math.round(this.volume * 100))
      .then(() => {
        if (tok === this.token) this.emit(true, false);
      })
      .catch((err) => {
        if (tok !== this.token) return; // user already switched away
        console.warn("[music] youtube play failed", err);
        this.emit(false);
        this.onError?.(String(err));
      });
  }

  /** Resume the current station after a pause (routes by engine). */
  resume() {
    if (this.isYoutube) {
      api.youtubeResume().catch(() => {});
      this.emit(true);
      return;
    }
    if (this.current) this.play(this.current);
  }

  /** Start (or switch to) a station and play it. */
  play(stationId: string) {
    // Leaving a YouTube station: stop the sidecar first.
    if (this.isYoutube) {
      api.youtubeStop().catch(() => {});
      this.isYoutube = false;
    }
    const st = STATIONS[stationId] ?? STATIONS.lofi;
    this.current = stationId;
    const tok = ++this.token;

    if (st.kind === "stream") {
      this.stopGenerated();
      const a = this.ensureAudio();
      if (!a.src.includes(st.url)) a.src = st.url;
      a.volume = this.volume;
      this.emit(true, true); // buffering until the 'playing' event fires
      a.play().catch((err) => {
        if (tok !== this.token) return; // superseded by a newer switch
        // Autoplay rejection (before a user gesture) is benign; a real network
        // failure is not — surface anything that isn't a NotAllowedError.
        if (err?.name !== "NotAllowedError") {
          console.warn("[music] play failed", err);
          this.emit(false);
          this.onError?.("Couldn't start the stream — check your connection.");
        } else {
          console.info("[music] autoplay blocked — waiting for a user gesture");
          this.emit(false);
        }
      });
      return;
    }

    // generated (noise / binaural) — starts instantly, no buffering
    this.audio?.pause();
    this.stopGenerated();
    this.emit(true);
    const ctx = this.ensureCtx();
    if (ctx.state === "suspended") ctx.resume().catch(() => {});
    const gain = ctx.createGain();
    gain.gain.value = this.volume * 0.4;
    gain.connect(ctx.destination);
    this.gain = gain;

    if (st.kind === "binaural") {
      // two slightly detuned oscillators, hard-panned, create a beat frequency
      for (const [freq, pan] of [
        [st.base, -1],
        [st.base + st.beat, 1],
      ] as const) {
        const osc = ctx.createOscillator();
        osc.type = "sine";
        osc.frequency.value = freq;
        const panner = ctx.createStereoPanner();
        panner.pan.value = pan;
        osc.connect(panner).connect(gain);
        osc.start();
        this.genNodes.push(osc);
      }
      return;
    }

    // noise: brown noise buffer, looped; filtered per tone
    const len = ctx.sampleRate * 5;
    const buf = ctx.createBuffer(1, len, ctx.sampleRate);
    const data = buf.getChannelData(0);
    let last = 0;
    for (let i = 0; i < len; i++) {
      const white = Math.random() * 2 - 1;
      last = (last + 0.02 * white) / 1.02;
      data[i] = last * 3.5;
    }
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.loop = true;
    const filter = ctx.createBiquadFilter();
    if (st.tone === "rain") { filter.type = "highpass"; filter.frequency.value = 600; }
    else if (st.tone === "cafe") { filter.type = "bandpass"; filter.frequency.value = 500; }
    else { filter.type = "lowpass"; filter.frequency.value = 1200; }
    src.connect(filter).connect(gain);
    src.start();
    this.genNodes.push(src);
  }

  pause() {
    this.token++; // cancel any in-flight start
    this.emit(false);
    if (this.isYoutube) { api.youtubePause().catch(() => {}); return; }
    this.audio?.pause();
    this.stopGenerated();
    this.ctx?.suspend().catch(() => {});
  }

  setVolume(v: number) {
    this.volume = Math.max(0, Math.min(1, v));
    if (this.isYoutube) { api.youtubeSetVolume(Math.round(this.volume * 100)).catch(() => {}); return; }
    if (this.audio) this.audio.volume = this.volume;
    if (this.gain) this.gain.gain.value = this.volume * 0.4;
  }
}

export const music = new MusicController();

/** Built-in station ids (used to validate a saved default still exists). */
export const BUILTIN_STATION_IDS = Object.keys(STATIONS);
