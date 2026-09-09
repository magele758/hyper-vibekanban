use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

use super::repo::Repo;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ProjectRepo {
    pub id: Uuid,
    pub project_id: Uuid,
    pub repo_id: Uuid,
}

impl ProjectRepo {
    pub async fn attach(
        pool: &SqlitePool,
        project_id: Uuid,
        repo_id: Uuid,
    ) -> Result<Self, sqlx::Error> {
        let id = Uuid::new_v4();
        sqlx::query_as!(
            ProjectRepo,
            r#"INSERT INTO project_repos (id, project_id, repo_id)
               VALUES ($1, $2, $3)
               ON CONFLICT(project_id, repo_id) DO UPDATE SET project_id = excluded.project_id
               RETURNING id as "id!: Uuid",
                         project_id as "project_id!: Uuid",
                         repo_id as "repo_id!: Uuid""#,
            id,
            project_id,
            repo_id
        )
        .fetch_one(pool)
        .await
    }

    pub async fn detach(
        pool: &SqlitePool,
        project_id: Uuid,
        repo_id: Uuid,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!(
            r#"DELETE FROM project_repos
               WHERE project_id = $1 AND repo_id = $2"#,
            project_id,
            repo_id
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_repos_for_project(
        pool: &SqlitePool,
        project_id: Uuid,
    ) -> Result<Vec<Repo>, sqlx::Error> {
        sqlx::query_as!(
            Repo,
            r#"SELECT r.id as "id!: Uuid",
                      r.path,
                      r.name,
                      r.display_name,
                      r.setup_script,
                      r.cleanup_script,
                      r.archive_script,
                      r.copy_files,
                      r.parallel_setup_script as "parallel_setup_script!: bool",
                      r.dev_server_script,
                      r.default_target_branch,
                      r.default_working_dir,
                      r.is_git as "is_git!: bool",
                      r.created_at as "created_at!: DateTime<Utc>",
                      r.updated_at as "updated_at!: DateTime<Utc>"
               FROM repos r
               JOIN project_repos pr ON pr.repo_id = r.id
               WHERE pr.project_id = $1
               ORDER BY r.display_name ASC"#,
            project_id
        )
        .fetch_all(pool)
        .await
    }
}
