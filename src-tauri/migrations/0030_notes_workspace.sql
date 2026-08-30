-- Durable note metadata and normalized derived indexes.
-- Existing notes remain valid through conservative defaults.
PRAGMA foreign_keys = ON;

ALTER TABLE notes ADD COLUMN slug TEXT NOT NULL DEFAULT '';
ALTER TABLE notes ADD COLUMN folder TEXT NOT NULL DEFAULT '';
ALTER TABLE notes ADD COLUMN properties TEXT NOT NULL DEFAULT '{}';

-- Backfill a stable key for rows created before this migration.
UPDATE notes SET slug = id WHERE slug = '';
CREATE UNIQUE INDEX IF NOT EXISTS idx_notes_slug ON notes(slug);
CREATE INDEX IF NOT EXISTS idx_notes_folder ON notes(folder);

CREATE TABLE IF NOT EXISTS note_links (
  source_note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  target_note_id TEXT REFERENCES notes(id) ON DELETE SET NULL,
  target_key TEXT NOT NULL,
  display_text TEXT NOT NULL DEFAULT '',
  position INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (source_note_id, position),
  UNIQUE (source_note_id, target_note_id, position)
);
CREATE INDEX IF NOT EXISTS idx_note_links_target ON note_links(target_note_id);
CREATE INDEX IF NOT EXISTS idx_note_links_source ON note_links(source_note_id);

CREATE TABLE IF NOT EXISTS note_tags (
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  tag TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (note_id, tag)
);
CREATE INDEX IF NOT EXISTS idx_note_tags_tag ON note_tags(tag);
