use sqlx::migrate::Migrator;

static MIGRATOR_SQLITE: Migrator = sqlx::migrate!("../../migrations/sqlite");

#[tokio::test]
async fn sqlite_migrations_create_fund_relate_theme_table() {
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

    let row = sqlx::query(
        r#"
        SELECT name
        FROM sqlite_master
        WHERE type='table' AND name='fund_relate_theme'
        "#,
    )
    .fetch_optional(&pool)
    .await
    .expect("query sqlite_master");

    assert!(row.is_some(), "fund_relate_theme table should exist");
}
