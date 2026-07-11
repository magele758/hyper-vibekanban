use api_types::{DeleteResponse, FeishuBotBinding};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use super::get_txid;

#[derive(Debug, Error)]
pub enum FeishuError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Full binding including secrets (server-side only).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FeishuBotBindingFull {
    pub id: Uuid,
    pub project_id: Uuid,
    pub agent_id: Uuid,
    pub name: String,
    pub app_id: String,
    pub app_secret: String,
    pub encrypt_key: Option<String>,
    pub verification_token: Option<String>,
    pub callback_token: String,
    pub enabled: bool,
    pub reply_on_complete: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FeishuBotBindingFull {
    pub fn to_public(&self) -> FeishuBotBinding {
        FeishuBotBinding {
            id: self.id,
            project_id: self.project_id,
            agent_id: self.agent_id,
            name: self.name.clone(),
            app_id: self.app_id.clone(),
            has_app_secret: !self.app_secret.is_empty(),
            has_encrypt_key: self.encrypt_key.as_ref().is_some_and(|s| !s.is_empty()),
            has_verification_token: self
                .verification_token
                .as_ref()
                .is_some_and(|s| !s.is_empty()),
            callback_token: self.callback_token.clone(),
            enabled: self.enabled,
            reply_on_complete: self.reply_on_complete,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FeishuInboundMessage {
    pub id: Uuid,
    pub binding_id: Uuid,
    pub message_id: String,
    pub chat_id: String,
    pub open_id: Option<String>,
    pub text_content: String,
    pub agent_task_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub reply_status: String,
    pub reply_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub struct FeishuRepository;

impl FeishuRepository {
    pub async fn list_by_project(
        pool: &PgPool,
        project_id: Uuid,
    ) -> Result<Vec<FeishuBotBinding>, FeishuError> {
        let rows = sqlx::query_as::<_, FeishuBotBindingFull>(
            r#"
            SELECT id, project_id, agent_id, name, app_id, app_secret,
                   encrypt_key, verification_token, callback_token,
                   enabled, reply_on_complete, created_at, updated_at
            FROM feishu_bot_bindings
            WHERE project_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        Ok(rows.iter().map(|r| r.to_public()).collect())
    }

    pub async fn find_by_id(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<Option<FeishuBotBindingFull>, FeishuError> {
        let row = sqlx::query_as::<_, FeishuBotBindingFull>(
            r#"
            SELECT id, project_id, agent_id, name, app_id, app_secret,
                   encrypt_key, verification_token, callback_token,
                   enabled, reply_on_complete, created_at, updated_at
            FROM feishu_bot_bindings
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_callback_token(
        pool: &PgPool,
        token: &str,
    ) -> Result<Option<FeishuBotBindingFull>, FeishuError> {
        let row = sqlx::query_as::<_, FeishuBotBindingFull>(
            r#"
            SELECT id, project_id, agent_id, name, app_id, app_secret,
                   encrypt_key, verification_token, callback_token,
                   enabled, reply_on_complete, created_at, updated_at
            FROM feishu_bot_bindings
            WHERE callback_token = $1
            "#,
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn create(
        pool: &PgPool,
        id: Option<Uuid>,
        project_id: Uuid,
        agent_id: Uuid,
        name: String,
        app_id: String,
        app_secret: String,
        encrypt_key: Option<String>,
        verification_token: Option<String>,
        reply_on_complete: bool,
    ) -> Result<FeishuBotBinding, FeishuError> {
        let id = id.unwrap_or_else(Uuid::new_v4);
        let row = sqlx::query_as::<_, FeishuBotBindingFull>(
            r#"
            INSERT INTO feishu_bot_bindings (
                id, project_id, agent_id, name, app_id, app_secret,
                encrypt_key, verification_token, reply_on_complete
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, project_id, agent_id, name, app_id, app_secret,
                      encrypt_key, verification_token, callback_token,
                      enabled, reply_on_complete, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(project_id)
        .bind(agent_id)
        .bind(name)
        .bind(app_id)
        .bind(app_secret)
        .bind(encrypt_key)
        .bind(verification_token)
        .bind(reply_on_complete)
        .fetch_one(pool)
        .await?;
        Ok(row.to_public())
    }

    pub async fn update(
        pool: &PgPool,
        id: Uuid,
        name: Option<String>,
        agent_id: Option<Uuid>,
        app_id: Option<String>,
        app_secret: Option<String>,
        encrypt_key: Option<Option<String>>,
        verification_token: Option<Option<String>>,
        enabled: Option<bool>,
        reply_on_complete: Option<bool>,
    ) -> Result<FeishuBotBinding, FeishuError> {
        let existing = Self::find_by_id(pool, id)
            .await?
            .ok_or_else(|| FeishuError::Database(sqlx::Error::RowNotFound))?;

        let name = name.unwrap_or(existing.name);
        let agent_id = agent_id.unwrap_or(existing.agent_id);
        let app_id = app_id.unwrap_or(existing.app_id);
        let app_secret = app_secret.unwrap_or(existing.app_secret);
        let encrypt_key = match encrypt_key {
            Some(v) => v,
            None => existing.encrypt_key,
        };
        let verification_token = match verification_token {
            Some(v) => v,
            None => existing.verification_token,
        };
        let enabled = enabled.unwrap_or(existing.enabled);
        let reply_on_complete = reply_on_complete.unwrap_or(existing.reply_on_complete);

        let row = sqlx::query_as::<_, FeishuBotBindingFull>(
            r#"
            UPDATE feishu_bot_bindings
            SET name = $2,
                agent_id = $3,
                app_id = $4,
                app_secret = $5,
                encrypt_key = $6,
                verification_token = $7,
                enabled = $8,
                reply_on_complete = $9,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, agent_id, name, app_id, app_secret,
                      encrypt_key, verification_token, callback_token,
                      enabled, reply_on_complete, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(agent_id)
        .bind(app_id)
        .bind(app_secret)
        .bind(encrypt_key)
        .bind(verification_token)
        .bind(enabled)
        .bind(reply_on_complete)
        .fetch_one(pool)
        .await?;
        Ok(row.to_public())
    }

    pub async fn delete(pool: &PgPool, id: Uuid) -> Result<DeleteResponse, FeishuError> {
        let mut tx = super::begin_tx(pool).await?;
        sqlx::query("DELETE FROM feishu_bot_bindings WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        let txid = get_txid(&mut *tx).await?;
        tx.commit().await?;
        Ok(DeleteResponse { txid })
    }

    pub async fn rotate_callback_token(
        pool: &PgPool,
        id: Uuid,
    ) -> Result<FeishuBotBinding, FeishuError> {
        let new_token = Uuid::new_v4().to_string().replace('-', "");
        let row = sqlx::query_as::<_, FeishuBotBindingFull>(
            r#"
            UPDATE feishu_bot_bindings
            SET callback_token = $2, updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, agent_id, name, app_id, app_secret,
                      encrypt_key, verification_token, callback_token,
                      enabled, reply_on_complete, created_at, updated_at
            "#,
        )
        .bind(id)
        .bind(new_token)
        .fetch_one(pool)
        .await?;
        Ok(row.to_public())
    }

    /// Insert inbound message; returns None if duplicate (idempotent).
    pub async fn insert_inbound_if_new(
        pool: &PgPool,
        binding_id: Uuid,
        message_id: &str,
        chat_id: &str,
        open_id: Option<&str>,
        text_content: &str,
    ) -> Result<Option<FeishuInboundMessage>, FeishuError> {
        let row = sqlx::query_as::<_, FeishuInboundMessage>(
            r#"
            INSERT INTO feishu_inbound_messages (
                binding_id, message_id, chat_id, open_id, text_content
            )
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (binding_id, message_id) DO NOTHING
            RETURNING id, binding_id, message_id, chat_id, open_id, text_content,
                      agent_task_id, issue_id, reply_status, reply_error, created_at
            "#,
        )
        .bind(binding_id)
        .bind(message_id)
        .bind(chat_id)
        .bind(open_id)
        .bind(text_content)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    pub async fn link_inbound_task(
        pool: &PgPool,
        inbound_id: Uuid,
        issue_id: Uuid,
        agent_task_id: Uuid,
    ) -> Result<(), FeishuError> {
        sqlx::query(
            r#"
            UPDATE feishu_inbound_messages
            SET issue_id = $2, agent_task_id = $3
            WHERE id = $1
            "#,
        )
        .bind(inbound_id)
        .bind(issue_id)
        .bind(agent_task_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_pending_by_task(
        pool: &PgPool,
        agent_task_id: Uuid,
    ) -> Result<Option<(FeishuInboundMessage, FeishuBotBindingFull)>, FeishuError> {
        #[derive(sqlx::FromRow)]
        struct Row {
            // inbound
            inbound_id: Uuid,
            binding_id: Uuid,
            message_id: String,
            chat_id: String,
            open_id: Option<String>,
            text_content: String,
            agent_task_id: Option<Uuid>,
            issue_id: Option<Uuid>,
            reply_status: String,
            reply_error: Option<String>,
            inbound_created_at: DateTime<Utc>,
            // binding
            project_id: Uuid,
            agent_id: Uuid,
            name: String,
            app_id: String,
            app_secret: String,
            encrypt_key: Option<String>,
            verification_token: Option<String>,
            callback_token: String,
            enabled: bool,
            reply_on_complete: bool,
            binding_created_at: DateTime<Utc>,
            binding_updated_at: DateTime<Utc>,
        }

        let row = sqlx::query_as::<_, Row>(
            r#"
            SELECT
                m.id AS inbound_id,
                m.binding_id,
                m.message_id,
                m.chat_id,
                m.open_id,
                m.text_content,
                m.agent_task_id,
                m.issue_id,
                m.reply_status,
                m.reply_error,
                m.created_at AS inbound_created_at,
                b.project_id,
                b.agent_id,
                b.name,
                b.app_id,
                b.app_secret,
                b.encrypt_key,
                b.verification_token,
                b.callback_token,
                b.enabled,
                b.reply_on_complete,
                b.created_at AS binding_created_at,
                b.updated_at AS binding_updated_at
            FROM feishu_inbound_messages m
            JOIN feishu_bot_bindings b ON b.id = m.binding_id
            WHERE m.agent_task_id = $1
              AND m.reply_status = 'pending'
            LIMIT 1
            "#,
        )
        .bind(agent_task_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| {
            (
                FeishuInboundMessage {
                    id: r.inbound_id,
                    binding_id: r.binding_id,
                    message_id: r.message_id,
                    chat_id: r.chat_id,
                    open_id: r.open_id,
                    text_content: r.text_content,
                    agent_task_id: r.agent_task_id,
                    issue_id: r.issue_id,
                    reply_status: r.reply_status,
                    reply_error: r.reply_error,
                    created_at: r.inbound_created_at,
                },
                FeishuBotBindingFull {
                    id: r.binding_id,
                    project_id: r.project_id,
                    agent_id: r.agent_id,
                    name: r.name,
                    app_id: r.app_id,
                    app_secret: r.app_secret,
                    encrypt_key: r.encrypt_key,
                    verification_token: r.verification_token,
                    callback_token: r.callback_token,
                    enabled: r.enabled,
                    reply_on_complete: r.reply_on_complete,
                    created_at: r.binding_created_at,
                    updated_at: r.binding_updated_at,
                },
            )
        }))
    }

    pub async fn mark_reply(
        pool: &PgPool,
        inbound_id: Uuid,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), FeishuError> {
        sqlx::query(
            r#"
            UPDATE feishu_inbound_messages
            SET reply_status = $2, reply_error = $3
            WHERE id = $1
            "#,
        )
        .bind(inbound_id)
        .bind(status)
        .bind(error)
        .execute(pool)
        .await?;
        Ok(())
    }
}
