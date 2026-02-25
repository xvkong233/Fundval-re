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
        .bind(api::ml::train::PEER_CODE_ALL)
        .bind(task)
        .bind(h)
        .bind(&feature_names_json)
        .bind(&model_json)
        .bind(&metrics_json)
        .execute(&pool)
        .await
        .expect("seed all-market model");
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let pool2 = pool.clone();
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .clone()
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

    assert_eq!(res.status(), 202);
    let v = body_json(res).await;
    let task_id = v
        .get("task_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    assert!(!task_id.trim().is_empty());

    api::tasks::run_due_task_jobs(&pool2, 10)
        .await
        .expect("run task queue");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/funds/signals/batch_async/{task_id}?page=1&page_size=10"
                ))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    assert_eq!(v["task_id"], task_id);
    let items = v["items"].as_array().expect("items array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["fund_code"], "000001");
    assert!(items[0]["best_peer"].is_object());
}

#[tokio::test]
async fn fund_signals_batch_includes_all_market_peer_even_without_relate_theme() {
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
        VALUES ('fund-1', '000001', '基金000001', '股票型', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed fund");

    let start = chrono::NaiveDate::from_ymd_opt(2026, 1, 1).expect("date");
    for i in 0..40 {
        let d = start + chrono::Duration::days(i);
        let nav = 1.0 + (i as f64) * 0.001;
        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1, 'tiantian', 'fund-1', $2, $3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(format!("nav-fund-1-{i}"))
        .bind(d.format("%Y-%m-%d").to_string())
        .bind(format!("{nav:.4}"))
        .execute(&pool)
        .await
        .expect("seed nav");
    }

    // Seed minimal ML models for all-market peer so async task does not need to train in test.
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
        .bind(api::ml::train::PEER_CODE_ALL)
        .bind(task)
        .bind(h)
        .bind(&feature_names_json)
        .bind(&model_json)
        .bind(&metrics_json)
        .execute(&pool)
        .await
        .expect("seed all-market model");
    }

    let config = api::config::ConfigStore::load();
    let jwt = api::jwt::JwtService::from_secret("test-secret");
    let pool2 = pool.clone();
    let state = AppState::new(Some(pool), config, jwt, api::db::DatabaseKind::Sqlite);
    let token = state.jwt().issue_access_token("1");
    let app = api::service(state);

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/funds/signals/batch")
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                      "fund_codes": ["000001"],
                      "source": "tiantian"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), 202);
    let v = body_json(res).await;
    let task_id = v
        .get("task_id")
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string();
    assert!(!task_id.trim().is_empty());

    api::tasks::run_due_task_jobs(&pool2, 10)
        .await
        .expect("run task queue");

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/funds/signals/batch_async/{task_id}?page=1&page_size=10"
                ))
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    let v = body_json(res).await;
    let items = v["items"].as_array().expect("items array");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["fund_code"], "000001");
    assert!(
        items[0]["best_peer"].is_object(),
        "batch_async should include best_peer (at least __all__) even without relate themes"
    );
}
