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
async fn rates_endpoints_require_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/rates/risk-free?tenor=3M")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn risk_free_rate_returns_cached_row_when_present() {
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
        INSERT INTO risk_free_rate_daily (id, rate_date, tenor, rate, source, fetched_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind("id-1")
    .bind("2026-02-14")
    .bind("3M")
    .bind("1.3428")
    .bind("chinabond")
    .bind("2026-02-14 12:00:00")
    .execute(&pool)
    .await
    .expect("seed rate");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/rates/risk-free?tenor=3M")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert_eq!(v["tenor"], "3M");
    assert_eq!(v["rate_date"], "2026-02-14");
    assert_eq!(v["rate_percent"], "1.3428");
    assert_eq!(v["source"], "chinabond");
    assert!(
        v["fetched_at"]
            .as_str()
            .unwrap_or("")
            .contains("2026-02-14")
    );
}
