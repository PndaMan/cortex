-- Tombstones for smart multi-device sync.
--
-- The homelab sync merges row-by-row (union by id, newest updated_at wins) so
-- nothing is ever deleted "needlessly" just because the other device hadn't
-- seen a row yet. But that union would also resurrect rows the user genuinely
-- deleted. Tombstones record intentional deletes so they propagate: a delete
-- wins over a row only when the row hasn't been edited *after* the delete.
--
-- Triggers (not app code) write tombstones, so deletes are captured no matter
-- which code path runs them — including ON DELETE CASCADE children. The
-- delete is stamped with epoch-ms (julianday math == db::now_ms()).

CREATE TABLE IF NOT EXISTS tombstones (
  entity_table TEXT NOT NULL,
  entity_id    TEXT NOT NULL,
  deleted_at   INTEGER NOT NULL,
  PRIMARY KEY (entity_table, entity_id)
);

-- epoch-ms expression reused by every trigger:
--   CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)
-- 2440587.5 = Julian day number of the Unix epoch.

CREATE TRIGGER IF NOT EXISTS tomb_subjects AFTER DELETE ON subjects BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('subjects', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_topics AFTER DELETE ON topics BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('topics', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_sources AFTER DELETE ON sources BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('sources', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_notes AFTER DELETE ON notes BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('notes', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_events AFTER DELETE ON events BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('events', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_citations AFTER DELETE ON citations BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('citations', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_materials AFTER DELETE ON materials BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('materials', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_exams AFTER DELETE ON exams BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('exams', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_user_memory AFTER DELETE ON user_memory BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('user_memory', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_custom_stations AFTER DELETE ON custom_stations BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('custom_stations', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_chat_threads AFTER DELETE ON chat_threads BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('chat_threads', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
CREATE TRIGGER IF NOT EXISTS tomb_srs_cards AFTER DELETE ON srs_cards BEGIN
  INSERT OR REPLACE INTO tombstones VALUES ('srs_cards', OLD.id, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER));
END;
