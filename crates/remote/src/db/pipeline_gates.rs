use api_types::PipelineHumanGate;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PipelineGateError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("gate not found")]
    NotFound,
    #[error("gate already decided")]
    AlreadyDecided,
    #[error("invalid decision")]
    InvalidDecision,
}

#[derive(Debug, FromRow)]
struct GateRow {
    id: Uuid,
    project_id: Uuid,
    issue_id: Uuid,
    squad_id: Option<Uuid>,
    gate_kind: String,
    local_workspace_id: Option<Uuid>,
    question: String,
    status: String,
    payload: Value,
    decision_note: Option<String>,
    decided_by: Option<Uuid>,
    decided_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<GateRow> for PipelineHumanGate {
    fn from(r: GateRow) -> Self {
        Self {
            id: r.id,
            project_id: r.project_id,
            issue_id: r.issue_id,
            squad_id: r.squad_id,
            gate_kind: r.gate_kind,
            local_workspace_id: r.local_workspace_id,
            question: r.question,
            status: r.status,
            payload: r.payload,
            decision_note: r.decision_note,
            decided_by: r.decided_by,
            decided_at: r.decided_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

pub struct PipelineGateRepository;

impl PipelineGateRepository {
    pub async fn create(
        pool: &PgPool,
        project_id: Uuid,
        issue_id: Uuid,
        squad_id: Option<Uuid>,
        gate_kind: &str,
        local_workspace_id: Option<Uuid>,
        question: &str,
        payload: Value,
    ) -> Result<PipelineHumanGate, PipelineGateError> {
        let record = sqlx::query_as::<_, GateRow>(
            r#"
            INSERT INTO pipeline_human_gates (
                project_id, issue_id, squad_id, gate_kind,
                local_workspace_id, question, payload
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING
                id, project_id, issue_id, squad_id, gate_kind,
                local_workspace_id, question, status, payload,
                decision_note, decided_by, decided_at, created_at, updated_at
            "#,
        )
        .bind(project_id)
        .bind(issue_id)
        .bind(squad_id)
        .bind(gate_kind)
        .bind(local_workspace_id)
        .bind(question)
        .bind(payload)
        .fetch_one(pool)
        .await?;
        Ok(record.into())
    }

    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<PipelineHumanGate>, PipelineGateError> {
        let record = sqlx::query_as::<_, GateRow>(
            r#"
            SELECT
                id, project_id, issue_id, squad_id, gate_kind,
                local_workspace_id, question, status, payload,
                decision_note, decided_by, decided_at, created_at, updated_at
            FROM pipeline_human_gates
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(record.map(Into::into))
    }

    pub async fn respond(
        pool: &PgPool,
        id: Uuid,
        user_id: Uuid,
        decision: &str,
        note: Option<&str>,
    ) -> Result<PipelineHumanGate, PipelineGateError> {
        let status = match decision.trim().to_lowercase().as_str() {
            "approve" | "approved" | "yes" => "approved",
            "reject" | "rejected" | "no" => "rejected",
            _ => return Err(PipelineGateError::InvalidDecision),
        };

        let existing = Self::find_by_id(pool, id)
            .await?
            .ok_or(PipelineGateError::NotFound)?;
        if existing.status != "pending" {
            return Err(PipelineGateError::AlreadyDecided);
        }

        let record = sqlx::query_as::<_, GateRow>(
            r#"
            UPDATE pipeline_human_gates
            SET status = $2,
                decision_note = $3,
                decided_by = $4,
                decided_at = NOW(),
                updated_at = NOW()
            WHERE id = $1 AND status = 'pending'
            RETURNING
                id, project_id, issue_id, squad_id, gate_kind,
                local_workspace_id, question, status, payload,
                decision_note, decided_by, decided_at, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(note)
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(PipelineGateError::AlreadyDecided)?;

        Ok(record.into())
    }

    pub async fn expire(pool: &PgPool, id: Uuid) -> Result<(), PipelineGateError> {
        sqlx::query(
            r#"
            UPDATE pipeline_human_gates
            SET status = 'expired', updated_at = NOW(), decided_at = NOW()
            WHERE id = $1 AND status = 'pending'
            "#,
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }
}
