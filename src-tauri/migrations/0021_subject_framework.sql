-- Per-subject module framework: the official course/module outline document
-- (weights, assessment breakdown, pass requirements). Stored as extracted text
-- so chat can reference it on demand — but ONLY when the user explicitly asks
-- about marks/weighting (gated in chat_answer), never injected into normal RAG.
-- One framework per subject; id + updated_at so the homelab merge upserts it.
CREATE TABLE IF NOT EXISTS subject_frameworks (
  subject_id  TEXT PRIMARY KEY,
  filename    TEXT,
  text        TEXT,
  updated_at  INTEGER NOT NULL
);
