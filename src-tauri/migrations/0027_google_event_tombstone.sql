-- Make calendar deletions stick across Google syncs.
--
-- Deleting a calendar event writes an ('events', id) tombstone, but a Google
-- re-pull matches by google_id and inserts a brand-new row (new id), so the
-- event resurrects. Record the deleted google_id too; the pull skips any
-- google_id with a tombstone here. Synced like any other tombstone, so the
-- delete propagates to other devices' pulls as well.
CREATE TRIGGER IF NOT EXISTS tomb_events_google AFTER DELETE ON events
WHEN OLD.google_id IS NOT NULL BEGIN
  INSERT OR REPLACE INTO tombstones
    VALUES ('google_event', OLD.google_id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
