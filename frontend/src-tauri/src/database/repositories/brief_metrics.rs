use crate::database::models::LocalBriefMetricsModel;
use chrono::Utc;
use sqlx::{Error as SqlxError, SqlitePool};
use uuid::Uuid;

pub struct BriefMetricsRepository;

impl BriefMetricsRepository {
    pub async fn get(pool: &SqlitePool) -> Result<LocalBriefMetricsModel, SqlxError> {
        let (enabled, brief_open_count, source_open_count) = sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT p.enabled,
                        SUM(CASE WHEN e.event_type = 'brief_opened' THEN 1 ELSE 0 END),
                        SUM(CASE WHEN e.event_type = 'source_opened' THEN 1 ELSE 0 END)
                 FROM brief_usage_preferences p
                 LEFT JOIN brief_usage_events e ON 1 = 1
                 WHERE p.id = 1
                 GROUP BY p.enabled",
        )
        .fetch_one(pool)
        .await?;
        Ok(LocalBriefMetricsModel {
            enabled: enabled == 1,
            brief_open_count,
            source_open_count,
        })
    }

    pub async fn set_enabled(pool: &SqlitePool, enabled: bool) -> Result<(), SqlxError> {
        sqlx::query("UPDATE brief_usage_preferences SET enabled = ?, updated_at = ? WHERE id = 1")
            .bind(enabled)
            .bind(Utc::now().to_rfc3339())
            .execute(pool)
            .await?;
        Ok(())
    }

    pub async fn record(
        pool: &SqlitePool,
        event_type: &str,
        space_id: &str,
        memory_id: Option<&str>,
    ) -> Result<bool, SqlxError> {
        if !matches!(event_type, "brief_opened" | "source_opened") {
            return Err(SqlxError::Protocol("invalid brief usage event".into()));
        }
        if space_id.trim().is_empty() {
            return Err(SqlxError::Protocol("space id is required".into()));
        }
        let enabled = sqlx::query_scalar::<_, i64>(
            "SELECT enabled FROM brief_usage_preferences WHERE id = 1",
        )
        .fetch_one(pool)
        .await?;
        if enabled != 1 {
            return Ok(false);
        }

        sqlx::query(
            "INSERT INTO brief_usage_events (id, event_type, space_id, memory_id, occurred_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(format!("brief-event-{}", Uuid::new_v4()))
        .bind(event_type)
        .bind(space_id)
        .bind(memory_id)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
        Ok(true)
    }

    pub async fn clear(pool: &SqlitePool) -> Result<(), SqlxError> {
        sqlx::query("DELETE FROM brief_usage_events")
            .execute(pool)
            .await?;
        Ok(())
    }
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

    #[tokio::test]
    async fn disabled_by_default_and_does_not_record() {
        let pool = test_pool().await;
        let initial = BriefMetricsRepository::get(&pool).await.unwrap();
        assert!(!initial.enabled);
        assert!(
            !BriefMetricsRepository::record(&pool, "brief_opened", "space-1", None)
                .await
                .unwrap()
        );
        assert_eq!(BriefMetricsRepository::get(&pool).await.unwrap(), initial);
    }

    #[tokio::test]
    async fn records_only_valid_events_after_opt_in_and_can_clear() {
        let pool = test_pool().await;
        BriefMetricsRepository::set_enabled(&pool, true)
            .await
            .unwrap();
        assert!(
            BriefMetricsRepository::record(&pool, "brief_opened", "space-1", None)
                .await
                .unwrap()
        );
        assert!(BriefMetricsRepository::record(
            &pool,
            "source_opened",
            "space-1",
            Some("memory-1"),
        )
        .await
        .unwrap());
        assert!(
            BriefMetricsRepository::record(&pool, "unknown", "space-1", None)
                .await
                .is_err()
        );

        let metrics = BriefMetricsRepository::get(&pool).await.unwrap();
        assert!(metrics.enabled);
        assert_eq!(metrics.brief_open_count, 1);
        assert_eq!(metrics.source_open_count, 1);

        BriefMetricsRepository::clear(&pool).await.unwrap();
        let cleared = BriefMetricsRepository::get(&pool).await.unwrap();
        assert!(cleared.enabled);
        assert_eq!(cleared.brief_open_count, 0);
        assert_eq!(cleared.source_open_count, 0);
    }
}
