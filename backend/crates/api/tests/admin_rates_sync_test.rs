use axum::{Json, Router, body::Body, http::Request, routing::get};
use serde_json::json;
use sqlx::Row;
use tower::ServiceExt;

use api::state::AppState;

#[tokio::test]
async fn admin_can_sync_risk_free_rate_into_db() {
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
        INSERT INTO auth_user (id, password, is_superuser, username, is_staff, is_active)
        VALUES (1, 'x', 1, 'admin', 1, 1)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed admin");

    let stub = Router::new().route(
        "/cbweb-czb-web/czb/czbChartIndex",
        get(|| async {
            Json(json!({
              "worktime": "2026-02-14",
              "seriesData": [[0.25, 1.3428]]
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, stub).await.expect("serve");
    });
    let url = format!("http://{addr}/cbweb-czb-web/czb/czbChartIndex");
    let old = std::env::var("RISK_FREE_CHINABOND_URL").ok();
    unsafe {
        std::env::set_var("RISK_FREE_CHINABOND_URL", &url);
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let pool_verify = pool.clone();
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/rates/risk-free/sync")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let row = sqlx::query(
        "SELECT CAST(rate AS TEXT) as rate FROM risk_free_rate_daily WHERE rate_date = '2026-02-14' AND tenor = '3M' AND source = 'chinabond'",
    )
    .fetch_one(&pool_verify)
    .await
    .expect("row exists");
    let rate: String = row.get("rate");
    assert!(rate.contains("1.3428"), "rate={rate}");

    match old {
        Some(v) => unsafe {
            std::env::set_var("RISK_FREE_CHINABOND_URL", v);
        },
        None => unsafe {
            std::env::remove_var("RISK_FREE_CHINABOND_URL");
        },
    }
}
