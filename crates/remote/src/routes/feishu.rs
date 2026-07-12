use api_types::{
    AgentTaskTrigger, CreateFeishuBotBindingRequest, DeleteResponse, FeishuBotBinding,
    ListFeishuBotBindingsQuery, ListFeishuBotBindingsResponse, UpdateFeishuBotBindingRequest,
};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_project_access,
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{
        agent_tasks::AgentTaskRepository,
        agents::AgentRepository,
        feishu::{FeishuBotBindingFull, FeishuRepository},
    },
    feishu::{FeishuClient, decrypt_event, verify_signature},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/feishu/bindings", get(list_bindings).post(create_binding))
        .route(
            "/feishu/bindings/{id}",
            put(update_binding).delete(delete_binding),
        )
        .route(
            "/feishu/bindings/{id}/rotate-token",
            post(rotate_callback_token),
        )
}

/// Public Feishu event ingress — no session auth.
pub fn public_router() -> Router<AppState> {
    Router::new().route("/feishu/events/{token}", post(feishu_events))
}

#[instrument(name = "feishu.list", skip(state, ctx), fields(user_id = %ctx.user.id))]
async fn list_bindings(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListFeishuBotBindingsQuery>,
) -> Result<Json<ListFeishuBotBindingsResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, query.project_id).await?;
    let bindings = FeishuRepository::list_by_project(state.pool(), query.project_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list feishu bindings");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list feishu bindings",
            )
        })?;
    Ok(Json(ListFeishuBotBindingsResponse { bindings }))
}

#[instrument(name = "feishu.create", skip(state, ctx, payload), fields(user_id = %ctx.user.id))]
async fn create_binding(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateFeishuBotBindingRequest>,
) -> Result<Json<FeishuBotBinding>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;

    let agent = AgentRepository::find_by_id(state.pool(), payload.agent_id)
        .await
        .map_err(|e| db_error(e, "failed to load agent"))?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;
    if agent.project_id != payload.project_id {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "agent does not belong to this project",
        ));
    }

    if payload.app_id.trim().is_empty() || payload.app_secret.trim().is_empty() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "app_id and app_secret are required",
        ));
    }

    let binding = FeishuRepository::create(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.agent_id,
        payload.name.unwrap_or_else(|| "飞书机器人".to_string()),
        payload.app_id.trim().to_string(),
        payload.app_secret,
        empty_to_none(payload.encrypt_key),
        empty_to_none(payload.verification_token),
        payload.reply_on_complete.unwrap_or(true),
    )
    .await
    .map_err(|e| db_error(e, "failed to create feishu binding"))?;

    Ok(Json(binding))
}

#[instrument(name = "feishu.update", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn update_binding(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateFeishuBotBindingRequest>,
) -> Result<Json<FeishuBotBinding>, ErrorResponse> {
    let existing = FeishuRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load feishu binding");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load binding")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "feishu binding not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, existing.project_id).await?;

    if let Some(agent_id) = payload.agent_id {
        let agent = AgentRepository::find_by_id(state.pool(), agent_id)
            .await
            .map_err(|e| db_error(e, "failed to load agent"))?
            .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "agent not found"))?;
        if agent.project_id != existing.project_id {
            return Err(ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "agent does not belong to this project",
            ));
        }
    }

    let binding = FeishuRepository::update(
        state.pool(),
        id,
        payload.name,
        payload.agent_id,
        payload.app_id,
        payload.app_secret,
        payload.encrypt_key.map(|v| empty_to_none(v)),
        payload.verification_token.map(|v| empty_to_none(v)),
        payload.enabled,
        payload.reply_on_complete,
    )
    .await
    .map_err(|e| db_error(e, "failed to update feishu binding"))?;

    Ok(Json(binding))
}

#[instrument(name = "feishu.delete", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn delete_binding(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let existing = FeishuRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load feishu binding");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load binding")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "feishu binding not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, existing.project_id).await?;

    let response = FeishuRepository::delete(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to delete feishu binding");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

#[instrument(name = "feishu.rotate_token", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn rotate_callback_token(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<FeishuBotBinding>, ErrorResponse> {
    let existing = FeishuRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load feishu binding");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load binding")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "feishu binding not found"))?;

    ensure_project_access(state.pool(), ctx.user.id, existing.project_id).await?;

    let updated = FeishuRepository::rotate_callback_token(state.pool(), id)
        .await
        .map_err(|e| db_error(e, "failed to rotate callback token"))?;

    Ok(Json(updated))
}

fn empty_to_none(v: Option<String>) -> Option<String> {
    v.and_then(|s| {
        let t = s.trim().to_string();
        if t.is_empty() { None } else { Some(t) }
    })
}

/// Public: POST /v1/feishu/events/{callback_token}
#[instrument(name = "feishu.events", skip(state, body, headers), fields(token = %token))]
async fn feishu_events(
    State(state): State<AppState>,
    Path(token): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ErrorResponse> {
    let binding = FeishuRepository::find_by_callback_token(state.pool(), &token)
        .await
        .map_err(|e| {
            tracing::error!(?e, "feishu callback token lookup failed");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "feishu binding not found"))?;

    if !binding.enabled {
        return Ok(Json(json!({})));
    }

    if let Some(encrypt_key) = binding.encrypt_key.as_deref().filter(|s| !s.is_empty()) {
        let timestamp = header_str(&headers, "x-lark-request-timestamp");
        let nonce = header_str(&headers, "x-lark-request-nonce");
        let signature = header_str(&headers, "x-lark-signature");
        // URL verification may arrive without signature headers — still decrypt.
        if let (Some(ts), Some(n), Some(sig)) = (timestamp, nonce, signature)
            && !verify_signature(encrypt_key, ts, n, &body, sig)
        {
            return Err(ErrorResponse::new(
                StatusCode::UNAUTHORIZED,
                "invalid feishu signature",
            ));
        }
    }

    let raw: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);

    // Encrypted envelope: { "encrypt": "..." }
    let event_json = if let Some(enc) = raw.get("encrypt").and_then(|v| v.as_str()) {
        let Some(encrypt_key) = binding.encrypt_key.as_deref().filter(|s| !s.is_empty()) else {
            return Err(ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "encrypted event but binding has no encrypt_key",
            ));
        };
        let plain = decrypt_event(encrypt_key, enc).map_err(|e| {
            tracing::warn!(?e, "feishu decrypt failed");
            ErrorResponse::new(StatusCode::BAD_REQUEST, "failed to decrypt feishu event")
        })?;
        serde_json::from_str(&plain).map_err(|_| {
            ErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "decrypted feishu payload is not JSON",
            )
        })?
    } else {
        raw
    };

    // URL verification challenge
    if event_json.get("type").and_then(|v| v.as_str()) == Some("url_verification") {
        if let Some(expected) = binding
            .verification_token
            .as_deref()
            .filter(|s| !s.is_empty())
        {
            let token = event_json
                .get("token")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if token != expected {
                return Err(ErrorResponse::new(
                    StatusCode::UNAUTHORIZED,
                    "invalid verification token",
                ));
            }
        }
        let challenge = event_json.get("challenge").cloned().unwrap_or(Value::Null);
        return Ok(Json(json!({ "challenge": challenge })));
    }

    // Verification token on event payloads (schema 1.0 / 2.0)
    if let Some(expected) = binding
        .verification_token
        .as_deref()
        .filter(|s| !s.is_empty())
    {
        let got = event_json
            .pointer("/header/token")
            .or_else(|| event_json.get("token"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !got.is_empty() && got != expected {
            return Err(ErrorResponse::new(
                StatusCode::UNAUTHORIZED,
                "invalid verification token",
            ));
        }
    }

    let event_type = event_json
        .pointer("/header/event_type")
        .and_then(|v| v.as_str())
        .or_else(|| event_json.get("type").and_then(|v| v.as_str()))
        .unwrap_or("");

    // Schema 1.0 wraps event under "event" with type "event_callback"
    let is_message = matches!(
        event_type,
        "im.message.receive_v1" | "event_callback" | "message"
    ) || event_json.pointer("/event/message").is_some()
        || event_json.pointer("/event/message_type").is_some();

    if is_message {
        let pool = state.pool().clone();
        let binding_clone = binding.clone();
        let event_clone = event_json.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_message_event(&pool, &binding_clone, &event_clone).await {
                tracing::error!(?e, binding_id = %binding_clone.id, "feishu message handler failed");
            }
        });
    } else {
        tracing::debug!(event_type, "ignoring unhandled feishu event");
    }

    Ok(Json(json!({})))
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

#[derive(Debug, Deserialize)]
struct TextContent {
    text: Option<String>,
}

async fn handle_message_event(
    pool: &sqlx::PgPool,
    binding: &FeishuBotBindingFull,
    event_json: &Value,
) -> anyhow::Result<()> {
    let sender_type = event_json
        .pointer("/event/sender/sender_type")
        .and_then(|v| v.as_str())
        .unwrap_or("user");
    if sender_type == "app" {
        tracing::debug!("skip feishu message from app/bot itself");
        return Ok(());
    }

    let message = event_json
        .pointer("/event/message")
        .cloned()
        .or_else(|| event_json.get("event").cloned())
        .unwrap_or(Value::Null);

    let message_type = message
        .get("message_type")
        .or_else(|| message.get("msg_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if message_type != "text" && message_type != "" {
        // Only text messages trigger agents in MVP.
        tracing::debug!(message_type, "skip non-text feishu message");
        return Ok(());
    }

    let message_id = message
        .get("message_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let chat_id = message
        .get("chat_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if message_id.is_empty() || chat_id.is_empty() {
        anyhow::bail!("feishu message missing message_id or chat_id");
    }

    let content_raw = message
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("{}");
    let text = serde_json::from_str::<TextContent>(content_raw)
        .ok()
        .and_then(|c| c.text)
        .unwrap_or_else(|| content_raw.to_string());
    let text = text.trim();
    if text.is_empty() {
        return Ok(());
    }

    // Strip Feishu @mention tokens like @_user_1
    let text = strip_feishu_mentions(text);
    if text.is_empty() {
        return Ok(());
    }

    let open_id = event_json
        .pointer("/event/sender/sender_id/open_id")
        .or_else(|| event_json.pointer("/event/sender/sender_id/user_id"))
        .and_then(|v| v.as_str());

    let Some(inbound) = FeishuRepository::insert_inbound_if_new(
        pool, binding.id, message_id, chat_id, open_id, &text,
    )
    .await?
    else {
        tracing::debug!(message_id, "duplicate feishu message, skipping");
        return Ok(());
    };

    let issue_id =
        create_issue_for_feishu(pool, binding.project_id, binding.agent_id, &text).await?;

    let task = AgentTaskRepository::enqueue(
        pool,
        None,
        binding.agent_id,
        issue_id,
        AgentTaskTrigger::Feishu,
        0,
        true, // fresh session for each Feishu trigger
        None,
        false,
        None,
        None,
    )
    .await?;

    FeishuRepository::link_inbound_task(pool, inbound.id, issue_id, task.data.id).await?;

    // Immediate ack (best-effort)
    let client = FeishuClient::new(&binding.app_id, &binding.app_secret);
    let ack = format!("已收到，正在调度 Agent 处理…\nIssue: {issue_id}");
    if let Err(e) = client.reply_text(message_id, &ack).await {
        tracing::warn!(?e, "feishu ack reply failed; trying send_text");
        let _ = client.send_text_to_chat(chat_id, &ack).await;
    }

    Ok(())
}

fn strip_feishu_mentions(text: &str) -> String {
    // Remove @_user_N and @_all style tokens
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '@' && chars.peek() == Some(&'_') {
            // consume @_xxx until whitespace
            while let Some(&n) = chars.peek() {
                if n.is_whitespace() {
                    break;
                }
                chars.next();
            }
            // skip following space
            if chars.peek().copied() == Some(' ') {
                chars.next();
            }
            continue;
        }
        out.push(c);
    }
    out.trim().to_string()
}

async fn create_issue_for_feishu(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    agent_id: Uuid,
    text: &str,
) -> anyhow::Result<Uuid> {
    let title: String = {
        let one_line = text.lines().next().unwrap_or(text).trim();
        let truncated: String = one_line.chars().take(80).collect();
        format!("[飞书] {truncated}")
    };
    let description = format!("来自飞书机器人的消息触发。\n\n---\n\n{text}");

    let status_id: (Uuid,) = sqlx::query_as(
        "SELECT id FROM project_statuses WHERE project_id = $1 ORDER BY sort_order ASC LIMIT 1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;

    let issue_id = Uuid::new_v4();
    let sort_order: f64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(sort_order), 0) + 1.0 FROM issues WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap_or(1.0);

    sqlx::query(
        r#"
        INSERT INTO issues (
            id, project_id, status_id, title, description,
            sort_order, extension_metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, '{}')
        "#,
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(status_id.0)
    .bind(&title)
    .bind(&description)
    .bind(sort_order)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO issue_assignees (issue_id, agent_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(issue_id)
    .bind(agent_id)
    .execute(pool)
    .await?;

    Ok(issue_id)
}

/// Called when an agent task reaches a terminal state — reply to Feishu if linked.
pub async fn maybe_reply_feishu_on_terminal(pool: &sqlx::PgPool, task: &api_types::AgentTask) {
    let Ok(Some((inbound, binding))) = FeishuRepository::find_pending_by_task(pool, task.id).await
    else {
        return;
    };

    if !binding.reply_on_complete {
        let _ = FeishuRepository::mark_reply(pool, inbound.id, "skipped", None).await;
        return;
    }

    let status_label = match task.status {
        api_types::AgentTaskStatus::Completed => "已完成",
        api_types::AgentTaskStatus::Failed => "失败",
        api_types::AgentTaskStatus::Cancelled => "已取消",
        _ => return,
    };

    let mut text = format!(
        "Agent 任务{status_label}。\nIssue: {}\nTask: {}",
        task.issue_id, task.id
    );
    if let Some(reason) = &task.failure_reason {
        text.push_str(&format!("\n原因: {reason}"));
    }

    let client = FeishuClient::new(&binding.app_id, &binding.app_secret);
    let result = match client.reply_text(&inbound.message_id, &text).await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::warn!(?e, "feishu reply failed; falling back to send");
            client.send_text_to_chat(&inbound.chat_id, &text).await
        }
    };

    match result {
        Ok(()) => {
            let _ = FeishuRepository::mark_reply(pool, inbound.id, "sent", None).await;
        }
        Err(e) => {
            tracing::warn!(?e, inbound_id = %inbound.id, "feishu completion reply failed");
            let _ = FeishuRepository::mark_reply(pool, inbound.id, "failed", Some(&e.to_string()))
                .await;
        }
    }
}
