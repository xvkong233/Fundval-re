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
async fn batch_estimate_enqueues_refresh_jobs_when_stale_or_missing() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    // A: 有估值但已过期；B: 无估值
    sqlx::query(
        r#"
        INSERT INTO fund (
          id, fund_code, fund_name, fund_type,
          estimate_nav, estimate_growth, estimate_time,
          latest_nav, latest_nav_date,
          created_at, updated_at
        ) VALUES (
          'f-a', 'A', 'fund-A', '股票型',
          1.2345, 0.12, '2000-01-01 00:00:00',
          1.2000, '2026-02-20',
          CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed fund A");

    sqlx::query(
        r#"
        INSERT INTO fund (
          id, fund_code, fund_name, fund_type,
          latest_nav, latest_nav_date,
          created_at, updated_at
        ) VALUES (
          'f-b', 'B', 'fund-B', '股票型',
          0.9999, '2026-02-20',
          CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed fund B");

    let config = api::config::ConfigStore::load();
    config.set_bool("estimate_async_enabled", true);
    config.set_i64("estimate_cache_ttl", Some(5));
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/funds/batch_estimate")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"fund_codes":["A","B"],"source":"tiantian"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert!(v.get("A").is_some());
    assert!(v.get("B").is_some());

    let queued: i64 = sqlx::query(
        r#"
        SELECT COUNT(*) as c
        FROM crawl_job
        WHERE job_type='estimate_sync' AND source_name='tiantian' AND fund_code IN ('A','B')
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("count jobs")
    .get("c");
    assert_eq!(queued, 2);
}

#[tokio::test]
async fn batch_estimate_does_not_enqueue_when_enqueue_refresh_is_false() {
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
        INSERT INTO fund (
          id, fund_code, fund_name, fund_type,
          estimate_nav, estimate_growth, estimate_time,
          latest_nav, latest_nav_date,
          created_at, updated_at
        ) VALUES (
          'f-a', 'A', 'fund-A', '股票型',
          1.2345, 0.12, '2000-01-01 00:00:00',
          1.2000, '2026-02-20',
          CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed fund A");

    let config = api::config::ConfigStore::load();
    config.set_bool("estimate_async_enabled", true);
    config.set_i64("estimate_cache_ttl", Some(5));
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/funds/batch_estimate")
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"fund_codes":["A"],"source":"tiantian","enqueue_refresh":false}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);

    let queued: i64 = sqlx::query(
        r#"
        SELECT COUNT(*) as c
        FROM crawl_job
        WHERE job_type='estimate_sync' AND source_name='tiantian' AND fund_code IN ('A')
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("count jobs")
    .get("c");
    assert_eq!(queued, 0);
}
