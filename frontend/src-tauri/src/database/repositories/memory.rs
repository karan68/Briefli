use crate::database::models::{MeetingMemoryModel, MemoryOwnerAliasModel};
use chrono::Utc;
use sqlx::{Error as SqlxError, SqlitePool};
use std::collections::HashSet;
use uuid::Uuid;

const EVIDENCE_MATCH_THRESHOLD: f64 = 0.35;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    Decision,
    Commitment,
    OpenQuestion,
}

impl MemoryKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::Commitment => "commitment",
            Self::OpenQuestion => "open_question",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStatus {
    Suggested,
    Confirmed,
    Corrected,
    Rejected,
}

impl ReviewStatus {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "suggested" => Some(Self::Suggested),
            "confirmed" => Some(Self::Confirmed),
            "corrected" => Some(Self::Corrected),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Suggested => "suggested",
            Self::Confirmed => "confirmed",
            Self::Corrected => "corrected",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug)]
pub struct ReviewMemoryInput<'a> {
    pub status: ReviewStatus,
    pub text: &'a str,
    pub owner: Option<&'a str>,
    pub due_date: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq)]
struct MemoryCandidate {
    kind: MemoryKind,
    text: String,
    owner: Option<String>,
    due_date: Option<String>,
    reference_excerpt: Option<String>,
}

#[derive(Debug, Default)]
struct ParsedMemories {
    decisions: Option<Vec<MemoryCandidate>>,
    commitments: Option<Vec<MemoryCandidate>>,
    open_questions: Option<Vec<MemoryCandidate>>,
}

#[derive(Debug, Clone)]
struct TranscriptEvidence {
    id: String,
    text: String,
    timestamp: String,
    audio_start_time: Option<f64>,
    audio_end_time: Option<f64>,
}

#[derive(Debug)]
struct EvidenceMatch<'a> {
    transcript: &'a TranscriptEvidence,
    confidence: f64,
}

pub struct MeetingMemoriesRepository;

impl MeetingMemoriesRepository {
    pub async fn sync_all(pool: &SqlitePool) -> Result<u64, SqlxError> {
        let summaries = sqlx::query_as::<_, (String, String)>(
            "SELECT meeting_id, result FROM summary_processes
             WHERE result IS NOT NULL AND result != ''",
        )
        .fetch_all(pool)
        .await?;

        let now = Utc::now().to_rfc3339();
        let mut tx = pool.begin().await?;

        for (meeting_id, result) in summaries {
            let Some(markdown) = extract_summary_markdown(&result) else {
                continue;
            };
            let parsed = parse_memories(&markdown);
            if parsed.decisions.is_none()
                && parsed.commitments.is_none()
                && parsed.open_questions.is_none()
            {
                continue;
            }

            let transcripts =
                sqlx::query_as::<_, (String, String, String, Option<f64>, Option<f64>)>(
                    "SELECT id, transcript, timestamp, audio_start_time, audio_end_time
                 FROM transcripts WHERE meeting_id = ? ORDER BY timestamp",
                )
                .bind(&meeting_id)
                .fetch_all(&mut *tx)
                .await?
                .into_iter()
                .map(
                    |(id, text, timestamp, audio_start_time, audio_end_time)| TranscriptEvidence {
                        id,
                        text,
                        timestamp,
                        audio_start_time,
                        audio_end_time,
                    },
                )
                .collect::<Vec<_>>();

            for (kind, candidates) in [
                (MemoryKind::Decision, parsed.decisions),
                (MemoryKind::Commitment, parsed.commitments),
                (MemoryKind::OpenQuestion, parsed.open_questions),
            ] {
                let Some(candidates) = candidates else {
                    continue;
                };

                let fingerprints = candidates
                    .iter()
                    .map(|candidate| source_fingerprint(&candidate.text))
                    .collect::<HashSet<_>>();
                let existing = sqlx::query_as::<_, (String, String, String)>(
                    "SELECT id, source_fingerprint, review_status
                     FROM meeting_memories WHERE meeting_id = ? AND kind = ?",
                )
                .bind(&meeting_id)
                .bind(kind.as_str())
                .fetch_all(&mut *tx)
                .await?;

                for (id, fingerprint, review_status) in existing {
                    if review_status == "suggested" && !fingerprints.contains(&fingerprint) {
                        sqlx::query("DELETE FROM meeting_memories WHERE id = ?")
                            .bind(id)
                            .execute(&mut *tx)
                            .await?;
                    }
                }

                for mut candidate in candidates {
                    if let Some(owner) = candidate.owner.as_deref() {
                        candidate.owner = resolve_owner_alias(&mut tx, owner).await?;
                    }
                    let fingerprint = source_fingerprint(&candidate.text);
                    if fingerprint.is_empty() {
                        continue;
                    }
                    let evidence = find_evidence(&candidate, &transcripts);
                    let id = format!("memory-{}", Uuid::new_v4());
                    let (transcript_id, excerpt, timestamp, start_time, end_time, confidence) =
                        match evidence {
                            Some(matched) => (
                                Some(matched.transcript.id.as_str()),
                                Some(matched.transcript.text.as_str()),
                                Some(matched.transcript.timestamp.as_str()),
                                matched.transcript.audio_start_time,
                                matched.transcript.audio_end_time,
                                Some(matched.confidence),
                            ),
                            None => (None, None, None, None, None, None),
                        };

                    sqlx::query(
                        "INSERT INTO meeting_memories
                         (id, meeting_id, kind, text, suggested_text, source_fingerprint,
                          owner, due_date, review_status, resolution_status,
                          evidence_confidence, source_transcript_id, source_excerpt,
                          source_timestamp, source_audio_start_time, source_audio_end_time,
                          created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'suggested', 'open', ?, ?, ?, ?, ?, ?, ?, ?)
                         ON CONFLICT(meeting_id, kind, source_fingerprint) DO UPDATE SET
                           text = CASE WHEN meeting_memories.review_status = 'suggested'
                                      THEN excluded.text ELSE meeting_memories.text END,
                           suggested_text = excluded.suggested_text,
                           owner = CASE WHEN meeting_memories.review_status = 'suggested'
                                        THEN excluded.owner ELSE meeting_memories.owner END,
                           due_date = CASE WHEN meeting_memories.review_status = 'suggested'
                                           THEN excluded.due_date ELSE meeting_memories.due_date END,
                           evidence_confidence = CASE WHEN meeting_memories.review_status = 'suggested'
                                                      THEN excluded.evidence_confidence ELSE meeting_memories.evidence_confidence END,
                           source_transcript_id = CASE WHEN meeting_memories.review_status = 'suggested'
                                                       THEN excluded.source_transcript_id ELSE meeting_memories.source_transcript_id END,
                           source_excerpt = CASE WHEN meeting_memories.review_status = 'suggested'
                                                 THEN excluded.source_excerpt ELSE meeting_memories.source_excerpt END,
                           source_timestamp = CASE WHEN meeting_memories.review_status = 'suggested'
                                                   THEN excluded.source_timestamp ELSE meeting_memories.source_timestamp END,
                           source_audio_start_time = CASE WHEN meeting_memories.review_status = 'suggested'
                                                          THEN excluded.source_audio_start_time ELSE meeting_memories.source_audio_start_time END,
                           source_audio_end_time = CASE WHEN meeting_memories.review_status = 'suggested'
                                                        THEN excluded.source_audio_end_time ELSE meeting_memories.source_audio_end_time END,
                           updated_at = CASE WHEN meeting_memories.review_status = 'suggested'
                                             THEN excluded.updated_at ELSE meeting_memories.updated_at END",
                    )
                    .bind(id)
                    .bind(&meeting_id)
                    .bind(kind.as_str())
                    .bind(&candidate.text)
                    .bind(&candidate.text)
                    .bind(fingerprint)
                    .bind(candidate.owner.as_deref())
                    .bind(candidate.due_date.as_deref())
                    .bind(confidence)
                    .bind(transcript_id)
                    .bind(excerpt)
                    .bind(timestamp)
                    .bind(start_time)
                    .bind(end_time)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        tx.commit().await?;
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meeting_memories")
            .fetch_one(pool)
            .await?;
        Ok(count.max(0) as u64)
    }

    pub async fn list(pool: &SqlitePool) -> Result<Vec<MeetingMemoryModel>, SqlxError> {
        sqlx::query_as::<_, MeetingMemoryModel>(
            "SELECT mm.id, mm.meeting_id, m.title AS meeting_title, mm.kind, mm.text,
                    mm.suggested_text, mm.owner, mm.due_date, mm.review_status,
                    mm.resolution_status, mm.evidence_confidence,
                    mm.source_transcript_id, mm.source_excerpt, mm.source_timestamp,
                    mm.source_audio_start_time, mm.source_audio_end_time,
                    mm.created_at, mm.updated_at, mm.reviewed_at,
                    mm.follow_up_reviewed_at
             FROM meeting_memories mm
             JOIN meetings m ON m.id = mm.meeting_id
             WHERE mm.review_status != 'rejected'
             ORDER BY CASE mm.review_status WHEN 'suggested' THEN 0 ELSE 1 END,
                      mm.updated_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn review(
        pool: &SqlitePool,
        id: &str,
        input: ReviewMemoryInput<'_>,
    ) -> Result<(), SqlxError> {
        let text = input.text.trim();
        if text.is_empty() || input.status == ReviewStatus::Suggested {
            return Err(SqlxError::Protocol(
                "reviewed memory requires text and a final review status".into(),
            ));
        }
        let previous_owner = sqlx::query_scalar::<_, Option<String>>(
            "SELECT owner FROM meeting_memories WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(SqlxError::RowNotFound)?;
        let now = Utc::now().to_rfc3339();
        let mut tx = pool.begin().await?;
        let result = sqlx::query(
            "UPDATE meeting_memories
             SET text = ?, owner = ?, due_date = ?, review_status = ?,
                 reviewed_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(text)
        .bind(clean_optional(input.owner))
        .bind(clean_optional(input.due_date))
        .bind(input.status.as_str())
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(SqlxError::RowNotFound);
        }
        if input.status == ReviewStatus::Corrected {
            if let (Some(alias), Some(canonical_name)) =
                (previous_owner.as_deref(), clean_optional(input.owner))
            {
                learn_owner_alias(&mut tx, alias, canonical_name, &now).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_owner_aliases(
        pool: &SqlitePool,
    ) -> Result<Vec<MemoryOwnerAliasModel>, SqlxError> {
        sqlx::query_as::<_, MemoryOwnerAliasModel>(
            "SELECT alias, canonical_name FROM memory_owner_aliases
             ORDER BY alias_key",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn delete_owner_alias(pool: &SqlitePool, alias: &str) -> Result<(), SqlxError> {
        let alias_key = owner_key(alias);
        if alias_key.is_empty() {
            return Err(SqlxError::Protocol("owner alias cannot be empty".into()));
        }
        let result = sqlx::query("DELETE FROM memory_owner_aliases WHERE alias_key = ?")
            .bind(alias_key)
            .execute(pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(SqlxError::RowNotFound);
        }
        Ok(())
    }

    pub async fn set_resolution(
        pool: &SqlitePool,
        id: &str,
        status: &str,
    ) -> Result<(), SqlxError> {
        if !matches!(status, "open" | "done") {
            return Err(SqlxError::Protocol("invalid resolution status".into()));
        }
        let result = sqlx::query(
            "UPDATE meeting_memories
             SET resolution_status = ?,
                 follow_up_reviewed_at = CASE WHEN ? = 'open' THEN NULL ELSE follow_up_reviewed_at END,
                 updated_at = ?
             WHERE id = ?",
        )
        .bind(status)
        .bind(status)
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(SqlxError::RowNotFound);
        }
        Ok(())
    }

    pub async fn mark_follow_up_reviewed(pool: &SqlitePool, id: &str) -> Result<(), SqlxError> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE meeting_memories
             SET follow_up_reviewed_at = ?, updated_at = ?
             WHERE id = ?
               AND kind IN ('commitment', 'open_question')
               AND review_status IN ('confirmed', 'corrected')
               AND resolution_status = 'open'",
        )
        .bind(&now)
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(SqlxError::RowNotFound);
        }
        Ok(())
    }
}

fn owner_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

async fn resolve_owner_alias(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    owner: &str,
) -> Result<Option<String>, SqlxError> {
    let key = owner_key(owner);
    if key.is_empty() {
        return Ok(None);
    }
    let canonical = sqlx::query_scalar::<_, String>(
        "SELECT canonical_name FROM memory_owner_aliases WHERE alias_key = ?",
    )
    .bind(key)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(Some(canonical.unwrap_or_else(|| owner.trim().to_string())))
}

async fn learn_owner_alias(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    alias: &str,
    canonical_name: &str,
    now: &str,
) -> Result<(), SqlxError> {
    let alias = alias.trim();
    let canonical_name = canonical_name.trim();
    let alias_key = owner_key(alias);
    let canonical_key = owner_key(canonical_name);
    if alias_key.is_empty() || canonical_key.is_empty() || alias_key == canonical_key {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO memory_owner_aliases
         (alias_key, alias, canonical_key, canonical_name, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(alias_key) DO UPDATE SET
           alias = excluded.alias,
           canonical_key = excluded.canonical_key,
           canonical_name = excluded.canonical_name,
           updated_at = excluded.updated_at",
    )
    .bind(alias_key)
    .bind(alias)
    .bind(canonical_key)
    .bind(canonical_name)
    .bind(now)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn clean_optional(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn extract_summary_markdown(result: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(result).ok()?;
    value
        .get("markdown")
        .and_then(|markdown| markdown.as_str())
        .or_else(|| {
            value
                .get("english_cache")
                .and_then(|cache| cache.get("markdown"))
                .and_then(|markdown| markdown.as_str())
        })
        .map(str::to_string)
}

fn parse_memories(markdown: &str) -> ParsedMemories {
    ParsedMemories {
        decisions: parse_list_section(
            markdown,
            &["key decisions", "decisions"],
            MemoryKind::Decision,
        ),
        commitments: parse_action_items(markdown),
        open_questions: parse_list_section(
            markdown,
            &["open questions", "unresolved questions"],
            MemoryKind::OpenQuestion,
        ),
    }
}

fn section_lines<'a>(markdown: &'a str, names: &[&str]) -> Option<Vec<&'a str>> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let start = lines.iter().position(|line| {
        line.trim_start().starts_with('#') && {
            let heading = normalize_heading(line);
            names.iter().any(|name| heading == *name)
        }
    })?;
    let mut section = Vec::new();
    for line in lines.into_iter().skip(start + 1) {
        if line.trim_start().starts_with('#') {
            break;
        }
        section.push(line);
    }
    Some(section)
}

fn normalize_heading(line: &str) -> String {
    line.trim()
        .trim_start_matches('#')
        .trim()
        .trim_matches('*')
        .trim()
        .trim_end_matches(':')
        .trim()
        .to_lowercase()
}

fn parse_list_section(
    markdown: &str,
    names: &[&str],
    kind: MemoryKind,
) -> Option<Vec<MemoryCandidate>> {
    let lines = section_lines(markdown, names)?;
    let rows = lines
        .iter()
        .map(|line| line.trim())
        .filter(|line| line.starts_with('|'))
        .collect::<Vec<_>>();
    if rows.len() >= 2 {
        let header = parse_cells(rows[0])
            .into_iter()
            .map(|cell| cell.to_lowercase())
            .collect::<Vec<_>>();
        let value_names: &[&str] = match kind {
            MemoryKind::Decision => &["decision"],
            MemoryKind::Commitment => &["task", "action", "commitment"],
            MemoryKind::OpenQuestion => &["question"],
        };
        let value_column = header
            .iter()
            .position(|heading| value_names.iter().any(|name| heading.contains(name)))
            .unwrap_or(0);
        let reference_column = header.iter().position(|heading| {
            ["reference", "evidence", "quote"]
                .iter()
                .any(|name| heading.contains(name))
        });
        let candidates = rows
            .into_iter()
            .skip(1)
            .filter(|row| {
                !row.chars()
                    .all(|character| matches!(character, '|' | '-' | ':' | ' '))
            })
            .filter_map(|row| {
                let cells = parse_cells(row);
                let text = cells.get(value_column).cloned().unwrap_or_default();
                (!is_noise(&text)).then(|| MemoryCandidate {
                    kind,
                    text,
                    owner: None,
                    due_date: None,
                    reference_excerpt: reference_column
                        .and_then(|index| cells.get(index).cloned())
                        .filter(|value| !is_noise(value)),
                })
            })
            .collect();
        return Some(candidates);
    }

    let mut candidates = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        let item = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
            .or_else(|| strip_numbered_prefix(trimmed));
        let Some(item) = item else {
            continue;
        };
        let text = strip_markdown(item);
        if !is_noise(&text) {
            candidates.push(MemoryCandidate {
                kind,
                text,
                owner: None,
                due_date: None,
                reference_excerpt: None,
            });
        }
    }
    Some(candidates)
}

fn strip_numbered_prefix(value: &str) -> Option<&str> {
    let delimiter = value.find(['.', ')'])?;
    if value[..delimiter]
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        Some(value[delimiter + 1..].trim_start())
    } else {
        None
    }
}

fn parse_action_items(markdown: &str) -> Option<Vec<MemoryCandidate>> {
    let section_names = &["action items", "commitments", "next steps"];
    let lines = section_lines(markdown, section_names)?;
    let rows = lines
        .into_iter()
        .map(str::trim)
        .filter(|line| line.starts_with('|'))
        .collect::<Vec<_>>();
    if rows.len() < 2 {
        return parse_list_section(markdown, section_names, MemoryKind::Commitment);
    }

    let header = parse_cells(rows[0])
        .into_iter()
        .map(|cell| cell.to_lowercase())
        .collect::<Vec<_>>();
    let find_column = |names: &[&str]| {
        header
            .iter()
            .position(|heading| names.iter().any(|name| heading.contains(name)))
    };
    let task_column = find_column(&["task", "action", "commitment"]).unwrap_or(1);
    let owner_column = find_column(&["owner", "assignee"]);
    let due_column = find_column(&["due", "deadline"]);
    let reference_column = find_column(&["reference", "evidence", "quote"]);

    let mut candidates = Vec::new();
    for row in rows.into_iter().skip(1) {
        if row
            .chars()
            .all(|character| matches!(character, '|' | '-' | ':' | ' '))
        {
            continue;
        }
        let cells = parse_cells(row);
        let text = cells.get(task_column).cloned().unwrap_or_default();
        if is_noise(&text) {
            continue;
        }
        candidates.push(MemoryCandidate {
            kind: MemoryKind::Commitment,
            text,
            owner: owner_column
                .and_then(|index| cells.get(index).cloned())
                .filter(|value| !is_noise(value)),
            due_date: due_column
                .and_then(|index| cells.get(index).cloned())
                .filter(|value| !is_noise(value)),
            reference_excerpt: reference_column
                .and_then(|index| cells.get(index).cloned())
                .filter(|value| !is_noise(value)),
        });
    }
    Some(candidates)
}

fn parse_cells(row: &str) -> Vec<String> {
    row.trim()
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(strip_markdown)
        .collect()
}

fn strip_markdown(value: &str) -> String {
    value
        .replace("**", "")
        .replace(['`', '"'], "")
        .trim()
        .trim_start_matches("[ ]")
        .trim_start_matches("[x]")
        .trim()
        .to_string()
}

fn is_noise(value: &str) -> bool {
    let normalized = value.trim().trim_end_matches('.').trim().to_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "n/a" | "na" | "none" | "tbd" | "unspecified"
        )
        || normalized.starts_with("none noted")
}

fn source_fingerprint(text: &str) -> String {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|character| character.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn evidence_tokens(text: &str) -> HashSet<String> {
    const STOP_WORDS: &[&str] = &[
        "and", "the", "that", "this", "with", "from", "will", "for", "was", "are", "to", "of",
        "in", "on", "a", "an", "is", "be", "by", "it",
    ];
    source_fingerprint(text)
        .split_whitespace()
        .filter(|token| token.len() > 2 && !STOP_WORDS.contains(token))
        .map(str::to_string)
        .collect()
}

fn find_evidence<'a>(
    candidate: &MemoryCandidate,
    transcripts: &'a [TranscriptEvidence],
) -> Option<EvidenceMatch<'a>> {
    let reference = candidate.reference_excerpt.as_deref().unwrap_or("");
    let normalized_reference = source_fingerprint(reference);
    let normalized_candidate = source_fingerprint(&candidate.text);
    let query = if normalized_reference.is_empty() {
        candidate.text.as_str()
    } else {
        reference
    };
    let query_tokens = evidence_tokens(query);

    transcripts
        .iter()
        .filter_map(|transcript| {
            let normalized_transcript = source_fingerprint(&transcript.text);
            if normalized_reference.len() >= 8
                && normalized_transcript.contains(&normalized_reference)
            {
                return Some(EvidenceMatch {
                    transcript,
                    confidence: 1.0,
                });
            }
            if normalized_candidate.len() >= 12
                && normalized_transcript.contains(&normalized_candidate)
            {
                return Some(EvidenceMatch {
                    transcript,
                    confidence: 0.95,
                });
            }
            if query_tokens.is_empty() {
                return None;
            }
            let transcript_tokens = evidence_tokens(&transcript.text);
            let shared = query_tokens.intersection(&transcript_tokens).count();
            let coverage = shared as f64 / query_tokens.len() as f64;
            (shared >= 2 && coverage >= EVIDENCE_MATCH_THRESHOLD).then_some(EvidenceMatch {
                transcript,
                confidence: coverage.min(0.9),
            })
        })
        .max_by(|left, right| left.confidence.total_cmp(&right.confidence))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory database");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("run migrations");
        pool
    }

    async fn seed_meeting(pool: &SqlitePool, markdown: &str) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO meetings (id, title, created_at, updated_at)
             VALUES ('meeting-1', 'Client review', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcripts
             (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time)
             VALUES ('transcript-1', 'meeting-1',
                     'Sam will send the revised proposal by Friday.', '00:04:12', 252.0, 258.0)",
        )
        .execute(pool)
        .await
        .unwrap();
        let result = serde_json::json!({ "markdown": markdown }).to_string();
        sqlx::query(
            "INSERT INTO summary_processes
             (meeting_id, status, created_at, updated_at, result)
             VALUES ('meeting-1', 'completed', ?, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .bind(result)
        .execute(pool)
        .await
        .unwrap();
    }

    const SUMMARY: &str = r#"
## Key Decisions
- Use the revised proposal for the pilot.

## Action Items
| Owner | Task | Due | Reference Transcript Segment |
| --- | --- | --- | --- |
| Sam | Send the revised proposal | Friday | Sam will send the revised proposal by Friday. |

## Open Questions
- Which region should join the pilot?
"#;

    #[test]
    fn parses_all_supported_memory_types() {
        let parsed = parse_memories(SUMMARY);
        assert_eq!(parsed.decisions.unwrap().len(), 1);
        let commitments = parsed.commitments.unwrap();
        assert_eq!(commitments.len(), 1);
        assert_eq!(commitments[0].owner.as_deref(), Some("Sam"));
        assert_eq!(commitments[0].due_date.as_deref(), Some("Friday"));
        assert_eq!(parsed.open_questions.unwrap().len(), 1);
    }

    #[test]
    fn parses_table_decisions_and_next_step_bullets() {
        let markdown = r#"
## Key Decisions
| Decision | Rationale | Evidence Quote |
| --- | --- | --- |
| Start with the pilot | Lowest risk | We should start with the pilot. |

## Next Steps
- Send the revised proposal.
"#;
        let parsed = parse_memories(markdown);
        let decisions = parsed.decisions.unwrap();
        assert_eq!(decisions[0].text, "Start with the pilot");
        assert_eq!(
            decisions[0].reference_excerpt.as_deref(),
            Some("We should start with the pilot.")
        );
        assert_eq!(
            parsed.commitments.unwrap()[0].text,
            "Send the revised proposal."
        );
    }

    #[test]
    fn unknown_sections_are_not_treated_as_empty() {
        let parsed = parse_memories("## Summary\nNothing structured here.");
        assert!(parsed.decisions.is_none());
        assert!(parsed.commitments.is_none());
        assert!(parsed.open_questions.is_none());
    }

    #[tokio::test]
    async fn sync_links_only_stored_transcript_evidence() {
        let pool = test_pool().await;
        seed_meeting(&pool, SUMMARY).await;

        assert_eq!(MeetingMemoriesRepository::sync_all(&pool).await.unwrap(), 3);
        let memories = MeetingMemoriesRepository::list(&pool).await.unwrap();
        let commitment = memories
            .iter()
            .find(|memory| memory.kind == "commitment")
            .unwrap();
        assert_eq!(
            commitment.source_transcript_id.as_deref(),
            Some("transcript-1")
        );
        assert_eq!(
            commitment.source_excerpt.as_deref(),
            Some("Sam will send the revised proposal by Friday.")
        );
        assert_eq!(commitment.source_audio_start_time, Some(252.0));
        assert_eq!(commitment.evidence_confidence, Some(1.0));
    }

    #[tokio::test]
    async fn corrected_memory_survives_summary_regeneration_without_duplicate() {
        let pool = test_pool().await;
        seed_meeting(&pool, SUMMARY).await;
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();
        let memory = MeetingMemoriesRepository::list(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|memory| memory.kind == "commitment")
            .unwrap();

        MeetingMemoriesRepository::review(
            &pool,
            &memory.id,
            ReviewMemoryInput {
                status: ReviewStatus::Corrected,
                text: "Sam will send the revised pilot proposal",
                owner: Some("Sam"),
                due_date: Some("Friday"),
            },
        )
        .await
        .unwrap();
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();

        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT text, review_status FROM meeting_memories
             WHERE meeting_id = 'meeting-1' AND kind = 'commitment'",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(
            rows,
            vec![(
                "Sam will send the revised pilot proposal".into(),
                "corrected".into()
            )]
        );
    }

    #[tokio::test]
    async fn regeneration_removes_only_stale_suggestions() {
        let pool = test_pool().await;
        seed_meeting(&pool, SUMMARY).await;
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();
        let decision = MeetingMemoriesRepository::list(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|memory| memory.kind == "decision")
            .unwrap();
        MeetingMemoriesRepository::review(
            &pool,
            &decision.id,
            ReviewMemoryInput {
                status: ReviewStatus::Confirmed,
                text: &decision.text,
                owner: None,
                due_date: None,
            },
        )
        .await
        .unwrap();

        let changed = serde_json::json!({
            "markdown": "## Key Decisions\n- A different generated decision.\n\n## Action Items\nNone noted.\n\n## Open Questions\nNone noted."
        })
        .to_string();
        sqlx::query("UPDATE summary_processes SET result = ? WHERE meeting_id = 'meeting-1'")
            .bind(changed)
            .execute(&pool)
            .await
            .unwrap();
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();

        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT text, review_status FROM meeting_memories WHERE kind = 'decision' ORDER BY text",
        )
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|(_, status)| status == "confirmed"));
        assert!(rows.iter().any(|(_, status)| status == "suggested"));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM meeting_memories WHERE kind != 'decision'"
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }

    #[tokio::test]
    async fn review_and_resolution_reject_invalid_states() {
        let pool = test_pool().await;
        seed_meeting(&pool, SUMMARY).await;
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();
        let memory = MeetingMemoriesRepository::list(&pool)
            .await
            .unwrap()
            .remove(0);

        let review_result = MeetingMemoriesRepository::review(
            &pool,
            &memory.id,
            ReviewMemoryInput {
                status: ReviewStatus::Suggested,
                text: &memory.text,
                owner: None,
                due_date: None,
            },
        )
        .await;
        assert!(review_result.is_err());
        assert!(
            MeetingMemoriesRepository::set_resolution(&pool, &memory.id, "later")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejected_suggestion_stays_suppressed_after_regeneration() {
        let pool = test_pool().await;
        seed_meeting(&pool, SUMMARY).await;
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();
        let commitment = MeetingMemoriesRepository::list(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|memory| memory.kind == "commitment")
            .unwrap();
        MeetingMemoriesRepository::review(
            &pool,
            &commitment.id,
            ReviewMemoryInput {
                status: ReviewStatus::Rejected,
                text: &commitment.text,
                owner: commitment.owner.as_deref(),
                due_date: commitment.due_date.as_deref(),
            },
        )
        .await
        .unwrap();

        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM meeting_memories
                 WHERE kind = 'commitment' AND review_status = 'rejected'",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
        assert!(MeetingMemoriesRepository::list(&pool)
            .await
            .unwrap()
            .iter()
            .all(|memory| memory.id != commitment.id));
    }

    #[tokio::test]
    async fn follow_up_review_only_accepts_confirmed_open_loops_and_resets_on_reopen() {
        let pool = test_pool().await;
        seed_meeting(&pool, SUMMARY).await;
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();
        let memories = MeetingMemoriesRepository::list(&pool).await.unwrap();
        let commitment = memories
            .iter()
            .find(|memory| memory.kind == "commitment")
            .unwrap();
        let decision = memories
            .iter()
            .find(|memory| memory.kind == "decision")
            .unwrap();

        assert!(
            MeetingMemoriesRepository::mark_follow_up_reviewed(&pool, &commitment.id)
                .await
                .is_err()
        );
        MeetingMemoriesRepository::review(
            &pool,
            &commitment.id,
            ReviewMemoryInput {
                status: ReviewStatus::Confirmed,
                text: &commitment.text,
                owner: commitment.owner.as_deref(),
                due_date: commitment.due_date.as_deref(),
            },
        )
        .await
        .unwrap();
        MeetingMemoriesRepository::mark_follow_up_reviewed(&pool, &commitment.id)
            .await
            .unwrap();
        assert!(
            MeetingMemoriesRepository::mark_follow_up_reviewed(&pool, &decision.id)
                .await
                .is_err()
        );

        MeetingMemoriesRepository::set_resolution(&pool, &commitment.id, "done")
            .await
            .unwrap();
        assert!(
            MeetingMemoriesRepository::mark_follow_up_reviewed(&pool, &commitment.id)
                .await
                .is_err()
        );
        MeetingMemoriesRepository::set_resolution(&pool, &commitment.id, "open")
            .await
            .unwrap();
        let reviewed_at = sqlx::query_scalar::<_, Option<String>>(
            "SELECT follow_up_reviewed_at FROM meeting_memories WHERE id = ?",
        )
        .bind(&commitment.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(reviewed_at.is_none());
    }

    #[tokio::test]
    async fn owner_aliases_are_learned_only_from_corrections_and_apply_to_future_sync() {
        let pool = test_pool().await;
        seed_meeting(&pool, SUMMARY).await;
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();
        assert!(MeetingMemoriesRepository::list_owner_aliases(&pool)
            .await
            .unwrap()
            .is_empty());

        let commitment = MeetingMemoriesRepository::list(&pool)
            .await
            .unwrap()
            .into_iter()
            .find(|memory| memory.kind == "commitment")
            .unwrap();
        MeetingMemoriesRepository::review(
            &pool,
            &commitment.id,
            ReviewMemoryInput {
                status: ReviewStatus::Corrected,
                text: &commitment.text,
                owner: Some("Samantha Lee"),
                due_date: commitment.due_date.as_deref(),
            },
        )
        .await
        .unwrap();
        assert_eq!(
            MeetingMemoriesRepository::list_owner_aliases(&pool)
                .await
                .unwrap(),
            vec![MemoryOwnerAliasModel {
                alias: "Sam".into(),
                canonical_name: "Samantha Lee".into(),
            }]
        );

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO meetings (id, title, created_at, updated_at)
             VALUES ('meeting-2', 'Follow-up', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();
        let result = serde_json::json!({
            "markdown": "## Action Items\n| Owner | Task | Due |\n| --- | --- | --- |\n| sam | Send the contract | Monday |"
        })
        .to_string();
        sqlx::query(
            "INSERT INTO summary_processes
             (meeting_id, status, created_at, updated_at, result)
             VALUES ('meeting-2', 'completed', ?, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .bind(result)
        .execute(&pool)
        .await
        .unwrap();
        MeetingMemoriesRepository::sync_all(&pool).await.unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT owner FROM meeting_memories WHERE meeting_id = 'meeting-2'",
            )
            .fetch_one(&pool)
            .await
            .unwrap(),
            "Samantha Lee"
        );

        MeetingMemoriesRepository::delete_owner_alias(&pool, "SAM")
            .await
            .unwrap();
        assert!(MeetingMemoriesRepository::list_owner_aliases(&pool)
            .await
            .unwrap()
            .is_empty());
    }

    #[test]
    fn weak_overlap_does_not_create_false_evidence() {
        let candidate = MemoryCandidate {
            kind: MemoryKind::Decision,
            text: "Move the launch to a different market".into(),
            owner: None,
            due_date: None,
            reference_excerpt: None,
        };
        let transcripts = vec![TranscriptEvidence {
            id: "transcript-1".into(),
            text: "The team briefly mentioned launch colors and then discussed lunch.".into(),
            timestamp: "00:01:00".into(),
            audio_start_time: Some(60.0),
            audio_end_time: Some(65.0),
        }];
        assert!(find_evidence(&candidate, &transcripts).is_none());
    }
}
