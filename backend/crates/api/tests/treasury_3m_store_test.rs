use api::rates::treasury_3m::Treasury3mRate;
use sqlx::Row;

#[tokio::test]
async fn upsert_risk_free_rate_3m_inserts_and_updates() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    api::rates::treasury_3m::upsert_risk_free_rate_3m(
        &pool,
        &Treasury3mRate {
            rate_date: "2026-02-14".to_string(),
            rate_percent: 1.3428,
        },
        "chinabond",
    )
    .await
    .expect("upsert ok");

    api::rates::treasury_3m::upsert_risk_free_rate_3m(
        &pool,
        &Treasury3mRate {
            rate_date: "2026-02-14".to_string(),
            rate_percent: 1.5000,
        },
        "chinabond",
    )
    .await
    .expect("upsert ok");

    let row = sqlx::query(
        "SELECT CAST(rate AS TEXT) as rate FROM risk_free_rate_daily WHERE rate_date = '2026-02-14' AND tenor = '3M' AND source = 'chinabond'",
    )
    .fetch_one(&pool)
    .await
    .expect("select");

    let rate: String = row.get("rate");
    assert!(rate.contains("1.5"), "rate={rate}");
}
