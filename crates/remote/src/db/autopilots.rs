use api_types::{
    Autopilot, AutopilotConcurrencyPolicy, AutopilotExecutionMode, AutopilotRun,
    AutopilotRunStatus, DeleteResponse, MutationResponse,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum AutopilotError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct AutopilotRepository;

impl AutopilotRepository {
    pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Autopilot>, AutopilotError> {
        let record = sqlx::query_as!(
            Autopilot,
            r#"
            SELECT
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                name            AS "name!",
                agent_id,
                enabled         AS "enabled!",
                execution_mode  AS "execution_mode!: AutopilotExecutionMode",
                cron_expression AS "cron_expression!",
                timezone        AS "timezone!",
                concurrency_policy AS "concurrency_policy!: AutopilotConcurrencyPolicy",
                issue_title_template AS "issue_title_template!",
                issue_description_template AS "issue_description_template!",
                next_run_at,
                last_run_at,
                created_at      AS "created_at!: DateTime<Utc>",
                updated_at      AS "updated_at!: DateTime<Utc>"
            FROM autopilots
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record)
    }

    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<Autopilot>, AutopilotError> {
        let records = sqlx::query_as!(
            Autopilot,
            r#"
            SELECT
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                name            AS "name!",
                agent_id,
                enabled         AS "enabled!",
                execution_mode  AS "execution_mode!: AutopilotExecutionMode",
                cron_expression AS "cron_expression!",
                timezone        AS "timezone!",
                concurrency_policy AS "concurrency_policy!: AutopilotConcurrencyPolicy",
                issue_title_template AS "issue_title_template!",
                issue_description_template AS "issue_description_template!",
                next_run_at,
                last_run_at,
                created_at      AS "created_at!: DateTime<Utc>",
                updated_at      AS "updated_at!: DateTime<Utc>"
            FROM autopilots
            WHERE project_id = $1
            ORDER BY name ASC
            "#,
            project_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }

    pub async fn create(
        pool: &PgPool,
        id: Option<Uuid>,
        project_id: Uuid,
        name: String,
        agent_id: Option<Uuid>,
        enabled: bool,
        execution_mode: AutopilotExecutionMode,
        cron_expression: String,
        timezone: String,
        concurrency_policy: AutopilotConcurrencyPolicy,
        issue_title_template: String,
        issue_description_template: String,
    ) -> Result<MutationResponse<Autopilot>, AutopilotError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let next_run_at = advance_cron_tz(&cron_expression, &timezone, Utc::now());
        let mut tx = super::begin_tx(pool).await?;

        let data = sqlx::query_as!(
            Autopilot,
            r#"
            INSERT INTO autopilots (
                id, project_id, name, agent_id, enabled,
                execution_mode, cron_expression, timezone,
                concurrency_policy, issue_title_template, issue_description_template,
                next_run_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                name            AS "name!",
                agent_id,
                enabled         AS "enabled!",
                execution_mode  AS "execution_mode!: AutopilotExecutionMode",
                cron_expression AS "cron_expression!",
                timezone        AS "timezone!",
                concurrency_policy AS "concurrency_policy!: AutopilotConcurrencyPolicy",
                issue_title_template AS "issue_title_template!",
                issue_description_template AS "issue_description_template!",
                next_run_at,
                last_run_at,
                created_at      AS "created_at!: DateTime<Utc>",
                updated_at      AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            project_id,
            name,
            agent_id,
            enabled,
            execution_mode as AutopilotExecutionMode,
            cron_expression,
            timezone,
            concurrency_policy as AutopilotConcurrencyPolicy,
            issue_title_template,
            issue_description_template,
            next_run_at,
        )
        .fetch_one(&mut *tx)
        .await?;

        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: Option<String>,
        agent_id: Option<Option<Uuid>>,
        enabled: Option<bool>,
        execution_mode: Option<AutopilotExecutionMode>,
        cron_expression: Option<String>,
        timezone: Option<String>,
        concurrency_policy: Option<AutopilotConcurrencyPolicy>,
        issue_title_template: Option<String>,
        issue_description_template: Option<String>,
    ) -> Result<MutationResponse<Autopilot>, AutopilotError> {
        let mut tx = super::begin_tx(pool).await?;

        let clear_agent = matches!(agent_id, Some(None));
        let set_agent = agent_id.flatten();

        let mut data = sqlx::query_as!(
            Autopilot,
            r#"
            UPDATE autopilots
            SET
                name = COALESCE($2, name),
                agent_id = CASE
                    WHEN $3 THEN NULL
                    WHEN $4::uuid IS NOT NULL THEN $4
                    ELSE agent_id
                END,
                enabled = COALESCE($5, enabled),
                execution_mode = COALESCE($6, execution_mode),
                cron_expression = COALESCE($7, cron_expression),
                timezone = COALESCE($8, timezone),
                concurrency_policy = COALESCE($9, concurrency_policy),
                issue_title_template = COALESCE($10, issue_title_template),
                issue_description_template = COALESCE($11, issue_description_template),
                updated_at = NOW()
            WHERE id = $1
            RETURNING
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                name            AS "name!",
                agent_id,
                enabled         AS "enabled!",
                execution_mode  AS "execution_mode!: AutopilotExecutionMode",
                cron_expression AS "cron_expression!",
                timezone        AS "timezone!",
                concurrency_policy AS "concurrency_policy!: AutopilotConcurrencyPolicy",
                issue_title_template AS "issue_title_template!",
                issue_description_template AS "issue_description_template!",
                next_run_at,
                last_run_at,
                created_at      AS "created_at!: DateTime<Utc>",
                updated_at      AS "updated_at!: DateTime<Utc>"
            "#,
            id,
            name,
            clear_agent,
            set_agent,
            enabled,
            execution_mode as Option<AutopilotExecutionMode>,
            cron_expression,
            timezone,
            concurrency_policy as Option<AutopilotConcurrencyPolicy>,
            issue_title_template,
            issue_description_template,
        )
        .fetch_one(&mut *tx)
        .await?;

        // Recalculate next_run_at when schedule fields change.
        if cron_expression.is_some() || timezone.is_some() {
            let next = advance_cron_tz(&data.cron_expression, &data.timezone, Utc::now());
            sqlx::query!(
                "UPDATE autopilots SET next_run_at = $2, updated_at = NOW() WHERE id = $1",
                id,
                next
            )
            .execute(&mut *tx)
            .await?;
            data.next_run_at = Some(next);
        }

        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(MutationResponse { data, txid })
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, AutopilotError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM autopilots WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    /// Claim due autopilots with row locks so multi-instance Remote won't double-dispatch.
    pub async fn claim_due(pool: &PgPool) -> Result<Vec<Autopilot>, AutopilotError> {
        let mut tx = super::begin_tx(pool).await?;
        let records = sqlx::query_as!(
            Autopilot,
            r#"
            SELECT
                id              AS "id!: Uuid",
                project_id      AS "project_id!: Uuid",
                name            AS "name!",
                agent_id,
                enabled         AS "enabled!",
                execution_mode  AS "execution_mode!: AutopilotExecutionMode",
                cron_expression AS "cron_expression!",
                timezone        AS "timezone!",
                concurrency_policy AS "concurrency_policy!: AutopilotConcurrencyPolicy",
                issue_title_template AS "issue_title_template!",
                issue_description_template AS "issue_description_template!",
                next_run_at,
                last_run_at,
                created_at      AS "created_at!: DateTime<Utc>",
                updated_at      AS "updated_at!: DateTime<Utc>"
            FROM autopilots
            WHERE enabled = TRUE
              AND next_run_at IS NOT NULL
              AND next_run_at <= NOW()
            ORDER BY next_run_at ASC
            FOR UPDATE SKIP LOCKED
            "#
        )
        .fetch_all(&mut *tx)
        .await?;

        // Advance each claimed row inside the same transaction before releasing locks.
        let now = Utc::now();
        let mut claimed = Vec::with_capacity(records.len());
        for ap in records {
            let next = advance_cron_tz(&ap.cron_expression, &ap.timezone, now);
            sqlx::query!(
                "UPDATE autopilots SET last_run_at = $2, next_run_at = $3, updated_at = NOW() WHERE id = $1",
                ap.id,
                now,
                next
            )
            .execute(&mut *tx)
            .await?;
            claimed.push(ap);
        }
        tx.commit().await?;
        Ok(claimed)
    }

    /// Find all enabled autopilots whose next_run_at <= now (read-only; prefer claim_due).
    pub async fn find_due(pool: &PgPool) -> Result<Vec<Autopilot>, AutopilotError> {
        Self::claim_due(pool).await
    }

    /// Advance next_run_at after a scheduled run.
    pub async fn advance_schedule(
        pool: &PgPool,
        id: Uuid,
        last_run_at: DateTime<Utc>,
        next_run_at: DateTime<Utc>,
    ) -> Result<(), AutopilotError> {
        sqlx::query!(
            "UPDATE autopilots SET last_run_at = $2, next_run_at = $3, updated_at = NOW() WHERE id = $1",
            id,
            last_run_at,
            next_run_at
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    // --- Runs ---

    pub async fn create_run(
        pool: &PgPool,
        autopilot_id: Uuid,
    ) -> Result<AutopilotRun, AutopilotError> {
        let record = sqlx::query_as!(
            AutopilotRun,
            r#"
            INSERT INTO autopilot_runs (autopilot_id)
            VALUES ($1)
            RETURNING
                id              AS "id!: Uuid",
                autopilot_id    AS "autopilot_id!: Uuid",
                status          AS "status!: AutopilotRunStatus",
                planned_at      AS "planned_at!: DateTime<Utc>",
                started_at,
                completed_at,
                issue_id,
                agent_task_id,
                error_message,
                created_at      AS "created_at!: DateTime<Utc>"
            "#,
            autopilot_id
        )
        .fetch_one(pool)
        .await?;

        Ok(record)
    }

    pub async fn update_run(
        pool: &PgPool,
        id: Uuid,
        status: AutopilotRunStatus,
        issue_id: Option<Uuid>,
        agent_task_id: Option<Uuid>,
        error_message: Option<String>,
    ) -> Result<(), AutopilotError> {
        let terminal = matches!(
            status,
            AutopilotRunStatus::Completed
                | AutopilotRunStatus::Failed
                | AutopilotRunStatus::Skipped
        );
        let starting = matches!(status, AutopilotRunStatus::Running);

        sqlx::query!(
            r#"
            UPDATE autopilot_runs
            SET
                status = $2,
                issue_id = COALESCE($3, issue_id),
                agent_task_id = COALESCE($4, agent_task_id),
                error_message = COALESCE($5, error_message),
                started_at = CASE WHEN $6 THEN COALESCE(started_at, NOW()) ELSE started_at END,
                completed_at = CASE WHEN $7 THEN NOW() ELSE completed_at END
            WHERE id = $1
            "#,
            id,
            status as AutopilotRunStatus,
            issue_id,
            agent_task_id,
            error_message,
            starting,
            terminal
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn list_runs(
        pool: &PgPool,
        autopilot_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AutopilotRun>, AutopilotError> {
        let records = sqlx::query_as!(
            AutopilotRun,
            r#"
            SELECT
                id              AS "id!: Uuid",
                autopilot_id    AS "autopilot_id!: Uuid",
                status          AS "status!: AutopilotRunStatus",
                planned_at      AS "planned_at!: DateTime<Utc>",
                started_at,
                completed_at,
                issue_id,
                agent_task_id,
                error_message,
                created_at      AS "created_at!: DateTime<Utc>"
            FROM autopilot_runs
            WHERE autopilot_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            autopilot_id,
            limit
        )
        .fetch_all(pool)
        .await?;

        Ok(records)
    }
}

/// Compute the next run time from a cron expression and a base time.
/// Supports common patterns:
///   `*/N * * * *`  -> every N minutes
/// Advance cron to the next run after `from`, respecting `timezone`.
/// Supports standard 5-field cron (`min hour dom mon dow`) plus `@hourly/@daily/@weekly`.
pub fn advance_cron(cron_expr: &str, from: DateTime<Utc>) -> DateTime<Utc> {
    advance_cron_tz(cron_expr, "UTC", from)
}

pub fn advance_cron_tz(cron_expr: &str, timezone: &str, from: DateTime<Utc>) -> DateTime<Utc> {
    use chrono::Duration;
    use chrono_tz::Tz;

    let expr = cron_expr.trim();
    match expr {
        "@hourly" => return from + Duration::hours(1),
        "@daily" | "@midnight" => return from + Duration::days(1),
        "@weekly" => return from + Duration::weeks(1),
        _ => {}
    }

    let tz: Tz = timezone.parse().unwrap_or(chrono_tz::UTC);
    // cron crate expects seconds field — prepend 0 for 5-field expressions.
    let schedule_expr = if expr.split_whitespace().count() == 5 {
        format!("0 {expr}")
    } else {
        expr.to_string()
    };

    if let Ok(schedule) = schedule_expr.parse::<cron::Schedule>() {
        let local = from.with_timezone(&tz);
        if let Some(next) = schedule.after(&local).next() {
            return next.with_timezone(&Utc);
        }
    }

    // Fallback heuristics for common shortcuts if cron parse fails.
    let parts: Vec<&str> = expr.split_whitespace().collect();
    if parts.len() >= 2 {
        let minute_field = parts[0];
        let hour_field = parts[1];
        if let Some(n_str) = minute_field.strip_prefix("*/")
            && let Ok(n) = n_str.parse::<i64>()
            && n > 0
        {
            return from + Duration::minutes(n);
        }
        if minute_field == "0" {
            if let Some(n_str) = hour_field.strip_prefix("*/")
                && let Ok(n) = n_str.parse::<i64>()
                && n > 0
            {
                return from + Duration::hours(n);
            }
            if hour_field == "*" {
                return from + Duration::hours(1);
            }
            if hour_field == "0" {
                return from + Duration::days(1);
            }
        }
    }

    from + Duration::hours(1)
}

/// Replace common autopilot template placeholders.
pub fn render_autopilot_template(template: &str, ap: &Autopilot) -> String {
    let date_str = Utc::now().format("%Y-%m-%d").to_string();
    template
        .replace("{{date}}", &date_str)
        .replace("{{autopilot_name}}", &ap.name)
        .replace("{{project_id}}", &ap.project_id.to_string())
        .replace(
            "{{agent_id}}",
            &ap.agent_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
        )
}
