-- Tracks when a confirmed open loop was last reviewed by the user.
ALTER TABLE meeting_memories ADD COLUMN follow_up_reviewed_at TEXT;

CREATE INDEX IF NOT EXISTS idx_meeting_memories_follow_up_review
    ON meeting_memories(kind, resolution_status, follow_up_reviewed_at);
