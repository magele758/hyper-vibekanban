use axum::{
    Router,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use db::models::{
    project::Project,
    scratch::{CreateScratch, Scratch, ScratchPayload, WorkspaceNotesData},
    task::Task,
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{
    DeploymentImpl,
    error::ApiError,
    import::{parse::map_issue_status, parse_web_export},
    routes::workspaces::create::create_workspace_record,
};

const MAX_IMPORT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportWebExportResult {
    pub projects_created: u32,
    pub projects_reused: u32,
    pub issues_created: u32,
    pub issues_skipped: u32,
    pub projects: Vec<ImportedProjectSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportedProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub issue_count: u32,
    pub reused: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ImportedProjectDetail {
    pub project: Project,
    pub tasks: Vec<Task>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CreateWorkspaceFromImportedTaskResponse {
    pub workspace_id: Uuid,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route(
            "/import/web-export",
            post(import_web_export).layer(DefaultBodyLimit::max(MAX_IMPORT_BYTES)),
        )
        .route("/imported/projects", get(list_imported_projects))
        .route("/imported/projects/{id}", get(get_imported_project))
        .route(
            "/imported/tasks/{id}/workspace",
            post(create_workspace_from_imported_task),
        )
}

async fn import_web_export(
    State(deployment): State<DeploymentImpl>,
    mut multipart: Multipart,
) -> Result<ResponseJson<ApiResponse<ImportWebExportResult>>, ApiError> {
    let mut file_bytes = None;
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") || file_bytes.is_none() {
            file_bytes = Some(field.bytes().await?.to_vec());
        }
    }
    let bytes = file_bytes.ok_or_else(|| {
        ApiError::BadRequest("missing export file (field name: file)".to_string())
    })?;

    let manifest =
        parse_web_export(&bytes).map_err(|error| ApiError::BadRequest(error.to_string()))?;
    let pool = &deployment.db().pool;

    let mut projects_created = 0;
    let mut projects_reused = 0;
    let mut issues_created = 0;
    let mut issues_skipped = 0;
    let mut summaries = Vec::new();

    for export_project in manifest.projects {
        let remote_id = export_project
            .id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok());
        let (project, reused) = if let Some(remote_id) = remote_id
            && let Some(existing) = Project::find_by_remote_project_id(pool, remote_id).await?
        {
            (existing, true)
        } else if let Some(existing) = Project::find_by_name(pool, &export_project.name).await? {
            (existing, true)
        } else {
            let created = Project::create(pool, &export_project.name, None, remote_id).await?;
            (created, false)
        };

        if reused {
            projects_reused += 1;
        } else {
            projects_created += 1;
        }

        let mut created_here = 0u32;
        for issue in export_project.issues {
            let title = issue.title.trim();
            if title.is_empty() {
                issues_skipped += 1;
                continue;
            }
            if Task::find_by_project_and_title(pool, project.id, title)
                .await?
                .is_some()
            {
                issues_skipped += 1;
                continue;
            }
            let description =
                format_imported_description(&issue.simple_id, issue.description.as_deref());
            let status = map_issue_status(issue.status.as_deref());
            Task::create(pool, project.id, title, description.as_deref(), status).await?;
            created_here += 1;
        }
        issues_created += created_here;

        let issue_count = Task::find_by_project_id(pool, project.id).await?.len() as u32;
        summaries.push(ImportedProjectSummary {
            id: project.id,
            name: project.name,
            issue_count,
            reused,
        });
    }

    Ok(ResponseJson(ApiResponse::success(ImportWebExportResult {
        projects_created,
        projects_reused,
        issues_created,
        issues_skipped,
        projects: summaries,
    })))
}

async fn list_imported_projects(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<ApiResponse<Vec<ImportedProjectSummary>>>, ApiError> {
    let pool = &deployment.db().pool;
    let projects = Project::find_all(pool).await?;
    let mut summaries = Vec::new();
    for project in projects {
        let issue_count = Task::find_by_project_id(pool, project.id).await?.len() as u32;
        summaries.push(ImportedProjectSummary {
            id: project.id,
            name: project.name,
            issue_count,
            reused: false,
        });
    }
    Ok(ResponseJson(ApiResponse::success(summaries)))
}

async fn get_imported_project(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<ImportedProjectDetail>>, ApiError> {
    let pool = &deployment.db().pool;
    let project = Project::find_by_id(pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("project not found".to_string()))?;
    let tasks = Task::find_by_project_id(pool, id).await?;
    Ok(ResponseJson(ApiResponse::success(ImportedProjectDetail {
        project,
        tasks,
    })))
}

async fn create_workspace_from_imported_task(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<Uuid>,
) -> Result<ResponseJson<ApiResponse<CreateWorkspaceFromImportedTaskResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let task = Task::find_by_id(pool, id)
        .await?
        .ok_or_else(|| ApiError::BadRequest("imported issue not found".to_string()))?;

    if let Some(workspace_id) = task.parent_workspace_id {
        return Ok(ResponseJson(ApiResponse::success(
            CreateWorkspaceFromImportedTaskResponse { workspace_id },
        )));
    }

    let workspace = create_workspace_record(
        &deployment,
        Some(task.title.clone()),
        db::models::workspace::WorkspaceKind::Worktree,
    )
    .await?;

    if let Some(description) = task
        .description
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        Scratch::create(
            pool,
            workspace.id,
            &CreateScratch {
                payload: ScratchPayload::WorkspaceNotes(WorkspaceNotesData {
                    content: description.to_string(),
                }),
            },
        )
        .await?;
    }

    Task::set_parent_workspace_id(pool, task.id, workspace.id).await?;

    Ok(ResponseJson(ApiResponse::success(
        CreateWorkspaceFromImportedTaskResponse {
            workspace_id: workspace.id,
        },
    )))
}

fn format_imported_description(simple_id: &str, description: Option<&str>) -> Option<String> {
    let description = description.map(str::trim).filter(|text| !text.is_empty());
    let simple_id = simple_id.trim();
    match (simple_id.is_empty(), description) {
        (true, None) => None,
        (true, Some(text)) => Some(text.to_string()),
        (false, None) => Some(format!("[{simple_id}]")),
        (false, Some(text)) => Some(format!("[{simple_id}]\n\n{text}")),
    }
}
