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
async fn fund_analysis_v2_task_writes_snapshot_row() {
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
                      "windows": [3]
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
        SELECT result_json, last_task_id
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
    assert_eq!(result["fund_code"], "000001");
    assert_eq!(result["source"], "tiantian");
    assert_eq!(result["profile"], "default");
    assert_eq!(result["refer_index_code"], "1.000001");
    assert_eq!(result["windows"][0]["window"], 3);

    let last_task_id: Option<String> = snap.try_get("last_task_id").ok();
    assert_eq!(last_task_id.as_deref(), Some(task_id.as_str()));
}
