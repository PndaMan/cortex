// Continuous study-music controller. A single module-level instance that
// outlives every view, so playback never stops/restarts on navigation.
//
// Every station — the built-ins below and any user-added one — is a YouTube
// video or livestream, streamed ad-free by the Rust mpv sidecar (see
// src-tauri/src/mpv.rs, which also auto-downloads mpv + yt-dlp). The controller
// just drives play/pause/volume over Tauri commands; there is no local
// HTML5/<audio> or Web Audio playback anymore.

import * as api from "./api";

// Built-in stations, keyed by the ids used in src/lib/mock.ts. All play through
// the mpv sidecar via api.youtubePlay, exactly like user-added stations.
const STATIONS: Record<string, { url: string }> = {
  // ── songs ──
  synthwave: { url: "https://www.youtube.com/watch?v=fhL67fnDXcU" },
  // id kept as "lofi" so any saved default-station setting survives the rework.
  lofi: { url: "https://www.youtube.com/watch?v=53gNFOqDFcE" },
  jazz: { url: "https://www.youtube.com/watch?v=fQIJb1h1RkA&list=PLA4bfv_RILLI9Vt180q_ke-kLuznho3Ff" },
  classical: { url: "https://www.youtube.com/watch?v=mdJU5ogrPMY&list=RDmdJU5ogrPMY&start_radio=1" },
  // ── noises ──
  rain: { url: "https://www.youtube.com/watch?v=TeMT9xO7d0g" },
  forest: { url: "https://www.youtube.com/watch?v=ZwJ0pY6sXoY" },
  binaural: { url: "https://www.youtube.com/watch?v=1_G60OdEzXs" },
};

class MusicController {
  private volume = 0.6;
  current: string | null = null;
  // Bumped on every play/switch so callbacks from a superseded station (late
  // errors, stale promises) can't clobber the state of the one now playing.
  private token = 0;
  // Set by the store so playback failures surface to the user instead of being
  // swallowed (the mpv sidecar needs internet + yt-dlp; failures show a toast).
  onError: ((msg: string) => void) | null = null;
  // Authoritative engine state, pushed to the store: the UI reflects what is
  // actually happening (buffering spinner, failed start → paused icon).
  onState: ((s: { playing: boolean; buffering: boolean }) => void) | null = null;

  private emit(playing: boolean, buffering = false) {
    this.onState?.({ playing, buffering });
  }

  /** Start (or switch to) a YouTube/URL station via the mpv sidecar. */
  playYoutube(stationId: string, url: string) {
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

  /** Start (or switch to) a built-in station — routes through the mpv sidecar. */
  play(stationId: string) {
    const st = STATIONS[stationId] ?? STATIONS.lofi;
    this.playYoutube(STATIONS[stationId] ? stationId : "lofi", st.url);
  }

  /** Resume the current station after a pause. */
  resume() {
    api.youtubeResume().catch(() => {});
    this.emit(true);
  }

  pause() {
    this.token++; // cancel any in-flight start
    this.emit(false);
    api.youtubePause().catch(() => {});
  }

  setVolume(v: number) {
    this.volume = Math.max(0, Math.min(1, v));
    api.youtubeSetVolume(Math.round(this.volume * 100)).catch(() => {});
  }
}

export const music = new MusicController();

/** Built-in station ids (used to validate a saved default still exists). */
export const BUILTIN_STATION_IDS = Object.keys(STATIONS);
