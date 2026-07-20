-- Migration: Add an FTS5 full-text index over transcript segments.
--
-- Why: cross-meeting transcript search previously loaded EVERY transcript
-- segment into memory on every query (a full table scan). This adds a SQLite
-- FTS5 virtual table so searches can be narrowed to the meetings that actually
-- contain the query terms before any Rust-side scoring runs. It is also the
-- retrieval foundation for meeting Q&A.
--
-- Design:
--   * `transcripts_fts` is a contentful (self-storing) FTS5 table. Its `rowid`
--     is kept equal to `transcripts.rowid` so the sync triggers can locate the
--     matching index row on UPDATE/DELETE. The app never runs VACUUM, so those
--     rowids remain stable.
--   * `meeting_id` is stored UNINDEXED purely so a match can report which
--     meeting it belongs to without a join back to `transcripts`.
--   * unicode61 tokenizer with diacritic folding gives case- and accent-
--     insensitive matching that mirrors the previous lowercase comparison.

CREATE VIRTUAL TABLE IF NOT EXISTS transcripts_fts USING fts5(
    transcript,
    meeting_id UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- Backfill the index from any transcripts that already exist.
INSERT INTO transcripts_fts (rowid, transcript, meeting_id)
SELECT rowid, transcript, meeting_id FROM transcripts;

-- Keep the index in sync with the transcripts table.
CREATE TRIGGER IF NOT EXISTS transcripts_fts_ai AFTER INSERT ON transcripts BEGIN
    INSERT INTO transcripts_fts (rowid, transcript, meeting_id)
    VALUES (new.rowid, new.transcript, new.meeting_id);
END;

CREATE TRIGGER IF NOT EXISTS transcripts_fts_ad AFTER DELETE ON transcripts BEGIN
    DELETE FROM transcripts_fts WHERE rowid = old.rowid;
END;

CREATE TRIGGER IF NOT EXISTS transcripts_fts_au AFTER UPDATE ON transcripts BEGIN
    DELETE FROM transcripts_fts WHERE rowid = old.rowid;
    INSERT INTO transcripts_fts (rowid, transcript, meeting_id)
    VALUES (new.rowid, new.transcript, new.meeting_id);
END;
