use sqlx::migrate::Migrator;

static MIGRATOR_SQLITE: Migrator = sqlx::migrate!("../../migrations/sqlite");

#[tokio::test]
async fn sqlite_migrations_can_run_in_memory() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    MIGRATOR_SQLITE
        .run(&pool)
        .await
        .expect("run sqlite migrations");

    // 只要能查询就说明表存在（无数据也 OK）。
    sqlx::query("SELECT 1 FROM auth_user LIMIT 1")
        .fetch_optional(&pool)
        .await
        .expect("auth_user exists");
    sqlx::query("SELECT 1 FROM fund LIMIT 1")
        .fetch_optional(&pool)
        .await
        .expect("fund exists");
    sqlx::query("SELECT 1 FROM watchlist LIMIT 1")
        .fetch_optional(&pool)
        .await
        .expect("watchlist exists");
}

