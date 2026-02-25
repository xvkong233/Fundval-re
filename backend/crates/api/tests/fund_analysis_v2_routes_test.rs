use axum::{body::Body, http::Request};
use serde_json::{Value, json};
use sqlx::Row;
use tower::ServiceExt;

use api::state::AppState;

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn fund_analysis_v2_requires_auth() {
    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(None, config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000001/analysis_v2?source=tiantian")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn fund_analysis_v2_compute_enqueues_task_job() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let body = json!({
        "source": "tiantian",
        "profile": "default",
        "windows": [60, 120]
    });

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/funds/000001/analysis_v2/compute")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 202);
    let v = body_json(res).await;
    let task_id = v["task_id"].as_str().expect("task_id");

    let row = sqlx::query(
        r#"
        SELECT task_type, payload_json
        FROM task_job
        WHERE id = $1
        "#,
    )
    .bind(task_id)
    .fetch_one(&pool)
    .await
    .expect("task_job row");

    let task_type: String = row.get("task_type");
    assert_eq!(task_type, "fund_analysis_v2_compute");

    let payload_json: String = row.get("payload_json");
    let payload: Value = serde_json::from_str(&payload_json).expect("payload json");
    assert_eq!(payload["fund_code"], "000001");
    assert_eq!(payload["source"], "tiantian");
    assert_eq!(payload["profile"], "default");
    assert_eq!(payload["windows"][0], 60);
    assert_eq!(payload["refer_index_code"], "1.000001");
}

#[tokio::test]
async fn fund_analysis_v2_retrieve_returns_missing_placeholder_when_absent() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");
    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000001/analysis_v2?source=tiantian&profile=default&refer_index_code=1.000001")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert_eq!(v["fund_code"], "000001");
    assert_eq!(v["missing"], true);
    assert!(v["result"].is_object());
}
