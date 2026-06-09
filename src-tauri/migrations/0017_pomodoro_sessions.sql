-- Persisted Pomodoro focus sessions. The timer is otherwise frontend-only
-- (src/components/PomodoroPanel.svelte + store `pomo`); this table records each
-- FINISHED work/break segment so the analytics dashboard can chart study
-- minutes per day and per subject. `subject_id` is the subject that was active
-- when the segment completed (NULL if none was open). Only "work" rows count as
-- study time; "break" rows are logged for completeness.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS pomodoro_sessions (
  id          TEXT PRIMARY KEY,
  subject_id  TEXT,                       -- active subject when the segment ended (nullable)
  kind        TEXT NOT NULL,              -- work | break
  started_ms  INTEGER NOT NULL,           -- ms epoch the segment began
  ended_ms    INTEGER NOT NULL,           -- ms epoch the segment finished
  created_at  INTEGER NOT NULL
);
-- Analytics queries scan by start time (per-day buckets, last-N-days windows).
CREATE INDEX IF NOT EXISTS idx_pomo_started ON pomodoro_sessions(started_ms);
