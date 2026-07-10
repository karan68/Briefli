-- User-managed recurring contexts for deterministic pre-conversation briefs.
CREATE TABLE IF NOT EXISTS memory_spaces (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meeting_space_assignments (
    meeting_id TEXT PRIMARY KEY,
    space_id TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
    FOREIGN KEY (space_id) REFERENCES memory_spaces(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_meeting_space_assignments_space
    ON meeting_space_assignments(space_id, assigned_at DESC);
