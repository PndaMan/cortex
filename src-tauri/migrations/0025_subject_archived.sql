-- Archived subjects: hidden from all normal views, retained for data storage.
-- 0 = active (default), 1 = archived.
ALTER TABLE subjects ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
