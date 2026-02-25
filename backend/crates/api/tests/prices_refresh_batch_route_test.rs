use axum::{body::Body, http::Request};
use serde_json::json;
use sqlx::Row;
use tower::ServiceExt;

use api::state::AppState;

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json")
}

async fn seed_user_and_login(app: &axum::Router) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "username": "admin", "password": "pw12345678" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    v["access_token"]
        .as_str()
        .expect("access_token")
        .to_string()
}

#[tokio::test]
async fn prices_refresh_batch_async_enqueues_one_task_job() {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");
    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    let password_hash = api::django_password::hash_password("pw12345678");
    sqlx::query(
        r#"
        INSERT INTO auth_user (id, password, is_superuser, username, email, is_staff, is_active, date_joined)
        VALUES (1, $1, 1, 'admin', 'admin@example.com', 1, 1, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(password_hash)
    .execute(&pool)
    .await
    .expect("seed user");

    for (id, code) in [("f-a", "A"), ("f-b", "B")] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1,$2,$3,'股票型',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
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
    let app = api::app(state);

    let access = seed_user_and_login(&app).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/funds/prices/refresh_batch_async")
                .header("Authorization", format!("Bearer {access}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "fund_codes": ["A", "B"], "source": "tiantian" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 202);
    let v = json_body(res).await;
    let task_id = v["task_id"].as_str().unwrap().to_string();
    assert!(!task_id.trim().is_empty());

    let row = sqlx::query(
        r#"
        SELECT task_type, status, payload_json
        FROM task_job
        WHERE CAST(id AS TEXT) = $1
        "#,
    )
    .bind(task_id.trim())
    .fetch_one(&pool)
    .await
    .expect("task_job exists");

    let task_type: String = row.get("task_type");
    let status: String = row.get("status");
    let payload_json: String = row.get("payload_json");

    assert_eq!(task_type, "prices_refresh_batch");
    assert_eq!(status, "queued");

    let payload: serde_json::Value = serde_json::from_str(&payload_json).expect("payload json");
    let fund_codes = payload["fund_codes"].as_array().unwrap();
    assert_eq!(fund_codes.len(), 2);
}

