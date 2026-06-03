-- Add an optional per-topic emoji glyph, used by the UI to tint topic rows
-- (mirrors the per-subject glyph). NULL = no custom glyph.
ALTER TABLE topics ADD COLUMN glyph TEXT;
