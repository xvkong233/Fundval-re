use api::ml::dataset::{DatasetConfig, build_trigger_samples_for_peer};

#[tokio::test]
async fn build_trigger_samples_uses_sector_peer_trigger_and_labels() {
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

    // 30 个交易日：2026-01-01 .. 2026-01-30
    let mut dates: Vec<String> = Vec::new();
    for day in 1..=30 {
        dates.push(format!("2026-01-{:02}", day));
    }

    let navs_a: Vec<f64> = (0..30).map(|i| 1.0 + (i as f64) * 0.001).collect();
    let navs_b: Vec<f64> = vec![1.0; 30];
    let mut navs_c: Vec<f64> = vec![1.0; 30];
    navs_c[15] = 0.75; // 2026-01-16 深回撤触发
    navs_c[16] = 0.78;
    navs_c[17] = 0.80;
    navs_c[18] = 0.82;
    navs_c[19] = 0.83;
    navs_c[20] = 0.85; // 5T 后累计收益 > 0 且 max rebound >= 3%

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

    let cfg = DatasetConfig {
        lookback_days: 10,
        horizon_days: 5,
        stride_days: 1,
    };
    let samples = build_trigger_samples_for_peer(&pool, "BK000156", "tiantian", &cfg)
        .await
        .expect("build");

    assert!(!samples.is_empty(), "should build some trigger samples");

    let hit = samples
        .iter()
        .find(|s| s.fund_code == "000003" && s.as_of_date == "2026-01-16")
        .expect("expected trigger sample for 000003 at drawdown date");

    assert!(hit.dip_buy_success, "dip_buy_success should be true");
    assert!(hit.magic_rebound, "magic_rebound should be true");
}
