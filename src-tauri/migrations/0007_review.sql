-- Spaced-review attempt log. Each row records one answered quiz/flashcard item;
-- the "wrong ones to re-study" query looks at the LATEST attempt per item_key.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS attempts (
  id           TEXT PRIMARY KEY,
  subject_id   TEXT NOT NULL,
  material_id  TEXT,
  kind         TEXT NOT NULL,                    -- quiz | flashcard
  item_index   INTEGER NOT NULL,
  item_key     TEXT NOT NULL,                    -- the question text or flashcard front
  correct      INTEGER NOT NULL,
  created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_attempts_subject_kind ON attempts(subject_id, kind);
