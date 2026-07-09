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

    /// Searches for a query string within the transcripts.
    /// It returns a list of matching transcripts with context.
    /// Cross-meeting transcript search.
    ///
    /// Splits the query into whitespace-separated terms and returns one result
    /// per meeting whose full transcript (all segments concatenated) contains
    /// EVERY term. Results are ranked by total term occurrences (most relevant
    /// first), so a search like "sarah report" surfaces meetings that mention
    /// both words, even in different parts of the conversation.
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

        // One row per meeting with all transcript segments concatenated in
        // chronological order.
        let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>)>(
            "SELECT m.id, m.title,
                    GROUP_CONCAT(t.transcript, ' ') AS full_text,
                    MIN(t.timestamp) AS ts
             FROM meetings m
             JOIN transcripts t ON m.id = t.meeting_id
             GROUP BY m.id, m.title",
        )
        .fetch_all(pool)
        .await?;

        let mut scored: Vec<(usize, TranscriptSearchResult)> = Vec::new();

        for (id, title, full_text, timestamp) in rows {
            let full_text = full_text.unwrap_or_default();
            let text_lower = full_text.to_lowercase();

            // A meeting matches only if it contains ALL search terms.
            if !terms.iter().all(|term| text_lower.contains(term.as_str())) {
                continue;
            }

            // Relevance = total number of term occurrences across the meeting.
            let score: usize = terms
                .iter()
                .map(|term| text_lower.matches(term.as_str()).count())
                .sum();

            scored.push((
                score,
                TranscriptSearchResult {
                    id,
                    title,
                    match_context: Self::get_match_context(&full_text, &terms),
                    timestamp: timestamp.unwrap_or_default(),
                },
            ));
        }

        // Highest relevance first.
        scored.sort_by(|a, b| b.0.cmp(&a.0));

        Ok(scored.into_iter().map(|(_, result)| result).collect())
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
