-- Owner aliases are learned only from explicit user corrections.
CREATE TABLE IF NOT EXISTS memory_owner_aliases (
    alias_key TEXT PRIMARY KEY,
    alias TEXT NOT NULL,
    canonical_key TEXT NOT NULL,
    canonical_name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_memory_owner_aliases_canonical
    ON memory_owner_aliases(canonical_key);