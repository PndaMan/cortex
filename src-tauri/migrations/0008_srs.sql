-- SM-2 spaced-repetition scheduling state, one row per distinct study item
-- (keyed by subject + kind + item_key, the question/front text). The `attempts`
-- log (0007) still records every answer for the "review missed" set; this table
-- adds the SM-2 schedule (ease, interval, repetitions, due date) on top so cards
-- resurface on their due date instead of only when last answered wrong.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS srs_cards (
  id          TEXT PRIMARY KEY,
  subject_id  TEXT NOT NULL,
  material_id TEXT,
  kind        TEXT NOT NULL,                 -- quiz | flashcard
  item_index  INTEGER NOT NULL,
  item_key    TEXT NOT NULL,                 -- question text / flashcard front (stable id)
  ease        REAL NOT NULL DEFAULT 2.5,     -- SM-2 ease factor (>= 1.3)
  interval_d  INTEGER NOT NULL DEFAULT 0,    -- current inter-repetition interval, days
  reps        INTEGER NOT NULL DEFAULT 0,    -- consecutive successful repetitions
  lapses      INTEGER NOT NULL DEFAULT 0,    -- times graded "again"
  last_grade  INTEGER,                       -- last quality 0-5
  due_at      INTEGER NOT NULL,              -- ms epoch when next due
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL,
  UNIQUE(subject_id, kind, item_key)
);
CREATE INDEX IF NOT EXISTS idx_srs_due ON srs_cards(subject_id, kind, due_at);
