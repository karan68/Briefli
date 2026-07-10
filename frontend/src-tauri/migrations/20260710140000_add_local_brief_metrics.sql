-- Optional local-only usage counters for evaluating the Prepare workflow.
-- Disabled by default. No event is written until the user explicitly opts in.
CREATE TABLE IF NOT EXISTS brief_usage_preferences (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    updated_at TEXT NOT NULL
);

INSERT OR IGNORE INTO brief_usage_preferences (id, enabled, updated_at)
VALUES (1, 0, CURRENT_TIMESTAMP);

CREATE TABLE IF NOT EXISTS brief_usage_events (
    id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL CHECK (event_type IN ('brief_opened', 'source_opened')),
    space_id TEXT NOT NULL,
    memory_id TEXT,
    occurred_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_brief_usage_events_type_time
    ON brief_usage_events(event_type, occurred_at DESC);
