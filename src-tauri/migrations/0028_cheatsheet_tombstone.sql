-- Make cheatsheet regeneration propagate across sync.
--
-- Regenerating a cheatsheet DELETEs the old row and inserts a new one with a fresh id.
-- Without a tombstone the delete syncs as an orphan: the peer keeps BOTH the old and the
-- new cheatsheet for the same (subject, topic) and could load the stale one. Record the
-- delete so the peer drops the old row on its next pull. Synced like any other tombstone.
CREATE TRIGGER IF NOT EXISTS tomb_cheatsheets AFTER DELETE ON cheatsheets BEGIN
  INSERT OR REPLACE INTO tombstones
    VALUES ('cheatsheets', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
