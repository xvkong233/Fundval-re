use axum::{body::Body, http::Request};
use serde_json::json;
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
async fn tasks_job_detail_and_logs_work() {
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

    let job_id = uuid::Uuid::new_v4().to_string();
    let run_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO task_job (
          id, task_type, payload_json, priority, not_before, status, attempt, error,
          created_by, started_at, finished_at, created_at, updated_at
        )
        VALUES ($1,'nav_history_sync_batch','{"fund_codes":["000001","000002"]}',200,CURRENT_TIMESTAMP,'running',0,NULL,1,CURRENT_TIMESTAMP,NULL,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&job_id)
    .execute(&pool)
    .await
    .expect("seed task_job");

    sqlx::query(
        r#"
        INSERT INTO task_run (
          id, queue_type, job_id, job_type, fund_code, source_name, status, error, started_at, finished_at, created_at
        )
        VALUES ($1,'task_job',$2,'nav_history_sync_batch',NULL,'tiantian','running',NULL,CURRENT_TIMESTAMP,NULL,CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&run_id)
    .bind(&job_id)
    .execute(&pool)
    .await
    .expect("seed task_run");

    for (lvl, msg) in [
        ("INFO", "[000001] start"),
        ("INFO", "[000001] ok"),
        ("INFO", "[000002] start"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO task_run_log (id, run_id, level, message, created_at)
            VALUES ($1,$2,$3,$4,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&run_id)
        .bind(lvl)
        .bind(msg)
        .execute(&pool)
        .await
        .expect("seed task_run_log");
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::app(state);

    let access = seed_user_and_login(&app).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/tasks/jobs/{job_id}"))
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    assert_eq!(v["job"]["id"].as_str().unwrap(), job_id);
    assert_eq!(v["last_run"]["id"].as_str().unwrap(), run_id);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/tasks/jobs/{job_id}/logs?limit=100"))
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    let arr = v.as_array().expect("array");
    assert!(arr.len() >= 3);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/tasks/jobs/{job_id}/runs?limit=10"))
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    let arr = v.as_array().expect("array");
    assert_eq!(arr[0]["id"].as_str().unwrap(), run_id);
}

