use api_types::{
    AgentTaskTrigger, CreateIssueCommentRequest, DeleteResponse, IssueComment,
    ListIssueCommentsQuery, ListIssueCommentsResponse, MemberRole, MutationResponse,
    NotificationPayload, NotificationType, UpdateIssueCommentRequest,
};
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_issue_access,
};
use crate::{
    AppState,
    auth::RequestContext,
    db::{
        agent_tasks::AgentTaskRepository, issue_comments::IssueCommentRepository,
        issues::IssueRepository, organization_members::check_user_role,
    },
    mutation_definition::MutationBuilder,
    notifications::notify_issue_subscribers,
};

/// Mutation definition for IssueComment - provides both router and TypeScript metadata.
pub fn mutation()
-> MutationBuilder<IssueComment, CreateIssueCommentRequest, UpdateIssueCommentRequest> {
    MutationBuilder::new("issue_comments")
        .list(list_issue_comments)
        .get(get_issue_comment)
        .create(create_issue_comment)
        .update(update_issue_comment)
        .delete(delete_issue_comment)
}

pub fn router() -> axum::Router<AppState> {
    mutation().router()
}

#[instrument(
    name = "issue_comments.list_issue_comments",
    skip(state, ctx),
    fields(issue_id = %query.issue_id, user_id = %ctx.user.id)
)]
async fn list_issue_comments(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListIssueCommentsQuery>,
) -> Result<Json<ListIssueCommentsResponse>, ErrorResponse> {
    ensure_issue_access(state.pool(), ctx.user.id, query.issue_id).await?;

    let issue_comments = IssueCommentRepository::list_by_issue(state.pool(), query.issue_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, issue_id = %query.issue_id, "failed to list issue comments");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list issue comments",
            )
        })?;

    Ok(Json(ListIssueCommentsResponse { issue_comments }))
}

#[instrument(
    name = "issue_comments.get_issue_comment",
    skip(state, ctx),
    fields(issue_comment_id = %issue_comment_id, user_id = %ctx.user.id)
)]
async fn get_issue_comment(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_comment_id): Path<Uuid>,
) -> Result<Json<IssueComment>, ErrorResponse> {
    let comment = IssueCommentRepository::find_by_id(state.pool(), issue_comment_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %issue_comment_id, "failed to load issue comment");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load issue comment",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue comment not found"))?;

    ensure_issue_access(state.pool(), ctx.user.id, comment.issue_id).await?;

    Ok(Json(comment))
}

#[instrument(
    name = "issue_comments.create_issue_comment",
    skip(state, ctx, payload),
    fields(issue_id = %payload.issue_id, user_id = %ctx.user.id)
)]
async fn create_issue_comment(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateIssueCommentRequest>,
) -> Result<Json<MutationResponse<IssueComment>>, ErrorResponse> {
    let organization_id = ensure_issue_access(state.pool(), ctx.user.id, payload.issue_id).await?;

    let is_reply = payload.parent_id.is_some();

    let response = IssueCommentRepository::create(
        state.pool(),
        payload.id,
        payload.issue_id,
        ctx.user.id,
        payload.parent_id,
        payload.message,
    )
    .await
    .map_err(|error| {
        tracing::error!(?error, "failed to create issue comment");
        db_error(error, "failed to create issue comment")
    })?;

    if let Some(analytics) = state.analytics() {
        analytics.track(
            ctx.user.id,
            "issue_comment_created",
            serde_json::json!({
                "comment_id": response.data.id,
                "issue_id": response.data.issue_id,
                "organization_id": organization_id,
                "is_reply": is_reply,
            }),
        );
    }

    if let Ok(Some(issue)) = IssueRepository::find_by_id(state.pool(), response.data.issue_id).await
    {
        let comment_preview = response.data.message.chars().take(100).collect::<String>();
        notify_issue_subscribers(
            state.pool(),
            organization_id,
            ctx.user.id,
            &issue,
            NotificationType::IssueCommentAdded,
            NotificationPayload {
                comment_preview: Some(comment_preview),
                ..Default::default()
            },
            Some(response.data.id),
        )
        .await;
    }

    // Parse [@Name](mention://agent/<uuid>) to enqueue agent tasks.
    enqueue_mention_tasks(state.pool(), &response.data.message, response.data.issue_id).await;

    Ok(Json(response))
}

#[instrument(
    name = "issue_comments.update_issue_comment",
    skip(state, ctx, payload),
    fields(issue_comment_id = %issue_comment_id, user_id = %ctx.user.id)
)]
async fn update_issue_comment(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_comment_id): Path<Uuid>,
    Json(payload): Json<UpdateIssueCommentRequest>,
) -> Result<Json<MutationResponse<IssueComment>>, ErrorResponse> {
    let comment = IssueCommentRepository::find_by_id(state.pool(), issue_comment_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %issue_comment_id, "failed to load issue comment");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load issue comment",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue comment not found"))?;

    let organization_id = ensure_issue_access(state.pool(), ctx.user.id, comment.issue_id).await?;

    let is_author = comment
        .author_id
        .map(|id| id == ctx.user.id)
        .unwrap_or(false);
    let is_admin = check_user_role(state.pool(), organization_id, ctx.user.id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to check user role");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?
        .map(|role| role == MemberRole::Admin)
        .unwrap_or(false);

    if !is_author && !is_admin {
        return Err(ErrorResponse::new(
            StatusCode::FORBIDDEN,
            "you do not have permission to edit this comment",
        ));
    }

    let response = IssueCommentRepository::update(state.pool(), issue_comment_id, payload.message)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to update issue comment");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

#[instrument(
    name = "issue_comments.delete_issue_comment",
    skip(state, ctx),
    fields(issue_comment_id = %issue_comment_id, user_id = %ctx.user.id)
)]
async fn delete_issue_comment(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(issue_comment_id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    let comment = IssueCommentRepository::find_by_id(state.pool(), issue_comment_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, %issue_comment_id, "failed to load issue comment");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to load issue comment",
            )
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "issue comment not found"))?;

    let organization_id = ensure_issue_access(state.pool(), ctx.user.id, comment.issue_id).await?;

    let is_author = comment
        .author_id
        .map(|id| id == ctx.user.id)
        .unwrap_or(false);
    let is_admin = check_user_role(state.pool(), organization_id, ctx.user.id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to check user role");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?
        .map(|role| role == MemberRole::Admin)
        .unwrap_or(false);

    if !is_author && !is_admin {
        return Err(ErrorResponse::new(
            StatusCode::FORBIDDEN,
            "you do not have permission to delete this comment",
        ));
    }

    let response = IssueCommentRepository::delete(state.pool(), issue_comment_id)
        .await
        .map_err(|error| {
            tracing::error!(?error, "failed to delete issue comment");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

/// Parse `[@Name](mention://agent/<uuid>)` patterns and enqueue a Mention task for each agent.
async fn enqueue_mention_tasks(pool: &sqlx::PgPool, message: &str, issue_id: uuid::Uuid) {
    // Find all mention://agent/<uuid> hrefs in the message.
    let mut search = message;
    while let Some(pos) = search.find("mention://agent/") {
        let after = &search[pos + "mention://agent/".len()..];
        let end = after
            .find(|c: char| !c.is_ascii_hexdigit() && c != '-')
            .unwrap_or(after.len());
        let uuid_str = &after[..end];
        if let Ok(agent_id) = uuid_str.parse::<uuid::Uuid>() {
            if let Err(e) = AgentTaskRepository::enqueue(
                pool,
                None,
                agent_id,
                issue_id,
                AgentTaskTrigger::Mention,
                0,
                false,
                None,
                false,
                None,
            )
            .await
            {
                tracing::warn!(?e, %agent_id, %issue_id, "failed to enqueue mention task");
            }
        }
        // Advance past this mention to find more.
        search = &search[pos + 1..];
    }
}
