use std::path::{Path, PathBuf};

use db::{
    DBService,
    models::{
        file::WorkspaceAttachment,
        repo::{Repo, RepoError},
        requests::WorkspaceRepoInput,
        session::Session,
        workspace::{Workspace as DbWorkspace, WorkspaceKind},
        workspace_repo::{CreateWorkspaceRepo, RepoWithTargetBranch, WorkspaceRepo},
    },
};
use git::{GitService, GitServiceError};
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use worktree_manager::{WorktreeCleanup, WorktreeError, WorktreeManager};

#[derive(Debug, Clone)]
pub struct RepoWorkspaceInput {
    pub repo: Repo,
    pub target_branch: String,
}

impl RepoWorkspaceInput {
    pub fn new(repo: Repo, target_branch: String) -> Self {
        Self {
            repo,
            target_branch,
        }
    }
}

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Repo(#[from] RepoError),
    #[error(transparent)]
    Worktree(#[from] WorktreeError),
    #[error(transparent)]
    GitService(#[from] GitServiceError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Workspace not found")]
    WorkspaceNotFound,
    #[error("Repository already attached to workspace")]
    RepoAlreadyAttached,
    #[error("Branch '{branch}' does not exist in repository '{repo_name}'")]
    BranchNotFound { repo_name: String, branch: String },
    #[error("No repositories provided")]
    NoRepositories,
    #[error("In-place workspaces support exactly one repository (found {0})")]
    InPlaceRequiresSingleRepo(usize),
    #[error(
        "Repository '{repo_name}' has uncommitted changes; commit or stash them before starting an in-place workspace"
    )]
    InPlaceDirtyWorkingTree { repo_name: String },
    #[error("Partial workspace creation failed: {0}")]
    PartialCreation(String),
}

/// Info about a single repo's worktree within a workspace
#[derive(Debug, Clone)]
pub struct RepoWorktree {
    pub repo_id: Uuid,
    pub repo_name: String,
    pub source_repo_path: PathBuf,
    pub worktree_path: PathBuf,
}

/// A container directory holding worktrees for all project repos
#[derive(Debug, Clone)]
pub struct WorktreeContainer {
    pub workspace_dir: PathBuf,
    pub worktrees: Vec<RepoWorktree>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceDeletionContext {
    pub workspace_id: Uuid,
    pub branch_name: String,
    pub workspace_dir: Option<PathBuf>,
    pub repositories: Vec<Repo>,
    pub repo_paths: Vec<PathBuf>,
    pub session_ids: Vec<Uuid>,
    pub kind: WorkspaceKind,
    /// For in-place workspaces: target branch to restore the repo to before
    /// deleting the workspace branch. Worktree workspaces leave this `None`.
    pub restore_branch: Option<String>,
}

#[derive(Clone)]
pub struct ManagedWorkspace {
    pub workspace: DbWorkspace,
    pub repos: Vec<RepoWithTargetBranch>,
    db: DBService,
}

impl ManagedWorkspace {
    fn new(db: DBService, workspace: DbWorkspace, repos: Vec<RepoWithTargetBranch>) -> Self {
        Self {
            workspace,
            repos,
            db,
        }
    }

    async fn attach_repository(&self, repo: &WorkspaceRepoInput) -> Result<(), sqlx::Error> {
        let create_repo = CreateWorkspaceRepo {
            repo_id: repo.repo_id,
            target_branch: repo.target_branch.clone(),
        };

        WorkspaceRepo::create_many(
            &self.db.pool,
            self.workspace.id,
            std::slice::from_ref(&create_repo),
        )
        .await
        .map(|_| ())
    }

    async fn refresh(&mut self) -> Result<(), WorkspaceError> {
        self.workspace = DbWorkspace::find_by_id(&self.db.pool, self.workspace.id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound)?;
        self.repos = WorkspaceRepo::find_repos_with_target_branch_for_workspace(
            &self.db.pool,
            self.workspace.id,
        )
        .await?;
        Ok(())
    }

    pub async fn add_repository(
        &mut self,
        repo_ref: &WorkspaceRepoInput,
        git: &GitService,
    ) -> Result<(), WorkspaceError> {
        let repo = Repo::find_by_id(&self.db.pool, repo_ref.repo_id)
            .await?
            .ok_or(RepoError::NotFound)?;

        if !git.check_branch_exists(&repo.path, &repo_ref.target_branch)? {
            return Err(WorkspaceError::BranchNotFound {
                repo_name: repo.name,
                branch: repo_ref.target_branch.clone(),
            });
        }

        if WorkspaceRepo::find_by_workspace_and_repo_id(
            &self.db.pool,
            self.workspace.id,
            repo_ref.repo_id,
        )
        .await?
        .is_some()
        {
            return Err(WorkspaceError::RepoAlreadyAttached);
        }

        self.attach_repository(repo_ref).await?;
        self.refresh().await?;
        Ok(())
    }

    pub async fn associate_attachments(&self, attachment_ids: &[Uuid]) -> Result<(), sqlx::Error> {
        if attachment_ids.is_empty() {
            return Ok(());
        }

        WorkspaceAttachment::associate_many_dedup(&self.db.pool, self.workspace.id, attachment_ids)
            .await
    }

    pub async fn prepare_deletion_context(&self) -> Result<WorkspaceDeletionContext, sqlx::Error> {
        let repositories =
            WorkspaceRepo::find_repos_for_workspace(&self.db.pool, self.workspace.id).await?;
        let session_ids = Session::find_by_workspace_id(&self.db.pool, self.workspace.id)
            .await?
            .into_iter()
            .map(|session| session.id)
            .collect::<Vec<_>>();
        let repo_paths = repositories
            .iter()
            .map(|repo| repo.path.clone())
            .collect::<Vec<_>>();

        // For in-place workspaces, capture the target branch to restore the repo to
        // before deleting the workspace branch (you can't delete a checked-out branch).
        let restore_branch = if self.workspace.kind.is_in_place() {
            self.repos.first().map(|r| r.target_branch.clone())
        } else {
            None
        };

        Ok(WorkspaceDeletionContext {
            workspace_id: self.workspace.id,
            branch_name: self.workspace.branch.clone(),
            workspace_dir: self.workspace.container_ref.clone().map(PathBuf::from),
            repositories,
            repo_paths,
            session_ids,
            kind: self.workspace.kind,
            restore_branch,
        })
    }

    pub async fn delete_record(&self) -> Result<u64, sqlx::Error> {
        DbWorkspace::delete(&self.db.pool, self.workspace.id).await
    }
}

#[derive(Clone)]
pub struct WorkspaceManager {
    db: DBService,
}

impl WorkspaceManager {
    pub fn new(db: DBService) -> Self {
        Self { db }
    }

    pub async fn load_managed_workspace(
        &self,
        workspace: DbWorkspace,
    ) -> Result<ManagedWorkspace, sqlx::Error> {
        let repos =
            WorkspaceRepo::find_repos_with_target_branch_for_workspace(&self.db.pool, workspace.id)
                .await?;
        Ok(ManagedWorkspace::new(self.db.clone(), workspace, repos))
    }

    pub fn spawn_workspace_deletion_cleanup(
        context: WorkspaceDeletionContext,
        delete_branches: bool,
    ) {
        tokio::spawn(async move {
            let WorkspaceDeletionContext {
                workspace_id,
                branch_name,
                workspace_dir,
                repositories,
                repo_paths,
                session_ids,
                kind,
                restore_branch,
            } = context;

            for session_id in session_ids {
                if let Err(e) = Self::remove_session_process_logs(session_id).await {
                    warn!(
                        "Failed to remove filesystem process logs for session {}: {}",
                        session_id, e
                    );
                }
            }

            if let Some(workspace_dir) = workspace_dir {
                info!(
                    "Starting background cleanup for workspace {} at {}",
                    workspace_id,
                    workspace_dir.display()
                );

                if let Err(e) = Self::cleanup_workspace(&workspace_dir, &repositories, kind).await {
                    error!(
                        "Background workspace cleanup failed for {} at {}: {}",
                        workspace_id,
                        workspace_dir.display(),
                        e
                    );
                } else {
                    info!(
                        "Background cleanup completed for workspace {}",
                        workspace_id
                    );
                }
            }

            // For in-place workspaces, restore the repo to its target branch before any
            // branch deletion (a checked-out branch cannot be deleted).
            if kind.is_in_place()
                && let Some(restore_branch) = &restore_branch
            {
                let git_service = GitService::new();
                for repo_path in &repo_paths {
                    if let Err(e) = git_service.restore_branch_in_place(repo_path, restore_branch) {
                        warn!(
                            "Failed to restore branch '{}' in repo {:?}: {}",
                            restore_branch, repo_path, e
                        );
                    }
                }
            }

            // Only delete the workspace's branch if the workspace actually owns
            // one. Console workspaces attach to the repo's *existing* current
            // branch (e.g. `main`) without creating it, so `branch_name` here is
            // the user's own branch — deleting it would be data loss. `git branch
            // -D` refuses to drop a checked-out branch, but if the user has since
            // switched away, the delete would succeed and wipe their branch.
            // `manages_own_branch()` is false for Console, true for Worktree/InPlace.
            if delete_branches && kind.manages_own_branch() {
                let git_service = GitService::new();
                for repo_path in repo_paths {
                    match git_service.delete_branch(&repo_path, &branch_name) {
                        Ok(()) => {
                            info!("Deleted branch '{}' from repo {:?}", branch_name, repo_path);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to delete branch '{}' from repo {:?}: {}",
                                branch_name, repo_path, e
                            );
                        }
                    }
                }
            }
        });
    }

    async fn remove_session_process_logs(session_id: Uuid) -> Result<(), std::io::Error> {
        let dir = utils::execution_logs::process_logs_session_dir(session_id);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Create a workspace with worktrees for all repositories.
    /// On failure, rolls back any already-created worktrees.
    ///
    /// For `WorkspaceKind::InPlace` no worktree is created: the single repo's own
    /// working tree is checked out onto `branch_name` directly. In-place workspaces
    /// must contain exactly one repo and require a clean working tree.
    pub async fn create_workspace(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
        kind: WorkspaceKind,
    ) -> Result<WorktreeContainer, WorkspaceError> {
        if repos.is_empty() {
            return Err(WorkspaceError::NoRepositories);
        }

        if kind.is_console() {
            return Self::create_console_workspace(repos).await;
        }

        if kind.is_in_place() {
            return Self::create_in_place_workspace(repos, branch_name).await;
        }

        info!(
            "Creating workspace at {} with {} repositories",
            workspace_dir.display(),
            repos.len()
        );

        tokio::fs::create_dir_all(workspace_dir).await?;

        let mut created_worktrees: Vec<RepoWorktree> = Vec::new();

        for input in repos {
            let worktree_path = workspace_dir.join(&input.repo.name);

            debug!(
                "Creating worktree for repo '{}' at {}",
                input.repo.name,
                worktree_path.display()
            );

            match WorktreeManager::create_worktree(
                &input.repo.path,
                branch_name,
                &worktree_path,
                &input.target_branch,
                true,
            )
            .await
            {
                Ok(()) => {
                    created_worktrees.push(RepoWorktree {
                        repo_id: input.repo.id,
                        repo_name: input.repo.name.clone(),
                        source_repo_path: input.repo.path.clone(),
                        worktree_path,
                    });
                }
                Err(e) => {
                    error!(
                        "Failed to create worktree for repo '{}': {}. Rolling back...",
                        input.repo.name, e
                    );

                    // Rollback: cleanup all worktrees we've created so far
                    Self::cleanup_created_worktrees(&created_worktrees).await;

                    // Also remove the workspace directory if it's empty
                    if let Err(cleanup_err) = tokio::fs::remove_dir(workspace_dir).await {
                        debug!(
                            "Could not remove workspace dir during rollback: {}",
                            cleanup_err
                        );
                    }

                    return Err(WorkspaceError::PartialCreation(format!(
                        "Failed to create worktree for repo '{}': {}",
                        input.repo.name, e
                    )));
                }
            }
        }

        info!(
            "Successfully created workspace with {} worktrees",
            created_worktrees.len()
        );

        Ok(WorktreeContainer {
            workspace_dir: workspace_dir.to_path_buf(),
            worktrees: created_worktrees,
        })
    }

    /// Create an in-place workspace: check out `branch_name` directly in the repo's
    /// own working tree. The "workspace dir" IS the repo path. No directory is
    /// created and no worktree is registered.
    async fn create_in_place_workspace(
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
    ) -> Result<WorktreeContainer, WorkspaceError> {
        if repos.len() != 1 {
            return Err(WorkspaceError::InPlaceRequiresSingleRepo(repos.len()));
        }
        let input = &repos[0];
        let repo = &input.repo;

        info!(
            "Creating in-place workspace for repo '{}' at {} on branch '{}'",
            repo.name,
            repo.path.display(),
            branch_name
        );

        let repo_path = repo.path.clone();
        let repo_name = repo.name.clone();
        let branch = branch_name.to_string();
        let target_branch = input.target_branch.clone();

        // Guard against clobbering uncommitted work, then check out the branch.
        // Run the blocking git work off the async runtime.
        tokio::task::spawn_blocking(move || -> Result<(), WorkspaceError> {
            let git = GitService::new();
            let (uncommitted, untracked) = git.get_worktree_change_counts(&repo_path)?;
            if uncommitted > 0 || untracked > 0 {
                return Err(WorkspaceError::InPlaceDirtyWorkingTree {
                    repo_name: repo_name.clone(),
                });
            }
            git.checkout_branch_in_place(&repo_path, &branch, &target_branch)?;
            Ok(())
        })
        .await
        .map_err(|e| {
            WorkspaceError::PartialCreation(format!("in-place checkout join error: {e}"))
        })??;

        Ok(WorktreeContainer {
            workspace_dir: repo.path.clone(),
            worktrees: vec![RepoWorktree {
                repo_id: repo.id,
                repo_name: repo.name.clone(),
                source_repo_path: repo.path.clone(),
                worktree_path: repo.path.clone(),
            }],
        })
    }

    /// Create a console workspace: the agent attaches to the repo's own working
    /// tree on whatever branch it is *already* on. Unlike in-place, NO branch is
    /// created or checked out, the tree is NOT required to be clean, and nothing
    /// on disk is mutated. The "workspace dir" IS the repo path.
    async fn create_console_workspace(
        repos: &[RepoWorkspaceInput],
    ) -> Result<WorktreeContainer, WorkspaceError> {
        if repos.len() != 1 {
            return Err(WorkspaceError::InPlaceRequiresSingleRepo(repos.len()));
        }
        let input = &repos[0];
        let repo = &input.repo;

        info!(
            "Creating console workspace for repo '{}' at {} (current branch, no checkout)",
            repo.name,
            repo.path.display(),
        );

        Ok(WorktreeContainer {
            workspace_dir: repo.path.clone(),
            worktrees: vec![RepoWorktree {
                repo_id: repo.id,
                repo_name: repo.name.clone(),
                source_repo_path: repo.path.clone(),
                worktree_path: repo.path.clone(),
            }],
        })
    }

    /// Ensure all worktrees in a workspace exist (for cold restart scenarios)
    pub async fn ensure_workspace_exists(
        workspace_dir: &Path,
        repos: &[RepoWorkspaceInput],
        branch_name: &str,
        kind: WorkspaceKind,
    ) -> Result<(), WorkspaceError> {
        if repos.is_empty() {
            return Err(WorkspaceError::NoRepositories);
        }

        // Console workspaces attach to the repo's own working tree on its current
        // branch. Nothing is materialized, no branch is created or checked out —
        // there is nothing to "ensure" on disk.
        if kind.is_console() {
            if repos.len() != 1 {
                return Err(WorkspaceError::InPlaceRequiresSingleRepo(repos.len()));
            }
            return Ok(());
        }

        // In-place workspaces are backed by the repo's own working tree; there is
        // no worktree to recreate. Just make sure the branch is checked out.
        if kind.is_in_place() {
            if repos.len() != 1 {
                return Err(WorkspaceError::InPlaceRequiresSingleRepo(repos.len()));
            }
            let input = &repos[0];
            let repo_path = input.repo.path.clone();
            let branch = branch_name.to_string();
            let target_branch = input.target_branch.clone();
            tokio::task::spawn_blocking(move || -> Result<(), WorkspaceError> {
                let git = GitService::new();
                if git.get_current_branch(&repo_path)? != branch {
                    git.checkout_branch_in_place(&repo_path, &branch, &target_branch)?;
                }
                Ok(())
            })
            .await
            .map_err(|e| {
                WorkspaceError::PartialCreation(format!("in-place ensure join error: {e}"))
            })??;
            return Ok(());
        }

        // Try legacy migration first (single repo projects only)
        // Old layout had worktree directly at workspace_dir; new layout has it at workspace_dir/{repo_name}
        if repos.len() == 1 && Self::migrate_legacy_worktree(workspace_dir, &repos[0].repo).await? {
            return Ok(());
        }

        if !workspace_dir.exists() {
            tokio::fs::create_dir_all(workspace_dir).await?;
        }

        let git = GitService::new();

        for input in repos {
            let repo = &input.repo;
            let worktree_path = workspace_dir.join(&repo.name);

            debug!(
                "Ensuring worktree exists for repo '{}' at {}",
                repo.name,
                worktree_path.display()
            );

            if git.check_branch_exists(&repo.path, branch_name)? {
                WorktreeManager::ensure_worktree_exists(&repo.path, branch_name, &worktree_path)
                    .await?;
            } else {
                info!(
                    "Workspace branch '{}' missing in repo '{}'; creating from target branch '{}'",
                    branch_name, repo.name, input.target_branch
                );
                WorktreeManager::create_worktree(
                    &repo.path,
                    branch_name,
                    &worktree_path,
                    &input.target_branch,
                    true,
                )
                .await?;
            }
        }

        Ok(())
    }

    /// Clean up all worktrees in a workspace.
    ///
    /// For workspaces backed by the repo's own working tree (`InPlace`/`Console`)
    /// this is a no-op on disk: the workspace IS the user's own repo, so we must
    /// NOT remove worktrees or delete the directory. Branch restoration (in-place
    /// only) is handled separately by the deletion path on the real repo.
    pub async fn cleanup_workspace(
        workspace_dir: &Path,
        repos: &[Repo],
        kind: WorkspaceKind,
    ) -> Result<(), WorkspaceError> {
        if kind.uses_repo_working_tree() {
            info!(
                "Skipping filesystem cleanup for repo-working-tree workspace at {} (repo working tree is preserved)",
                workspace_dir.display()
            );
            return Ok(());
        }

        info!("Cleaning up workspace at {}", workspace_dir.display());

        let cleanup_data: Vec<WorktreeCleanup> = repos
            .iter()
            .map(|repo| {
                let worktree_path = workspace_dir.join(&repo.name);
                WorktreeCleanup::new(worktree_path, Some(repo.path.clone()))
            })
            .collect();

        WorktreeManager::batch_cleanup_worktrees(&cleanup_data).await?;

        // Remove the workspace directory itself
        if workspace_dir.exists()
            && let Err(e) = tokio::fs::remove_dir_all(workspace_dir).await
        {
            debug!(
                "Could not remove workspace directory {}: {}",
                workspace_dir.display(),
                e
            );
        }

        Ok(())
    }

    /// Get the base directory for workspaces (same as worktree base dir)
    pub fn get_workspace_base_dir() -> PathBuf {
        WorktreeManager::get_worktree_base_dir()
    }

    /// Migrate a legacy single-worktree layout to the new workspace layout.
    /// Old layout: workspace_dir IS the worktree
    /// New layout: workspace_dir contains worktrees at workspace_dir/{repo_name}
    ///
    /// Returns Ok(true) if migration was performed, Ok(false) if no migration needed.
    async fn migrate_legacy_worktree(
        workspace_dir: &Path,
        repo: &Repo,
    ) -> Result<bool, WorkspaceError> {
        let expected_worktree_path = workspace_dir.join(&repo.name);

        // Detect old-style: workspace_dir exists AND has .git file (worktree marker)
        // AND expected new location doesn't exist
        let git_file = workspace_dir.join(".git");
        let is_old_style = workspace_dir.exists()
            && git_file.exists()
            && git_file.is_file() // .git file = worktree, .git dir = main repo
            && !expected_worktree_path.exists();

        if !is_old_style {
            return Ok(false);
        }

        info!(
            "Detected legacy worktree at {}, migrating to new layout",
            workspace_dir.display()
        );

        // Move old worktree to temp location (can't move into subdirectory of itself)
        let temp_name = format!(
            "{}-migrating",
            workspace_dir
                .file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        );
        let temp_path = workspace_dir.with_file_name(temp_name);

        WorktreeManager::move_worktree(&repo.path, workspace_dir, &temp_path).await?;

        // Create new workspace directory
        tokio::fs::create_dir_all(workspace_dir).await?;

        // Move worktree to final location using git worktree move
        WorktreeManager::move_worktree(&repo.path, &temp_path, &expected_worktree_path).await?;

        if temp_path.exists() {
            let _ = tokio::fs::remove_dir_all(&temp_path).await;
        }

        info!(
            "Successfully migrated legacy worktree to {}",
            expected_worktree_path.display()
        );

        Ok(true)
    }

    /// Helper to cleanup worktrees during rollback
    async fn cleanup_created_worktrees(worktrees: &[RepoWorktree]) {
        for worktree in worktrees {
            let cleanup = WorktreeCleanup::new(
                worktree.worktree_path.clone(),
                Some(worktree.source_repo_path.clone()),
            );

            if let Err(e) = WorktreeManager::cleanup_worktree(&cleanup).await {
                error!(
                    "Failed to cleanup worktree '{}' during rollback: {}",
                    worktree.repo_name, e
                );
            }
        }
    }

    pub async fn cleanup_orphan_workspaces(&self) {
        if std::env::var("DISABLE_WORKTREE_CLEANUP").is_ok() {
            info!(
                "Orphan workspace cleanup is disabled via DISABLE_WORKTREE_CLEANUP environment variable"
            );
            return;
        }

        // Always clean up the default directory
        let default_dir = WorktreeManager::get_default_worktree_base_dir();
        self.cleanup_orphans_in_directory(&default_dir).await;

        // Also clean up custom directory if it's different from the default
        let current_dir = Self::get_workspace_base_dir();
        if current_dir != default_dir {
            self.cleanup_orphans_in_directory(&current_dir).await;
        }
    }

    async fn cleanup_orphans_in_directory(&self, workspace_base_dir: &Path) {
        if !workspace_base_dir.exists() {
            debug!(
                "Workspace base directory {} does not exist, skipping orphan cleanup",
                workspace_base_dir.display()
            );
            return;
        }

        let entries = match std::fs::read_dir(workspace_base_dir) {
            Ok(entries) => entries,
            Err(e) => {
                error!(
                    "Failed to read workspace base directory {}: {}",
                    workspace_base_dir.display(),
                    e
                );
                return;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("Failed to read directory entry: {}", e);
                    continue;
                }
            };

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let workspace_path_str = path.to_string_lossy().to_string();
            if let Ok(false) =
                DbWorkspace::container_ref_exists(&self.db.pool, &workspace_path_str).await
            {
                info!("Found orphaned workspace: {}", workspace_path_str);
                if let Err(e) = Self::cleanup_workspace_without_repos(&path).await {
                    error!(
                        "Failed to remove orphaned workspace {}: {}",
                        workspace_path_str, e
                    );
                } else {
                    info!(
                        "Successfully removed orphaned workspace: {}",
                        workspace_path_str
                    );
                }
            }
        }
    }

    async fn cleanup_workspace_without_repos(workspace_dir: &Path) -> Result<(), WorkspaceError> {
        info!(
            "Cleaning up orphaned workspace at {}",
            workspace_dir.display()
        );

        let entries = match std::fs::read_dir(workspace_dir) {
            Ok(entries) => entries,
            Err(e) => {
                debug!(
                    "Cannot read workspace directory {}, attempting direct removal: {}",
                    workspace_dir.display(),
                    e
                );
                return tokio::fs::remove_dir_all(workspace_dir)
                    .await
                    .map_err(WorkspaceError::Io);
            }
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir()
                && let Err(e) = WorktreeManager::cleanup_suspected_worktree(&path).await
            {
                warn!("Failed to cleanup suspected worktree: {}", e);
            }
        }

        if workspace_dir.exists()
            && let Err(e) = tokio::fs::remove_dir_all(workspace_dir).await
        {
            debug!(
                "Could not remove workspace directory {}: {}",
                workspace_dir.display(),
                e
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod in_place_tests {
    use db::models::repo::Repo;
    use git::GitService;
    use uuid::Uuid;

    use super::{RepoWorkspaceInput, WorkspaceKind, WorkspaceManager};

    fn make_repo(path: std::path::PathBuf) -> Repo {
        let now = chrono::Utc::now();
        Repo {
            id: Uuid::new_v4(),
            path,
            name: "my-repo".to_string(),
            display_name: "my-repo".to_string(),
            setup_script: None,
            cleanup_script: None,
            archive_script: None,
            copy_files: None,
            parallel_setup_script: false,
            dev_server_script: None,
            default_target_branch: None,
            default_working_dir: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Data-loss guard: in-place cleanup must NEVER touch the repo's working tree,
    /// `.git`, or committed files. The workspace dir IS the user's repo.
    #[tokio::test]
    async fn in_place_cleanup_preserves_repo() {
        let td = tempfile::TempDir::new().unwrap();
        let repo_path = td.path().join("repo");
        let git = GitService::new();
        git.initialize_repo_with_main_branch(&repo_path).unwrap();

        let sentinel = repo_path.join("KEEP_ME.txt");
        std::fs::write(&sentinel, b"do not delete the user's repo").unwrap();

        let repo = make_repo(repo_path.clone());

        // In-place cleanup: workspace_dir == repo_path, kind == InPlace.
        WorkspaceManager::cleanup_workspace(&repo_path, &[repo], WorkspaceKind::InPlace)
            .await
            .unwrap();

        assert!(
            repo_path.exists(),
            "repo root must survive in-place cleanup"
        );
        assert!(
            repo_path.join(".git").exists(),
            "repo .git must survive in-place cleanup"
        );
        assert!(
            sentinel.exists(),
            "sentinel file in repo working tree must survive in-place cleanup"
        );
        assert_eq!(
            std::fs::read(&sentinel).unwrap(),
            b"do not delete the user's repo",
            "sentinel contents must be unchanged"
        );
    }

    /// In-place creation checks out the workspace branch directly in the repo.
    #[tokio::test]
    async fn in_place_create_checks_out_branch_in_repo() {
        let td = tempfile::TempDir::new().unwrap();
        let repo_path = td.path().join("repo");
        let git = GitService::new();
        git.initialize_repo_with_main_branch(&repo_path).unwrap();

        let repo = make_repo(repo_path.clone());
        let input = RepoWorkspaceInput::new(repo, "main".to_string());

        let container = WorkspaceManager::create_workspace(
            // workspace_dir is ignored for in-place (repo path is used)
            std::path::Path::new("/unused"),
            std::slice::from_ref(&input),
            "vk/in-place-branch",
            WorkspaceKind::InPlace,
        )
        .await
        .unwrap();

        // The workspace dir resolves to the repo itself, no new dir created.
        assert_eq!(container.workspace_dir, repo_path);
        // The repo is now on the workspace branch.
        assert_eq!(
            git.get_current_branch(&repo_path).unwrap(),
            "vk/in-place-branch"
        );
    }

    /// In-place creation refuses to run when the repo has uncommitted changes.
    #[tokio::test]
    async fn in_place_create_rejects_dirty_tree() {
        let td = tempfile::TempDir::new().unwrap();
        let repo_path = td.path().join("repo");
        let git = GitService::new();
        git.initialize_repo_with_main_branch(&repo_path).unwrap();

        // Introduce an uncommitted change.
        std::fs::write(repo_path.join("dirty.txt"), b"uncommitted").unwrap();

        let repo = make_repo(repo_path.clone());
        let input = RepoWorkspaceInput::new(repo, "main".to_string());

        let result = WorkspaceManager::create_workspace(
            std::path::Path::new("/unused"),
            &[input],
            "vk/in-place-branch",
            WorkspaceKind::InPlace,
        )
        .await;

        assert!(
            matches!(
                result,
                Err(super::WorkspaceError::InPlaceDirtyWorkingTree { .. })
            ),
            "expected dirty-tree rejection, got {result:?}"
        );
    }

    /// In-place workspaces are limited to a single repository.
    #[tokio::test]
    async fn in_place_create_rejects_multiple_repos() {
        let td = tempfile::TempDir::new().unwrap();
        let git = GitService::new();

        let repo_a_path = td.path().join("repo-a");
        let repo_b_path = td.path().join("repo-b");
        git.initialize_repo_with_main_branch(&repo_a_path).unwrap();
        git.initialize_repo_with_main_branch(&repo_b_path).unwrap();

        let inputs = vec![
            RepoWorkspaceInput::new(make_repo(repo_a_path), "main".to_string()),
            RepoWorkspaceInput::new(make_repo(repo_b_path), "main".to_string()),
        ];

        let result = WorkspaceManager::create_workspace(
            std::path::Path::new("/unused"),
            &inputs,
            "vk/in-place-branch",
            WorkspaceKind::InPlace,
        )
        .await;

        assert!(
            matches!(
                result,
                Err(super::WorkspaceError::InPlaceRequiresSingleRepo(2))
            ),
            "expected single-repo rejection, got {result:?}"
        );
    }
}
