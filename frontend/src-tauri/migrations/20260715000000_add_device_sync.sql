CREATE TABLE paired_devices (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    public_key_spki_base64 TEXT NOT NULL,
    last_sequence INTEGER NOT NULL DEFAULT 0 CHECK (last_sequence >= 0),
    paired_at TEXT NOT NULL,
    last_seen_at TEXT,
    revoked_at TEXT
);

CREATE TABLE mobile_captures (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    title TEXT NOT NULL,
    started_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL CHECK (duration_ms > 0),
    byte_length INTEGER NOT NULL CHECK (byte_length > 0),
    bytes_received INTEGER NOT NULL DEFAULT 0 CHECK (bytes_received >= 0),
    media_type TEXT NOT NULL,
    file_extension TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'receiving', 'received', 'importing', 'imported', 'failed')),
    inbox_path TEXT NOT NULL,
    meeting_id TEXT,
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (device_id) REFERENCES paired_devices(id),
    FOREIGN KEY (meeting_id) REFERENCES meetings(id),
    CHECK (bytes_received <= byte_length)
);

CREATE INDEX idx_mobile_captures_device_status
    ON mobile_captures(device_id, status, created_at DESC);

CREATE INDEX idx_mobile_captures_status
    ON mobile_captures(status, created_at ASC);