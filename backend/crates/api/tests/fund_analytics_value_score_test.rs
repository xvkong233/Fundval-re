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
async fn fund_analytics_includes_value_score_and_ce_with_gamma() {
    sqlx::any::install_default_drivers();

    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");

    let migrator = sqlx::migrate!("../../migrations/sqlite");
    migrator.run(&pool).await.expect("migrate");

    for (id, code, name) in [
        ("fund-1", "000001", "基金A"),
        ("fund-2", "000002", "基金B"),
        ("fund-3", "000003", "基金C"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(id)
        .bind(code)
        .bind(name)
        .bind("股票型")
        .execute(&pool)
        .await
        .expect("seed fund");
    }

    // Seed relate themes (peer group): same sector for all, plus an extra sector for fund-1.
    for (code, sec_code, sec_name) in [
        ("000001", "BK000156", "国防军工"),
        ("000002", "BK000156", "国防军工"),
        ("000003", "BK000156", "国防军工"),
        ("000001", "BK000158", "航空装备"),
    ] {
        sqlx::query(
            r#"
            INSERT INTO fund_relate_theme (fund_code, sec_code, sec_name, corr_1y, ol2top, source, fetched_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(code)
        .bind(sec_code)
        .bind(sec_name)
        .bind(80.0_f64)
        .bind(80.0_f64)
        .bind("tiantian_h5")
        .execute(&pool)
        .await
        .expect("seed relate theme");
    }

    let dates = [
        "2026-02-10",
        "2026-02-11",
        "2026-02-12",
        "2026-02-13",
        "2026-02-14",
    ];
    let navs_a = ["1.0000", "1.0100", "1.0050", "1.0200", "1.0300"];
    let navs_b = ["1.0000", "0.9950", "1.0000", "0.9900", "1.0000"];
    let navs_c = ["1.0000", "1.0020", "1.0010", "1.0030", "1.0025"];

    for (fund_id, navs) in [("fund-1", navs_a), ("fund-2", navs_b), ("fund-3", navs_c)] {
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
            .bind(*d)
            .bind(navs[i])
            .execute(&pool)
            .await
            .expect("seed nav");
        }
    }

    sqlx::query(
        r#"
        INSERT INTO risk_free_rate_daily (id, rate_date, tenor, rate, source, fetched_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind("rf-1")
    .bind("2026-02-14")
    .bind("3M")
    .bind("2.0000")
    .bind("chinabond")
    .bind("2026-02-14 12:00:00")
    .execute(&pool)
    .await
    .expect("seed rf");

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .uri("/api/funds/000001/analytics?range=5T&source=tiantian&gamma=5")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;

    assert_eq!(v["fund_code"], "000001");
    assert_eq!(v["range"], "5T");
    assert_eq!(v["source"], "tiantian");

    assert!(v.get("value_score").is_some());
    assert_eq!(v["value_score"]["peer_kind"], "sector");
    assert_eq!(v["value_score"]["peer_name"], "国防军工");
    assert_eq!(v["value_score"]["sample_size"], 3);

    assert!(v.get("value_scores").is_some());
    assert!(v["value_scores"].is_array());
    assert!(v["value_scores"].as_array().unwrap().len() >= 1);

    assert!(v.get("ce").is_some());
    assert_eq!(v["ce"]["gamma"], 5.0);
}
