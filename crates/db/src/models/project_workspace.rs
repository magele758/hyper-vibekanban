use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::workspace::{Workspace, WorkspaceKind};

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProjectWorkspace {
    pub project_id: Uuid,
    pub workspace_id: Uuid,
    pub created_at: DateTime<Utc>,
}

impl ProjectWorkspace {
    pub async fn attach(
        pool: &SqlitePool,
        project_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            ProjectWorkspace,
            r#"INSERT INTO project_workspaces (project_id, workspace_id)
               VALUES ($1, $2)
               ON CONFLICT(project_id, workspace_id) DO UPDATE SET project_id = excluded.project_id
               RETURNING project_id as "project_id!: Uuid",
                         workspace_id as "workspace_id!: Uuid",
                         created_at as "created_at!: DateTime<Utc>""#,
            project_id,
            workspace_id
        )
        .fetch_one(pool)
        .await
    }

    pub async fn list_workspaces_for_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Workspace>, sqlx::Error> {
        sqlx::query_as!(
            Workspace,
            r#"SELECT w.id AS "id!: Uuid",
                      w.task_id AS "task_id: Uuid",
                      w.container_ref,
                      w.branch,
                      w.setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                      w.created_at AS "created_at!: DateTime<Utc>",
                      w.updated_at AS "updated_at!: DateTime<Utc>",
                      w.archived AS "archived!: bool",
                      w.pinned AS "pinned!: bool",
                      w.name,
                      w.worktree_deleted AS "worktree_deleted!: bool",
                      w.kind AS "kind!: WorkspaceKind"
               FROM workspaces w
               JOIN project_workspaces pw ON pw.workspace_id = w.id
               WHERE pw.project_id = $1
               ORDER BY w.updated_at DESC"#,
            project_id
        )
        .fetch_all(pool)
        .await
    }

    pub async fn count_by_project(pool: &SqlitePool, project_id: Uuid) -> Result<i64, sqlx::Error> {
        let row = sqlx::query_scalar!(
            r#"SELECT COUNT(*) as "count!: i64"
               FROM project_workspaces
               WHERE project_id = $1"#,
            project_id
        )
        .fetch_one(pool)
        .await?;
        Ok(row)
    }
}
