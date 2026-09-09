use axum::{
    Json, Router,
    extract::{Path, State},
    response::Json as ResponseJson,
    routing::{delete, get},
};
use db::models::{
    project::Project, project_repo::ProjectRepo, project_workspace::ProjectWorkspace, repo::Repo,
    workspace::Workspace,
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

#[derive(Debug, Deserialize, TS)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    #[ts(optional)]
    pub default_agent_working_dir: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
pub struct UpdateProjectRequest {
    #[serde(default)]
    #[ts(optional)]
    pub name: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub default_agent_working_dir: Option<String>,
}

#[derive(Debug, Deserialize, TS)]
pub struct AttachProjectRepoRequest {
    pub repo_id: Uuid,
}

#[derive(Debug, Deserialize, TS)]
pub struct AttachProjectWorkspaceRequest {
    pub workspace_id: Uuid,
}

#[derive(Debug, Serialize, TS)]
pub struct ProjectSummary {
    #[serde(flatten)]
    #[ts(flatten)]
    pub project: Project,
    pub repos: Vec<Repo>,
    pub workspace_count: u32,
}

#[derive(Debug, Serialize, TS)]
pub struct ProjectDetail {
    #[serde(flatten)]
    #[ts(flatten)]
    pub project: Project,
    pub repos: Vec<Repo>,
    pub workspaces: Vec<Workspace>,
}

async fn load_project(deployment: &DeploymentImpl, id: Uuid) -> Result<Project, ApiError> {
    Project::find_by_id(&deployment.db().pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Project not found".to_string()))
}

async fn project_summary(
    deployment: &DeploymentImpl,
    project: Project,
) -> Result<ProjectSummary, ApiError> {
    let repos = ProjectRepo::list_repos_for_project(&deployment.db().pool, project.id).await?;
    let workspace_count =
        ProjectWorkspace::count_by_project(&deployment.db().pool, project.id).await? as u32;
    Ok(ProjectSummary {
        project,
        repos,
        workspace_count,
    })
}

async fn project_detail(
    deployment: &DeploymentImpl,
    project: Project,
) -> Result<ProjectDetail, ApiError> {
    let repos = ProjectRepo::list_repos_for_project(&deployment.db().pool, project.id).await?;
    let workspaces =
        ProjectWorkspace::list_workspaces_for_project(&deployment.db().pool, project.id).await?;
    Ok(ProjectDetail {
        project,
        repos,
        workspaces,
    })
}

pub async fn list_projects(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ProjectSummary>>>, ApiError> {
    let projects = Project::find_all(&deployment.db().pool).await?;
    let mut summaries = Vec::with_capacity(projects.len());
    for project in projects {
        summaries.push(project_summary(&deployment, project).await?);
    }
    Ok(ResponseJson(ApiResponse::success(summaries)))
}

pub async fn create_project(
    State(deployment): State<DeploymentImpl>,
    Json(payload): Json<CreateProjectRequest>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    let name = Project::normalize_name(&payload.name).map_err(ApiError::BadRequest)?;
    let working_dir = payload
        .default_agent_working_dir
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let project = Project::create(&deployment.db().pool, name, working_dir, None).await?;
    Ok(ResponseJson(ApiResponse::success(project)))
}

pub async fn get_project(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<ProjectDetail>>, ApiError> {
    let project = load_project(&deployment, project_id).await?;
    Ok(ResponseJson(ApiResponse::success(
        project_detail(&deployment, project).await?,
    )))
}

pub async fn update_project(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<UpdateProjectRequest>,
) -> Result<ResponseJson<ApiResponse<Project>>, ApiError> {
    let existing = load_project(&deployment, project_id).await?;
    let name = match payload.name {
        Some(name) => Project::normalize_name(&name)
            .map_err(ApiError::BadRequest)?
            .to_string(),
        None => existing.name,
    };
    let working_dir = match payload.default_agent_working_dir {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        None => existing.default_agent_working_dir,
    };
    let project = Project::update(
        &deployment.db().pool,
        project_id,
        &name,
        working_dir.as_deref(),
    )
    .await?;
    Ok(ResponseJson(ApiResponse::success(project)))
}

pub async fn delete_project(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let rows = Project::delete(&deployment.db().pool, project_id).await?;
    if rows == 0 {
        return Err(ApiError::BadRequest("Project not found".to_string()));
    }
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn list_project_repos(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Vec<Repo>>>, ApiError> {
    load_project(&deployment, project_id).await?;
    let repos = ProjectRepo::list_repos_for_project(&deployment.db().pool, project_id).await?;
    Ok(ResponseJson(ApiResponse::success(repos)))
}

pub async fn attach_project_repo(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<AttachProjectRepoRequest>,
) -> Result<ResponseJson<ApiResponse<Repo>>, ApiError> {
    load_project(&deployment, project_id).await?;
    let repo = Repo::find_by_id(&deployment.db().pool, payload.repo_id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("Repository not found".to_string()))?;
    ProjectRepo::attach(&deployment.db().pool, project_id, payload.repo_id).await?;
    Ok(ResponseJson(ApiResponse::success(repo)))
}

pub async fn detach_project_repo(
    State(deployment): State<DeploymentImpl>,
    Path((project_id, repo_id)): Path<(Uuid, Uuid)>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    load_project(&deployment, project_id).await?;
    ProjectRepo::detach(&deployment.db().pool, project_id, repo_id).await?;
    Ok(ResponseJson(ApiResponse::success(())))
}

pub async fn list_project_workspaces(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<Vec<Workspace>>>, ApiError> {
    load_project(&deployment, project_id).await?;
    let workspaces =
        ProjectWorkspace::list_workspaces_for_project(&deployment.db().pool, project_id).await?;
    Ok(ResponseJson(ApiResponse::success(workspaces)))
}

pub async fn attach_project_workspace(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<Uuid>,
    Json(payload): Json<AttachProjectWorkspaceRequest>,
) -> Result<ResponseJson<ApiResponse<Workspace>>, ApiError> {
    load_project(&deployment, project_id).await?;
    let workspace = Workspace::find_by_id(&deployment.db().pool, payload.workspace_id)
        .await?
        .ok_or_else(|| {
            ApiError::Workspace(db::models::workspace::WorkspaceError::WorkspaceNotFound)
        })?;
    ProjectWorkspace::attach(&deployment.db().pool, project_id, payload.workspace_id).await?;
    Ok(ResponseJson(ApiResponse::success(workspace)))
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new().nest(
        "/projects",
        Router::new()
            .route("/", get(list_projects).post(create_project))
            .route(
                "/{project_id}",
                get(get_project)
                    .patch(update_project)
                    .delete(delete_project),
            )
            .route(
                "/{project_id}/repos",
                get(list_project_repos).post(attach_project_repo),
            )
            .route("/{project_id}/repos/{repo_id}", delete(detach_project_repo))
            .route(
                "/{project_id}/workspaces",
                get(list_project_workspaces).post(attach_project_workspace),
            ),
    )
}
