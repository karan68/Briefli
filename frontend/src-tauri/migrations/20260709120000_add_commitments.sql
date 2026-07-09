-- Cross-meeting commitments / action-item tracker.
-- Action items are extracted per meeting by the AI summary; this table
-- aggregates them across all meetings so they can be tracked (open/done) and,
-- in a later phase, attributed to people (owner) with due dates.
CREATE TABLE IF NOT EXISTS commitments (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    text TEXT NOT NULL,
    owner TEXT,
    due_date TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(meeting_id, text),
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_commitments_meeting ON commitments(meeting_id);
CREATE INDEX IF NOT EXISTS idx_commitments_status ON commitments(status);
