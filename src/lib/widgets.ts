// Builds the study snapshot that feeds the iOS Home/Lock-screen widgets, and pushes it into the
// App Group via the native plugin. Off-iOS this is inert. Everything is best-effort: a missing
// field just renders a lighter widget — never throws.

import { app } from "./store.svelte";
import * as api from "./api";
import { isIOS } from "./platform";
import { setWidgetSnapshot, listInbox, readRecordingBytes, deleteRecording } from "./nativeRecorder";
import { notifyNow } from "./notifications";

interface DeadlineItem { id: string; title: string; course: string; dueAt: number; kind: string }
interface AgendaItem { id: string; title: string; at: number; course: string }
interface SubjectProgress { id: string; name: string; progress: number; accentHex: string }
interface PomodoroState { running: boolean; phase: string; remainingSec: number; progress: number; label: string }
interface ThemeColors { bg: string; surface: string; fg: string; fgBright: string; fgMuted: string; accent: string; warn: string; err: string }
interface WidgetSnapshot {
  updatedAt: number;
  theme: ThemeColors;
  deadlines: DeadlineItem[];
  agenda: AgendaItem[];
  subjects: SubjectProgress[];
  streak: number;
  flashcardsDue: number;
  pomodoro: PomodoroState | null;
  activeSubject: string | null;
}

function cssVar(name: string, fallback: string): string {
  try {
    const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return v || fallback;
  } catch {
    return fallback;
  }
}

/** Mirror the live in-app palette so widgets re-skin with the chosen theme. */
function readTheme(): ThemeColors {
  return {
    bg: cssVar("--bg", "#0e1813"),
    surface: cssVar("--surface", "#111c18"),
    fg: cssVar("--fg", "#c1c497"),
    fgBright: cssVar("--fg-bright", "#f6f5dd"),
    fgMuted: cssVar("--fg-muted", "#8a9a7e"),
    accent: cssVar("--accent", "#2dd5b7"),
    warn: cssVar("--warn", "#e5c736"),
    err: cssVar("--err", "#ff5345"),
  };
}

// Cache the analytics summary (streak + due forecast) so a snapshot rebuild is cheap.
let statsCache: api.AnalyticsSummary | null = null;
let statsAt = 0;
async function stats(): Promise<api.AnalyticsSummary | null> {
  if (statsCache && Date.now() - statsAt < 5 * 60_000) return statsCache;
  try {
    statsCache = await api.analyticsSummary(30);
    statsAt = Date.now();
  } catch {
    /* keep the previous cache (or null) */
  }
  return statsCache;
}

function todayISO(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

async function buildSnapshot(): Promise<WidgetSnapshot> {
  const theme = readTheme();
  const accent = theme.accent;

  // Deadlines / exams from the Moodle-derived notification feed (already in memory).
  const notifs = (app.notifications ?? []).filter((n) => n.kind === "deadline" || n.kind === "exam");
  const deadlines: DeadlineItem[] = notifs
    .map((n) => ({ id: n.id, title: n.title, course: n.course, dueAt: Math.round(n.ts / 1000), kind: n.kind === "exam" ? "exam" : "deadline" }))
    .sort((a, b) => a.dueAt - b.dueAt)
    .slice(0, 8);

  // Today's agenda = items due today (Cortex has no separate timetable source).
  const startToday = new Date(); startToday.setHours(0, 0, 0, 0);
  const endToday = new Date(); endToday.setHours(23, 59, 59, 999);
  const agenda: AgendaItem[] = notifs
    .filter((n) => n.ts >= startToday.getTime() && n.ts <= endToday.getTime())
    .map((n) => ({ id: n.id, title: n.title, at: Math.round(n.ts / 1000), course: n.course }))
    .sort((a, b) => a.at - b.at)
    .slice(0, 5);

  // Subject progress: relative material volume (truthful + needs no extra call).
  const subs = app.subjects ?? [];
  const maxSrc = Math.max(1, ...subs.map((s) => s.sourceCount ?? 0));
  const subjects: SubjectProgress[] = subs
    .slice()
    .sort((a, b) => (a.position ?? 0) - (b.position ?? 0))
    .slice(0, 5)
    .map((s) => ({
      id: s.id,
      name: s.name,
      progress: Math.min(1, (s.sourceCount ?? 0) / maxSrc),
      accentHex: s.color || accent,
    }));

  // Streak + cards-due from analytics (cached); fall back to subject streaks.
  const st = await stats();
  const streak = st?.streak ?? Math.max(0, ...subs.map((s) => s.streak ?? 0));
  const today = todayISO();
  const flashcardsDue = st?.due_forecast?.find((d) => d.day === today)?.due
    ?? st?.due_forecast?.[0]?.due
    ?? 0;

  // Focus timer mirror.
  const p = app.pomo;
  const pomodoro: PomodoroState | null = p
    ? {
        running: p.running,
        phase: p.phase,
        remainingSec: Math.max(0, Math.ceil(p.remainingMs / 1000)),
        progress: p.progress,
        label: p.phaseLabel,
      }
    : null;

  return {
    updatedAt: Math.round(Date.now() / 1000),
    theme,
    deadlines,
    agenda,
    subjects,
    streak,
    flashcardsDue,
    pomodoro,
    activeSubject: app.activeSubject?.name ?? null,
  };
}

let pushing = false;
let lastPush = 0;

/** Build + push the widget snapshot to iOS. Throttled; safe to call liberally. */
export async function refreshWidgets(force = false): Promise<void> {
  if (!isIOS) return;
  if (pushing) return;
  if (!force && Date.now() - lastPush < 5_000) return;
  pushing = true;
  try {
    const snap = await buildSnapshot();
    await setWidgetSnapshot(JSON.stringify(snap));
    lastPush = Date.now();
  } catch {
    /* plugin unavailable / off-iOS — ignore */
  } finally {
    pushing = false;
  }
}

/** Start the widget feed: an initial push + a periodic refresh. Call once after mount. */
export function startWidgetFeed(): void {
  if (!isIOS) return;
  setTimeout(() => void refreshWidgets(true), 1500);
  setInterval(() => void refreshWidgets(true), 60_000);
  // Drain any lectures recorded entirely in the background (from a widget / the Live Activity)
  // while the app was closed. Retry a few times so subjects have time to load.
  let tries = 0;
  const drain = () => {
    void drainBackgroundRecordings();
    if (++tries < 4) setTimeout(drain, 4000);
  };
  setTimeout(drain, 4000);
}

let draining = false;
/**
 * Ingest finished recordings sitting in the App Group inbox (a lecture captured from the
 * Home/Lock-screen record button or Live Activity Stop while the app wasn't running). Each file
 * is matched to its subject by name (the snapshot's active subject at record time), saved through
 * the normal transcribe pipeline, then removed.
 */
export async function drainBackgroundRecordings(): Promise<void> {
  if (!isIOS || draining) return;
  draining = true;
  try {
    const files = await listInbox();
    if (!files.length) return;
    const subjects = app.subjects ?? [];
    if (!subjects.length) return; // not loaded yet — a later retry will catch it
    let saved = 0;
    for (const f of files) {
      try {
        const subj = subjects.find((s) => s.name === f.subject) ?? subjects[0];
        if (!subj) continue;
        const bytes = await readRecordingBytes(f.path);
        if (!bytes.length) { await deleteRecording(f.path); continue; }
        const name = f.name.replace(/\.m4a$/i, "").replace(/^lecture-/i, "Lecture ");
        await api.saveRecording(subj.id, name, bytes, undefined, "m4a");
        await deleteRecording(f.path);
        saved++;
      } catch {
        /* leave this file for a later attempt */
      }
    }
    if (saved > 0) {
      try { await app.refresh(); } catch { /* ignore */ }
      const title = saved === 1 ? "Background lecture saved" : `${saved} background lectures saved`;
      app.pushToast?.({ kind: "success", title, body: "Recorded from your Lock Screen and transcribed." });
      notifyNow("transcribed", `✅ ${title}`, "Recorded from your Lock Screen and transcribed.");
      void refreshWidgets(true);
    }
  } catch {
    /* plugin unavailable — ignore */
  } finally {
    draining = false;
  }
}
