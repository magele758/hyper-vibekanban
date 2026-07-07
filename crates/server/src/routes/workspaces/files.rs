use std::path::{Component, Path, PathBuf};

use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::Json as ResponseJson,
    routing::get,
};
use db::models::{
    repo::{Repo, RepoError},
    workspace::Workspace,
    workspace_repo::WorkspaceRepo,
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use services::services::container::ContainerService;
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Maximum size of a file we are willing to read into memory and return inline.
const MAX_INLINE_FILE_BYTES: u64 = 5 * 1024 * 1024; // 5 MiB

#[derive(Debug, Deserialize)]
pub struct ReadFileQuery {
    /// Path to the file, relative to the repo working directory.
    pub path: String,
    /// Optional repo to resolve the path against. When omitted, each repo in
    /// the workspace is probed until the file is found.
    pub repo_id: Option<Uuid>,
}

#[derive(Debug, Serialize, TS)]
pub struct ReadFileResponse {
    /// The (normalized, relative) path that was resolved.
    pub path: String,
    /// Raw file contents as UTF-8 text.
    pub content: String,
    /// The repo the file was resolved in. Pass this back when writing so edits
    /// land in the same repo (avoids ambiguity when multiple repos are present).
    #[ts(type = "string")]
    pub repo_id: Uuid,
}

#[derive(Debug, Deserialize, Serialize, TS)]
pub struct WriteFileRequest {
    /// Path to the file, relative to the repo working directory.
    pub path: String,
    /// New UTF-8 contents to write.
    pub content: String,
    /// Repo to resolve the path against. Required for writes to avoid ambiguity.
    #[ts(type = "string")]
    pub repo_id: Uuid,
}

/// Normalize a caller-supplied relative path, rejecting anything that would
/// escape the repo working directory (absolute paths, `..`, etc.).
#[allow(clippy::result_large_err)]
fn safe_relative_path(raw: &str) -> Result<PathBuf, ApiError> {
    let candidate = Path::new(raw);
    if candidate.is_absolute() {
        return Err(ApiError::BadRequest(
            "File path must be relative to the repository root".to_string(),
        ));
    }

    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ApiError::BadRequest(
                    "File path may not traverse outside the repository".to_string(),
                ));
            }
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(ApiError::BadRequest("File path is empty".to_string()));
    }

    Ok(normalized)
}

/// Read a text file from within a workspace worktree.
///
/// `GET /api/workspaces/{id}/files?path=<relative>&repo_id=<uuid>`
pub async fn read_workspace_file(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Query(query): Query<ReadFileQuery>,
) -> Result<ResponseJson<ApiResponse<ReadFileResponse>>, ApiError> {
    let pool = &deployment.db().pool;
    let rel_path = safe_relative_path(&query.path)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_dir = PathBuf::from(&container_ref);

    // Resolve the candidate repos to probe. When a repo_id is given, only that
    // repo is used; otherwise every repo in the workspace is tried in order.
    let repos: Vec<Repo> = if let Some(repo_id) = query.repo_id {
        let workspace_repo =
            WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, repo_id)
                .await?
                .ok_or(RepoError::NotFound)?;
        let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;
        vec![repo]
    } else {
        WorkspaceRepo::find_repos_for_workspace(pool, workspace.id).await?
    };

    for repo in repos {
        let worktree_path = workspace.kind.repo_working_path(&workspace_dir, &repo.name);
        let full_path = worktree_path.join(&rel_path);

        // Ensure the resolved path stays within the worktree even after any
        // symlink resolution the OS may perform.
        let canonical_root = match worktree_path.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let canonical_file = match full_path.canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !canonical_file.starts_with(&canonical_root) {
            return Err(ApiError::Forbidden(
                "Resolved path escapes the repository".to_string(),
            ));
        }

        let metadata = match tokio::fs::metadata(&canonical_file).await {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !metadata.is_file() {
            continue;
        }
        if metadata.len() > MAX_INLINE_FILE_BYTES {
            return Err(ApiError::PayloadTooLarge);
        }

        match tokio::fs::read(&canonical_file).await {
            Ok(bytes) => {
                let content = String::from_utf8(bytes).map_err(|_| {
                    ApiError::BadRequest("File is not valid UTF-8 text".to_string())
                })?;
                return Ok(ResponseJson(ApiResponse::success(ReadFileResponse {
                    path: rel_path.to_string_lossy().to_string(),
                    content,
                    repo_id: repo.id,
                })));
            }
            Err(_) => continue,
        }
    }

    Ok(ResponseJson(ApiResponse::error("File not found")))
}

/// Write UTF-8 contents to an existing text file within a workspace worktree.
///
/// `PUT /api/workspaces/{id}/files`
///
/// Only overwrites files that already exist (no creating new files or dirs),
/// and refuses paths that escape the worktree.
pub async fn write_workspace_file(
    Extension(workspace): Extension<Workspace>,
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<WriteFileRequest>,
) -> Result<ResponseJson<ApiResponse<()>>, ApiError> {
    let pool = &deployment.db().pool;
    let rel_path = safe_relative_path(&request.path)?;

    if request.content.len() as u64 > MAX_INLINE_FILE_BYTES {
        return Err(ApiError::PayloadTooLarge);
    }

    let workspace_repo =
        WorkspaceRepo::find_by_workspace_and_repo_id(pool, workspace.id, request.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;
    let repo = Repo::find_by_id(pool, workspace_repo.repo_id)
        .await?
        .ok_or(RepoError::NotFound)?;

    let container_ref = deployment
        .container()
        .ensure_container_exists(&workspace)
        .await?;
    let workspace_dir = PathBuf::from(&container_ref);
    let worktree_path = workspace.kind.repo_working_path(&workspace_dir, &repo.name);
    let full_path = worktree_path.join(&rel_path);

    // The file must already exist; we only overwrite, never create.
    let canonical_root = worktree_path
        .canonicalize()
        .map_err(|_| ApiError::BadRequest("Repository worktree not found".to_string()))?;
    let canonical_file = full_path
        .canonicalize()
        .map_err(|_| ApiError::BadRequest("File not found".to_string()))?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err(ApiError::Forbidden(
            "Resolved path escapes the repository".to_string(),
        ));
    }

    let metadata = tokio::fs::metadata(&canonical_file).await?;
    if !metadata.is_file() {
        return Err(ApiError::BadRequest(
            "Target path is not a regular file".to_string(),
        ));
    }

    tokio::fs::write(&canonical_file, request.content.as_bytes()).await?;

    Ok(ResponseJson(ApiResponse::success(())))
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new().route("/", get(read_workspace_file).put(write_workspace_file))
}
