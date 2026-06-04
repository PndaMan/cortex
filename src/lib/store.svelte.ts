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
  | "notes"
  | "calendar"
  | "settings";
export type Mode = "NOR" | "INS" | "SEL";
export type Toast = {
  id: string;
  kind: "info" | "success" | "warning" | "error";
  title: string;
  body?: string;
  action?: { label: string; run: () => void };
};

export const THEMES = [
  "osaka-jade", "tokyo-night", "catppuccin",
  "gruvbox", "nord", "dracula", "rose-pine", "everforest", "solarized", "kanagawa",
] as const;
export type Theme = (typeof THEMES)[number];
export const THEME_LABELS: Record<Theme, string> = {
  "osaka-jade": "Osaka Jade",
  "tokyo-night": "Tokyo Night",
  catppuccin: "Catppuccin Mocha",
  gruvbox: "Gruvbox",
  nord: "Nord",
  dracula: "Dracula",
  "rose-pine": "Rosé Pine",
  everforest: "Everforest",
  solarized: "Solarized",
  kanagawa: "Kanagawa",
};

export type Music = { current: string; playing: boolean; volume: number };

export type PomoPhase = "work" | "break" | "long";

export type DialogSpec = {
  kind: "confirm" | "prompt";
  title: string;
  body?: string;
  label?: string;
  value?: string;
  placeholder?: string;
  danger?: boolean;
  okLabel?: string;
};

// Rich edit-modal targets — carry the current field values to seed the form.
export type EditTarget =
  | { kind: "subject"; id: string; name: string; code: string; glyph: string; color: string }
  | { kind: "topic"; id: string; name: string; subjectId: string; glyph: string }
  | {
      kind: "source";
      id: string;
      name: string;
      subjectId: string; // the source's current subject (for cross-subject moves)
      topicId: string | null;
      tags: string[];
      topicOptions: { id: string; label: string }[];
    };

// Stable default subject colors (drawn from the shipped theme accents) used when
// a subject has no explicit color set.
export const SUBJECT_COLORS = [
  "#2dd5b7", "#7aa2f7", "#f7768e", "#e0af68",
  "#9ece6a", "#bb9af7", "#7dcfff", "#ff9e64",
];

// Selectable subject glyphs — subject-relevant emojis. Rendered as text so they
// show in full where displayed; the subject's color accents the card/border.
export const GLYPHS = [
  "📘", "📗", "📙", "📕", "📐", "📏", "🧠", "⚛️", "🔬", "🧪",
  "🧫", "🦠", "🧬", "💻", "🖥️", "⌨️", "📊", "📈", "📉", "🧮",
  "🔢", "➗", "🎨", "🖌️", "🎵", "🎼", "🎹", "🎸", "🌍", "🗺️",
  "🧭", "⚖️", "🏛️", "📜", "🏺", "🗿", "🩺", "💊", "💉", "🫀",
  "🔭", "🧰", "🪐", "🚀", "🛰️", "✍️", "🖊️", "📝", "🧩", "💡",
  "🎭", "🎬", "🎮", "⚙️", "🔧", "🔌", "🔋", "🌐", "📡", "🛡️",
  "⚔️", "🏹", "🎯", "🌱", "🌿", "🍃", "🌋", "🌊", "🔥", "❄️",
  "🐍", "🦴", "🗣️", "💬", "📚", "🔑",
];

// Clean, minimalist topic emojis, assigned deterministically per topic so each
// has a stable little icon. Rendered slightly desaturated so they read as a
// subtle accent rather than loud color.
export const TOPIC_GLYPHS = [
  "📄", "📝", "📃", "📐", "🔖", "🏷️", "📌", "📍", "📎", "🗂️",
  "📑", "🗃️", "🗄️", "📁", "📂", "💬", "🗨️", "💭", "📊", "📈",
  "📉", "🔬", "🧪", "⚗️", "🧫", "🧮", "📚", "📖", "📓", "📔",
  "🗒️", "📋", "🎯", "🧩", "🔭", "💡", "⚙️", "🧠", "🪶", "🧭",
  "🔑", "📦", "🌱", "🔍", "🔎", "✏️", "🖊️", "⭐", "✨", "🔆",
];
export function topicGlyph(id: string): string {
  let h = 0;
  for (const c of id) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return TOPIC_GLYPHS[h % TOPIC_GLYPHS.length];
}

function uid() {
  return Math.random().toString(36).slice(2);
}

// ───────────────────────────── Pomodoro timer ─────────────────────────────
// App-wide focus timer. Lives in the store so it keeps ticking after the panel
// closes and the LiveActivity widget can mirror it. A single setInterval (started
// lazily on first pomoStart) drives `remainingMs` down and auto-advances phases:
// work → break → work … with a long break after every Nth focus session.
class PomoTimer {
  phase = $state<PomoPhase>("work");
  running = $state(false);
  completedSessions = $state(0); // total focus sessions finished
  cycle = $state(1); // 1..sessionsBeforeLong — focus session within the current set

  // configurable durations (minutes)
  workMin = $state(25);
  breakMin = $state(5);
  longBreakMin = $state(15);
  sessionsBeforeLong = $state(4);

  remainingMs = $state(25 * 60_000);

  // anchor for the running segment + the interval handle
  #lastAt = 0;
  #interval: ReturnType<typeof setInterval> | null = null;
  #onPhaseChange: ((to: PomoPhase) => void) | null = null;

  phaseMin(p: PomoPhase = this.phase): number {
    return p === "work" ? this.workMin : p === "long" ? this.longBreakMin : this.breakMin;
  }
  totalMs(p: PomoPhase = this.phase): number {
    return Math.max(1, this.phaseMin(p)) * 60_000;
  }
  /** 0..1 elapsed fraction of the current phase. */
  get progress(): number {
    const total = this.totalMs();
    return Math.min(1, Math.max(0, 1 - this.remainingMs / total));
  }
  get mmss(): string {
    const s = Math.max(0, Math.ceil(this.remainingMs / 1000));
    const m = Math.floor(s / 60);
    return `${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
  }
  get phaseLabel(): string {
    if (this.phase === "work") return `Focus ${this.cycle} of ${this.sessionsBeforeLong}`;
    return this.phase === "long" ? "Long break" : "Short break";
  }
  /** Whether the live activity has anything worth showing. */
  get active(): boolean {
    return this.running || this.remainingMs < this.totalMs();
  }

  onPhaseChange(fn: (to: PomoPhase) => void) {
    this.#onPhaseChange = fn;
  }

  #ensureInterval() {
    if (this.#interval) return;
    this.#lastAt = Date.now();
    this.#interval = setInterval(() => this.#tick(), 250);
  }
  #stopInterval() {
    if (this.#interval) {
      clearInterval(this.#interval);
      this.#interval = null;
    }
  }
  #tick() {
    if (!this.running) return;
    const now = Date.now();
    this.remainingMs = Math.max(0, this.remainingMs - (now - this.#lastAt));
    this.#lastAt = now;
    if (this.remainingMs <= 0) this.#advance();
  }

  #advance() {
    const from = this.phase;
    if (from === "work") {
      this.completedSessions += 1;
      const isLong = this.cycle % this.sessionsBeforeLong === 0;
      this.phase = isLong ? "long" : "break";
    } else {
      if (from === "long") this.cycle = 1;
      else this.cycle += 1;
      this.phase = "work";
    }
    this.remainingMs = this.totalMs();
    this.#lastAt = Date.now();
    this.running = true; // auto-continue into the next phase
    this.#onPhaseChange?.(this.phase);
  }

  // ── actions ──
  pomoStart() {
    if (this.running) return;
    if (this.remainingMs <= 0) this.remainingMs = this.totalMs();
    this.running = true;
    this.#lastAt = Date.now();
    this.#ensureInterval();
  }
  pomoPause() {
    this.running = false;
  }
  pomoToggle() {
    this.running ? this.pomoPause() : this.pomoStart();
  }
  pomoReset() {
    this.running = false;
    this.remainingMs = this.totalMs();
    this.#stopInterval();
  }
  /** Skip to the next phase immediately (counts a completed focus session). */
  pomoSkip() {
    this.remainingMs = 0;
    this.#advance();
    this.#ensureInterval();
  }
  /** Apply a duration change; if it's the current phase and idle, reflect it. */
  setDurations(work: number, brk: number, long: number) {
    const clamp = (v: number) => Math.max(1, Math.min(180, Math.round(v) || 1));
    this.workMin = clamp(work);
    this.breakMin = clamp(brk);
    this.longBreakMin = clamp(long);
    if (!this.running && this.remainingMs >= this.totalMs() - 50) {
      this.remainingMs = this.totalMs();
    }
  }
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
  dashFocus = $state(-1); // -1 = nothing focused on load (no stray focus ring)
  // Current chat scope (set by ChatPanel) so the status-bar PWD reflects it.
  chatScope = $state<{ topicName?: string; sourceName?: string } | null>(null);
  // When set, AddSource preselects this topic (used by the per-topic + button).
  addSourceTopicId = $state<string | null>(null);

  // chrome / modal state
  mode = $state<Mode>("NOR");
  theme = $state<Theme>("osaka-jade");
  cmdkOpen = $state(false);
  leaderOpen = $state(false);
  chatOpen = $state(true);
  sidebarCollapsed = $state(false);
  findOpen = $state(false);
  musicOpen = $state(false);
  diffOpen = $state(false);
  helpOpen = $state(false);
  pomodoroOpen = $state(false);
  // App-wide focus timer + its floating live-activity widget.
  pomo = new PomoTimer();
  pomoCorner = $state<"tl" | "tr" | "bl" | "br">("br"); // snapped corner (session)
  pomoLiveMin = $state(false); // live widget minimised to a pill
  pomoLiveForce = $state(false); // keep showing the widget even when idle
  onboarding = $state(false);
  metaModal = $state<any | null>(null);
  toasts = $state<Toast[]>([]);
  // themed confirm/prompt dialog (replaces native window.confirm / window.prompt)
  dialog = $state<DialogSpec | null>(null);
  // rich multi-field edit modal (subjects/topics/sources fully editable)
  editing = $state<EditTarget | null>(null);
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
    // Toast on every focus↔break transition.
    this.pomo.onPhaseChange((to) => {
      if (to === "work") {
        this.pushToast({ kind: "info", title: "Back to focus", body: `Session ${this.pomo.cycle} — let's go.` });
      } else {
        this.pushToast({
          kind: "success",
          title: to === "long" ? "Long break" : "Break time",
          body: "Nice focus — step away for a bit.",
        });
      }
    });
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
      // Reopen the last-viewed subject (its chat history loads with it).
      const last = all["last_subject_id"];
      if (last && this.subjects.some((s) => s.id === last)) this.activeSubjectId = last;
      if (all["default_station"]) this.music = { ...this.music, current: all["default_station"] };
      if (all["autoplay"] === "true") this.toggleMusic();
      // Restore focus-timer (pomodoro) durations.
      for (const k of ["workMin", "breakMin", "longBreakMin", "sessionsBeforeLong"] as const) {
        const v = parseInt(all["pomo_" + k] ?? "", 10);
        if (Number.isFinite(v) && v > 0) (this.pomo as unknown as Record<string, number>)[k] = v;
      }
    } catch {
      this.applyTheme(this.theme);
    }
    this.startReminderPolling();
  }

  // ---- calendar reminders (in-app notifications) ----
  #reminderTimer: ReturnType<typeof setInterval> | null = null;
  startReminderPolling() {
    if (this.#reminderTimer) return; // single poller
    const tick = async () => {
      try {
        const due = await api.checkReminders();
        for (const e of due) {
          this.pushToast({
            kind: "info",
            title: `⏰ ${e.title}`,
            body: e.location ? `at ${e.location}` : "Reminder",
          });
        }
      } catch {
        /* offline / no events table yet — ignore */
      }
    };
    tick();
    this.#reminderTimer = setInterval(tick, 60_000);
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
    api.setSetting("last_subject_id", id).catch(() => {}); // reopen on next launch
  }
  openSource(src: Source) {
    this.activeSource = src;
    this.view = "source";
  }
  // Open the add-source view with a topic preselected (per-topic + button).
  newSourceInTopic(topicId: string) {
    this.addSourceTopicId = topicId;
    this.view = "add-source";
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
  // Bumped to ask the Cheatsheet view (which owns generation + the live view
  // refresh) to (re)generate the whole-subject sheet — used by the command
  // palette's "Regenerate cheatsheet" so it actually does something.
  cheatsheetRegenNonce = $state(0);
  regenCheatsheet() {
    if (!this.activeSubject) {
      this.pushToast({ kind: "warning", title: "Open a subject first" });
      return;
    }
    this.setView("subject");
    this.subjectTab = "cheatsheet";
    this.cheatsheetRegenNonce++;
  }
  setMode(m: Mode) {
    this.mode = m;
  }
  toggleChat() {
    this.chatOpen = !this.chatOpen;
  }
  toggleSidebar() {
    this.sidebarCollapsed = !this.sidebarCollapsed;
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

  async updateSubject(id: string, name: string, code?: string, glyph?: string, color?: string) {
    try {
      await api.updateSubject(id, name, code, glyph, color);
      this.subjects = await api.listSubjects();
      this.pushToast({ kind: "success", title: "Subject updated" });
    } catch (e) {
      this.pushToast({ kind: "error", title: "Update failed", body: String(e) });
    }
  }

  async updateTopic(id: string, name: string, subjectId?: string, glyph?: string) {
    const sid = subjectId ?? this.activeSubjectId;
    if (!sid || !name.trim()) return;
    try {
      await api.updateTopic(id, name.trim(), sid, glyph);
      await this.refresh();
      this.pushToast({ kind: "success", title: "Topic updated" });
    } catch (e) {
      this.pushToast({ kind: "error", title: "Rename failed", body: String(e) });
    }
  }

  async updateSource(id: string, name: string, topicId?: string | null, tags?: string[]) {
    try {
      const updated = await api.updateSource(id, name, topicId ?? null, tags);
      if (this.activeSource?.id === id) this.activeSource = updated;
      await this.refresh();
      this.pushToast({ kind: "success", title: "Source updated" });
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

  async createTopic(name: string, glyph?: string) {
    const sid = this.activeSubjectId;
    if (!sid || !name.trim()) return;
    try {
      await api.createTopic(sid, name.trim(), glyph);
      await this.refresh();
      this.pushToast({ kind: "success", title: "Topic added" });
    } catch (e) {
      this.pushToast({ kind: "error", title: "Add topic failed", body: String(e) });
    }
  }

  async deleteTopic(id: string, subjectId?: string) {
    const sid = subjectId ?? this.activeSubjectId;
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

  // ---- rich edit modal ----
  openEdit(t: EditTarget) {
    this.editing = t;
  }
  closeEdit() {
    this.editing = null;
  }

  /** A subject's display color: its own color, or a stable default from the palette. */
  subjectColor(s: { id: string; color?: string | null; position?: number } | null | undefined): string {
    if (!s) return "var(--accent)";
    if (s.color) return s.color;
    const i = typeof s.position === "number"
      ? s.position
      : [...s.id].reduce((a, c) => a + c.charCodeAt(0), 0);
    return SUBJECT_COLORS[((i % SUBJECT_COLORS.length) + SUBJECT_COLORS.length) % SUBJECT_COLORS.length];
  }

  // ---- themed dialogs (confirm / prompt) ----
  #dialogResolve: ((v: any) => void) | null = null;
  /** Themed replacement for window.confirm. Resolves true on OK, false otherwise. */
  confirm(opts: { title: string; body?: string; danger?: boolean; okLabel?: string }): Promise<boolean> {
    return new Promise((resolve) => {
      this.#dialogResolve = resolve;
      this.dialog = { kind: "confirm", okLabel: "Confirm", ...opts };
    });
  }
  /** Themed replacement for window.prompt. Resolves the trimmed string, or null if cancelled. */
  prompt(opts: { title: string; body?: string; label?: string; value?: string; placeholder?: string; okLabel?: string }): Promise<string | null> {
    return new Promise((resolve) => {
      this.#dialogResolve = resolve;
      this.dialog = { kind: "prompt", okLabel: "OK", value: "", ...opts };
    });
  }
  /** Called by the Dialog component to settle the active dialog. */
  resolveDialog(v: boolean | string | null) {
    const r = this.#dialogResolve;
    this.dialog = null;
    this.#dialogResolve = null;
    r?.(v);
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
