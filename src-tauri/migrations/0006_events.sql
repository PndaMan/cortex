-- Calendar events + tasks. Reminders are absolute epoch-ms timestamps the
-- frontend polls for (check_reminders). google_id + upsert-by-google-id are
-- created now so a later Google Calendar sync slice only adds rows.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS events (
  id           TEXT PRIMARY KEY,
  subject_id   TEXT REFERENCES subjects(id) ON DELETE SET NULL,
  title        TEXT NOT NULL,
  description  TEXT,
  location     TEXT,
  color        TEXT,
  start_ms     INTEGER NOT NULL,
  end_ms       INTEGER,
  all_day      INTEGER NOT NULL DEFAULT 0,
  kind         TEXT NOT NULL DEFAULT 'event',   -- event | task
  done         INTEGER NOT NULL DEFAULT 0,
  reminder_ms  INTEGER,                          -- absolute epoch ms to notify at
  notified     INTEGER NOT NULL DEFAULT 0,
  google_id    TEXT,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_start ON events(start_ms);
