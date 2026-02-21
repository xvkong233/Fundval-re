use axum::{body::Body, http::Request};
use serde_json::Value;
use serde_json::json;
use tower::ServiceExt;

use api::state::AppState;

async fn body_json(res: axum::response::Response) -> Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn fund_signals_batch_returns_best_peer_summary() {
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

    // Seed minimal ML models (constant proba 0.5) so snapshot compute can fill probabilities without training.
    let feature_names_json =
        serde_json::to_string(&vec!["dd_mag", "ret5", "ret20", "vol20"]).expect("feature json");
    let model_json = serde_json::to_string(&api::ml::logreg::LogRegModel {
        weights: vec![0.0, 0.0, 0.0, 0.0],
        bias: 0.0,
        mean: vec![0.0, 0.0, 0.0, 0.0],
        std: vec![1.0, 1.0, 1.0, 1.0],
    })
    .expect("model json");
    let metrics_json = json!({ "sample_size": 120 }).to_string();

    for (task, h) in [
        ("dip_buy", 5_i64),
        ("dip_buy", 20),
        ("magic_rebound", 5),
        ("magic_rebound", 20),
    ] {
        sqlx::query(
            r#"
            INSERT INTO ml_sector_model (
              peer_code, task, horizon_days,
              feature_names_json, model_json, metrics_json,
              trained_at, created_at, updated_at
            )
            VALUES ($1,$2,$3,$4,$5,$6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind("BK000156")
        .bind(task)
        .bind(h)
        .bind(&feature_names_json)
        .bind(&model_json)
        .bind(&metrics_json)
        .execute(&pool)
        .await
        .expect("seed model");
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let state = AppState::new(Some(pool), config, jwt);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/funds/signals/batch")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "fund_codes": ["000001", "000003"],
                      "source": "tiantian"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert!(v.is_array());
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["fund_code"], "000001");
    assert!(arr[0].get("best_peer").is_some());
}
