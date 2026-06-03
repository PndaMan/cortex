-- Cortex v0.1 schema — foundation + ingestion milestone.
-- Local SQLite is the source of truth (decision: locked). Forward-compatible tables
-- for cheatsheets / chat / materials are created now (cheap) so later slices only add rows.
-- Account namespacing for v0.3: every top-level row carries owner_id (default 'local').

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS subjects (
  id           TEXT PRIMARY KEY,
  owner_id     TEXT NOT NULL DEFAULT 'local',
  name         TEXT NOT NULL,
  code         TEXT,
  glyph        TEXT NOT NULL DEFAULT '◆',
  status       TEXT NOT NULL DEFAULT 'ready',   -- ready | review
  streak       INTEGER NOT NULL DEFAULT 0,
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS topics (
  id           TEXT PRIMARY KEY,
  subject_id   TEXT NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
  name         TEXT NOT NULL,
  position     INTEGER NOT NULL DEFAULT 0,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_topics_subject ON topics(subject_id);

CREATE TABLE IF NOT EXISTS sources (
  id           TEXT PRIMARY KEY,
  subject_id   TEXT NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
  topic_id     TEXT REFERENCES topics(id) ON DELETE SET NULL,
  name         TEXT NOT NULL,
  kind         TEXT NOT NULL,                   -- pdf|docx|pptx|txt|md|web|yt|audio|image
  status       TEXT NOT NULL DEFAULT 'pending', -- pending | ingesting | ready | draft | error
  meta         TEXT,                            -- human label e.g. "18 pages"
  origin       TEXT,                            -- path or URL the source came from
  content      TEXT,                            -- extracted plaintext
  error        TEXT,
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sources_subject ON sources(subject_id);
CREATE INDEX IF NOT EXISTS idx_sources_topic ON sources(topic_id);

CREATE TABLE IF NOT EXISTS chunks (
  id           TEXT PRIMARY KEY,
  source_id    TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  subject_id   TEXT NOT NULL,
  topic_id     TEXT,
  ord          INTEGER NOT NULL,                -- chunk order within source
  text         TEXT NOT NULL,
  loc          TEXT,                            -- citation locator e.g. "p.14" / "12:30"
  dim          INTEGER NOT NULL,
  embedding    BLOB NOT NULL,                   -- little-endian f32[dim]; sqlite-vec drop-in later
  created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source_id);
CREATE INDEX IF NOT EXISTS idx_chunks_subject ON chunks(subject_id);

CREATE TABLE IF NOT EXISTS tags (
  id    TEXT PRIMARY KEY,
  name  TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS source_tags (
  source_id TEXT NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  tag_id    TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
  PRIMARY KEY (source_id, tag_id)
);

CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- Forward-compat (created now, populated by later slices) ------------------

CREATE TABLE IF NOT EXISTS cheatsheets (
  id          TEXT PRIMARY KEY,
  subject_id  TEXT NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
  topic_id    TEXT REFERENCES topics(id) ON DELETE CASCADE,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS cheatsheet_sections (
  id            TEXT PRIMARY KEY,
  cheatsheet_id TEXT NOT NULL REFERENCES cheatsheets(id) ON DELETE CASCADE,
  title         TEXT NOT NULL,
  state         TEXT NOT NULL DEFAULT 'idle',  -- idle | draft-pending | approved
  ord           INTEGER NOT NULL DEFAULT 0,
  body          TEXT,                          -- json items
  source_ids    TEXT                           -- json array, for delta updates
);

CREATE TABLE IF NOT EXISTS chat_threads (
  id          TEXT PRIMARY KEY,
  subject_id  TEXT REFERENCES subjects(id) ON DELETE CASCADE,
  topic_id    TEXT REFERENCES topics(id) ON DELETE SET NULL,
  source_id   TEXT REFERENCES sources(id) ON DELETE SET NULL,
  scope       TEXT NOT NULL,                   -- subject | topic | source
  title       TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chat_messages (
  id         TEXT PRIMARY KEY,
  thread_id  TEXT NOT NULL REFERENCES chat_threads(id) ON DELETE CASCADE,
  role       TEXT NOT NULL,                    -- user | assistant | system
  text       TEXT NOT NULL,
  citations  TEXT,                             -- json
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS materials (
  id          TEXT PRIMARY KEY,
  subject_id  TEXT NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
  topic_id    TEXT REFERENCES topics(id) ON DELETE SET NULL,
  kind        TEXT NOT NULL,                   -- flashcards | quiz | audio | slideshow | infographic
  title       TEXT NOT NULL,
  meta        TEXT,
  status      TEXT NOT NULL DEFAULT 'ready',
  payload     TEXT,                            -- json
  created_at  INTEGER NOT NULL
);
