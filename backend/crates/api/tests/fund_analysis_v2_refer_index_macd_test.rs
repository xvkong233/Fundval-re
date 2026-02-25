use axum::{Json, Router, body::Body, http::Request, routing::post};
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
async fn fund_analysis_v2_uses_reference_index_series_for_macd_when_available() {
    // NOTE: this test intentionally forces `sqlx::migrate!` to re-expand when migrations change.
    // stub quant-service endpoints used by fund_analysis_v2_compute
    let stub = Router::new()
        .route(
            "/api/quant/xalpha/metrics",
            post(|Json(_body): Json<Value>| async move {
                Json(json!({
                  "metrics": {
                    "total_return": 0.0,
                    "cagr": 0.0,
                    "vol_annual": 0.0,
                    "sharpe": 0.0,
                    "max_drawdown": 0.0
                  },
                  "drawdown_series": []
                }))
            }),
        )
        .route(
            "/api/quant/xalpha/grid",
            post(|Json(_body): Json<Value>| async move { Json(json!({ "actions": [] })) }),
        )
        .route(
            "/api/quant/xalpha/scheduled",
            post(|Json(_body): Json<Value>| async move { Json(json!({ "actions": [] })) }),
        )
        .route(
            "/api/quant/macd",
            post(|Json(body): Json<Value>| async move {
                let series = body.get("series").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                assert!(
                    !series.is_empty(),
                    "macd should receive series"
                );

                // When reference index is used, dates should be real ISO dates (YYYY-MM-DD), not "f+N".
                let first_date = series[0].get("date").and_then(|v| v.as_str()).unwrap_or("");
                assert!(
                    first_date.len() == 10 && first_date.chars().nth(4) == Some('-'),
                    "expected reference index date, got: {first_date}"
                );

                let first_val = series[0].get("val").and_then(|v| v.as_f64()).unwrap_or(-1.0);
                assert!(
                    first_val >= 99.0,
                    "expected reference index close value (~100), got: {first_val}"
                );

                Json(json!({ "points": [] }))
            }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let _ = axum::serve(listener, stub).await;
    });
    let stub_url = format!("http://{addr}");

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
        INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
        VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind("fund-1")
    .bind("000001")
    .bind("测试基金")
    .bind("股票型")
    .execute(&pool)
    .await
    .expect("seed fund");

    // fund nav history (values around 1.0; should NOT be used for macd when reference index available)
    for (i, d) in ["2026-02-12", "2026-02-13", "2026-02-14", "2026-02-15"]
        .iter()
        .enumerate()
    {
        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(format!("nav-{i}"))
        .bind("tiantian")
        .bind("fund-1")
        .bind(*d)
        .bind("1.0")
        .execute(&pool)
        .await
        .expect("seed nav");
    }

    // reference index history
    // secid: 1.000001 (上证综指) from Qbot IndexFund.ShangZheng
    for (i, (d, close)) in [
        ("2026-02-12", "100.0"),
        ("2026-02-13", "101.0"),
        ("2026-02-14", "102.0"),
        ("2026-02-15", "103.0"),
    ]
    .into_iter()
    .enumerate()
    {
        sqlx::query(
            r#"
            INSERT INTO index_daily_price (id, index_code, source_name, trade_date, close, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(format!("idx-{i}"))
        .bind("1.000001")
        .bind("eastmoney")
        .bind(d)
        .bind(close)
        .execute(&pool)
        .await
        .expect("seed index price");
    }

    let config = api::config::ConfigStore::load();
    config.set_string("quant_service_url", Some(stub_url));
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/funds/000001/analysis_v2/compute")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "source": "tiantian",
                      "profile": "default",
                      "windows": [3],
                      "refer_index_code": "1.000001"
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
}
