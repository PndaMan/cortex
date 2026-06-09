-- Assignments previously overloaded two event fields: priority was encoded as a
-- colour hex in `color` (lossy round-trip; broke if the user picked a custom
-- colour) and covered-topic ids squatted in the shared `tags` field. Give both
-- real columns and backfill existing assignment rows.
ALTER TABLE events ADD COLUMN priority TEXT;
ALTER TABLE events ADD COLUMN topic_ids TEXT;

UPDATE events SET priority = CASE color
    WHEN '#3b9eff' THEN 'low'
    WHEN '#f5a623' THEN 'med'
    WHEN '#e5484d' THEN 'high'
    ELSE NULL END
  WHERE kind IN ('exam', 'assignment', 'project', 'deadline');

UPDATE events SET topic_ids = tags, tags = NULL
  WHERE kind IN ('exam', 'assignment', 'project', 'deadline')
    AND tags IS NOT NULL AND tags <> '';
