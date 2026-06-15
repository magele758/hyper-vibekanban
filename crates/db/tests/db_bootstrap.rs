//! Smoke test for the **production** database bootstrap path.
//!
//! `crates/db/tests/migrations.rs` validates that the migration *files* apply
//! to a hand-built pool. This test instead drives the real `DBService::new()`
//! constructor the server/desktop use at startup — exercising its connect
//! options (`create_if_missing`, journal mode), the on-disk SQLite file
//! creation, and the automatic migration run — against an isolated temp asset
//! directory via the `VK_ASSET_DIR` override.
//!
//! It lives in its own integration-test binary so the process-global
//! `VK_ASSET_DIR` env var it sets cannot leak into other tests.

use db::DBService;
use sqlx::Row;

#[tokio::test]
async fn db_service_new_bootstraps_a_fresh_database_on_disk() {
    let tmp = tempfile::tempdir().expect("temp asset dir");

    // Point every asset-dir-derived path (the SQLite DB included) at the temp
    // dir. Safe here: this is the only test in this binary, set before any
    // asset_dir() call. `set_var` is unsafe under edition 2024.
    unsafe {
        std::env::set_var("VK_ASSET_DIR", tmp.path());
    }

    // The real startup constructor: connect + run all migrations on a brand-new
    // on-disk SQLite file. If migrations or connect options regress, this fails.
    let db = DBService::new()
        .await
        .expect("DBService::new must bootstrap a fresh database");

    // The DB file was actually created in the override directory.
    let db_file = tmp.path().join("db.v2.sqlite");
    assert!(
        db_file.exists(),
        "DBService::new should create db.v2.sqlite in VK_ASSET_DIR"
    );

    // Migrations ran: the bookkeeping table exists and has applied versions.
    let applied: i64 = sqlx::query("SELECT COUNT(*) AS n FROM _sqlx_migrations")
        .fetch_one(&db.pool)
        .await
        .expect("query _sqlx_migrations")
        .get("n");
    assert!(
        applied > 0,
        "expected migrations to be recorded, got {applied}"
    );

    // A core table from the migrations is queryable end-to-end through the
    // production pool (not just the schema table).
    let workspace_count: i64 = sqlx::query("SELECT COUNT(*) AS n FROM workspaces")
        .fetch_one(&db.pool)
        .await
        .expect("workspaces table must exist and be queryable")
        .get("n");
    assert_eq!(
        workspace_count, 0,
        "fresh database starts with no workspaces"
    );
}
