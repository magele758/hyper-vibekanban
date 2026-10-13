use api_types::{
    CreateProjectRequest, ListProjectsResponse, MutationResponse, Project, UpdateProjectRequest,
};
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use serde::Deserialize;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize)]
pub(super) struct ListRemoteProjectsQuery {
    pub organization_id: Uuid,
}

pub(super) fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/projects",
            get(list_remote_projects).post(create_remote_project),
        )
        .route(
            "/projects/{project_id}",
            get(get_remote_project).patch(update_remote_project),
        )
}

async fn list_remote_projects(
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ListRemoteProjectsQuery>,
) -> Result<ResponseJson<ApiResponse<ListProjectsResponse>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.list_remote_projects(query.organization_id).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn get_remote_project(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    let client = deployment.remote_client()?;
    let project = client.get_remote_project(project_id).await?;
    Ok(ResponseJson(ApiResponse::success(project)))
}

async fn create_remote_project(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Project>>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.create_remote_project(&request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

async fn update_remote_project(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
    Json(request): Json<UpdateProjectRequest>,
) -> Result<ResponseJson<ApiResponse<MutationResponse<Project>>>, ApiError> {
    let client = deployment.remote_client()?;
    let response = client.update_remote_project(project_id, &request).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}
