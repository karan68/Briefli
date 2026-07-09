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

        // All transcript segments with their meeting, ordered so segments of
        // the same meeting are grouped and in chronological order.
        let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
            "SELECT m.id, m.title, t.id, t.transcript, t.timestamp
             FROM meetings m
             JOIN transcripts t ON m.id = t.meeting_id
             ORDER BY m.id, t.timestamp",
        )
        .fetch_all(pool)
        .await?;

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
