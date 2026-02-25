use axum::{Json, Router, body::Body, http::Request, routing::post};
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

#[tokio::test]
async fn quant_metrics_batch_async_enqueues_one_task() {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");
    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quant/xalpha/metrics_batch_async")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "fund_codes": ["000001", "000002"],
                      "source": "tiantian",
                      "window": 5
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
    assert!(!task_id.trim().is_empty());

    let row = sqlx::query("SELECT task_type FROM task_job WHERE id = $1")
        .bind(&task_id)
        .fetch_one(&pool)
        .await
        .expect("task_job exists");
    let task_type: String = row.get("task_type");
    assert_eq!(task_type, "quant_xalpha_metrics_batch");
}

#[tokio::test]
async fn quant_metrics_batch_task_executes_and_logs_per_fund_code() {
    // stub quant-service
    let stub = Router::new().route(
        "/api/quant/xalpha/metrics",
        post(|Json(_body): Json<serde_json::Value>| async move {
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

        for (i, d) in ["2026-02-12", "2026-02-13", "2026-02-14"].iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("nav-{code}-{i}"))
            .bind("tiantian")
            .bind(fund_id)
            .bind(*d)
            .bind("1.0")
            .execute(&pool)
            .await
            .expect("seed nav");
        }
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
                .uri("/api/quant/xalpha/metrics_batch_async")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "fund_codes": ["000001", "000002"],
                      "source": "tiantian",
                      "window": 3
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

    // run task executor once
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

    let run_row = sqlx::query(
        r#"
        SELECT CAST(id AS TEXT) as id
        FROM task_run
        WHERE job_id = $1
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .bind(&task_id)
    .fetch_one(&pool)
    .await
    .expect("task_run exists");
    let run_id: String = run_row.get("id");

    let logs = sqlx::query("SELECT message FROM task_run_log WHERE run_id = $1 ORDER BY created_at ASC")
        .bind(&run_id)
        .fetch_all(&pool)
        .await
        .expect("logs");
    let joined = logs
        .into_iter()
        .map(|r| r.get::<String, _>("message"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(joined.contains("[000001]"));
    assert!(joined.contains("[000002]"));
}

#[tokio::test]
async fn quant_grid_batch_async_enqueues_one_task() {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");
    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quant/xalpha/grid_batch_async")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "fund_codes": ["000001", "000002"],
                      "source": "tiantian",
                      "window": 10,
                      "grid_step_pct": 0.02
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

    let row = sqlx::query("SELECT task_type FROM task_job WHERE id = $1")
        .bind(&task_id)
        .fetch_one(&pool)
        .await
        .expect("task_job exists");
    let task_type: String = row.get("task_type");
    assert_eq!(task_type, "quant_xalpha_grid_batch");
}

#[tokio::test]
async fn quant_scheduled_batch_async_enqueues_one_task() {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");
    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/quant/xalpha/scheduled_batch_async")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "fund_codes": ["000001", "000002"],
                      "source": "tiantian",
                      "window": 10,
                      "every_n": 2,
                      "amount": 100
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

    let row = sqlx::query("SELECT task_type FROM task_job WHERE id = $1")
        .bind(&task_id)
        .fetch_one(&pool)
        .await
        .expect("task_job exists");
    let task_type: String = row.get("task_type");
    assert_eq!(task_type, "quant_xalpha_scheduled_batch");
}

#[tokio::test]
async fn quant_grid_and_scheduled_tasks_execute_and_log() {
    // stub quant-service
    let stub = Router::new()
        .route(
            "/api/quant/xalpha/grid",
            post(|Json(_body): Json<serde_json::Value>| async move {
                Json(json!({ "actions": [{"index": 1, "action": "buy"}] }))
            }),
        )
        .route(
            "/api/quant/xalpha/scheduled",
            post(|Json(_body): Json<serde_json::Value>| async move {
                Json(json!({ "actions": [{"index": 0, "action": "buy"}] }))
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

        for (i, d) in ["2026-02-12", "2026-02-13", "2026-02-14"].iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("nav-{code}-{i}"))
            .bind("tiantian")
            .bind(fund_id)
            .bind(*d)
            .bind("1.0")
            .execute(&pool)
            .await
            .expect("seed nav");
        }
    }

    let config = api::config::ConfigStore::load();
    config.set_string("quant_service_url", Some(stub_url));
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool.clone()), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    for (uri, expect_type) in [
        ("/api/quant/xalpha/grid_batch_async", "quant_xalpha_grid_batch"),
        (
            "/api/quant/xalpha/scheduled_batch_async",
            "quant_xalpha_scheduled_batch",
        ),
    ] {
        let body = if uri.contains("grid") {
            json!({"fund_codes":["000001","000002"],"source":"tiantian","window":3,"grid_step_pct":0.02})
        } else {
            json!({"fund_codes":["000001","000002"],"source":"tiantian","window":3,"every_n":2,"amount":100})
        };

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("Authorization", format!("Bearer {token}"))
                    .header("Content-Type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), 202);
        let v = json_body(res).await;
        let task_id = v["task_id"].as_str().expect("task_id").to_string();

        let row = sqlx::query("SELECT task_type FROM task_job WHERE id = $1")
            .bind(&task_id)
            .fetch_one(&pool)
            .await
            .expect("task_job exists");
        let task_type: String = row.get("task_type");
        assert_eq!(task_type, expect_type);

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

        let run_row = sqlx::query(
            r#"
            SELECT CAST(id AS TEXT) as id
            FROM task_run
            WHERE job_id = $1
            ORDER BY started_at DESC
            LIMIT 1
            "#,
        )
        .bind(&task_id)
        .fetch_one(&pool)
        .await
        .expect("task_run exists");
        let run_id: String = run_row.get("id");

        let logs = sqlx::query(
            "SELECT message FROM task_run_log WHERE run_id = $1 ORDER BY created_at ASC",
        )
        .bind(&run_id)
        .fetch_all(&pool)
        .await
        .expect("logs");
        let joined = logs
            .into_iter()
            .map(|r| r.get::<String, _>("message"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(joined.contains("[000001]"));
        assert!(joined.contains("[000002]"));
    }
}

#[tokio::test]
async fn quant_qdiipredict_batch_async_enqueues_and_executes() {
    // stub quant-service
    let stub = Router::new().route(
        "/api/quant/xalpha/qdiipredict",
        post(|Json(body): Json<serde_json::Value>| async move {
            let last_value = body["last_value"].as_f64().unwrap_or(1.0);
            Json(json!({
              "last_value": last_value,
              "delta": 1.001,
              "predicted_value": last_value * 1.001,
              "components": []
            }))
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
                .uri("/api/quant/xalpha/qdiipredict_batch_async")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "items": [
                        {
                          "fund_code": "000001",
                          "last_value": 1.0,
                          "legs": [{"code":"IDX_US","percent":100.0,"ratio":1.01,"currency_ratio":1.0}]
                        },
                        {
                          "fund_code": "000002",
                          "last_value": 2.0,
                          "legs": []
                        }
                      ]
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

    let run_row = sqlx::query(
        r#"
        SELECT CAST(id AS TEXT) as id
        FROM task_run
        WHERE job_id = $1
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .bind(&task_id)
    .fetch_one(&pool)
    .await
    .expect("task_run exists");
    let run_id: String = run_row.get("id");

    let logs = sqlx::query("SELECT message FROM task_run_log WHERE run_id = $1 ORDER BY created_at ASC")
        .bind(&run_id)
        .fetch_all(&pool)
        .await
        .expect("logs");
    let joined = logs
        .into_iter()
        .map(|r| r.get::<String, _>("message"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("[000001]"));
    assert!(joined.contains("[000002]"));
}
