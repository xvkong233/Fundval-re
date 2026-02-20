use sqlx::migrate::Migrator;

static MIGRATOR_SQLITE: Migrator = sqlx::migrate!("../../migrations/sqlite");

#[tokio::test]
async fn sqlite_migrations_create_crawl_tables() {
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

    sqlx::query("SELECT 1 FROM crawl_job LIMIT 1")
        .fetch_optional(&pool)
        .await
        .expect("crawl_job exists");
    sqlx::query("SELECT 1 FROM crawl_state LIMIT 1")
        .fetch_optional(&pool)
        .await
        .expect("crawl_state exists");
}
