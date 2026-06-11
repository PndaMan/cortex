-- Moodle integration (experimental). Cortex pulls a student's data from a
-- Moodle uni portal via the Web Services REST API and caches it here. These
-- tables are read-only mirrors of remote data: re-synced, not user-edited.
-- They carry id + updated_at so the homelab merge upserts them harmlessly.

-- Link a Cortex subject to a Moodle course (the Moodle course id, as text).
ALTER TABLE subjects ADD COLUMN moodle_course_id TEXT;

CREATE TABLE IF NOT EXISTS moodle_courses (
  id          TEXT PRIMARY KEY,            -- Moodle course id
  shortname   TEXT,
  fullname    TEXT,
  updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS moodle_grades (
  id          TEXT PRIMARY KEY,            -- "<courseid>:<itemid>"
  course_id   TEXT NOT NULL,
  item_name   TEXT,
  grade       TEXT,                        -- formatted grade
  percentage  TEXT,
  feedback    TEXT,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_moodle_grades_course ON moodle_grades(course_id);

CREATE TABLE IF NOT EXISTS moodle_deadlines (
  id          TEXT PRIMARY KEY,            -- "assign:<id>" / "event:<id>"
  course_id   TEXT,
  name        TEXT NOT NULL,
  due_at      INTEGER,                     -- epoch seconds (Moodle native)
  kind        TEXT,                        -- assignment | event | exam
  status      TEXT,                        -- submitted | due | graded
  url         TEXT,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_moodle_deadlines_course ON moodle_deadlines(course_id);

CREATE TABLE IF NOT EXISTS moodle_announcements (
  id          TEXT PRIMARY KEY,            -- discussion id
  course_id   TEXT,
  subject     TEXT,
  message     TEXT,
  posted_at   INTEGER,                     -- epoch seconds
  updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_moodle_ann_course ON moodle_announcements(course_id);
