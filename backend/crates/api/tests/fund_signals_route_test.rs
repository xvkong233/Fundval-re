use axum::{body::Body, http::Request};
use serde_json::Value;
use tower::ServiceExt;

use api::state::AppState;

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn fund_signals_returns_shape() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    for (id, code) in [
        ("fund-1", "000001"),
        ("fund-2", "000002"),
        ("fund-3", "000003"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(id)
        .bind(code)
        .bind(format!("基金{code}"))
        .bind("股票型")
        .execute(&pool)
        .await
        .expect("seed fund");
    }

    for code in ["000001", "000002", "000003"] {
        sqlx::query(
            r#"
            INSERT INTO fund_relate_theme (fund_code, sec_code, sec_name, corr_1y, ol2top, source, fetched_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(code)
        .bind("BK000156")
        .bind("国防军工")
        .bind(80.0_f64)
        .bind(80.0_f64)
        .bind("tiantian_h5")
        .execute(&pool)
        .await
        .expect("seed relate theme");
    }

    let mut dates: Vec<String> = Vec::new();
    let start = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date");
    for i in 0..80 {
        let d = start + chrono::Duration::days(i);
        dates.push(d.format("%Y-%m-%d").to_string());
    }

    let navs_a: Vec<f64> = (0..80).map(|i| 1.0 + (i as f64) * 0.001).collect();
    let navs_b: Vec<f64> = vec![1.0; 80];
    let mut navs_c: Vec<f64> = vec![1.0; 80];
    navs_c[20] = 0.75;
    navs_c[25] = 0.85;
    navs_c[40] = 0.95;

    for (fund_id, navs) in [
        ("fund-1", &navs_a),
        ("fund-2", &navs_b),
        ("fund-3", &navs_c),
    ] {
        for (i, d) in dates.iter().enumerate() {
            sqlx::query(
                r#"
                INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                "#,
            )
            .bind(format!("nav-{fund_id}-{i}"))
            .bind("tiantian")
            .bind(fund_id)
            .bind(d)
            .bind(format!("{:.4}", navs[i]))
            .execute(&pool)
            .await
            .expect("seed nav");
        }
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000003/signals?source=tiantian")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert_eq!(v["fund_code"], "000003");
    assert_eq!(v["source"], "tiantian");
    assert!(v.get("peers").is_some());
    assert!(v["peers"].is_array());
    assert!(!v["peers"].as_array().unwrap().is_empty());
    assert!(v["peers"][0].get("dip_buy").is_some());
    assert!(v["peers"][0].get("magic_rebound").is_some());

    let peers = v["peers"].as_array().unwrap();
    assert!(
        peers
            .iter()
            .any(|p| p.get("peer_code").and_then(|v| v.as_str()) == Some(api::ml::train::PEER_CODE_ALL)),
        "peers 应包含全市场 (__all__) 兜底"
    );
}
