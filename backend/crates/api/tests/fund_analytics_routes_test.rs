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
async fn fund_analytics_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000001/analytics?range=3T&source=tiantian")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn fund_analytics_returns_metrics_from_seeded_nav_history() {
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

    for (i, d) in ["2026-02-12", "2026-02-13", "2026-02-14"].iter().enumerate() {
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
        .bind("1.0")
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
    .bind("2026-02-14")
    .bind("3M")
    .bind("1.5000")
    .bind("chinabond")
    .bind("2026-02-14 12:00:00")
    .execute(&pool)
    .await
    .expect("seed rf");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000001/analytics?range=3T&source=tiantian")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert_eq!(v["fund_code"], "000001");
    assert_eq!(v["range"], "3T");
    assert_eq!(v["source"], "tiantian");
    assert_eq!(v["rf"]["tenor"], "3M");
    assert_eq!(v["rf"]["rate_percent"], "1.5000");
    assert_eq!(v["metrics"]["max_drawdown"], "0");
    assert_eq!(v["metrics"]["ann_vol"], "0");
    assert_eq!(v["metrics"]["sharpe"], Value::Null);
}

