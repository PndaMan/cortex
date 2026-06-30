// Cortex notifications — local + scheduled (no server). Covers the 10 enabled types:
//   immediate  : lecture transcribed ✓ / failed ✗, material ready, new grade, new announcement
//   scheduled  : deadline (24h/3h), exam (3d/1d), flashcards-due (daily), daily review (daily),
//                long-recording-still-running (one-shot)
// Scheduled notifications fire even when the app is CLOSED (iOS UNUserNotificationCenter), so no
// push server is needed. New grades/announcements are detected here on foreground/after sync, and
// (while fully closed) by the native Background-App-Refresh task → Rust `moodle_background_check`.
//
// Desktop already shows in-app toasts for most of these, so system notifications are mobile-only
// to avoid double-notifying.

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
  cancel,
  Schedule,
} from "@tauri-apps/plugin-notification";
import { app } from "./store.svelte";
import * as api from "./api";
import { isMobile } from "./platform";

export type NotifKey =
  | "transcribed" | "transcribe_failed" | "long_recording"
  | "grade" | "announcement"
  | "deadline" | "exam"
  | "flashcards" | "daily_review" | "material";

// Fixed numeric ids for the repeating/singleton schedules; per-event ones are hashed.
const ID_FLASHCARDS = 900001;
const ID_DAILY_REVIEW = 900002;
const ID_LONG_RECORDING = 900003;
const REG_KEY = "cortex_notif_scheduled_ids";
const SEEN_KEY = "notif_moodle_seen"; // DB setting: JSON array of already-notified grade/announcement ids

let granted = false;
let prefs: Record<string, string> = {};

/** Stable positive 31-bit id from a string key (so we can cancel/replace by id). */
function hashId(key: string): number {
  let h = 2166136261;
  for (let i = 0; i < key.length; i++) { h ^= key.charCodeAt(i); h = Math.imul(h, 16777619); }
  return (h & 0x7fffffff) || 1;
}

function enabled(k: NotifKey): boolean {
  return prefs[`notif_${k}`] !== "false"; // default ON
}

function readReg(): number[] {
  try { return JSON.parse(localStorage.getItem(REG_KEY) || "[]"); } catch { return []; }
}
function writeReg(ids: number[]): void {
  try { localStorage.setItem(REG_KEY, JSON.stringify(ids)); } catch { /* quota */ }
}

/** Ask for notification permission (once) — call on launch. */
export async function initNotifications(): Promise<void> {
  try {
    prefs = await api.getAllSettings();
  } catch { prefs = {}; }
  try {
    granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
  } catch { granted = false; }
}

/** Re-read prefs (after the user toggles a setting). */
export async function reloadNotifPrefs(): Promise<void> {
  try { prefs = await api.getAllSettings(); } catch { /* keep */ }
}

/** Fire an immediate notification (mobile only — desktop uses in-app toasts). */
export function notifyNow(key: NotifKey, title: string, body: string): void {
  if (!isMobile || !enabled(key)) return;
  try { sendNotification({ title, body }); } catch { /* permission/plugin missing */ }
}

function fmtWhen(d: Date): string {
  return d.toLocaleString(undefined, { weekday: "short", hour: "numeric", minute: "2-digit" });
}

function scheduleAt(ids: number[], key: string, when: Date, title: string, body: string): void {
  if (when.getTime() <= Date.now() + 30_000) return; // skip past / too-soon
  const id = hashId(key);
  try {
    sendNotification({ id, title, body, schedule: Schedule.at(when) });
    ids.push(id);
  } catch { /* ignore */ }
}

/**
 * Cancel previously-scheduled Cortex notifications and re-schedule from current data. Cheap; call
 * on launch and after every Moodle sync / refresh.
 */
export async function rescheduleAll(): Promise<void> {
  if (!isMobile) return;
  if (!granted) await initNotifications();
  if (!granted) return;

  const prev = readReg();
  if (prev.length) { try { await cancel(prev); } catch { /* ignore */ } }
  try { await cancel([ID_FLASHCARDS, ID_DAILY_REVIEW]); } catch { /* ignore */ }

  const ids: number[] = [];

  // Deadlines + exams (from the Moodle-derived feed already in memory).
  for (const n of app.notifications ?? []) {
    const ts = n.ts;
    if (n.kind === "deadline" && enabled("deadline")) {
      scheduleAt(ids, `dl24-${n.id}`, new Date(ts - 24 * 3600e3), `⏰ Due tomorrow — ${n.title}`, `${n.course} · ${fmtWhen(new Date(ts))}`);
      scheduleAt(ids, `dl3-${n.id}`, new Date(ts - 3 * 3600e3), `⏰ Due in 3 hours — ${n.title}`, n.course);
    } else if (n.kind === "exam" && enabled("exam")) {
      scheduleAt(ids, `ex3d-${n.id}`, new Date(ts - 3 * 24 * 3600e3), `📝 Exam in 3 days — ${n.title}`, `${n.course} · ${fmtWhen(new Date(ts))}`);
      scheduleAt(ids, `ex1d-${n.id}`, new Date(ts - 24 * 3600e3), `📝 Exam tomorrow — ${n.title}`, n.course);
    }
  }

  // Daily flashcards-due reminder (08:00) — repeats while closed.
  if (enabled("flashcards")) {
    try {
      sendNotification({
        id: ID_FLASHCARDS, title: "🗂️ Flashcards due",
        body: "Cards are waiting for review — keep your memory sharp.",
        schedule: Schedule.interval({ hour: 8, minute: 0 }),
      });
    } catch { /* ignore */ }
  }
  // Daily review reminder (19:00).
  if (enabled("daily_review")) {
    try {
      sendNotification({
        id: ID_DAILY_REVIEW, title: "📚 Time to study",
        body: "Your daily Cortex review is ready.",
        schedule: Schedule.interval({ hour: 19, minute: 0 }),
      });
    } catch { /* ignore */ }
  }

  writeReg(ids);
}

// ── New grade / announcement detection (foreground; the BG task mirrors this in Rust) ──

function gradeId(g: api.MoodleGrade): string {
  return `grade:${g.course_id}:${g.item_name}:${g.percentage || g.grade}`;
}

/** Diff current Moodle data against the already-notified set; fire for anything new. */
export async function checkNewMoodle(): Promise<void> {
  if (!isMobile) return;
  if (!granted) return;
  const md = app.moodleData;
  if (!md) return;
  let seen: Set<string>;
  try {
    seen = new Set(JSON.parse((await api.getSetting(SEEN_KEY)) || "[]"));
  } catch { seen = new Set(); }

  const first = seen.size === 0; // don't spam on the very first run — seed silently
  const courseName = (cid: string) => md.courses.find((c) => c.id === cid)?.fullname
    ?? md.courses.find((c) => c.id === cid)?.shortname ?? "";

  const fresh: string[] = [];
  if (enabled("grade")) {
    for (const g of md.grades ?? []) {
      const id = gradeId(g);
      if (seen.has(id)) continue;
      fresh.push(id);
      if (!first) notifyNow("grade", `✅ Grade released — ${g.item_name}`, `${courseName(g.course_id)} · ${g.percentage || g.grade}`);
    }
  }
  if (enabled("announcement")) {
    for (const a of md.announcements ?? []) {
      const id = `ann:${a.id}`;
      if (seen.has(id)) continue;
      fresh.push(id);
      if (!first) notifyNow("announcement", `📣 ${a.subject}`, courseName(a.course_id));
    }
  }
  if (fresh.length) {
    for (const id of fresh) seen.add(id);
    // bound the stored set so it can't grow unbounded
    const arr = Array.from(seen).slice(-500);
    try { await api.setSetting(SEEN_KEY, JSON.stringify(arr)); } catch { /* ignore */ }
  }
}

// ── Long-recording safety reminder ──

export function scheduleLongRecording(): void {
  if (!isMobile || !enabled("long_recording")) return;
  try {
    sendNotification({
      id: ID_LONG_RECORDING, title: "🔴 Still recording",
      body: "Cortex has been recording for a while — stop it if you're done.",
      schedule: Schedule.at(new Date(Date.now() + 2 * 3600e3)),
    });
  } catch { /* ignore */ }
}
export async function cancelLongRecording(): Promise<void> {
  try { await cancel([ID_LONG_RECORDING]); } catch { /* ignore */ }
}
