use axum::{Json, Router, body::Body, http::Request, routing::post};
use chrono::NaiveDate;
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

async fn seed_nav_rows(
    pool: &sqlx::AnyPool,
    fund_id: &str,
    fund_code: &str,
    start: NaiveDate,
    days: usize,
) {
    let mut nav = 1.0_f64;
    for i in 0..days {
        // simple deterministic drift
        nav *= 1.0 + (0.0005 + (i as f64 % 7.0) * 0.00001);
        let d = start + chrono::Duration::days(i as i64);
        let _ = sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(format!("nav-{fund_code}-{i}"))
        .bind("tiantian")
        .bind(fund_id)
        .bind(d.format("%Y-%m-%d").to_string())
        .bind(format!("{nav:.6}"))
        .execute(pool)
        .await
        .expect("seed nav");
    }
}

#[tokio::test]
async fn fund_analysis_v2_includes_forecast_curve_60t() {
    // stub quant-service endpoints (analysis task calls them even if we focus on forecast)
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
            "/api/quant/macd",
            post(|Json(_body): Json<Value>| async move { Json(json!({ "points": [] })) }),
        )
        .route(
            "/api/quant/xalpha/grid",
            post(|Json(_body): Json<Value>| async move { Json(json!({ "actions": [] })) }),
        )
        .route(
            "/api/quant/xalpha/scheduled",
            post(|Json(_body): Json<Value>| async move { Json(json!({ "actions": [] })) }),
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

    for (fund_id, code) in [("fund-1", "000001"), ("fund-2", "000002")] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(fund_id)
        .bind(code)
        .bind(format!("基金{code}"))
        .bind("股票型")
        .execute(&pool)
        .await
        .expect("seed fund");

        seed_nav_rows(&pool, fund_id, code, NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(), 120).await;
    }

    // seed minimal reference index so the task won't try to fetch network.
    for (i, (d, close)) in [("2025-12-27", "100.0"), ("2025-12-28", "101.0"), ("2025-12-29", "102.0")]
        .iter()
        .enumerate()
    {
        sqlx::query(
            r#"
            INSERT INTO index_daily_price (id, index_code, source_name, trade_date, close, created_at, updated_at)
            VALUES ($1,$2,$3,$4,$5,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(format!("idx-{i}"))
        .bind("1.000001")
        .bind("eastmoney")
        .bind(*d)
        .bind(*close)
        .execute(&pool)
        .await
        .expect("seed index");
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
                      "windows": [60]
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

    let snap = sqlx::query(
        r#"
        SELECT result_json
        FROM fund_analysis_snapshot
        WHERE fund_code = $1 AND source = $2 AND profile = $3 AND refer_index_code = $4
        "#,
    )
    .bind("000001")
    .bind("tiantian")
    .bind("default")
    .bind("1.000001")
    .fetch_one(&pool)
    .await
    .expect("snapshot row");

    let result_json: String = snap.get("result_json");
    let result: Value = serde_json::from_str(&result_json).expect("result json");
    let w0 = &result["windows"][0];
    let forecast_points = w0["forecast"]["points"]
        .as_array()
        .expect("forecast points");
    assert_eq!(forecast_points.len(), 60);

    assert!(w0["forecast"]["low"]["step"].is_number());
    assert!(w0["forecast"]["high"]["step"].is_number());
}
