use axum::{body::Body, http::Request};
use serde_json::Value;
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
async fn nav_history_sync_enqueues_jobs_by_default() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    for (id, code) in [("f-a", "A"), ("f-b", "B")] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1, $2, $3, '股票型', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(id)
        .bind(code)
        .bind(format!("fund-{code}"))
        .execute(&pool)
        .await
        .expect("seed fund");
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/nav-history/sync")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"fund_codes":["A","B"],"source":"tiantian"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 202);
    let v = body_json(res).await;
    let task_id = v.get("task_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    assert!(!task_id.trim().is_empty());

    let row = sqlx::query(
        r#"
        SELECT
          task_type,
          payload_json,
          status
        FROM task_job
        WHERE CAST(id AS TEXT) = $1
        "#,
    )
    .bind(&task_id)
    .fetch_one(&pool)
    .await
    .expect("select task_job");

    assert_eq!(row.get::<String, _>("task_type"), "nav_history_sync_batch");
    assert_eq!(row.get::<String, _>("status"), "queued");
    let payload: String = row.get("payload_json");
    assert!(payload.contains("\"fund_codes\""));
}
