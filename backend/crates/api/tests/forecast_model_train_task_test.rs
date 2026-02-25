use axum::{body::Body, http::Request};
use serde_json::{Value, json};
use sqlx::Row;
use tower::ServiceExt;

use api::state::AppState;

async fn json_body(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn forecast_model_train_task_enqueues_and_trains() {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");
    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    // seed 2 funds with enough nav points for lag_k=20
    for (fund_id, code) in [("fund-1", "000001"), ("fund-2", "000002")] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(fund_id)
        .bind(code)
        .bind(format!("测试基金-{code}"))
        .bind("股票型")
        .execute(&pool)
        .await
        .expect("seed fund");

        for i in 0..32 {
            let day = 1 + i;
            let d = format!("2026-01-{day:02}");
            let nav = 1.0 + (i as f64) * 0.001;
            sqlx::query(
                r#"
                INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("nav-{code}-{i}"))
            .bind("tiantian")
            .bind(fund_id)
            .bind(d)
            .bind(format!("{nav:.4}"))
            .execute(&pool)
            .await
            .expect("seed nav");
        }
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/forecast/model/train")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "source": "tiantian",
                      "model_name": "global_ols_v1",
                      "horizon": 60,
                      "lag_k": 20
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 202);
    let v = json_body(res).await;
    let task_id = v["task_id"].as_str().expect("task_id").to_string();

    api::tasks::run_due_task_jobs(&pool, 10)
        .await
        .expect("run_due_task_jobs");

    let row = sqlx::query("SELECT status FROM task_job WHERE id = $1")
        .bind(&task_id)
        .fetch_one(&pool)
        .await
        .expect("task_job exists");
    let status: String = row.get("status");
    assert_eq!(status, "done");

    let model_row = sqlx::query(
        r#"
        SELECT model_name, source, horizon, lag_k, sample_count, CAST(trained_at AS TEXT) as trained_at
        FROM forecast_model
        WHERE model_name = $1 AND source = $2 AND horizon = $3 AND lag_k = $4
        "#,
    )
    .bind("global_ols_v1")
    .bind("tiantian")
    .bind(60_i64)
    .bind(20_i64)
    .fetch_one(&pool)
    .await
    .expect("forecast_model exists");

    let sample_count: i64 = model_row.get("sample_count");
    let trained_at: String = model_row.get("trained_at");
    assert!(sample_count >= 0);
    assert!(!trained_at.trim().is_empty());
}

