use sqlx::migrate::Migrator;

static MIGRATOR_SQLITE: Migrator = sqlx::migrate!("../../migrations/sqlite");
// NOTE: Adding new migration files does not always trigger rebuild; keep this test file changing when needed.

#[tokio::test]
async fn sqlite_migrations_create_sim_tables() {
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

    for table in [
        "sim_run",
        "sim_position",
        "sim_cash_receivable",
        "sim_order",
        "sim_trade",
        "sim_daily_equity",
    ] {
        let row = sqlx::query(
            r#"
            SELECT name
            FROM sqlite_master
            WHERE type='table' AND name=$1
            "#,
        )
        .bind(table)
        .fetch_optional(&pool)
        .await
        .expect("query sqlite_master");

        assert!(row.is_some(), "{table} table should exist");
    }
}
