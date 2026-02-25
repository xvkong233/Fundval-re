use axum::{body::Body, http::Request};
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

use api::state::AppState;

struct TempDirEnv {
    key: &'static str,
    path: std::path::PathBuf,
    old: Option<std::ffi::OsString>,
}

impl TempDirEnv {
    fn new() -> Self {
        let mut path = std::env::temp_dir();
        path.push(format!("fundval-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).expect("create temp dir");

        let key = "FUNDVAL_DATA_DIR";
        let old = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, &path);
        }

        Self { key, path, old }
    }
}

impl Drop for TempDirEnv {
    fn drop(&mut self) {
        match self.old.take() {
            Some(v) => unsafe {
                std::env::set_var(self.key, v);
            },
            None => unsafe {
                std::env::remove_var(self.key);
            },
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn admin_can_get_and_set_crawl_config() {
    let _env = TempDirEnv::new();
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

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/crawl/config")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert!(v.get("crawl_enabled").is_some());
    assert_eq!(v["crawl_source"], "tiantian");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/admin/crawl/config")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "crawl_enabled": true,
                      "crawl_source": "tiantian",
                      "crawl_tick_interval_seconds": 60,
                      "crawl_enqueue_max_jobs": 123,
                      "crawl_daily_run_limit": 456,
                      "crawl_run_max_jobs": 7,
                      "crawl_per_job_delay_ms": 800,
                      "crawl_per_job_jitter_ms": 90,
                      "crawl_source_fallbacks": "danjuan,ths"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/crawl/config")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert_eq!(v["crawl_tick_interval_seconds"], 60);
    assert_eq!(v["crawl_enqueue_max_jobs"], 123);
    assert_eq!(v["crawl_daily_run_limit"], 456);
    assert_eq!(v["crawl_run_max_jobs"], 7);
    assert_eq!(v["crawl_per_job_delay_ms"], 800);
    assert_eq!(v["crawl_per_job_jitter_ms"], 90);
    assert_eq!(v["crawl_source_fallbacks"], "danjuan,ths");
}

#[tokio::test]
async fn crawl_config_requires_staff() {
    let _env = TempDirEnv::new();
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
        VALUES (1, 'x', 0, 'u', 0, 1)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed user");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/admin/crawl/config")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 403);
}
