use axum::{body::Body, http::Request};
use serde_json::json;
use tower::ServiceExt;

use api::state::AppState;

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json")
}

async fn seed_user_and_login(app: &axum::Router) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/auth/login")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({ "username": "admin", "password": "pw12345678" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    v["access_token"]
        .as_str()
        .expect("access_token")
        .to_string()
}

#[tokio::test]
async fn sim_env_create_and_step_works() {
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

    // Seed one fund + nav history
    let fund_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO fund (id, fund_code, fund_name, fund_type, latest_nav, latest_nav_date, estimate_nav, estimate_growth, estimate_time, created_at, updated_at)
        VALUES ($1,'000001','T','',NULL,NULL,NULL,NULL,NULL,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&fund_id)
    .execute(&pool)
    .await
    .expect("seed fund");

    for (d, nav) in [
        ("2026-02-01", "1.0"),
        ("2026-02-02", "1.0"),
        ("2026-02-03", "1.0"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1,'tiantian',$2,$3,CAST($4 AS NUMERIC),CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&fund_id)
        .bind(d)
        .bind(nav)
        .execute(&pool)
        .await
        .expect("seed nav");
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::app(state);

    let access = seed_user_and_login(&app).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sim/runs")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::from(
                    json!({
                        "mode": "env",
                        "name": "env",
                        "source": "tiantian",
                        "fund_codes": ["000001"],
                        "start_date": "2026-02-01",
                        "end_date": "2026-02-03",
                        "initial_cash": "1000",
                        "buy_fee_rate": 0.0,
                        "sell_fee_rate": 0.0,
                        "settlement_days": 2
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    let run_id = v["run_id"].as_str().unwrap().to_string();

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/sim/envs/{run_id}/step"))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::from(
                    json!({
                        "actions": [
                            { "side": "BUY", "fund_code": "000001", "amount": "1000", "shares": null }
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    assert_eq!(v["date"], "2026-02-02");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/sim/envs/{run_id}/observation"))
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    assert_eq!(v["date"], "2026-02-02");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sim/runs")
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    assert!(v.as_array().unwrap().iter().any(|x| x["id"] == run_id));
}

#[tokio::test]
async fn sim_run_delete_works() {
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

    // Seed one fund + nav history (minimal for sim_run validation)
    let fund_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO fund (id, fund_code, fund_name, fund_type, latest_nav, latest_nav_date, estimate_nav, estimate_growth, estimate_time, created_at, updated_at)
        VALUES ($1,'000001','T','',NULL,NULL,NULL,NULL,NULL,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&fund_id)
    .execute(&pool)
    .await
    .expect("seed fund");

    for (d, nav) in [
        ("2026-02-01", "1.0"),
        ("2026-02-02", "1.0"),
        ("2026-02-03", "1.0"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1,'tiantian',$2,$3,CAST($4 AS NUMERIC),CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&fund_id)
        .bind(d)
        .bind(nav)
        .execute(&pool)
        .await
        .expect("seed nav");
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let app = api::app(state);
    let access = seed_user_and_login(&app).await;

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/sim/runs")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::from(
                    json!({
                        "mode": "backtest",
                        "name": "del",
                        "source": "tiantian",
                        "fund_codes": ["000001"],
                        "start_date": "2026-02-01",
                        "end_date": "2026-02-03",
                        "initial_cash": "1000",
                        "buy_fee_rate": 0.0,
                        "sell_fee_rate": 0.0,
                        "settlement_days": 2
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    let run_id = v["run_id"].as_str().unwrap().to_string();

    // Delete
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/sim/runs/{run_id}"))
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    if status != 200 {
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .expect("read body");
        panic!(
            "delete status unexpected: status={} body={}",
            status,
            String::from_utf8_lossy(&bytes)
        );
    }
    let v = json_body(res).await;
    assert_eq!(v["deleted"], true);

    // List should not contain it
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/sim/runs")
                .header("Authorization", format!("Bearer {access}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = json_body(res).await;
    assert!(!v.as_array().unwrap().iter().any(|x| x["id"] == run_id));
}
