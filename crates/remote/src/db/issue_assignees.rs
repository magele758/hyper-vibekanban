use api_types::{DeleteResponse, IssueAssignee, MutationResponse};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum IssueAssigneeError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("exactly one of user_id, agent_id, or squad_id is required")]
    InvalidAssignee,
}

pub struct IssueAssigneeRepository;

impl IssueAssigneeRepository {
    fn map_row(
        id: Uuid,
        issue_id: Uuid,
        user_id: Option<Uuid>,
        agent_id: Option<Uuid>,
        squad_id: Option<Uuid>,
        assigned_at: DateTime<Utc>,
    ) -> IssueAssignee {
        IssueAssignee {
            id,
            issue_id,
            user_id,
            agent_id,
            squad_id,
            assigned_at,
        }
    }

    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<IssueAssignee>, IssueAssigneeError> {
        let record = sqlx::query!(
            r#"
            SELECT
                id          AS "id!: Uuid",
                issue_id    AS "issue_id!: Uuid",
                user_id,
                agent_id,
                squad_id,
                assigned_at AS "assigned_at!: DateTime<Utc>"
            FROM issue_assignees
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(pool)
        .await?;

        Ok(record.map(|r| {
            Self::map_row(
                r.id,
                r.issue_id,
                r.user_id,
                r.agent_id,
                r.squad_id,
                r.assigned_at,
            )
        }))
    }

    pub async fn list_by_issue(
        pool: &PgPool,
        issue_id: Uuid,
    ) -> Result<Vec<IssueAssignee>, IssueAssigneeError> {
        let records = sqlx::query!(
            r#"
            SELECT
                id          AS "id!: Uuid",
                issue_id    AS "issue_id!: Uuid",
                user_id,
                agent_id,
                squad_id,
                assigned_at AS "assigned_at!: DateTime<Utc>"
            FROM issue_assignees
            WHERE issue_id = $1
            "#,
            issue_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records
            .into_iter()
            .map(|r| {
                Self::map_row(
                    r.id,
                    r.issue_id,
                    r.user_id,
                    r.agent_id,
                    r.squad_id,
                    r.assigned_at,
                )
            })
            .collect())
    }

    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<IssueAssignee>, IssueAssigneeError> {
        let records = sqlx::query!(
            r#"
            SELECT
                id          AS "id!: Uuid",
                issue_id    AS "issue_id!: Uuid",
                user_id,
                agent_id,
                squad_id,
                assigned_at AS "assigned_at!: DateTime<Utc>"
            FROM issue_assignees
            WHERE issue_id IN (SELECT id FROM issues WHERE project_id = $1)
            "#,
            project_id
        )
        .fetch_all(pool)
        .await?;

        Ok(records
            .into_iter()
            .map(|r| {
                Self::map_row(
                    r.id,
                    r.issue_id,
                    r.user_id,
                    r.agent_id,
                    r.squad_id,
                    r.assigned_at,
                )
            })
            .collect())
    }

    pub async fn create(
        pool: &PgPool,
        id: Option<Uuid>,
        issue_id: Uuid,
        user_id: Option<Uuid>,
        agent_id: Option<Uuid>,
        squad_id: Option<Uuid>,
    ) -> Result<MutationResponse<IssueAssignee>, IssueAssigneeError> {
        let count = [user_id, agent_id, squad_id]
            .iter()
            .filter(|x| x.is_some())
            .count();
        if count != 1 {
            return Err(IssueAssigneeError::InvalidAssignee);
        }

        let id = id.unwrap_or_else(Uuid::new_v4);
        let mut tx = super::begin_tx(pool).await?;
        let row = sqlx::query!(
            r#"
            INSERT INTO issue_assignees (id, issue_id, user_id, agent_id, squad_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id          AS "id!: Uuid",
                issue_id    AS "issue_id!: Uuid",
                user_id,
                agent_id,
                squad_id,
                assigned_at AS "assigned_at!: DateTime<Utc>"
            "#,
            id,
            issue_id,
            user_id,
            agent_id,
            squad_id
        )
        .fetch_one(&mut *tx)
        .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;

        Ok(MutationResponse {
            data: Self::map_row(
                row.id,
                row.issue_id,
                row.user_id,
                row.agent_id,
                row.squad_id,
                row.assigned_at,
            ),
            txid,
        })
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, IssueAssigneeError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query!("DELETE FROM issue_assignees WHERE id = $1", id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }
}
