//! Autopilot scheduler: every 60 seconds, find due autopilots and dispatch them.

use api_types::{
    AgentTaskTrigger, Autopilot, AutopilotConcurrencyPolicy, AutopilotExecutionMode,
    AutopilotRunStatus,
};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::autopilots::AutopilotRepository;

/// Spawn the background autopilot scheduler.
pub fn spawn_scheduler(pool: PgPool) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = run_scheduler_tick(&pool).await {
                tracing::error!(?e, "autopilot scheduler tick failed");
            }
        }
    });
}

async fn run_scheduler_tick(pool: &PgPool) -> anyhow::Result<()> {
    // claim_due locks rows and advances next_run_at atomically.
    let due = AutopilotRepository::claim_due(pool).await?;

    for ap in due {
        let ap_clone = ap.clone();
        let pool_clone = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = dispatch_autopilot(&pool_clone, &ap_clone).await {
                tracing::error!(?e, autopilot_id = %ap_clone.id, "autopilot dispatch failed");
            }
        });
    }

    Ok(())
}

/// Dispatch a single autopilot: either run a squad pipeline, or create/find an
/// issue and enqueue a single-agent task.
pub async fn dispatch_autopilot(pool: &PgPool, ap: &Autopilot) -> anyhow::Result<()> {
    let run = AutopilotRepository::create_run(pool, ap.id).await?;

    let result = if let Some(squad_id) = ap.squad_id {
        do_dispatch_squad(pool, ap, squad_id, run.id).await
    } else if let Some(agent_id) = ap.agent_id {
        do_dispatch(pool, ap, agent_id, run.id).await
    } else {
        tracing::debug!(
            autopilot_id = %ap.id,
            "autopilot has neither squad nor agent, skipping"
        );
        let _ = AutopilotRepository::update_run(
            pool,
            run.id,
            AutopilotRunStatus::Skipped,
            None,
            None,
            Some("skipped: no squad_id or agent_id".to_string()),
        )
        .await;
        return Ok(());
    };

    if let Err(ref e) = result {
        let _ = AutopilotRepository::update_run(
            pool,
            run.id,
            AutopilotRunStatus::Failed,
            None,
            None,
            Some(e.to_string()),
        )
        .await;
    }

    result
}

async fn do_dispatch_squad(
    pool: &PgPool,
    ap: &Autopilot,
    squad_id: Uuid,
    run_id: Uuid,
) -> anyhow::Result<()> {
    use api_types::RunSquadRequest;

    use crate::{db::squads::SquadRepository, routes::squads::execute_squad_pipeline};

    let _ = AutopilotRepository::update_run(
        pool,
        run_id,
        AutopilotRunStatus::Running,
        None,
        None,
        None,
    )
    .await;

    let squad = SquadRepository::find_by_id(pool, squad_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("squad {squad_id} not found"))?;

    if squad.project_id != ap.project_id {
        anyhow::bail!("squad does not belong to autopilot project");
    }

    let result = execute_squad_pipeline(pool, &squad, &RunSquadRequest::default()).await?;

    let first_task = result.agent_task_ids.first().copied();
    let _ = AutopilotRepository::update_run(
        pool,
        run_id,
        AutopilotRunStatus::Completed,
        Some(result.issue_id),
        first_task,
        None,
    )
    .await;

    Ok(())
}

async fn do_dispatch(
    pool: &PgPool,
    ap: &Autopilot,
    agent_id: Uuid,
    run_id: Uuid,
) -> anyhow::Result<()> {
    if ap.concurrency_policy == AutopilotConcurrencyPolicy::Skip {
        let active_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM agent_tasks WHERE agent_id = $1 AND status IN ('queued','dispatched','running')"
        )
        .bind(agent_id)
        .fetch_one(pool)
        .await?;

        if active_count.0 > 0 {
            tracing::debug!(autopilot_id = %ap.id, %agent_id, "concurrency skip: active task exists");
            let _ = AutopilotRepository::update_run(
                pool,
                run_id,
                AutopilotRunStatus::Skipped,
                None,
                None,
                Some("skipped: active task running".to_string()),
            )
            .await;
            return Ok(());
        }
    }

    let _ = AutopilotRepository::update_run(
        pool,
        run_id,
        AutopilotRunStatus::Running,
        None,
        None,
        None,
    )
    .await;

    let issue_id = match ap.execution_mode {
        AutopilotExecutionMode::CreateIssue => {
            create_issue_from_template(pool, ap, agent_id).await?
        }
        AutopilotExecutionMode::RunOnly => {
            find_or_create_run_only_issue(pool, ap, agent_id).await?
        }
    };

    let task = crate::db::agent_tasks::AgentTaskRepository::enqueue(
        pool,
        None,
        agent_id,
        issue_id,
        AgentTaskTrigger::Autopilot,
        0,
        false,
        None,
        false,
        None,
        None,
    )
    .await?;

    let _ = AutopilotRepository::update_run(
        pool,
        run_id,
        AutopilotRunStatus::Completed,
        Some(issue_id),
        Some(task.data.id),
        None,
    )
    .await;

    Ok(())
}

async fn create_issue_from_template(
    pool: &PgPool,
    ap: &Autopilot,
    agent_id: Uuid,
) -> anyhow::Result<Uuid> {
    let date_str = Utc::now().format("%Y-%m-%d").to_string();
    let title = crate::db::autopilots::render_autopilot_template(&ap.issue_title_template, ap)
        .replace("{{date}}", &date_str);
    let description =
        crate::db::autopilots::render_autopilot_template(&ap.issue_description_template, ap);

    // Find default (lowest sort_order) status for this project.
    let status_id: (Uuid,) = sqlx::query_as(
        "SELECT id FROM project_statuses WHERE project_id = $1 ORDER BY sort_order ASC LIMIT 1",
    )
    .bind(ap.project_id)
    .fetch_one(pool)
    .await?;

    let issue_id = Uuid::new_v4();
    let sort_order: f64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order), 0) + 1.0 FROM issues WHERE project_id = $1",
    )
    .bind(ap.project_id)
    .fetch_one(pool)
    .await
    .unwrap_or(1.0);

    sqlx::query!(
        r#"
        INSERT INTO issues (
            id, project_id, status_id, title, description,
            sort_order, extension_metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, '{}')
        "#,
        issue_id,
        ap.project_id,
        status_id.0,
        title,
        description,
        sort_order
    )
    .execute(pool)
    .await?;

    // Assign agent
    sqlx::query!(
        "INSERT INTO issue_assignees (issue_id, agent_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        issue_id,
        agent_id
    )
    .execute(pool)
    .await?;

    Ok(issue_id)
}

async fn find_or_create_run_only_issue(
    pool: &PgPool,
    ap: &Autopilot,
    agent_id: Uuid,
) -> anyhow::Result<Uuid> {
    // Find the most recent open issue assigned to this agent in this project.
    let row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT i.id
        FROM issues i
        JOIN issue_assignees ia ON ia.issue_id = i.id AND ia.agent_id = $1
        WHERE i.project_id = $2
        ORDER BY i.updated_at DESC
        LIMIT 1
        "#,
    )
    .bind(agent_id)
    .bind(ap.project_id)
    .fetch_optional(pool)
    .await?;

    if let Some((issue_id,)) = row {
        Ok(issue_id)
    } else {
        create_issue_from_template(pool, ap, agent_id).await
    }
}
