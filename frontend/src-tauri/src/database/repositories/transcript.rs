use crate::api::{TranscriptSearchResult, TranscriptSegment};
use chrono::Utc;
use sqlx::{Connection, Error as SqlxError, SqlitePool};
use tracing::{error, info};
use uuid::Uuid;

pub struct TranscriptsRepository;

impl TranscriptsRepository {
    /// Saves a new meeting and its associated transcript segments.
    /// This function uses a transaction to ensure that either both the meeting
    /// and all its transcripts are saved, or none of them are.
    pub async fn save_transcript(
        pool: &SqlitePool,
        meeting_title: &str,
        transcripts: &[TranscriptSegment],
        folder_path: Option<String>,
    ) -> Result<String, SqlxError> {
        let meeting_id = format!("meeting-{}", Uuid::new_v4());

        let mut conn = pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        let now = Utc::now();

        // 1. Create the new meeting
        let result = sqlx::query(
            "INSERT INTO meetings (id, title, created_at, updated_at, folder_path) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&meeting_id)
        .bind(meeting_title)
        .bind(now)
        .bind(now)
        .bind(&folder_path)
        .execute(&mut *transaction)
        .await;

        if let Err(e) = result {
            error!("Failed to create meeting '{}': {}", meeting_title, e);
            transaction.rollback().await?;
            return Err(e);
        }

        info!("Successfully created meeting with id: {}", meeting_id);

        // 2. Save each transcript segment with audio timing fields
        for segment in transcripts {
            let transcript_id = format!("transcript-{}", Uuid::new_v4());
            let result = sqlx::query(
                "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time, duration)
                 VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&transcript_id)
            .bind(&meeting_id)
            .bind(&segment.text)
            .bind(&segment.timestamp)
            .bind(segment.audio_start_time)
            .bind(segment.audio_end_time)
            .bind(segment.duration)
            .execute(&mut *transaction)
            .await;

            if let Err(e) = result {
                error!(
                    "Failed to save transcript segment for meeting {}: {}",
                    meeting_id, e
                );
                transaction.rollback().await?;
                return Err(e);
            }
        }

        info!(
            "Successfully saved {} transcript segments for meeting {}",
            transcripts.len(),
            meeting_id
        );

        // Commit the transaction
        transaction.commit().await?;

        Ok(meeting_id)
    }

    /// Cross-meeting transcript search.
    ///
    /// Returns one result per meeting whose transcript (all segments combined)
    /// contains EVERY whitespace-separated term in `query`, ranked by total term
    /// occurrences so the most relevant meeting comes first — a search like
    /// "sarah report" surfaces meetings that mention both words even in different
    /// parts of the conversation.
    ///
    /// Candidate meetings are found through the `transcripts_fts` FTS5 index
    /// instead of scanning every stored segment, then the exact scoring and
    /// snippet selection runs only over those candidates. When a query has no
    /// FTS-tokenizable terms (e.g. it is only punctuation) it falls back to
    /// scanning all meetings, so behaviour is never silently lost.
    ///
    /// Matching is token/prefix based ("report" matches "reporting" but not the
    /// middle of "teleport"): an intentional precision improvement over raw
    /// substring matching that also keeps the index fast.
    pub async fn search_transcripts(
        pool: &SqlitePool,
        query: &str,
    ) -> Result<Vec<TranscriptSearchResult>, SqlxError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        // Lowercased, whitespace-separated search terms.
        let terms: Vec<String> = trimmed
            .to_lowercase()
            .split_whitespace()
            .map(|t| t.to_string())
            .collect();
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        // Narrow to the meetings that contain every term using the FTS5 index.
        // `None` means the query had no FTS-tokenizable terms, so we scan all
        // meetings to preserve the previous behaviour for degenerate queries.
        let candidate_meeting_ids = Self::candidate_meeting_ids(pool, &terms).await?;
        if let Some(ids) = &candidate_meeting_ids {
            if ids.is_empty() {
                return Ok(Vec::new());
            }
        }

        // Transcript segments (only for candidate meetings when available),
        // ordered so segments of the same meeting are grouped chronologically.
        let rows = Self::fetch_segments(pool, candidate_meeting_ids.as_deref()).await?;

        // Group segments by meeting, preserving first-seen order.
        let mut order: Vec<String> = Vec::new();
        let mut groups: std::collections::HashMap<String, (String, Vec<(String, String, String)>)> =
            std::collections::HashMap::new();
        for (meeting_id, title, seg_id, text, ts) in rows {
            let entry = groups.entry(meeting_id.clone()).or_insert_with(|| {
                order.push(meeting_id.clone());
                (title, Vec::new())
            });
            entry.1.push((seg_id, text, ts));
        }

        let mut scored: Vec<(usize, TranscriptSearchResult)> = Vec::new();

        for meeting_id in order {
            let (title, segments) = match groups.remove(&meeting_id) {
                Some(value) => value,
                None => continue,
            };

            // Whole-meeting text to require ALL terms and compute relevance.
            let full_lower = segments
                .iter()
                .map(|(_, text, _)| text.to_lowercase())
                .collect::<Vec<_>>()
                .join(" ");

            if !terms.iter().all(|term| full_lower.contains(term.as_str())) {
                continue;
            }

            // Relevance = total number of term occurrences across the meeting.
            let score: usize = terms
                .iter()
                .map(|term| full_lower.matches(term.as_str()).count())
                .sum();

            // Best segment = the one containing the most distinct terms (ties
            // resolved by earliest), so the snippet and jump target point at the
            // most relevant moment.
            let mut best_index = 0usize;
            let mut best_hits = 0usize;
            for (i, (_, text, _)) in segments.iter().enumerate() {
                let text_lower = text.to_lowercase();
                let hits = terms
                    .iter()
                    .filter(|term| text_lower.contains(term.as_str()))
                    .count();
                if hits > best_hits {
                    best_hits = hits;
                    best_index = i;
                }
            }

            let (segment_id, best_text, timestamp) = segments[best_index].clone();

            scored.push((
                score,
                TranscriptSearchResult {
                    id: meeting_id,
                    title,
                    segment_id,
                    match_context: Self::get_match_context(&best_text, &terms),
                    timestamp,
                },
            ));
        }

        // Highest relevance first.
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        Ok(scored.into_iter().map(|(_, result)| result).collect())
    }

    /// Use the FTS5 index to find the meetings that contain EVERY term.
    ///
    /// Returns `Some(ids)` (possibly empty) when at least one term was
    /// FTS-tokenizable, or `None` when none were — in which case the caller
    /// scans all meetings so degenerate (e.g. punctuation-only) queries still
    /// behave as before. Non-tokenizable terms in an otherwise usable query are
    /// skipped here and left to the in-memory `contains` filter, which never
    /// widens the result set.
    async fn candidate_meeting_ids(
        pool: &SqlitePool,
        terms: &[String],
    ) -> Result<Option<Vec<String>>, SqlxError> {
        use std::collections::HashSet;

        let mut intersection: Option<HashSet<String>> = None;
        let mut used_fts = false;

        for term in terms {
            let match_expr = match Self::fts_prefix_query(term) {
                Some(expr) => expr,
                None => continue,
            };
            used_fts = true;

            let rows = sqlx::query_as::<_, (String,)>(
                "SELECT DISTINCT meeting_id FROM transcripts_fts WHERE transcripts_fts MATCH ?",
            )
            .bind(&match_expr)
            .fetch_all(pool)
            .await?;

            let matches: HashSet<String> = rows.into_iter().map(|(id,)| id).collect();

            intersection = Some(match intersection.take() {
                Some(existing) => existing.intersection(&matches).cloned().collect(),
                None => matches,
            });

            // Once the running intersection is empty it can never grow again.
            if intersection.as_ref().is_some_and(|s| s.is_empty()) {
                return Ok(Some(Vec::new()));
            }
        }

        if !used_fts {
            return Ok(None);
        }

        Ok(Some(intersection.unwrap_or_default().into_iter().collect()))
    }

    /// Load transcript segments joined to their meeting, optionally restricted to
    /// a set of meeting ids. Returned as `(meeting_id, title, segment_id, text,
    /// timestamp)`, grouped by meeting and in chronological order.
    async fn fetch_segments(
        pool: &SqlitePool,
        meeting_ids: Option<&[String]>,
    ) -> Result<Vec<(String, String, String, String, String)>, SqlxError> {
        match meeting_ids {
            // An empty slice is handled by the caller (it returns early), so a
            // non-empty `Some` is assumed here.
            Some(ids) => {
                let placeholders = std::iter::repeat("?")
                    .take(ids.len())
                    .collect::<Vec<_>>()
                    .join(", ");
                let sql = format!(
                    "SELECT m.id, m.title, t.id, t.transcript, t.timestamp
                     FROM meetings m
                     JOIN transcripts t ON m.id = t.meeting_id
                     WHERE m.id IN ({})
                     ORDER BY m.id, t.timestamp",
                    placeholders
                );
                let mut query = sqlx::query_as::<_, (String, String, String, String, String)>(&sql);
                for id in ids {
                    query = query.bind(id);
                }
                query.fetch_all(pool).await
            }
            None => {
                sqlx::query_as::<_, (String, String, String, String, String)>(
                    "SELECT m.id, m.title, t.id, t.transcript, t.timestamp
                     FROM meetings m
                     JOIN transcripts t ON m.id = t.meeting_id
                     ORDER BY m.id, t.timestamp",
                )
                .fetch_all(pool)
                .await
            }
        }
    }

    /// Build a safe FTS5 MATCH expression for a single user term: a quoted
    /// prefix query such as `"report"*`. Returns `None` when the term has no
    /// character the tokenizer can index (so it could never match on its own and
    /// would otherwise produce an FTS syntax error).
    fn fts_prefix_query(term: &str) -> Option<String> {
        if !term.chars().any(|c| c.is_alphanumeric()) {
            return None;
        }
        // Double embedded quotes and wrap in quotes so arbitrary punctuation is
        // safe inside the MATCH expression; the trailing `*` makes it a prefix.
        let escaped = term.replace('"', "\"\"");
        Some(format!("\"{}\"*", escaped))
    }

    /// Extract a ~200 character snippet centered on the earliest matching term.
    /// UTF-8 safe: window bounds are clamped to char boundaries so this never
    /// panics on multi-byte text.
    fn get_match_context(transcript: &str, terms: &[String]) -> String {
        let transcript_lower = transcript.to_lowercase();

        // Byte index of the earliest occurrence of any term.
        let match_index = terms
            .iter()
            .filter_map(|term| transcript_lower.find(term.as_str()))
            .min();

        let len = transcript.len();
        match match_index {
            Some(idx) => {
                let start = Self::char_boundary_floor(transcript, idx.saturating_sub(100));
                let end = Self::char_boundary_ceil(transcript, idx.saturating_add(100).min(len));

                let mut context = String::new();
                if start > 0 {
                    context.push_str("...");
                }
                context.push_str(&transcript[start..end]);
                if end < len {
                    context.push_str("...");
                }
                context
            }
            None => transcript.chars().take(200).collect(),
        }
    }

    /// Round a byte index down to the nearest UTF-8 char boundary.
    fn char_boundary_floor(s: &str, mut i: usize) -> usize {
        i = i.min(s.len());
        while i > 0 && !s.is_char_boundary(i) {
            i -= 1;
        }
        i
    }

    /// Round a byte index up to the nearest UTF-8 char boundary.
    fn char_boundary_ceil(s: &str, mut i: usize) -> usize {
        i = i.min(s.len());
        while i < s.len() && !s.is_char_boundary(i) {
            i += 1;
        }
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::TranscriptSegment;
    use sqlx::sqlite::SqlitePoolOptions;

    /// In-memory database with all migrations applied (including the FTS5 one).
    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    fn seg(text: &str, timestamp: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: String::new(),
            text: text.to_string(),
            timestamp: timestamp.to_string(),
            audio_start_time: None,
            audio_end_time: None,
            duration: None,
        }
    }

    async fn add_meeting(pool: &SqlitePool, title: &str, segments: &[TranscriptSegment]) -> String {
        TranscriptsRepository::save_transcript(pool, title, segments, None)
            .await
            .unwrap()
    }

    async fn search(pool: &SqlitePool, query: &str) -> Vec<TranscriptSearchResult> {
        TranscriptsRepository::search_transcripts(pool, query)
            .await
            .unwrap()
    }

    async fn fts_match_count(pool: &SqlitePool, match_expr: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM transcripts_fts WHERE transcripts_fts MATCH ?",
        )
        .bind(match_expr)
        .fetch_one(pool)
        .await
        .unwrap()
    }

    // --- Pure helper: FTS expression builder ---------------------------------

    #[test]
    fn fts_prefix_query_quotes_and_appends_star() {
        assert_eq!(
            TranscriptsRepository::fts_prefix_query("report"),
            Some("\"report\"*".to_string())
        );
    }

    #[test]
    fn fts_prefix_query_escapes_embedded_quotes() {
        assert_eq!(
            TranscriptsRepository::fts_prefix_query("a\"b"),
            Some("\"a\"\"b\"*".to_string())
        );
    }

    #[test]
    fn fts_prefix_query_rejects_non_tokenizable_terms() {
        assert_eq!(TranscriptsRepository::fts_prefix_query("!!!"), None);
        assert_eq!(TranscriptsRepository::fts_prefix_query("-"), None);
        assert_eq!(TranscriptsRepository::fts_prefix_query(""), None);
    }

    // --- Query parsing edge cases --------------------------------------------

    #[tokio::test]
    async fn empty_or_whitespace_query_returns_nothing() {
        let pool = test_pool().await;
        add_meeting(&pool, "Standup", &[seg("we discussed the budget", "00:00")]).await;
        assert!(search(&pool, "").await.is_empty());
        assert!(search(&pool, "    ").await.is_empty());
        assert!(search(&pool, "\t\n").await.is_empty());
    }

    #[tokio::test]
    async fn no_matches_returns_empty() {
        let pool = test_pool().await;
        add_meeting(&pool, "Standup", &[seg("we discussed the budget", "00:00")]).await;
        assert!(search(&pool, "helicopter").await.is_empty());
    }

    // --- Core matching -------------------------------------------------------

    #[tokio::test]
    async fn finds_meeting_containing_a_single_term() {
        let pool = test_pool().await;
        let id = add_meeting(
            &pool,
            "Budget review",
            &[seg("we discussed the budget", "00:00")],
        )
        .await;
        add_meeting(&pool, "Lunch", &[seg("we ordered pizza", "00:00")]).await;

        let results = search(&pool, "budget").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert_eq!(results[0].title, "Budget review");
    }

    #[tokio::test]
    async fn multi_term_matches_across_different_segments() {
        let pool = test_pool().await;
        // "sarah" and "report" appear in different segments of the same meeting.
        let id = add_meeting(
            &pool,
            "Sync",
            &[
                seg("sarah kicked things off", "00:00"),
                seg("she owes us the report", "00:05"),
            ],
        )
        .await;

        let results = search(&pool, "sarah report").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[tokio::test]
    async fn multi_term_requires_every_term() {
        let pool = test_pool().await;
        // Only "sarah" present, "report" missing -> no match.
        add_meeting(&pool, "Sync", &[seg("sarah kicked things off", "00:00")]).await;
        assert!(search(&pool, "sarah report").await.is_empty());
    }

    #[tokio::test]
    async fn ranks_by_total_occurrences() {
        let pool = test_pool().await;
        let few = add_meeting(
            &pool,
            "Few",
            &[seg("the budget was mentioned once", "00:00")],
        )
        .await;
        let many = add_meeting(
            &pool,
            "Many",
            &[seg("budget budget budget everywhere budget", "00:00")],
        )
        .await;

        let results = search(&pool, "budget").await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, many, "meeting with more hits ranks first");
        assert_eq!(results[1].id, few);
    }

    #[tokio::test]
    async fn matching_is_case_insensitive() {
        let pool = test_pool().await;
        let id = add_meeting(&pool, "Sync", &[seg("The BUDGET is approved", "00:00")]).await;
        assert_eq!(search(&pool, "budget").await[0].id, id);
        assert_eq!(search(&pool, "BuDgEt").await[0].id, id);
    }

    #[tokio::test]
    async fn prefix_matching_finds_word_extensions() {
        let pool = test_pool().await;
        let id = add_meeting(
            &pool,
            "Sync",
            &[seg("she is reporting the numbers", "00:00")],
        )
        .await;
        // "report" is a prefix of the token "reporting".
        assert_eq!(search(&pool, "report").await[0].id, id);
    }

    #[tokio::test]
    async fn does_not_match_mid_word_substrings() {
        let pool = test_pool().await;
        add_meeting(&pool, "Sync", &[seg("we will teleport there", "00:00")]).await;
        // "port" is inside "teleport" but is not a token prefix -> no match.
        assert!(search(&pool, "port").await.is_empty());
    }

    #[tokio::test]
    async fn picks_best_segment_for_snippet_and_jump_target() {
        let pool = test_pool().await;
        add_meeting(
            &pool,
            "Sync",
            &[
                seg("sarah said hello", "00:00"),
                seg("sarah delivered the report on time", "00:05"),
            ],
        )
        .await;

        let results = search(&pool, "sarah report").await;
        assert_eq!(results.len(), 1);
        // The best segment is the one containing the most distinct terms.
        assert_eq!(results[0].timestamp, "00:05");
        assert!(results[0].match_context.contains("report"));
    }

    // --- Snippet building ----------------------------------------------------

    #[tokio::test]
    async fn snippet_adds_ellipsis_for_long_segments() {
        let pool = test_pool().await;
        let long_text = format!("{} budget {}", "lorem ".repeat(40), "ipsum ".repeat(40));
        add_meeting(&pool, "Sync", &[seg(&long_text, "00:00")]).await;

        let results = search(&pool, "budget").await;
        assert_eq!(results.len(), 1);
        let ctx = &results[0].match_context;
        assert!(ctx.contains("budget"));
        assert!(ctx.starts_with("..."));
        assert!(ctx.ends_with("..."));
        // ~200 char window plus the two ellipses; never the whole segment.
        assert!(ctx.len() < long_text.len());
    }

    #[tokio::test]
    async fn snippet_is_utf8_safe_for_multibyte_text() {
        let pool = test_pool().await;
        // Multi-byte characters around the match must not panic on slicing.
        let text = format!("{} café résumé naïve budget garçon", "über ".repeat(30));
        let id = add_meeting(&pool, "Sync", &[seg(&text, "00:00")]).await;
        let results = search(&pool, "budget").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
        assert!(results[0].match_context.contains("budget"));
    }

    // --- Degenerate / adversarial queries ------------------------------------

    #[tokio::test]
    async fn punctuation_only_query_uses_fallback_scan() {
        let pool = test_pool().await;
        let id = add_meeting(&pool, "Sync", &[seg("wow !!! that is amazing", "00:00")]).await;
        // "!!!" is not FTS-tokenizable, so the search falls back to scanning all
        // meetings and the in-memory substring filter still finds it.
        let results = search(&pool, "!!!").await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[tokio::test]
    async fn fts_operator_characters_in_query_do_not_error() {
        let pool = test_pool().await;
        add_meeting(
            &pool,
            "Sync",
            &[seg("the quarterly report is ready", "00:00")],
        )
        .await;
        // Characters that are FTS5 operators/syntax must be treated as literal
        // text, never injected into the MATCH expression.
        for q in [
            "report\" OR 1=1",
            "report*",
            "(report",
            "report AND budget",
            "report -budget",
            "NEAR(report",
            "^report",
            "report:budget",
        ] {
            let results = TranscriptsRepository::search_transcripts(&pool, q).await;
            assert!(results.is_ok(), "query {q:?} should not error");
        }
    }

    // --- Index stays in sync with the transcripts table ----------------------

    #[tokio::test]
    async fn insert_populates_the_fts_index() {
        let pool = test_pool().await;
        assert_eq!(fts_match_count(&pool, "\"budget\"*").await, 0);
        add_meeting(&pool, "Sync", &[seg("the budget is fine", "00:00")]).await;
        assert_eq!(fts_match_count(&pool, "\"budget\"*").await, 1);
    }

    #[tokio::test]
    async fn deleting_transcripts_removes_them_from_the_index() {
        let pool = test_pool().await;
        let id = add_meeting(&pool, "Sync", &[seg("the budget is fine", "00:00")]).await;
        assert_eq!(fts_match_count(&pool, "\"budget\"*").await, 1);

        sqlx::query("DELETE FROM transcripts WHERE meeting_id = ?")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(fts_match_count(&pool, "\"budget\"*").await, 0);
        assert!(search(&pool, "budget").await.is_empty());
    }

    #[tokio::test]
    async fn updating_a_transcript_reindexes_it() {
        let pool = test_pool().await;
        let id = add_meeting(&pool, "Sync", &[seg("the budget is fine", "00:00")]).await;
        assert_eq!(fts_match_count(&pool, "\"budget\"*").await, 1);
        assert_eq!(fts_match_count(&pool, "\"roadmap\"*").await, 0);

        sqlx::query("UPDATE transcripts SET transcript = ? WHERE meeting_id = ?")
            .bind("we reviewed the roadmap")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(fts_match_count(&pool, "\"budget\"*").await, 0);
        assert_eq!(fts_match_count(&pool, "\"roadmap\"*").await, 1);
        assert!(search(&pool, "budget").await.is_empty());
        assert_eq!(search(&pool, "roadmap").await[0].id, id);
    }

    #[tokio::test]
    async fn only_candidate_meetings_are_returned_among_many() {
        let pool = test_pool().await;
        // Populate several meetings; only two contain "budget".
        for i in 0..20 {
            add_meeting(
                &pool,
                &format!("Chat {i}"),
                &[seg("just some small talk here", "00:00")],
            )
            .await;
        }
        let a = add_meeting(&pool, "Planning", &[seg("we set the budget", "00:00")]).await;
        let b = add_meeting(&pool, "Review", &[seg("the budget looks tight", "00:00")]).await;

        let results = search(&pool, "budget").await;
        let ids: std::collections::HashSet<_> = results.iter().map(|r| r.id.clone()).collect();
        assert_eq!(results.len(), 2);
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
    }
}
