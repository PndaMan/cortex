-- Tags link topics and deadlines for exam prep: tag topics (e.g. "A2"), tag a
-- deadline with the same tag, then chat by tag and show a per-topic study
-- checklist. Tags are stored as a ';'-separated text list. `checklist` on events
-- is a JSON array of topic ids the user has ticked off for that deadline.
ALTER TABLE topics ADD COLUMN tags TEXT;
ALTER TABLE events ADD COLUMN tags TEXT;
ALTER TABLE events ADD COLUMN checklist TEXT;
