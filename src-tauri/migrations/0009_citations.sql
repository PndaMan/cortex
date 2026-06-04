-- Citation/reference manager, scoped per subject. Formatting (APA/MLA) is done
-- in the frontend from these fields. Deadlines reuse the existing `events` table
-- via a new `kind = 'deadline'`, so no schema change is needed for those.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS citations (
  id          TEXT PRIMARY KEY,
  subject_id  TEXT NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
  ctype       TEXT NOT NULL DEFAULT 'article',  -- article | book | web | other
  title       TEXT NOT NULL,
  authors     TEXT,                             -- "Last, F.; Last, F." free text
  year        TEXT,                             -- "2024" / "n.d." (text: ranges/seasons)
  container   TEXT,                             -- journal / publisher / website
  url         TEXT,
  doi         TEXT,
  notes       TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_citations_subject ON citations(subject_id, created_at);
