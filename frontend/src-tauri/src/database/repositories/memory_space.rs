use crate::database::models::{
    ConversationBriefModel, MeetingMemoryModel, MeetingSpaceAssignmentModel, MemorySpaceModel,
};
use chrono::Utc;
use sqlx::{Error as SqlxError, SqlitePool};
use uuid::Uuid;

pub struct MemorySpacesRepository;

impl MemorySpacesRepository {
    pub async fn create(pool: &SqlitePool, name: &str) -> Result<MemorySpaceModel, SqlxError> {
        let name = validate_name(name)?;
        let id = format!("space-{}", Uuid::new_v4());
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO memory_spaces (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
        Self::get(pool, &id).await
    }

    pub async fn rename(
        pool: &SqlitePool,
        id: &str,
        name: &str,
    ) -> Result<MemorySpaceModel, SqlxError> {
        let name = validate_name(name)?;
        let result = sqlx::query("UPDATE memory_spaces SET name = ?, updated_at = ? WHERE id = ?")
            .bind(name)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(SqlxError::RowNotFound);
        }
        Self::get(pool, id).await
    }

    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), SqlxError> {
        let mut transaction = pool.begin().await?;
        sqlx::query("DELETE FROM meeting_space_assignments WHERE space_id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        let result = sqlx::query("DELETE FROM memory_spaces WHERE id = ?")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
        if result.rows_affected() == 0 {
            return Err(SqlxError::RowNotFound);
        }
        transaction.commit().await
    }

    pub async fn list(pool: &SqlitePool) -> Result<Vec<MemorySpaceModel>, SqlxError> {
        sqlx::query_as::<_, MemorySpaceModel>(
            "SELECT s.id, s.name, COUNT(a.meeting_id) AS meeting_count,
                    s.created_at, s.updated_at
             FROM memory_spaces s
             LEFT JOIN meeting_space_assignments a ON a.space_id = s.id
             GROUP BY s.id, s.name, s.created_at, s.updated_at
             ORDER BY s.updated_at DESC, s.name COLLATE NOCASE",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn assignments(
        pool: &SqlitePool,
    ) -> Result<Vec<MeetingSpaceAssignmentModel>, SqlxError> {
        sqlx::query_as::<_, MeetingSpaceAssignmentModel>(
            "SELECT m.id AS meeting_id, m.title AS meeting_title, a.space_id
             FROM meetings m
             LEFT JOIN meeting_space_assignments a ON a.meeting_id = m.id
             ORDER BY m.updated_at DESC",
        )
        .fetch_all(pool)
        .await
    }

    pub async fn assign(
        pool: &SqlitePool,
        meeting_id: &str,
        space_id: Option<&str>,
    ) -> Result<(), SqlxError> {
        let mut transaction = pool.begin().await?;
        let meeting_exists =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meetings WHERE id = ?")
                .bind(meeting_id)
                .fetch_one(&mut *transaction)
                .await?;
        if meeting_exists == 0 {
            return Err(SqlxError::RowNotFound);
        }

        match space_id {
            Some(space_id) => {
                let space_exists =
                    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM memory_spaces WHERE id = ?")
                        .bind(space_id)
                        .fetch_one(&mut *transaction)
                        .await?;
                if space_exists == 0 {
                    return Err(SqlxError::RowNotFound);
                }
                let now = Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT INTO meeting_space_assignments (meeting_id, space_id, assigned_at)
                     VALUES (?, ?, ?)
                     ON CONFLICT(meeting_id) DO UPDATE SET
                       space_id = excluded.space_id,
                       assigned_at = excluded.assigned_at",
                )
                .bind(meeting_id)
                .bind(space_id)
                .bind(&now)
                .execute(&mut *transaction)
                .await?;
                sqlx::query("UPDATE memory_spaces SET updated_at = ? WHERE id = ?")
                    .bind(now)
                    .bind(space_id)
                    .execute(&mut *transaction)
                    .await?;
            }
            None => {
                sqlx::query("DELETE FROM meeting_space_assignments WHERE meeting_id = ?")
                    .bind(meeting_id)
                    .execute(&mut *transaction)
                    .await?;
            }
        }
        transaction.commit().await
    }

    pub async fn brief(
        pool: &SqlitePool,
        space_id: &str,
    ) -> Result<ConversationBriefModel, SqlxError> {
        let space = Self::get(pool, space_id).await?;
        let memories = sqlx::query_as::<_, MeetingMemoryModel>(
            "SELECT mm.id, mm.meeting_id, m.title AS meeting_title, mm.kind, mm.text,
                    mm.suggested_text, mm.owner, mm.due_date, mm.review_status,
                    mm.resolution_status, mm.evidence_confidence,
                    mm.source_transcript_id, mm.source_excerpt, mm.source_timestamp,
                    mm.source_audio_start_time, mm.source_audio_end_time,
                    mm.created_at, mm.updated_at, mm.reviewed_at,
                    mm.follow_up_reviewed_at
             FROM meeting_memories mm
             JOIN meetings m ON m.id = mm.meeting_id
             JOIN meeting_space_assignments a ON a.meeting_id = mm.meeting_id
             WHERE a.space_id = ?
               AND mm.review_status IN ('confirmed', 'corrected')
             ORDER BY mm.updated_at DESC",
        )
        .bind(space_id)
        .fetch_all(pool)
        .await?;

        let mut decisions = Vec::new();
        let mut commitments = Vec::new();
        let mut open_questions = Vec::new();
        for memory in memories {
            match memory.kind.as_str() {
                "decision" => decisions.push(memory),
                "commitment" if memory.resolution_status == "open" => commitments.push(memory),
                "open_question" if memory.resolution_status == "open" => {
                    open_questions.push(memory)
                }
                _ => {}
            }
        }
        Ok(ConversationBriefModel {
            space,
            decisions,
            commitments,
            open_questions,
        })
    }

    async fn get(pool: &SqlitePool, id: &str) -> Result<MemorySpaceModel, SqlxError> {
        sqlx::query_as::<_, MemorySpaceModel>(
            "SELECT s.id, s.name, COUNT(a.meeting_id) AS meeting_count,
                    s.created_at, s.updated_at
             FROM memory_spaces s
             LEFT JOIN meeting_space_assignments a ON a.space_id = s.id
             WHERE s.id = ?
             GROUP BY s.id, s.name, s.created_at, s.updated_at",
        )
        .bind(id)
        .fetch_one(pool)
        .await
    }
}

fn validate_name(name: &str) -> Result<&str, SqlxError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(SqlxError::Protocol(
            "space name must contain between 1 and 100 characters".into(),
        ));
    }
    Ok(name)
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
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn seed_meeting(pool: &SqlitePool) {
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
    }

    async fn seed_memory(pool: &SqlitePool, id: &str, kind: &str, review: &str, resolution: &str) {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO meeting_memories
             (id, meeting_id, kind, text, suggested_text, source_fingerprint,
              review_status, resolution_status, created_at, updated_at)
             VALUES (?, 'meeting-1', ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(kind)
        .bind(format!("Text for {id}"))
        .bind(format!("Text for {id}"))
        .bind(id)
        .bind(review)
        .bind(resolution)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn creates_lists_and_assigns_spaces() {
        let pool = test_pool().await;
        seed_meeting(&pool).await;
        let space = MemorySpacesRepository::create(&pool, " Acme ")
            .await
            .unwrap();
        assert_eq!(space.name, "Acme");
        MemorySpacesRepository::assign(&pool, "meeting-1", Some(&space.id))
            .await
            .unwrap();
        assert_eq!(
            MemorySpacesRepository::list(&pool).await.unwrap()[0].meeting_count,
            1
        );
        assert_eq!(
            MemorySpacesRepository::assignments(&pool).await.unwrap()[0].space_id,
            Some(space.id)
        );
    }

    #[tokio::test]
    async fn brief_excludes_suggestions_and_closed_loops() {
        let pool = test_pool().await;
        seed_meeting(&pool).await;
        let space = MemorySpacesRepository::create(&pool, "Acme").await.unwrap();
        MemorySpacesRepository::assign(&pool, "meeting-1", Some(&space.id))
            .await
            .unwrap();
        seed_memory(&pool, "confirmed-decision", "decision", "confirmed", "open").await;
        seed_memory(&pool, "suggested-decision", "decision", "suggested", "open").await;
        seed_memory(&pool, "open-commitment", "commitment", "corrected", "open").await;
        seed_memory(&pool, "done-commitment", "commitment", "confirmed", "done").await;
        seed_memory(&pool, "open-question", "open_question", "confirmed", "open").await;

        let brief = MemorySpacesRepository::brief(&pool, &space.id)
            .await
            .unwrap();
        assert_eq!(brief.decisions.len(), 1);
        assert_eq!(brief.commitments.len(), 1);
        assert_eq!(brief.open_questions.len(), 1);
        assert!(brief
            .decisions
            .iter()
            .all(|memory| memory.review_status != "suggested"));
    }

    #[tokio::test]
    async fn rejects_missing_entities_and_invalid_names() {
        let pool = test_pool().await;
        assert!(MemorySpacesRepository::create(&pool, "  ").await.is_err());
        assert!(MemorySpacesRepository::assign(&pool, "missing", None)
            .await
            .is_err());
        seed_meeting(&pool).await;
        assert!(
            MemorySpacesRepository::assign(&pool, "meeting-1", Some("missing"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn renames_and_deletes_space_with_assignments() {
        let pool = test_pool().await;
        seed_meeting(&pool).await;
        let space = MemorySpacesRepository::create(&pool, "Acme").await.unwrap();
        MemorySpacesRepository::assign(&pool, "meeting-1", Some(&space.id))
            .await
            .unwrap();

        let renamed = MemorySpacesRepository::rename(&pool, &space.id, " Acme North ")
            .await
            .unwrap();
        assert_eq!(renamed.name, "Acme North");
        assert!(MemorySpacesRepository::rename(&pool, &space.id, " ")
            .await
            .is_err());

        MemorySpacesRepository::delete(&pool, &space.id)
            .await
            .unwrap();
        assert!(MemorySpacesRepository::list(&pool)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            MemorySpacesRepository::assignments(&pool).await.unwrap()[0].space_id,
            None
        );
        assert!(MemorySpacesRepository::delete(&pool, &space.id)
            .await
            .is_err());
    }
}
