use api::sim::engine;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::migrate::Migrator;
use sqlx::Row;

static MIGRATOR_SQLITE: Migrator = sqlx::migrate!("../../migrations/sqlite");

async fn setup_pool() -> sqlx::AnyPool {
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect sqlite in-memory");
    MIGRATOR_SQLITE.run(&pool).await.expect("migrations");

    sqlx::query(
        r#"
        INSERT INTO auth_user (id, password, is_superuser, username, email, is_staff, is_active, date_joined)
        VALUES (1, 'x', 1, 'admin', 'admin@example.com', 1, 1, CURRENT_TIMESTAMP)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed auth_user");

    pool
}

async fn seed_fund_nav(pool: &sqlx::AnyPool, fund_code: &str, dates_and_navs: &[(NaiveDate, &str)]) {
    let fund_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO fund (id, fund_code, fund_name, fund_type, latest_nav, latest_nav_date, estimate_nav, estimate_growth, estimate_time, created_at, updated_at)
        VALUES ($1,$2,'T','',NULL,NULL,NULL,NULL,NULL,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&fund_id)
    .bind(fund_code)
    .execute(pool)
    .await
    .expect("insert fund");

    for (d, nav) in dates_and_navs {
        sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, created_at, updated_at)
            VALUES ($1,'tiantian',$2,$3,CAST($4 AS NUMERIC),CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&fund_id)
        .bind(d.format("%Y-%m-%d").to_string())
        .bind(nav.to_string())
        .execute(pool)
        .await
        .expect("insert nav");
    }
}

async fn seed_snapshot(pool: &sqlx::AnyPool, fund_code: &str, date: NaiveDate, magic20: f64) {
    sqlx::query(
        r#"
        INSERT INTO fund_signal_snapshot (
          fund_code, peer_code, as_of_date,
          position_percentile_0_100, position_bucket,
          dip_buy_proba_5t, dip_buy_proba_20t,
          magic_rebound_proba_5t, magic_rebound_proba_20t,
          computed_at, created_at, updated_at
        )
        VALUES ($1,$2,$3,NULL,NULL,NULL,NULL,NULL,$4,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT (fund_code, peer_code, as_of_date) DO UPDATE SET
          magic_rebound_proba_20t = excluded.magic_rebound_proba_20t,
          updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(fund_code)
    .bind(api::ml::train::PEER_CODE_ALL)
    .bind(date.format("%Y-%m-%d").to_string())
    .bind(magic20)
    .execute(pool)
    .await
    .expect("seed snapshot");
}

#[tokio::test]
async fn auto_topk_snapshot_backtest_generates_equity_curve() {
    let pool = setup_pool().await;

    let dates = [
        NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 2).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 3).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 4).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(),
    ];

    seed_fund_nav(
        &pool,
        "AAA",
        &[
            (dates[0], "1.00"),
            (dates[1], "1.02"),
            (dates[2], "1.04"),
            (dates[3], "1.06"),
            (dates[4], "1.08"),
        ],
    )
    .await;
    seed_fund_nav(
        &pool,
        "BBB",
        &[
            (dates[0], "1.00"),
            (dates[1], "1.00"),
            (dates[2], "1.00"),
            (dates[3], "1.00"),
            (dates[4], "1.00"),
        ],
    )
    .await;

    for d in dates {
        seed_snapshot(&pool, "AAA", d, 0.9).await;
        seed_snapshot(&pool, "BBB", d, 0.1).await;
    }

    let run_id = engine::backtest_create_auto_topk_snapshot(
        &pool,
        1,
        "auto",
        "tiantian",
        NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(),
        Decimal::from(1000),
        0.0,
        0.0,
        0,
        engine::AutoTopkSnapshotParams {
            top_k: 1,
            rebalance_every: 1,
            weights: None,
        },
    )
    .await
    .expect("create");

    engine::backtest_run(&pool, &run_id).await.expect("run");

    let rows = sqlx::query(
        r#"
        SELECT COUNT(1) as c, MAX(total_equity) as max_eq
        FROM sim_daily_equity
        WHERE run_id = $1
        "#,
    )
    .bind(&run_id)
    .fetch_one(&pool)
    .await
    .expect("equity rows");
    let c: i64 = rows.get("c");
    let max_eq: f64 = rows.get("max_eq");
    assert!(c >= 3, "should write equity points");
    assert!(max_eq > 1000.0, "should profit when picking rising fund");
}

#[tokio::test]
async fn auto_topk_snapshot_training_writes_rounds_and_updates_weights() {
    let pool = setup_pool().await;

    let dates = [
        NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 2).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 3).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 4).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(),
    ];

    seed_fund_nav(
        &pool,
        "AAA",
        &[
            (dates[0], "1.00"),
            (dates[1], "1.02"),
            (dates[2], "1.04"),
            (dates[3], "1.06"),
            (dates[4], "1.08"),
        ],
    )
    .await;
    seed_fund_nav(
        &pool,
        "BBB",
        &[
            (dates[0], "1.00"),
            (dates[1], "1.00"),
            (dates[2], "1.00"),
            (dates[3], "1.00"),
            (dates[4], "1.00"),
        ],
    )
    .await;

    for d in dates {
        seed_snapshot(&pool, "AAA", d, 0.9).await;
        seed_snapshot(&pool, "BBB", d, 0.1).await;
    }

    let run_id = engine::backtest_create_auto_topk_snapshot(
        &pool,
        1,
        "train",
        "tiantian",
        NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(),
        Decimal::from(1000),
        0.0,
        0.0,
        0,
        engine::AutoTopkSnapshotParams {
            top_k: 1,
            rebalance_every: 2,
            weights: None,
        },
    )
    .await
    .expect("create");

    let rounds = engine::train_auto_topk_snapshot(&pool, &run_id, 3, 8, 0.25, Some(7))
        .await
        .expect("train");
    assert_eq!(rounds.len(), 3);

    let row = sqlx::query("SELECT COUNT(1) as c FROM sim_train_round WHERE run_id = $1")
        .bind(&run_id)
        .fetch_one(&pool)
        .await
        .expect("count");
    let c: i64 = row.get("c");
    assert_eq!(c, 3);

    let run = api::sim::db::load_run(&pool, &run_id)
        .await
        .expect("load")
        .expect("exists");
    assert!(run.strategy_params_json.contains("weights"));
}

