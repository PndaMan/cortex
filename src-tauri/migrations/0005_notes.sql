-- Notes — lightweight free-text notes that can later be "converted" into a
-- first-class source (chunked + embedded). source_id is set once converted so
-- the note row links to the generated source.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS notes (
  id          TEXT PRIMARY KEY,
  subject_id  TEXT REFERENCES subjects(id) ON DELETE SET NULL,
  topic_id    TEXT REFERENCES topics(id) ON DELETE SET NULL,
  title       TEXT NOT NULL DEFAULT '',
  body        TEXT NOT NULL DEFAULT '',
  source_id   TEXT REFERENCES sources(id) ON DELETE SET NULL,  -- set when converted to a source
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_notes_subject ON notes(subject_id);
