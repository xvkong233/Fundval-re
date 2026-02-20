use sqlx::migrate::Migrator;

static MIGRATOR_SQLITE: Migrator = sqlx::migrate!("../../migrations/sqlite");

#[tokio::test]
async fn sqlite_migrations_create_risk_free_rate_daily_table() {
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

    sqlx::query("SELECT 1 FROM risk_free_rate_daily LIMIT 1")
        .fetch_optional(&pool)
        .await
        .expect("risk_free_rate_daily exists");
}
