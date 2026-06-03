-- Cortex v0.2 — source preview support + long-term user memory.
-- Adds a stable on-disk path for the ORIGINAL bytes of file-based sources
-- (so the frontend can render PDFs/images via convertFileSrc) and a manual
-- long-term memory table whose rows are injected into chat/synthesis prompts.

ALTER TABLE sources ADD COLUMN stored_path TEXT;

CREATE TABLE IF NOT EXISTS user_memory (
  id          TEXT PRIMARY KEY,
  content     TEXT NOT NULL,
  source      TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
