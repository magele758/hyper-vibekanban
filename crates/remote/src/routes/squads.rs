use api_types::{
    AddSquadMemberRequest, CreateSquadRequest, DeleteResponse, ListSquadMembersResponse,
    ListSquadsQuery, ListSquadsResponse, MutationResponse, Squad, SquadMember, UpdateSquadRequest,
};
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
};
use tracing::instrument;
use uuid::Uuid;

use super::{
    error::{ErrorResponse, db_error},
    organization_members::ensure_project_access,
};
use crate::{AppState, auth::RequestContext, db::squads::SquadRepository};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/squads", get(list_squads).post(create_squad))
        .route(
            "/squads/:id",
            get(get_squad).put(update_squad).delete(delete_squad),
        )
        .route(
            "/squads/:id/members",
            get(list_squad_members).post(add_squad_member),
        )
        .route(
            "/squads/:squad_id/members/:member_id",
            delete(remove_squad_member),
        )
}

#[instrument(name = "squads.list", skip(state, ctx), fields(user_id = %ctx.user.id))]
async fn list_squads(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Query(query): Query<ListSquadsQuery>,
) -> Result<Json<ListSquadsResponse>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, query.project_id).await?;

    let squads = SquadRepository::list_by_project(state.pool(), query.project_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list squads");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to list squads")
        })?;

    Ok(Json(ListSquadsResponse { squads }))
}

#[instrument(name = "squads.get", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn get_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<Squad>, ErrorResponse> {
    let squad = load_and_authorize(&state, ctx.user.id, id).await?;
    Ok(Json(squad))
}

#[instrument(name = "squads.create", skip(state, ctx, payload), fields(user_id = %ctx.user.id))]
async fn create_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Json(payload): Json<CreateSquadRequest>,
) -> Result<Json<MutationResponse<Squad>>, ErrorResponse> {
    ensure_project_access(state.pool(), ctx.user.id, payload.project_id).await?;

    let response = SquadRepository::create(
        state.pool(),
        payload.id,
        payload.project_id,
        payload.name,
        payload.leader_agent_id,
    )
    .await
    .map_err(|e| db_error(e, "failed to create squad"))?;

    Ok(Json(response))
}

#[instrument(name = "squads.update", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn update_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSquadRequest>,
) -> Result<Json<MutationResponse<Squad>>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    let response = SquadRepository::update(state.pool(), id, payload.name, payload.leader_agent_id)
        .await
        .map_err(|e| db_error(e, "failed to update squad"))?;

    Ok(Json(response))
}

#[instrument(name = "squads.delete", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn delete_squad(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    let response = SquadRepository::delete(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to delete squad");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

#[instrument(name = "squads.list_members", skip(state, ctx), fields(id = %id, user_id = %ctx.user.id))]
async fn list_squad_members(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
) -> Result<Json<ListSquadMembersResponse>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    let members = SquadRepository::list_members(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to list squad members");
            ErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to list squad members",
            )
        })?;

    Ok(Json(ListSquadMembersResponse { members }))
}

#[instrument(name = "squads.add_member", skip(state, ctx, payload), fields(id = %id, user_id = %ctx.user.id))]
async fn add_squad_member(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AddSquadMemberRequest>,
) -> Result<Json<MutationResponse<SquadMember>>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, id).await?;

    if payload.agent_id.is_none() && payload.user_id.is_none() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "agent_id or user_id is required",
        ));
    }
    if payload.agent_id.is_some() && payload.user_id.is_some() {
        return Err(ErrorResponse::new(
            StatusCode::BAD_REQUEST,
            "only one of agent_id or user_id may be set",
        ));
    }

    let response = SquadRepository::add_member(state.pool(), id, payload.agent_id, payload.user_id)
        .await
        .map_err(|e| db_error(e, "failed to add squad member"))?;

    Ok(Json(response))
}

#[instrument(name = "squads.remove_member", skip(state, ctx), fields(squad_id = %squad_id, member_id = %member_id, user_id = %ctx.user.id))]
async fn remove_squad_member(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path((squad_id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<DeleteResponse>, ErrorResponse> {
    load_and_authorize(&state, ctx.user.id, squad_id).await?;

    let response = SquadRepository::remove_member(state.pool(), member_id)
        .await
        .map_err(|e| {
            tracing::error!(?e, "failed to remove squad member");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
        })?;

    Ok(Json(response))
}

async fn load_and_authorize(
    state: &AppState,
    user_id: Uuid,
    id: Uuid,
) -> Result<Squad, ErrorResponse> {
    let squad = SquadRepository::find_by_id(state.pool(), id)
        .await
        .map_err(|e| {
            tracing::error!(?e, %id, "failed to load squad");
            ErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, "failed to load squad")
        })?
        .ok_or_else(|| ErrorResponse::new(StatusCode::NOT_FOUND, "squad not found"))?;

    ensure_project_access(state.pool(), user_id, squad.project_id).await?;
    Ok(squad)
}
