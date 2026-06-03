-- Add an optional per-subject accent color (hex string), used by the UI to
-- tint subject cards / glyphs. NULL = use the default theme accent.
ALTER TABLE subjects ADD COLUMN color TEXT;
