use sqlx::Row;

use api::ml::compute::compute_and_store_fund_snapshot;
use api::ml::dataset::DatasetConfig;
use api::ml::train::{MlTask, train_and_store_sector_model};

#[tokio::test]
async fn compute_writes_fund_signal_snapshot_row() {
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
    for i in 0..60 {
        let d = start + chrono::Duration::days(i);
        dates.push(d.format("%Y-%m-%d").to_string());
    }

    let navs_a: Vec<f64> = (0..60).map(|i| 1.0 + (i as f64) * 0.001).collect();
    let navs_b: Vec<f64> = vec![1.0; 60];
    let mut navs_c: Vec<f64> = vec![1.0; 60];
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

    let cfg5 = DatasetConfig {
        lookback_days: 10,
        horizon_days: 5,
        stride_days: 2,
    };
    train_and_store_sector_model(&pool, "BK000156", "tiantian", MlTask::DipBuy, &cfg5)
        .await
        .expect("train dip_buy 5");

    compute_and_store_fund_snapshot(&pool, "000003", "BK000156", "tiantian")
        .await
        .expect("compute snapshot");

    let row = sqlx::query(
        r#"
        SELECT
          dip_buy_proba_5t
        FROM fund_signal_snapshot
        WHERE fund_code=$1 AND peer_code=$2
        ORDER BY computed_at DESC
        LIMIT 1
        "#,
    )
    .bind("000003")
    .bind("BK000156")
    .fetch_one(&pool)
    .await
    .expect("read snapshot");

    let p: Option<f64> = row.try_get("dip_buy_proba_5t").ok();
    let p = p.expect("dip_buy_proba_5t should exist");
    assert!(p >= 0.0 && p <= 1.0);
}
