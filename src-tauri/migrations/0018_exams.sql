-- Exam Mode: timed, locally-graded practice exams. An exam is an LLM-generated
-- mix of multiple-choice and written questions, scoped to a subject and (optionally)
-- a subset of its topics. `questions` holds the generated paper as JSON; `answers`
-- holds the student's submitted answers; `results` holds the local grading output
-- (per-question correctness/feedback + per-topic breakdown). `status` walks
-- ready → in_progress → graded as the exam is started and submitted.
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS exams (
  id            TEXT PRIMARY KEY,
  subject_id    TEXT NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
  topic_ids     TEXT,                         -- JSON array of topic ids the exam is scoped to (null = whole subject)
  title         TEXT NOT NULL,
  duration_min  INTEGER NOT NULL,             -- exam length in minutes (drives the countdown)
  questions     TEXT NOT NULL,                -- JSON array of generated questions
  answers       TEXT,                         -- JSON: student's submitted answers (null until submitted)
  results       TEXT,                         -- JSON: grading output (null until graded)
  status        TEXT NOT NULL DEFAULT 'ready',-- ready | in_progress | graded
  started_ms    INTEGER,                      -- ms epoch the exam was started (null until started)
  score         REAL,                         -- final percentage 0..100 (null until graded)
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);
-- The setup screen lists a subject's exams newest-first.
CREATE INDEX IF NOT EXISTS idx_exams_subject ON exams(subject_id, created_at);
