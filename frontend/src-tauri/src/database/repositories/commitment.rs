use crate::api::CommitmentItem;
use chrono::Utc;
use sqlx::{Error as SqlxError, SqlitePool};
use std::collections::HashSet;
use uuid::Uuid;

pub struct CommitmentsRepository;

impl CommitmentsRepository {
    /// Reconcile commitments with every meeting's current summary.
    ///
    /// For each meeting whose summary has a recognizable "Action Items" section:
    /// new items are added, items that disappeared are removed, and owner/due are
    /// refreshed — while each item's open/done status is preserved. Meetings whose
    /// summary has no parseable Action Items section are left untouched, so we
    /// never wipe commitments just because a format wasn't understood. Runs as a
    /// single transaction. Returns the total commitment count afterwards.
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
            // None => no Action Items section found => leave this meeting as-is.
            let Some(desired) = parse_action_items(&markdown) else {
                continue;
            };

            let desired_texts: HashSet<&str> = desired.iter().map(|i| i.text.as_str()).collect();

            // Remove commitments whose action item is no longer in the summary.
            let existing = sqlx::query_as::<_, (String, String)>(
                "SELECT id, text FROM commitments WHERE meeting_id = ?",
            )
            .bind(&meeting_id)
            .fetch_all(&mut *tx)
            .await?;

            for (id, text) in existing {
                if !desired_texts.contains(text.as_str()) {
                    sqlx::query("DELETE FROM commitments WHERE id = ?")
                        .bind(&id)
                        .execute(&mut *tx)
                        .await?;
                }
            }

            // Add new items; refresh owner/due on existing ones, keeping status.
            for item in &desired {
                let id = format!("commitment-{}", Uuid::new_v4());
                sqlx::query(
                    "INSERT INTO commitments
                     (id, meeting_id, text, owner, due_date, status, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, 'open', ?, ?)
                     ON CONFLICT(meeting_id, text) DO UPDATE SET
                        owner = excluded.owner,
                        due_date = excluded.due_date,
                        updated_at = excluded.updated_at",
                )
                .bind(&id)
                .bind(&meeting_id)
                .bind(&item.text)
                .bind(item.owner.as_deref())
                .bind(item.due.as_deref())
                .bind(&now)
                .bind(&now)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;

        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM commitments")
            .fetch_one(pool)
            .await?;
        Ok(count.max(0) as u64)
    }

    /// List all commitments across every meeting, open items first, then most
    /// recently updated. Joins the meeting title for display and navigation.
    pub async fn list(pool: &SqlitePool) -> Result<Vec<CommitmentItem>, SqlxError> {
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                String,
            ),
        >(
            "SELECT c.id, c.meeting_id, m.title, c.text, c.owner, c.due_date, c.status
             FROM commitments c
             JOIN meetings m ON c.meeting_id = m.id
             ORDER BY CASE c.status WHEN 'open' THEN 0 ELSE 1 END, c.updated_at DESC",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, meeting_id, meeting_title, text, owner, due_date, status)| CommitmentItem {
                    id,
                    meeting_id,
                    meeting_title,
                    text,
                    owner,
                    due_date,
                    status,
                },
            )
            .collect())
    }

    /// Update a single commitment's status (e.g. "open" or "done").
    pub async fn set_status(
        pool: &SqlitePool,
        id: &str,
        status: &str,
    ) -> Result<(), SqlxError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE commitments SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status)
            .bind(&now)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}

/// A single action item parsed out of a meeting summary.
struct ParsedActionItem {
    text: String,
    owner: Option<String>,
    due: Option<String>,
}

fn is_noise(value: &str) -> bool {
    let v = value.trim().trim_end_matches('.').trim().to_lowercase();
    v.is_empty()
        || v == "n/a"
        || v == "na"
        || v == "none"
        || v == "tbd"
        || v == "unspecified"
        || v.starts_with("none noted")
}

fn strip_md(value: &str) -> String {
    value.replace("**", "").replace('`', "").trim().to_string()
}

/// Pull the markdown summary text out of a `summary_processes.result` blob.
fn extract_summary_markdown(result: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(result).ok()?;
    if let Some(md) = value.get("markdown").and_then(|m| m.as_str()) {
        return Some(md.to_string());
    }
    value
        .get("english_cache")
        .and_then(|c| c.get("markdown"))
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
}

/// Parse the "Action Items" markdown table produced by the summary templates.
///
/// Returns `None` when no Action Items section is present (so callers can skip
/// reconciling against a format we don't understand), or `Some(items)` —
/// possibly empty — when the section exists.
fn parse_action_items(markdown: &str) -> Option<Vec<ParsedActionItem>> {
    let lines: Vec<&str> = markdown.lines().collect();

    let heading_idx = lines.iter().position(|line| {
        let normalized = line
            .trim()
            .trim_start_matches('#')
            .trim()
            .trim_matches('*')
            .trim()
            .trim_end_matches(':')
            .trim()
            .to_lowercase();
        normalized == "action items"
    })?;

    let mut rows: Vec<&str> = Vec::new();
    for line in lines.iter().skip(heading_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if rows.is_empty() {
                continue;
            }
            break;
        }
        if trimmed.starts_with('|') {
            rows.push(trimmed);
        } else {
            break;
        }
    }

    // Section exists but has no table (e.g. prose "None noted") => no items.
    if rows.len() < 2 {
        return Some(Vec::new());
    }

    let parse_cells = |row: &str| -> Vec<String> {
        row.trim()
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(strip_md)
            .collect()
    };

    let header: Vec<String> = parse_cells(rows[0]).iter().map(|h| h.to_lowercase()).collect();
    let find_col = |name: &str| header.iter().position(|h| h.contains(name));
    let task_col = find_col("task").unwrap_or(1);
    let owner_col = find_col("owner");
    let due_col = find_col("due");

    let mut items = Vec::new();
    for row in rows.iter().skip(1) {
        // Skip the markdown separator row (| --- | --- | ...).
        let is_separator =
            row.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')) && row.contains('-');
        if is_separator {
            continue;
        }

        let cells = parse_cells(row);
        let task = cells.get(task_col).map(|s| s.trim()).unwrap_or("").to_string();
        if task.is_empty() || is_noise(&task) {
            continue;
        }

        let owner = owner_col
            .and_then(|i| cells.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !is_noise(s));
        let due = due_col
            .and_then(|i| cells.get(i))
            .map(|s| s.trim().to_string())
            .filter(|s| !is_noise(s));

        items.push(ParsedActionItem {
            text: task,
            owner,
            due,
        });
    }

    Some(items)
}
