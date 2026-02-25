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
async fn sim_auto_topk_ts_timing_invests_only_on_index_buy_signal() {
    // stub quant-service macd endpoint: emit a BUY signal on 2026-02-13 only.
    let stub = Router::new().route(
        "/api/quant/macd",
        post(|Json(body): Json<Value>| async move {
            let mut points = body
                .get("series")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for p in points.iter_mut() {
                if p.get("date").and_then(|v| v.as_str()) == Some("2026-02-13") {
                    if let Some(obj) = p.as_object_mut() {
                        obj.insert("txnType".to_string(), Value::String("buy".to_string()));
                    }
                }
            }
            Json(json!({ "points": points }))
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

    // seed 2 funds with 3-day nav history
    for (fund_id, code, navs) in [
        ("fund-1", "000001", [("2026-02-13", "1.0"), ("2026-02-14", "1.1"), ("2026-02-15", "1.1")]),
        ("fund-2", "000002", [("2026-02-13", "1.0"), ("2026-02-14", "1.0"), ("2026-02-15", "1.0")]),
    ] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1,$2,'T','',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(fund_id)
        .bind(code)
        .execute(&pool)
        .await
        .expect("seed fund");
        for (i, (d, nav)) in navs.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
                VALUES ($1,'tiantian',$2,$3,CAST($4 AS NUMERIC),CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("nav-{code}-{i}"))
            .bind(fund_id)
            .bind(*d)
            .bind(*nav)
            .execute(&pool)
            .await
            .expect("seed nav");
        }
    }

    // seed fund_signal_snapshot for pick_topk (use simple scores)
    for (fund_code, pos) in [("000001", 99.0_f64), ("000002", 10.0_f64)] {
        sqlx::query(
            r#"
            INSERT INTO fund_signal_snapshot (
              peer_code, fund_code, as_of_date,
              position_percentile_0_100, dip_buy_proba_5t, dip_buy_proba_20t, magic_rebound_proba_5t, magic_rebound_proba_20t,
              created_at, updated_at
            )
            VALUES ($1,$2,$3,$4,0,0,0,0,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(api::ml::train::PEER_CODE_ALL)
        .bind(fund_code)
        .bind("2026-02-13")
        .bind(pos)
        .execute(&pool)
        .await
        .expect("seed snapshot");
    }

    // seed index closes for refer index and shangzheng index (same in this test)
    for code in ["1.000001", "1.000300"] {
        for (i, (d, close)) in [("2026-02-13", "3500"), ("2026-02-14", "3600"), ("2026-02-15", "3650")]
            .iter()
            .enumerate()
        {
            sqlx::query(
                r#"
                INSERT INTO index_daily_price (id, index_code, source_name, trade_date, close, created_at, updated_at)
                VALUES ($1,$2,'eastmoney',$3,$4,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("idx-{code}-{i}"))
            .bind(code)
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

    // create run with new strategy; fund_codes can be empty (full market)
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sim/runs")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "mode": "backtest",
                      "strategy": "auto_topk_ts_timing",
                      "source": "tiantian",
                      "fund_codes": [],
                      "start_date": "2026-02-13",
                      "end_date": "2026-02-15",
                      "initial_cash": "10000",
                      "buy_fee_rate": 0.0,
                      "sell_fee_rate": 0.0,
                      "settlement_days": 2,
                      "top_k": 1,
                      "rebalance_every": 3,
                      "refer_index_code": "1.000300",
                      "buy_macd_point": 50
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    let run_id = v["run_id"].as_str().expect("run_id").to_string();

    // run backtest
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sim/runs/{run_id}/run"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    // on 2026-02-13, we should have invested (positions_value > 0)
    let row = sqlx::query(
        r#"
        SELECT positions_value
        FROM sim_daily_equity
        WHERE CAST(run_id AS TEXT) = $1 AND CAST(date AS TEXT) = $2
        "#,
    )
    .bind(&run_id)
    .bind("2026-02-13")
    .fetch_one(&pool)
    .await
    .expect("daily equity row");
    let pos_value: f64 = row.get("positions_value");
    assert!(pos_value > 0.0);
}

#[tokio::test]
async fn sim_auto_topk_ts_timing_respects_buy_amount_percent_budget() {
    // stub quant-service macd endpoint: emit a BUY signal on 2026-02-13 only.
    let stub = Router::new().route(
        "/api/quant/macd",
        post(|Json(body): Json<Value>| async move {
            let mut points = body
                .get("series")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for p in points.iter_mut() {
                if p.get("date").and_then(|v| v.as_str()) == Some("2026-02-13") {
                    if let Some(obj) = p.as_object_mut() {
                        obj.insert("txnType".to_string(), Value::String("buy".to_string()));
                    }
                }
            }
            Json(json!({ "points": points }))
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

    // seed 2 funds with 3-day nav history
    for (fund_id, code, navs) in [
        (
            "fund-1",
            "000001",
            [("2026-02-13", "1.0"), ("2026-02-14", "1.1"), ("2026-02-15", "1.1")],
        ),
        (
            "fund-2",
            "000002",
            [("2026-02-13", "1.0"), ("2026-02-14", "1.0"), ("2026-02-15", "1.0")],
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1,$2,'T','',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(fund_id)
        .bind(code)
        .execute(&pool)
        .await
        .expect("seed fund");
        for (i, (d, nav)) in navs.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
                VALUES ($1,'tiantian',$2,$3,CAST($4 AS NUMERIC),CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("nav-{code}-{i}"))
            .bind(fund_id)
            .bind(*d)
            .bind(*nav)
            .execute(&pool)
            .await
            .expect("seed nav");
        }
    }

    // seed fund_signal_snapshot for pick_topk (use simple scores)
    for (fund_code, pos) in [("000001", 99.0_f64), ("000002", 10.0_f64)] {
        sqlx::query(
            r#"
            INSERT INTO fund_signal_snapshot (
              peer_code, fund_code, as_of_date,
              position_percentile_0_100, dip_buy_proba_5t, dip_buy_proba_20t, magic_rebound_proba_5t, magic_rebound_proba_20t,
              created_at, updated_at
            )
            VALUES ($1,$2,$3,$4,0,0,0,0,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(api::ml::train::PEER_CODE_ALL)
        .bind(fund_code)
        .bind("2026-02-13")
        .bind(pos)
        .execute(&pool)
        .await
        .expect("seed snapshot");
    }

    // seed index closes for refer index and shangzheng index (same in this test)
    for code in ["1.000001", "1.000300"] {
        for (i, (d, close)) in [("2026-02-13", "3500"), ("2026-02-14", "3600"), ("2026-02-15", "3650")]
            .iter()
            .enumerate()
        {
            sqlx::query(
                r#"
                INSERT INTO index_daily_price (id, index_code, source_name, trade_date, close, created_at, updated_at)
                VALUES ($1,$2,'eastmoney',$3,$4,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("idx-{code}-{i}"))
            .bind(code)
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

    // create run: buy_amount_percent=20 means only invest ~20% cash at buy signal day
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sim/runs")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "mode": "backtest",
                      "strategy": "auto_topk_ts_timing",
                      "source": "tiantian",
                      "fund_codes": [],
                      "start_date": "2026-02-13",
                      "end_date": "2026-02-15",
                      "initial_cash": "10000",
                      "buy_fee_rate": 0.0,
                      "sell_fee_rate": 0.0,
                      "settlement_days": 2,
                      "top_k": 1,
                      "rebalance_every": 3,
                      "refer_index_code": "1.000300",
                      "buy_macd_point": 50,
                      "buy_amount_percent": 20
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    let run_id = v["run_id"].as_str().expect("run_id").to_string();

    // run backtest
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sim/runs/{run_id}/run"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let row = sqlx::query(
        r#"
        SELECT positions_value
        FROM sim_daily_equity
        WHERE CAST(run_id AS TEXT) = $1 AND CAST(date AS TEXT) = $2
        "#,
    )
    .bind(&run_id)
    .bind("2026-02-13")
    .fetch_one(&pool)
    .await
    .expect("daily equity row");
    let pos_value: f64 = row.get("positions_value");
    assert!(pos_value > 0.0);
    assert!(pos_value < 5000.0, "positions_value should be far less than all-in cash");
}

#[tokio::test]
async fn sim_auto_topk_ts_timing_adds_on_multiple_buy_signals_without_forced_liquidation() {
    // stub quant-service macd endpoint: emit BUY on 2026-02-13 and 2026-02-14.
    let stub = Router::new().route(
        "/api/quant/macd",
        post(|Json(body): Json<Value>| async move {
            let mut points = body
                .get("series")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for p in points.iter_mut() {
                let d = p.get("date").and_then(|v| v.as_str()).unwrap_or("");
                if d == "2026-02-13" || d == "2026-02-14" {
                    if let Some(obj) = p.as_object_mut() {
                        obj.insert("txnType".to_string(), Value::String("buy".to_string()));
                    }
                }
            }
            Json(json!({ "points": points }))
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

    // seed 1 fund with 3-day nav history (constant nav)
    sqlx::query(
        r#"
        INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
        VALUES ('fund-1','000001','T','',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed fund");
    for (i, (d, nav)) in [("2026-02-13", "1.0"), ("2026-02-14", "1.0"), ("2026-02-15", "1.0")]
        .iter()
        .enumerate()
    {
        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1,'tiantian','fund-1',$2,CAST($3 AS NUMERIC),CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(format!("nav-000001-{i}"))
        .bind(*d)
        .bind(*nav)
        .execute(&pool)
        .await
        .expect("seed nav");
    }

    // seed fund_signal_snapshot for both buy days so pick_topk works each time
    for (i, d) in ["2026-02-13", "2026-02-14"].iter().enumerate() {
        sqlx::query(
            r#"
            INSERT INTO fund_signal_snapshot (
              peer_code, fund_code, as_of_date,
              position_percentile_0_100, dip_buy_proba_5t, dip_buy_proba_20t, magic_rebound_proba_5t, magic_rebound_proba_20t,
              created_at, updated_at
            )
            VALUES ($1,'000001',$2,99,0,0,0,0,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(api::ml::train::PEER_CODE_ALL)
        .bind(*d)
        .execute(&pool)
        .await
        .expect("seed snapshot");
        let _ = i;
    }

    // seed index closes for refer index and shangzheng index
    for code in ["1.000001", "1.000300"] {
        for (i, (d, close)) in [("2026-02-13", "3500"), ("2026-02-14", "3600"), ("2026-02-15", "3650")]
            .iter()
            .enumerate()
        {
            sqlx::query(
                r#"
                INSERT INTO index_daily_price (id, index_code, source_name, trade_date, close, created_at, updated_at)
                VALUES ($1,$2,'eastmoney',$3,$4,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("idx-{code}-{i}"))
            .bind(code)
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

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sim/runs")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "mode": "backtest",
                      "strategy": "auto_topk_ts_timing",
                      "source": "tiantian",
                      "fund_codes": [],
                      "start_date": "2026-02-13",
                      "end_date": "2026-02-15",
                      "initial_cash": "10000",
                      "buy_fee_rate": 0.0,
                      "sell_fee_rate": 0.0,
                      "settlement_days": 2,
                      "top_k": 1,
                      "rebalance_every": 60,
                      "refer_index_code": "1.000300",
                      "buy_macd_point": 50,
                      "buy_amount_percent": 20
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    let run_id = v["run_id"].as_str().expect("run_id").to_string();

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sim/runs/{run_id}/run"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let row13 = sqlx::query(
        r#"
        SELECT positions_value
        FROM sim_daily_equity
        WHERE CAST(run_id AS TEXT) = $1 AND CAST(date AS TEXT) = $2
        "#,
    )
    .bind(&run_id)
    .bind("2026-02-13")
    .fetch_one(&pool)
    .await
    .expect("row13");
    let v13: f64 = row13.get("positions_value");

    let row14 = sqlx::query(
        r#"
        SELECT positions_value
        FROM sim_daily_equity
        WHERE CAST(run_id AS TEXT) = $1 AND CAST(date AS TEXT) = $2
        "#,
    )
    .bind(&run_id)
    .bind("2026-02-14")
    .fetch_one(&pool)
    .await
    .expect("row14");
    let v14: f64 = row14.get("positions_value");

    assert!(v13 > 0.0);
    assert!(v14 > v13 + 1e-9, "second BUY day should add position without forced liquidation");
}

#[tokio::test]
async fn sim_auto_topk_ts_timing_stop_profit_blocks_buy_on_same_day() {
    // BUY on both 2026-02-13 and 2026-02-14.
    let stub = Router::new().route(
        "/api/quant/macd",
        post(|Json(body): Json<Value>| async move {
            let mut points = body
                .get("series")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for p in points.iter_mut() {
                let d = p.get("date").and_then(|v| v.as_str()).unwrap_or("");
                if d == "2026-02-13" || d == "2026-02-14" {
                    if let Some(obj) = p.as_object_mut() {
                        obj.insert("txnType".to_string(), Value::String("buy".to_string()));
                    }
                }
            }
            Json(json!({ "points": points }))
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

    // seed 1 fund with constant nav
    sqlx::query(
        r#"
        INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
        VALUES ('fund-1','000001','T','',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed fund");
    for (i, (d, nav)) in [("2026-02-13", "1.0"), ("2026-02-14", "1.0"), ("2026-02-15", "1.0")]
        .iter()
        .enumerate()
    {
        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1,'tiantian','fund-1',$2,CAST($3 AS NUMERIC),CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(format!("nav-000001-stop-{i}"))
        .bind(*d)
        .bind(*nav)
        .execute(&pool)
        .await
        .expect("seed nav");
    }

    // seed snapshot for both buy days
    for d in ["2026-02-13", "2026-02-14"] {
        sqlx::query(
            r#"
            INSERT INTO fund_signal_snapshot (
              peer_code, fund_code, as_of_date,
              position_percentile_0_100, dip_buy_proba_5t, dip_buy_proba_20t, magic_rebound_proba_5t, magic_rebound_proba_20t,
              created_at, updated_at
            )
            VALUES ($1,'000001',$2,99,0,0,0,0,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(api::ml::train::PEER_CODE_ALL)
        .bind(d)
        .execute(&pool)
        .await
        .expect("seed snapshot");
    }

    // seed index closes
    for code in ["1.000001", "1.000300"] {
        for (i, (d, close)) in [("2026-02-13", "3500"), ("2026-02-14", "3600"), ("2026-02-15", "3650")]
            .iter()
            .enumerate()
        {
            sqlx::query(
                r#"
                INSERT INTO index_daily_price (id, index_code, source_name, trade_date, close, created_at, updated_at)
                VALUES ($1,$2,'eastmoney',$3,$4,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("idx-stop-{code}-{i}"))
            .bind(code)
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

    // make stop-profit always eligible (low thresholds), and ensure it sells a bit each day.
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sim/runs")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "mode": "backtest",
                      "strategy": "auto_topk_ts_timing",
                      "source": "tiantian",
                      "fund_codes": [],
                      "start_date": "2026-02-13",
                      "end_date": "2026-02-15",
                      "initial_cash": "10000",
                      "buy_fee_rate": 0.0,
                      "sell_fee_rate": 0.0,
                      "settlement_days": 2,
                      "top_k": 1,
                      "rebalance_every": 60,
                      "refer_index_code": "1.000300",
                      "buy_macd_point": 50,
                      "buy_amount_percent": 20,
                      "sh_composite_index": 0,
                      "fund_position": 0,
                      "profit_rate": -1,
                      "sell_at_top": false,
                      "sell_unit": "fundPercent",
                      "sell_num": 10
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    let run_id = v["run_id"].as_str().expect("run_id").to_string();

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sim/runs/{run_id}/run"))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);

    let row13 = sqlx::query(
        r#"
        SELECT positions_value
        FROM sim_daily_equity
        WHERE CAST(run_id AS TEXT) = $1 AND CAST(date AS TEXT) = $2
        "#,
    )
    .bind(&run_id)
    .bind("2026-02-13")
    .fetch_one(&pool)
    .await
    .expect("row13");
    let v13: f64 = row13.get("positions_value");

    let row14 = sqlx::query(
        r#"
        SELECT positions_value
        FROM sim_daily_equity
        WHERE CAST(run_id AS TEXT) = $1 AND CAST(date AS TEXT) = $2
        "#,
    )
    .bind(&run_id)
    .bind("2026-02-14")
    .fetch_one(&pool)
    .await
    .expect("row14");
    let v14: f64 = row14.get("positions_value");

    // if stop-profit triggers on a BUY day, it should block buying on that same day.
    // Therefore, with constant nav and sell 10%, the position value should decrease day-over-day.
    assert!(v13 > 0.0);
    assert!(v14 < v13 - 1e-9, "stop-profit should block buy and only sell on the same day");
}
