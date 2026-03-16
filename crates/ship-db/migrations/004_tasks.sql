CREATE TABLE IF NOT EXISTS tasks (
    id TEXT PRIMARY KEY NOT NULL,
    -- Which room (lane) this task belongs to
    room_id TEXT NOT NULL REFERENCES rooms(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    -- TaskPhase as text: assigned, working, pending_review, rebase_conflict, accepted, cancelled
    phase TEXT NOT NULL DEFAULT 'assigned',
    created_at TEXT NOT NULL,
    -- Set when the task reaches a terminal phase
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_tasks_room ON tasks(room_id);
CREATE INDEX IF NOT EXISTS idx_tasks_phase ON tasks(phase);

-- Track which task is currently active for each room (at most one).
-- NULL means the lane is idle.
ALTER TABLE rooms ADD COLUMN current_task_id TEXT REFERENCES tasks(id);
