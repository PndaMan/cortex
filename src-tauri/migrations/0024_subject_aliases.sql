-- Comma-separated keywords that map calendar/timetable event titles to this
-- subject — deterministic, no AI. e.g. "GenLing, General Linguistics, GL178".
-- Matched case- and punctuation-insensitively against the event title.
ALTER TABLE subjects ADD COLUMN calendar_aliases TEXT;
