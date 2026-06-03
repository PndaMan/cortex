// Central app state using Svelte 5 runes. Single source of UI truth; views read
// `app.*` and call its actions. Real data flows in via src/lib/api.ts; content
// for not-yet-backed screens comes from src/lib/mock.ts.

import * as api from "./api";
import type { Subject, Source } from "./api";
import { music } from "./music";
import { keybinds } from "./keybinds.svelte";

export type View =
  | "dashboard"
  | "subject"
  | "source"
  | "add-source"
  | "add-subject"
  | "websearch"
  | "recorder"
  | "gen-material"
  | "settings";
export type Mode = "NOR" | "INS" | "SEL";
export type Toast = {
  id: string;
  kind: "info" | "success" | "warning" | "error";
  title: string;
  body?: string;
  action?: { label: string; run: () => void };
};

export const THEMES = ["osaka-jade", "tokyo-night", "catppuccin"] as const;
export type Theme = (typeof THEMES)[number];
const THEME_LABELS: Record<Theme, string> = {
  "osaka-jade": "Osaka Jade",
  "tokyo-night": "Tokyo Night",
  catppuccin: "Catppuccin Mocha",
};

export type Music = { current: string; playing: boolean; volume: number };

function uid() {
  return Math.random().toString(36).slice(2);
}

class AppStore {
  // data
  subjects = $state<Subject[]>([]);
  activeSubjectId = $state<string | null>(null);
  activeSource = $state<Source | null>(null);
  loading = $state(true);

  // navigation
  view = $state<View>("subject");
  subjectTab = $state<string>("cheatsheet"); // cheatsheet is the default page
  dashFocus = $state(0);

  // chrome / modal state
  mode = $state<Mode>("NOR");
  theme = $state<Theme>("osaka-jade");
  cmdkOpen = $state(false);
  leaderOpen = $state(false);
  chatOpen = $state(true);
  musicOpen = $state(false);
  diffOpen = $state(false);
  helpOpen = $state(false);
  onboarding = $state(false);
  metaModal = $state<any | null>(null);
  toasts = $state<Toast[]>([]);
  pending = $state(0); // cheatsheet draft sections awaiting review (real count set by Cheatsheet view)
  // playing starts false — browsers block autoplay until a user gesture.
  music = $state<Music>({ current: "lofi", playing: false, volume: 60 });

  activeSubject = $derived(
    this.subjects.find((s) => s.id === this.activeSubjectId) ?? null
  );

  async init() {
    this.loading = true;
    // Surface stream/playback failures instead of swallowing them.
    music.onError = (m) => this.pushToast({ kind: "warning", title: "Music", body: m });
    try {
      // Start empty on a fresh install — no demo seeding. Every view renders a
      // proper empty state when there are no subjects/sources.
      const subs = await api.listSubjects();
      this.subjects = subs;
      if (subs.length && !this.activeSubjectId) this.activeSubjectId = subs[0].id;
    } catch (e) {
      this.pushToast({ kind: "error", title: "Failed to load", body: String(e) });
    } finally {
      this.loading = false;
    }
    // Restore preferences: theme, keybinds, and audio defaults.
    try {
      const all = await api.getAllSettings();
      const savedTheme = all["theme"] as Theme | undefined;
      if (savedTheme && THEMES.includes(savedTheme)) this.setTheme(savedTheme);
      else this.applyTheme(this.theme);
      await keybinds.load(all); // reuse the settings we already fetched
      if (all["default_station"]) this.music = { ...this.music, current: all["default_station"] };
      if (all["autoplay"] === "true") this.toggleMusic();
    } catch {
      this.applyTheme(this.theme);
    }
  }

  async refresh() {
    this.subjects = await api.listSubjects();
    if (this.activeSource) {
      this.activeSource =
        (await api.getSource(this.activeSource.id).catch(() => null)) ?? this.activeSource;
    }
  }

  // ---- navigation actions ----
  openSubject(id: string) {
    this.activeSubjectId = id;
    this.view = "subject";
    this.subjectTab = "cheatsheet"; // land on the cheatsheet (the default page)
  }
  openSource(src: Source) {
    this.activeSource = src;
    this.view = "source";
  }
  closeSource() {
    this.view = "subject";
  }
  setView(v: View) {
    this.view = v;
  }
  setTab(t: string) {
    this.subjectTab = t;
  }
  setMode(m: Mode) {
    this.mode = m;
  }
  toggleChat() {
    this.chatOpen = !this.chatOpen;
  }
  openDiff() {
    this.diffOpen = true;
  }
  reviewDiff() {
    this.view = "subject";
    this.subjectTab = "cheatsheet";
    setTimeout(() => (this.diffOpen = true), 30);
  }
  mergeDiff() {
    this.diffOpen = false;
    this.pending = 0;
    this.pushToast({ kind: "success", title: "Cheatsheet merged", body: "Approved sections are now part of the cheatsheet." });
  }

  // ---- subject / source / topic CRUD ----
  async deleteSubject(id: string) {
    try {
      await api.deleteSubject(id);
      const wasActive = this.activeSubjectId === id;
      this.subjects = await api.listSubjects();
      if (wasActive) {
        this.activeSubjectId = this.subjects[0]?.id ?? null;
        this.activeSource = null;
        this.view = this.subjects.length ? "subject" : "dashboard";
      }
      this.pushToast({ kind: "success", title: "Subject deleted" });
    } catch (e) {
      this.pushToast({ kind: "error", title: "Delete failed", body: String(e) });
    }
  }

  async updateSubject(id: string, name: string, code?: string) {
    try {
      await api.updateSubject(id, name, code);
      this.subjects = await api.listSubjects();
      this.pushToast({ kind: "success", title: "Subject updated" });
    } catch (e) {
      this.pushToast({ kind: "error", title: "Update failed", body: String(e) });
    }
  }

  async deleteSource(id: string) {
    try {
      await api.deleteSource(id);
      if (this.activeSource?.id === id) {
        this.activeSource = null;
        this.view = "subject";
      }
      await this.refresh();
      this.pushToast({ kind: "success", title: "Source deleted" });
    } catch (e) {
      this.pushToast({ kind: "error", title: "Delete failed", body: String(e) });
    }
  }

  async createTopic(name: string) {
    const sid = this.activeSubjectId;
    if (!sid || !name.trim()) return;
    try {
      await api.createTopic(sid, name.trim());
      await this.refresh();
      this.pushToast({ kind: "success", title: "Topic added" });
    } catch (e) {
      this.pushToast({ kind: "error", title: "Add topic failed", body: String(e) });
    }
  }

  async deleteTopic(id: string) {
    const sid = this.activeSubjectId;
    if (!sid) return;
    try {
      await api.deleteTopic(id, sid);
      await this.refresh();
      this.pushToast({ kind: "success", title: "Topic deleted" });
    } catch (e) {
      this.pushToast({ kind: "error", title: "Delete topic failed", body: String(e) });
    }
  }

  // ---- theme ----
  applyTheme(t: Theme) {
    document.documentElement.setAttribute("data-theme", t);
  }
  setTheme(t: Theme) {
    this.theme = t;
    this.applyTheme(t);
    api.setSetting("theme", t).catch(() => {});
  }
  cycleTheme() {
    const next = THEMES[(THEMES.indexOf(this.theme) + 1) % THEMES.length];
    this.setTheme(next);
    this.pushToast({
      kind: "info",
      title: "Theme synced",
      body: `Matched Omarchy palette → ${THEME_LABELS[next]}.`,
    });
  }

  // ---- music (continuous; driven by lib/music controller) ----
  toggleMusic() {
    const playing = !this.music.playing;
    this.music = { ...this.music, playing };
    if (playing) music.play(this.music.current);
    else music.pause();
  }
  pickStation(id: string) {
    this.music = { ...this.music, current: id, playing: true };
    music.play(id);
  }
  setVolume(v: number) {
    this.music = { ...this.music, volume: v };
    music.setVolume(v / 100);
  }

  // ---- toasts ----
  pushToast(t: Omit<Toast, "id">) {
    const id = uid();
    this.toasts = [...this.toasts, { ...t, id }];
    setTimeout(() => this.dismissToast(id), 5200);
  }
  dismissToast(id: string) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  // sources of the active subject, flattened across topics
  activeSources(): Source[] {
    const s = this.activeSubject;
    if (!s) return [];
    return s.topics.flatMap((t) => t.sources);
  }
}

export const app = new AppStore();
