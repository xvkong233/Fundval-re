use axum::{body::Body, http::Request};
use serde_json::Value;
use tower::ServiceExt;

use api::state::AppState;

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn fund_analytics_includes_short_term_strategy() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    sqlx::query(
        r#"
        INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
        VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind("fund-1")
    .bind("000001")
    .bind("测试基金")
    .bind("股票型")
    .execute(&pool)
    .await
    .expect("seed fund");

    for (i, d) in [
        "2026-02-03",
        "2026-02-04",
        "2026-02-05",
        "2026-02-06",
        "2026-02-07",
        "2026-02-10",
        "2026-02-11",
        "2026-02-12",
        "2026-02-13",
        "2026-02-14",
        "2026-02-17",
        "2026-02-18",
    ]
    .iter()
    .enumerate()
    {
        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(format!("nav-{i}"))
        .bind("tiantian")
        .bind("fund-1")
        .bind(*d)
        .bind(format!("{:.4}", 1.0 + (i as f64) * 0.005))
        .execute(&pool)
        .await
        .expect("seed nav");
    }

    sqlx::query(
        r#"
        INSERT INTO risk_free_rate_daily (id, rate_date, tenor, rate, source, fetched_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind("rf-1")
    .bind("2026-02-18")
    .bind("3M")
    .bind("2.0000")
    .bind("chinabond")
    .bind("2026-02-18 12:00:00")
    .execute(&pool)
    .await
    .expect("seed rf");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000001/analytics?range=12T&source=tiantian")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert!(v.get("short_term").is_some());
    assert!(v["short_term"]["combined"]["bucket"].is_string());
}
