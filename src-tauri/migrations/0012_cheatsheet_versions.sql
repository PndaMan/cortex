-- Version history for cheatsheets: every save (generation OR manual edit)
-- snapshots the full section set as JSON so the editor can show a git-like diff
-- of previous changes. Pruned to the most recent ~20 versions per scope in code.
CREATE TABLE IF NOT EXISTS cheatsheet_versions (
    id         TEXT PRIMARY KEY,
    subject_id TEXT NOT NULL,
    topic_id   TEXT,                 -- NULL = whole-subject sheet
    created_at INTEGER NOT NULL,
    note       TEXT NOT NULL DEFAULT '',  -- "generated" | "edited"
    sections   TEXT NOT NULL              -- JSON array of CsSection
);
CREATE INDEX IF NOT EXISTS idx_cs_versions_scope
    ON cheatsheet_versions (subject_id, IFNULL(topic_id, ''), created_at DESC);
