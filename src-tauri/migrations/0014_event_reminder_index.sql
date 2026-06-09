-- The 60s reminder poll (due_reminders) filtered on reminder_ms + notified with
-- no index, doing a full events scan every minute.
CREATE INDEX IF NOT EXISTS idx_events_reminder_notified
  ON events(reminder_ms, notified);
