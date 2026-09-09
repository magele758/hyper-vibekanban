//! Local project CRUD used by lite Desktop (SQLite only, no Remote).

use std::{path::Path, str::FromStr};

use db::models::{
    project::Project,
    project_repo::ProjectRepo,
    project_workspace::ProjectWorkspace,
    repo::Repo,
    workspace::{CreateWorkspace, Workspace, WorkspaceKind},
};
use sqlx::{
    Pool, Sqlite,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use uuid::Uuid;

async fn fresh_migrated_pool() -> Pool<Sqlite> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .expect("valid sqlite url")
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Memory);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("connect in-memory sqlite");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations apply");

    pool
}

#[tokio::test]
async fn local_project_crud_attach_repo_and_workspace() {
    let pool = fresh_migrated_pool().await;

    let project = Project::create(&pool, "Lite Demo", None)
        .await
        .expect("create project");
    assert_eq!(project.name, "Lite Demo");
    assert!(project.remote_project_id.is_none());

    let listed = Project::find_all(&pool).await.expect("list projects");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, project.id);

    let found = Project::find_by_id(&pool, project.id)
        .await
        .expect("find")
        .expect("exists");
    assert_eq!(found.name, "Lite Demo");

    let updated = Project::update(&pool, project.id, "Lite Demo Renamed", None)
        .await
        .expect("update");
    assert_eq!(updated.name, "Lite Demo Renamed");

    let repo = Repo::find_or_create(&pool, Path::new("/tmp/lite-demo-repo"), "demo", false)
        .await
        .expect("create repo");
    ProjectRepo::attach(&pool, project.id, repo.id)
        .await
        .expect("attach repo");
    let repos = ProjectRepo::list_repos_for_project(&pool, project.id)
        .await
        .expect("list repos");
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].id, repo.id);

    let workspace = Workspace::create(
        &pool,
        &CreateWorkspace {
            branch: "main".to_string(),
            name: Some("first workspace".to_string()),
            kind: WorkspaceKind::Worktree,
        },
        Uuid::new_v4(),
    )
    .await
    .expect("create workspace");
    ProjectWorkspace::attach(&pool, project.id, workspace.id)
        .await
        .expect("attach workspace");

    let workspaces = ProjectWorkspace::list_workspaces_for_project(&pool, project.id)
        .await
        .expect("list workspaces");
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].id, workspace.id);
    assert_eq!(
        ProjectWorkspace::count_by_project(&pool, project.id)
            .await
            .expect("count"),
        1
    );

    ProjectRepo::detach(&pool, project.id, repo.id)
        .await
        .expect("detach repo");
    assert!(
        ProjectRepo::list_repos_for_project(&pool, project.id)
            .await
            .expect("list after detach")
            .is_empty()
    );

    let deleted = Project::delete(&pool, project.id).await.expect("delete");
    assert_eq!(deleted, 1);
    assert!(
        Project::find_by_id(&pool, project.id)
            .await
            .expect("find after delete")
            .is_none()
    );
    assert_eq!(
        ProjectWorkspace::count_by_project(&pool, project.id)
            .await
            .expect("count after delete"),
        0
    );
}

#[tokio::test]
async fn normalize_name_is_used_by_create_path() {
    assert_eq!(Project::normalize_name("  Alpha  ").unwrap(), "Alpha");
    assert!(Project::normalize_name("A").is_err());
}
