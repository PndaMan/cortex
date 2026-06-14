-- Kanban board status for assignments/tasks: 'todo' | 'doing' | 'done'.
-- NULL means "not yet placed" — derived from the existing `done` flag at read
-- time (done=1 → done, else todo) so old rows slot in without a backfill.
ALTER TABLE events ADD COLUMN status TEXT;
