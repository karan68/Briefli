-- Durable, evidence-backed memory extracted from meetings.
-- Existing commitments remain in place while callers migrate to this ledger.
CREATE TABLE IF NOT EXISTS meeting_memories (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('decision', 'commitment', 'open_question')),
    text TEXT NOT NULL,
    suggested_text TEXT NOT NULL,
    source_fingerprint TEXT NOT NULL,
    owner TEXT,
    due_date TEXT,
    review_status TEXT NOT NULL DEFAULT 'suggested'
        CHECK (review_status IN ('suggested', 'confirmed', 'corrected', 'rejected')),
    resolution_status TEXT NOT NULL DEFAULT 'open'
        CHECK (resolution_status IN ('open', 'done')),
    evidence_confidence REAL CHECK (
        evidence_confidence IS NULL OR
        (evidence_confidence >= 0.0 AND evidence_confidence <= 1.0)
    ),
    source_transcript_id TEXT,
    source_excerpt TEXT,
    source_timestamp TEXT,
    source_audio_start_time REAL,
    source_audio_end_time REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    reviewed_at TEXT,
    UNIQUE(meeting_id, kind, source_fingerprint),
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE,
    FOREIGN KEY (source_transcript_id) REFERENCES transcripts(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_meeting_memories_meeting
    ON meeting_memories(meeting_id);
CREATE INDEX IF NOT EXISTS idx_meeting_memories_review
    ON meeting_memories(review_status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_meeting_memories_kind_resolution
    ON meeting_memories(kind, resolution_status, updated_at DESC);