-- Deep link to each announcement's Moodle forum discussion, so the UI can open
-- the full post (with attachments/links) in the browser. Populated on next sync.
ALTER TABLE moodle_announcements ADD COLUMN url TEXT;
