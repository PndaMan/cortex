// Continuous study-music controller. A single module-level instance that
// outlives every view, so playback never stops/restarts on navigation.
//
// Two engines:
//  - Streaming stations use one persistent <audio> pointed at an ad-free,
//    continuous SomaFM stream (changing station swaps the src).
//  - Noise/binaural stations are generated locally with the Web Audio API, so
//    they loop seamlessly and work fully offline.

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

  private ensureAudio(): HTMLAudioElement {
    if (!this.audio) {
      this.audio = new Audio();
      this.audio.loop = true; // streams are endless, but loop covers any EOF
      this.audio.preload = "none";
      this.audio.crossOrigin = "anonymous";
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

  /** Start (or switch to) a station and play it. */
  play(stationId: string) {
    const st = STATIONS[stationId] ?? STATIONS.lofi;
    this.current = stationId;

    if (st.kind === "stream") {
      this.stopGenerated();
      const a = this.ensureAudio();
      if (!a.src.includes(st.url)) a.src = st.url;
      a.volume = this.volume;
      a.play().catch(() => {}); // ignore autoplay rejection until a user gesture
      return;
    }

    // generated (noise / binaural)
    this.audio?.pause();
    this.stopGenerated();
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
    this.audio?.pause();
    this.stopGenerated();
    this.ctx?.suspend().catch(() => {});
  }

  setVolume(v: number) {
    this.volume = Math.max(0, Math.min(1, v));
    if (this.audio) this.audio.volume = this.volume;
    if (this.gain) this.gain.gain.value = this.volume * 0.4;
  }
}

export const music = new MusicController();
