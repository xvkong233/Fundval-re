#[tokio::test]
async fn sqlite_anypool_can_connect_and_query() {
    // AnyPool 在运行前需要安装默认 driver（sqlite/postgres/...）
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .expect("query ok");
}

