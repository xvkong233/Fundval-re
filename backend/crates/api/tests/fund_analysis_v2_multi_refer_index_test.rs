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
async fn fund_analysis_v2_snapshots_are_isolated_by_refer_index_code() {
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

    for (i, d) in ["2026-02-13", "2026-02-14", "2026-02-15"]
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

    // seed minimal index_daily_price for both refer index codes so the task won't try to fetch network.
    for (idx, code, a, b, c) in [
        (0, "1.000001", "100.0", "101.0", "102.0"),
        (1, "1.000300", "200.0", "201.0", "202.0"),
    ] {
        for (j, (d, close)) in [("2026-02-13", a), ("2026-02-14", b), ("2026-02-15", c)]
            .iter()
            .enumerate()
        {
            sqlx::query(
                r#"
                INSERT INTO index_daily_price (id, index_code, source_name, trade_date, close, created_at, updated_at)
                VALUES ($1,$2,$3,$4,$5,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("idx-{idx}-{j}"))
            .bind(code)
            .bind("eastmoney")
            .bind(*d)
            .bind(*close)
            .execute(&pool)
            .await
            .expect("seed index");
        }
    }

    let config = api::config::ConfigStore::load();
    config.set_string("quant_service_url", Some(stub_url));
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let mut task_ids: Vec<String> = Vec::new();
    for code in ["1.000001", "1.000300"] {
        let res = app
            .clone()
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
                          "refer_index_code": code
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
        task_ids.push(task_id);
    }

    api::tasks::run_due_task_jobs(&pool, 10)
        .await
        .expect("run_due_task_jobs");

    for id in task_ids.iter() {
        let row = sqlx::query("SELECT status FROM task_job WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("task_job exists");
        let status: String = row.get("status");
        assert_eq!(status, "done", "task should be done: {id}");
    }

    let row = sqlx::query("SELECT COUNT(1) as c FROM fund_analysis_snapshot WHERE fund_code = $1")
        .bind("000001")
        .fetch_one(&pool)
        .await
        .expect("count snapshots");
    let c: i64 = row.get("c");
    assert_eq!(c, 2, "should keep one snapshot per refer_index_code");

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000001/analysis_v2?source=tiantian&profile=default&refer_index_code=1.000300")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    assert_eq!(v["fund_code"], "000001");
    assert_eq!(v["source"], "tiantian");
    assert_eq!(v["profile"], "default");
    assert_eq!(v["refer_index_code"], "1.000300");
}
