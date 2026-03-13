use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sqlx::{Row, migrate::Migrator};
use tower::ServiceExt;

use api::state::AppState;

static MIGRATOR_SQLITE: Migrator = sqlx::migrate!("../../migrations/sqlite");

async fn new_sqlite_pool() -> sqlx::AnyPool {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    MIGRATOR_SQLITE
        .run(&pool)
        .await
        .expect("run sqlite migrations");

    pool
}

fn new_state(pool: sqlx::AnyPool) -> AppState {
    let config = api::config::ConfigStore::load();
    config.set_system_initialized(false);
    config.set_allow_register(false);
    config.set_string("bootstrap_key", Some("bootstrap-test-key".to_string()));
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite)
}

async fn read_json(response: axum::response::Response) -> Value {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        panic!(
            "response should be json, status={status}, body={}",
            String::from_utf8_lossy(&bytes)
        )
    })
}

#[tokio::test]
async fn bootstrap_initialize_creates_admin_on_sqlite() {
    let pool = new_sqlite_pool().await;
    let state = new_state(pool.clone());
    let app = api::service(state.clone());

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/admin/bootstrap/initialize/")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "bootstrap_key": "bootstrap-test-key",
                        "admin_username": "admin",
                        "admin_password": "Admin12345!",
                        "allow_register": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let payload = read_json(res).await;
    assert_eq!(payload["admin_created"], true);
    assert!(state.config().system_initialized());
    assert!(state.config().allow_register());

    let row = sqlx::query(
        "SELECT username, email, CAST(date_joined AS TEXT) AS date_joined, is_superuser, is_staff FROM auth_user WHERE username = $1",
    )
    .bind("admin")
    .fetch_one(&pool)
    .await
    .expect("admin created");

    assert_eq!(row.get::<String, _>("username"), "admin");
    assert_eq!(row.get::<String, _>("email"), "admin@fundval.local");
    assert!(row.get::<String, _>("date_joined").starts_with("20"));
    assert_eq!(row.get::<i64, _>("is_superuser"), 1);
    assert_eq!(row.get::<i64, _>("is_staff"), 1);
}

#[tokio::test]
async fn register_creates_user_on_sqlite() {
    let pool = new_sqlite_pool().await;
    let state = new_state(pool.clone());
    state.config().set_allow_register(true);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/users/register/")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "username": "alice",
                        "password": "Password123!",
                        "password_confirm": "Password123!",
                        "email": "alice@example.com"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::CREATED);
    let payload = read_json(res).await;
    assert_eq!(payload["user"]["username"], "alice");

    let row = sqlx::query(
        "SELECT username, email, CAST(date_joined AS TEXT) AS date_joined FROM auth_user WHERE username = $1",
    )
        .bind("alice")
        .fetch_one(&pool)
        .await
        .expect("user created");

    assert_eq!(row.get::<String, _>("username"), "alice");
    assert_eq!(row.get::<String, _>("email"), "alice@example.com");
    assert!(row.get::<String, _>("date_joined").starts_with("20"));
}

#[tokio::test]
async fn accounts_list_reads_sqlite_integer_booleans() {
    let pool = new_sqlite_pool().await;
    sqlx::query(
        r#"
        INSERT INTO auth_user (id, password, username, is_staff, is_active)
        VALUES (1, 'pwd', 'tester', 0, 1)
        "#,
    )
    .execute(&pool)
    .await
    .expect("insert user");
    sqlx::query(
        r#"
        INSERT INTO account (id, user_id, name, is_default)
        VALUES ('acc-parent', 1, '主账户', 1)
        "#,
    )
    .execute(&pool)
    .await
    .expect("insert account");

    let state = new_state(pool);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/accounts/")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let payload = read_json(res).await;
    let list = payload
        .as_array()
        .expect("accounts response should be array");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["name"], "主账户");
    assert_eq!(list[0]["is_default"], true);
}

#[tokio::test]
async fn source_accuracy_without_samples_returns_null_error_rate() {
    let pool = new_sqlite_pool().await;
    let state = new_state(pool);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/sources/tiantian/accuracy/?days=30")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let payload = read_json(res).await;
    assert_eq!(payload["record_count"], 0);
    assert!(payload["avg_error_rate"].is_null());
}
