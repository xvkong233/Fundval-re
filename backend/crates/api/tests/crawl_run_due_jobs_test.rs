use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

struct TempSqliteDb {
    path: PathBuf,
}

impl Drop for TempSqliteDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn new_temp_sqlite_url() -> (String, TempSqliteDb) {
    let mut path = std::env::temp_dir();
    path.push(format!("fundval-crawl-{}.sqlite", Uuid::new_v4()));
    std::fs::File::create(&path).expect("create temp sqlite file");

    // sqlx sqlite 支持：sqlite:data.db / sqlite://data.db / sqlite:///abs/path
    // Windows 下绝对路径优先使用 `sqlite:C:/...` 形式，避免 URI/盘符解析差异。
    let abs = path.to_string_lossy().replace('\\', "/");
    (format!("sqlite:{abs}"), TempSqliteDb { path })
}

async fn new_pool() -> (sqlx::AnyPool, TempSqliteDb) {
    sqlx::any::install_default_drivers();

    let (url, db) = new_temp_sqlite_url();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect sqlite temp file");

    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    (pool, db)
}

#[tokio::test]
async fn run_due_jobs_marks_success_and_increments_attempt() {
    let (pool, _db) = new_pool().await;

    sqlx::query(
        r#"
        INSERT INTO crawl_job (
          id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at
        ) VALUES (
          'job-1', 'noop', 'A', 'tiantian', 1, DATETIME(CURRENT_TIMESTAMP, '-1 day'), 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed job");

    let ran = api::crawl::scheduler::run_due_jobs(&pool, 10, |_| async { Ok(()) })
        .await
        .expect("run ok");
    assert_eq!(ran, 1);

    let row = sqlx::query(
        "SELECT status, attempt, CAST(last_ok_at AS TEXT) as last_ok_at, last_error FROM crawl_job WHERE id = 'job-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("select");

    assert_eq!(row.get::<String, _>("status"), "queued");
    assert_eq!(row.get::<i64, _>("attempt"), 1);
    let last_ok_at: Option<String> = row.get("last_ok_at");
    assert!(last_ok_at.unwrap_or_default().trim().len() > 0);
    let last_error: Option<String> = row.get("last_error");
    assert!(last_error.is_none());
}

#[tokio::test]
async fn run_due_jobs_records_error_and_backoff() {
    let (pool, _db) = new_pool().await;

    sqlx::query(
        r#"
        INSERT INTO crawl_job (
          id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at
        ) VALUES (
          'job-2', 'noop', 'B', 'tiantian', 1, DATETIME(CURRENT_TIMESTAMP, '-1 day'), 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed job");

    let ran = api::crawl::scheduler::run_due_jobs(&pool, 10, |_| async { Err("boom".to_string()) })
        .await
        .expect("run ok");
    assert_eq!(ran, 1);

    let row = sqlx::query("SELECT status, attempt, CAST(last_ok_at AS TEXT) as last_ok_at, last_error FROM crawl_job WHERE id = 'job-2'")
        .fetch_one(&pool)
        .await
        .expect("select");

    assert_eq!(row.get::<String, _>("status"), "queued");
    assert_eq!(row.get::<i64, _>("attempt"), 1);
    let last_ok_at: Option<String> = row.get("last_ok_at");
    assert!(last_ok_at.is_none());
    let last_error: Option<String> = row.get("last_error");
    assert_eq!(last_error.unwrap_or_default(), "boom");
}
