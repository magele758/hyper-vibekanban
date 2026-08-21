//! Concurrent Sessions on one Issue workspace.
//!
//! These tests drive the shipped start / stop / turn-finish functions used by
//! `start_execution`, session stop/reset, and auto-commit — not a reimplementation.

use std::str::FromStr;

use db::models::{
    execution_process::{
        CodingAgentTurnAdmission, CreateExecutionProcess, ExecutionProcess,
        ExecutionProcessRunReason, ExecutionProcessStatus, SharedTreeWritePolicy,
    },
    session::{CreateSession, Session},
    workspace::{CreateWorkspace, Workspace, WorkspaceKind},
};
use executors::{
    actions::{
        ExecutorAction, ExecutorActionType, coding_agent_initial::CodingAgentInitialRequest,
    },
    executors::BaseCodingAgent,
    profile::ExecutorConfig,
};
use sqlx::{
    Pool, Sqlite, SqlitePool,
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
        .expect("all migrations apply cleanly on a fresh database");

    pool
}

async fn create_workspace(pool: &SqlitePool) -> Workspace {
    Workspace::create(
        pool,
        &CreateWorkspace {
            branch: format!("vk/{}", Uuid::new_v4().simple()),
            name: Some("concurrent-sessions".to_string()),
            kind: WorkspaceKind::Worktree,
        },
        Uuid::new_v4(),
    )
    .await
    .expect("create workspace")
}

async fn create_session(pool: &SqlitePool, workspace_id: Uuid, name: &str) -> Session {
    Session::create(
        pool,
        &CreateSession {
            executor: Some("CLAUDE_CODE".to_string()),
            name: Some(name.to_string()),
        },
        Uuid::new_v4(),
        workspace_id,
    )
    .await
    .expect("create session")
}

fn coding_agent_action(prompt: &str) -> ExecutorAction {
    ExecutorAction::new(
        ExecutorActionType::CodingAgentInitialRequest(CodingAgentInitialRequest {
            prompt: prompt.to_string(),
            executor_config: ExecutorConfig::new(BaseCodingAgent::ClaudeCode),
            working_dir: None,
        }),
        None,
    )
}

/// The start/follow-up path: admit (session-scoped), then
/// `ExecutionProcess::create` (same insert `start_execution` uses).
async fn start_coding_agent_turn(
    pool: &SqlitePool,
    session_id: Uuid,
    prompt: &str,
) -> Result<ExecutionProcess, String> {
    match ExecutionProcess::admit_coding_agent_turn(pool, session_id)
        .await
        .expect("admit query")
    {
        CodingAgentTurnAdmission::Allow => {}
        CodingAgentTurnAdmission::SessionAlreadyRunning => {
            return Err("session already running".to_string());
        }
    }

    ExecutionProcess::create(
        pool,
        &CreateExecutionProcess {
            session_id,
            executor_action: coding_agent_action(prompt),
            run_reason: ExecutionProcessRunReason::CodingAgent,
        },
        Uuid::new_v4(),
        &[],
    )
    .await
    .map_err(|e| e.to_string())
}

async fn start_two_sessions(
    pool: &SqlitePool,
) -> (
    Workspace,
    Session,
    Session,
    ExecutionProcess,
    ExecutionProcess,
) {
    let workspace = create_workspace(pool).await;
    let session_a = create_session(pool, workspace.id, "A").await;
    let session_b = create_session(pool, workspace.id, "B").await;

    let process_a = start_coding_agent_turn(pool, session_a.id, "work on A")
        .await
        .expect("session A turn must start");
    let process_b = start_coding_agent_turn(pool, session_b.id, "work on B")
        .await
        .expect("session B turn must start while A is running");

    (workspace, session_a, session_b, process_a, process_b)
}

#[tokio::test]
async fn two_sessions_on_one_workspace_can_both_run_coding_agents() {
    let pool = fresh_migrated_pool().await;
    let (_workspace, session_a, session_b, process_a, process_b) = start_two_sessions(&pool).await;

    assert_eq!(process_a.status, ExecutionProcessStatus::Running);
    assert_eq!(process_b.status, ExecutionProcessStatus::Running);
    assert_eq!(process_a.session_id, session_a.id);
    assert_eq!(process_b.session_id, session_b.id);
    assert_ne!(process_a.id, process_b.id);

    assert!(
        ExecutionProcess::has_running_coding_agent_for_session(&pool, session_a.id)
            .await
            .unwrap()
    );
    assert!(
        ExecutionProcess::has_running_coding_agent_for_session(&pool, session_b.id)
            .await
            .unwrap()
    );

    // One runner per Session: a second turn on A is rejected even though
    // concurrency across sessions is allowed.
    let second_a = start_coding_agent_turn(&pool, session_a.id, "another turn on A").await;
    assert_eq!(
        second_a.err().as_deref(),
        Some("session already running"),
        "same session must keep a single coding-agent runner"
    );
}

#[tokio::test]
async fn session_stop_does_not_kill_the_other_sessions_runner() {
    let pool = fresh_migrated_pool().await;
    let (workspace, session_a, session_b, process_a, process_b) = start_two_sessions(&pool).await;

    let session_b_targets =
        ExecutionProcess::find_stop_targets_for_session(&pool, session_b.id, false)
            .await
            .expect("session stop targets");
    assert_eq!(
        session_b_targets.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![process_b.id],
        "session stop must target only that session"
    );
    assert!(
        session_b_targets.iter().all(|p| p.id != process_a.id),
        "session B stop must not include session A's process"
    );

    // Shipped completion update used by stop_execution after it claims the child.
    ExecutionProcess::update_completion(&pool, process_b.id, ExecutionProcessStatus::Killed, None)
        .await
        .expect("stop session B");

    let process_a_after = ExecutionProcess::find_by_id(&pool, process_a.id)
        .await
        .expect("load A")
        .expect("A still exists");
    assert_eq!(
        process_a_after.status,
        ExecutionProcessStatus::Running,
        "stopping session B must leave session A running"
    );

    let process_b_after = ExecutionProcess::find_by_id(&pool, process_b.id)
        .await
        .expect("load B")
        .expect("B still exists");
    assert_eq!(process_b_after.status, ExecutionProcessStatus::Killed);

    let workspace_targets =
        ExecutionProcess::find_stop_targets_for_workspace(&pool, workspace.id, false)
            .await
            .expect("workspace stop targets");
    assert_eq!(
        workspace_targets.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![process_a.id],
        "workspace-level stop still includes remaining runners"
    );
    assert!(
        ExecutionProcess::has_running_coding_agent_for_session(&pool, session_a.id)
            .await
            .unwrap()
    );
    assert!(
        !ExecutionProcess::has_running_coding_agent_for_session(&pool, session_b.id)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn finishing_one_session_defers_shared_tree_writes_while_another_runs() {
    let pool = fresh_migrated_pool().await;
    let (workspace, session_a, session_b, process_a, process_b) = start_two_sessions(&pool).await;

    // While B is still running, finishing A must not auto-commit / git-reset /
    // cleanup the shared tree (no mid-flight inter-session merge).
    let policy_while_b_runs =
        ExecutionProcess::shared_tree_write_policy(&pool, workspace.id, session_a.id)
            .await
            .expect("policy for finishing A");
    assert_eq!(policy_while_b_runs, SharedTreeWritePolicy::Defer);
    assert!(
        !policy_while_b_runs.allows_shared_tree_git_write(),
        "must not combine or commit the shared tree while another agent writes"
    );

    // Reset of B must also refuse to git-reset the shared tree.
    let reset_policy =
        ExecutionProcess::shared_tree_write_policy(&pool, workspace.id, session_b.id)
            .await
            .expect("policy for resetting B");
    assert_eq!(reset_policy, SharedTreeWritePolicy::Defer);

    // drop_at_and_after is session-scoped (reset history), not a tree combine.
    ExecutionProcess::drop_at_and_after(&pool, session_b.id, process_b.id)
        .await
        .expect("drop B history");
    let a_after_drop = ExecutionProcess::find_by_id(&pool, process_a.id)
        .await
        .expect("load A")
        .expect("A exists");
    assert!(
        !a_after_drop.dropped,
        "resetting session B must not drop session A's process"
    );
    assert_eq!(a_after_drop.status, ExecutionProcessStatus::Running);

    // After B is no longer running, A (or the last idle session) owns the
    // shared tree again — the existing workspace merge path, not a new combiner.
    ExecutionProcess::update_completion(
        &pool,
        process_b.id,
        ExecutionProcessStatus::Completed,
        Some(0),
    )
    .await
    .expect("B finished");

    let policy_after_b =
        ExecutionProcess::shared_tree_write_policy(&pool, workspace.id, session_a.id)
            .await
            .expect("policy after B finished");
    assert_eq!(policy_after_b, SharedTreeWritePolicy::Exclusive);
    assert!(policy_after_b.allows_shared_tree_git_write());
}

#[tokio::test]
async fn single_session_workspace_keeps_exclusive_tree_policy() {
    let pool = fresh_migrated_pool().await;
    let workspace = create_workspace(&pool).await;
    let session = create_session(&pool, workspace.id, "only").await;
    let process = start_coding_agent_turn(&pool, session.id, "solo")
        .await
        .expect("single session starts");
    assert_eq!(process.status, ExecutionProcessStatus::Running);

    let policy = ExecutionProcess::shared_tree_write_policy(&pool, workspace.id, session.id)
        .await
        .expect("solo policy");
    assert_eq!(
        policy,
        SharedTreeWritePolicy::Exclusive,
        "one-session workspaces must still auto-commit / merge as today"
    );

    ExecutionProcess::update_completion(
        &pool,
        process.id,
        ExecutionProcessStatus::Completed,
        Some(0),
    )
    .await
    .expect("solo finished");

    let after = ExecutionProcess::shared_tree_write_policy(&pool, workspace.id, session.id)
        .await
        .expect("solo after finish");
    assert_eq!(after, SharedTreeWritePolicy::Exclusive);
}

#[test]
fn shipped_start_stop_and_commit_paths_use_these_gates() {
    let start = include_str!("../../services/src/services/container.rs");
    assert!(
        start.contains("admit_coding_agent_turn"),
        "start_execution must call admit_coding_agent_turn"
    );
    assert!(
        start.contains("try_stop_session"),
        "reset/stop must be session-scoped via try_stop_session"
    );
    assert!(
        start.contains("shared_tree_write_policy"),
        "reset must consult shared_tree_write_policy before git-reset"
    );

    let finish = include_str!("../../local-deployment/src/container.rs");
    assert!(
        finish.contains("shared_tree_write_policy"),
        "turn-finish auto-commit must consult shared_tree_write_policy"
    );
    assert!(
        !finish.contains("inter_session_merge") && !finish.contains("merge_session"),
        "must not add a mid-flight inter-session merge"
    );

    let follow_up = include_str!("../../server/src/routes/sessions/mod.rs");
    assert!(
        follow_up.contains("start_execution"),
        "follow-up must keep using start_execution"
    );
    assert!(
        follow_up.contains("try_stop_session"),
        "session stop route must call try_stop_session"
    );
}
