//! Regression tests for the database layer.
//!
//! These guard the two highest-risk parts of `crates/db`:
//!  1. **Migrations apply cleanly on a fresh database.** Migrations run
//!     automatically on every user's SQLite DB at startup, so a broken or
//!     mis-ordered migration corrupts/blocks real user data on upgrade. This
//!     test fails fast in CI before a release ever ships.
//!  2. **The `find_expired_for_cleanup` predicate is correct.** Its result
//!     feeds a `remove_dir_all` worktree-deletion path, so a wrong predicate
//!     means data loss. We assert exactly which rows it selects.

use std::str::FromStr;

use db::models::workspace::Workspace;
use sqlx::{
    Pool, Sqlite, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use uuid::Uuid;

/// Build a fresh, isolated in-memory SQLite pool with all migrations applied.
///
/// `max_connections(1)` is required: each connection to `sqlite::memory:` gets
/// its own private database, so a multi-connection pool would migrate one
/// connection and query a different (empty) one. Pinning to a single connection
/// keeps the migrated schema and the test queries on the same in-memory DB.
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

/// Insert a full cleanup-candidate chain: a workspace plus a session plus a
/// **completed** execution process. This mirrors what production rows look
/// like, which matters because `find_expired_for_cleanup` only ever considers
/// workspaces that have at least one completed execution process (its `HAVING`
/// clause compares against `max(updated_at, completed_at)`, and SQLite's scalar
/// `max(x, NULL)` is NULL — so a workspace with no completed process is never
/// expired). Every fixture therefore gets a completed process, and the negative
/// cases are excluded for the *specific* field under test, not for lacking a
/// process.
///
/// Timestamps are set via SQLite's own `datetime('now', ?)` arithmetic (not a
/// bound chrono value) so they match the format the cleanup query compares
/// against.
#[allow(clippy::too_many_arguments)] // fixture builder: explicit fields read clearer than a struct here
async fn insert_candidate(
    pool: &SqlitePool,
    id: Uuid,
    container_ref: Option<&str>,
    branch: &str,
    archived: bool,
    worktree_deleted: bool,
    age_hours: i64,
    process_running: bool,
) {
    insert_candidate_pinned(
        pool,
        id,
        container_ref,
        branch,
        archived,
        worktree_deleted,
        age_hours,
        process_running,
        false,
    )
    .await;
}

/// Same as [`insert_candidate`] but with an explicit `pinned` flag. Pinned
/// workspaces are user-protected and must never be selected for expiry, no
/// matter how stale.
#[allow(clippy::too_many_arguments)] // fixture builder: explicit fields read clearer than a struct here
async fn insert_candidate_pinned(
    pool: &SqlitePool,
    id: Uuid,
    container_ref: Option<&str>,
    branch: &str,
    archived: bool,
    worktree_deleted: bool,
    age_hours: i64,
    process_running: bool,
    pinned: bool,
) {
    let offset = format!("-{age_hours} hours");

    sqlx::query(
        "INSERT INTO workspaces (id, container_ref, branch, archived, pinned, worktree_deleted, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?7, ?5, datetime('now', ?6), datetime('now', ?6))",
    )
    .bind(id)
    .bind(container_ref)
    .bind(branch)
    .bind(archived)
    .bind(worktree_deleted)
    .bind(&offset)
    .bind(pinned)
    .execute(pool)
    .await
    .expect("insert workspace fixture");

    let session_id = Uuid::new_v4();
    sqlx::query("INSERT INTO sessions (id, workspace_id) VALUES (?1, ?2)")
        .bind(session_id)
        .bind(id)
        .execute(pool)
        .await
        .expect("insert session fixture");

    // A "running" process has completed_at = NULL (which excludes the whole
    // workspace from cleanup); a completed process has an old completed_at.
    let process_id = Uuid::new_v4();
    if process_running {
        sqlx::query(
            "INSERT INTO execution_processes (id, session_id, completed_at) VALUES (?1, ?2, NULL)",
        )
        .bind(process_id)
        .bind(session_id)
        .execute(pool)
        .await
        .expect("insert running process fixture");
    } else {
        sqlx::query(
            "INSERT INTO execution_processes (id, session_id, completed_at)
             VALUES (?1, ?2, datetime('now', ?3))",
        )
        .bind(process_id)
        .bind(session_id)
        .bind(&offset)
        .execute(pool)
        .await
        .expect("insert completed process fixture");
    }
}

#[tokio::test]
async fn all_migrations_apply_on_fresh_database() {
    // The act of building the pool runs every migration; if any migration is
    // malformed or out of order this panics. We also assert the migrator ran
    // and recorded at least one version.
    let pool = fresh_migrated_pool().await;

    let applied: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("read _sqlx_migrations");

    assert!(
        applied > 0,
        "expected at least one applied migration, found {applied}"
    );
}

#[tokio::test]
async fn find_expired_for_cleanup_runs_against_real_schema() {
    // Guards that the (complex) cleanup SQL stays valid against the migrated
    // schema. An empty DB must return an empty list, never error.
    let pool = fresh_migrated_pool().await;

    let expired = Workspace::find_expired_for_cleanup(&pool)
        .await
        .expect("cleanup query must be valid against the migrated schema");

    assert!(
        expired.is_empty(),
        "no rows inserted, expected none expired"
    );
}

#[tokio::test]
async fn find_expired_for_cleanup_selects_only_eligible_workspaces() {
    let pool = fresh_migrated_pool().await;

    // (a) Stale, has container_ref, not deleted, not archived, completed
    //     process 100h old -> EXPIRED (>72h).
    let stale = Uuid::new_v4();
    insert_candidate(
        &pool,
        stale,
        Some("/tmp/ws/stale"),
        "stale",
        false,
        false,
        100,
        false,
    )
    .await;

    // (b) Recently active (1h) -> NOT expired (within 72h).
    let fresh = Uuid::new_v4();
    insert_candidate(
        &pool,
        fresh,
        Some("/tmp/ws/fresh"),
        "fresh",
        false,
        false,
        1,
        false,
    )
    .await;

    // (c) No container_ref -> never a cleanup candidate, even when stale.
    let no_ref = Uuid::new_v4();
    insert_candidate(&pool, no_ref, None, "no-ref", false, false, 100, false).await;

    // (d) Already worktree_deleted -> excluded even when stale.
    let deleted = Uuid::new_v4();
    insert_candidate(
        &pool,
        deleted,
        Some("/tmp/ws/deleted"),
        "deleted",
        false,
        true,
        100,
        false,
    )
    .await;

    // (e) Archived + stale beyond the 1h archived threshold -> EXPIRED.
    //     Wide margin (10h) so the localtime/UTC offset cannot flip it.
    let archived = Uuid::new_v4();
    insert_candidate(
        &pool,
        archived,
        Some("/tmp/ws/arch"),
        "arch",
        true,
        false,
        10,
        false,
    )
    .await;

    // (f) Stale but has a STILL-RUNNING process (completed_at NULL) -> excluded;
    //     we must never clean up a workspace with active work.
    let running = Uuid::new_v4();
    insert_candidate(
        &pool,
        running,
        Some("/tmp/ws/running"),
        "running",
        false,
        false,
        100,
        true,
    )
    .await;

    // (g) Stale enough to expire, but PINNED -> excluded. Pinning is a user-set
    //     "do not auto-clean" flag and must win over every staleness threshold.
    let pinned = Uuid::new_v4();
    insert_candidate_pinned(
        &pool,
        pinned,
        Some("/tmp/ws/pinned"),
        "pinned",
        false,
        false,
        100,
        false,
        true,
    )
    .await;

    let expired: Vec<Uuid> = Workspace::find_expired_for_cleanup(&pool)
        .await
        .expect("cleanup query")
        .into_iter()
        .map(|w| w.id)
        .collect();

    assert!(
        expired.contains(&stale),
        "stale workspace should be expired"
    );
    assert!(
        expired.contains(&archived),
        "archived workspace past 1h threshold should be expired"
    );
    assert!(
        !expired.contains(&fresh),
        "recently-active workspace must not be expired"
    );
    assert!(
        !expired.contains(&no_ref),
        "workspace without container_ref must never be a cleanup candidate"
    );
    assert!(
        !expired.contains(&deleted),
        "already-deleted workspace must be excluded"
    );
    assert!(
        !expired.contains(&running),
        "workspace with a running process must never be cleaned up"
    );
    assert!(
        !expired.contains(&pinned),
        "pinned workspace must never be expired, no matter how stale"
    );
}
