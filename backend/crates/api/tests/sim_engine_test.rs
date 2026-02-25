use api::sim::engine::{Action, Side, env_create, env_step};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::migrate::Migrator;

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
        VALUES (1, 'pbkdf2_sha256$1$salt$hash', 1, 'admin', 'admin@example.com', 1, 1, CURRENT_TIMESTAMP)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed auth_user");

    pool
}

async fn seed_fund_nav(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    dates_and_navs: &[(NaiveDate, &str)],
) {
    // Insert fund
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

#[tokio::test]
async fn env_step_sell_settles_cash_on_t_plus_2() {
    let pool = setup_pool().await;

    let dates = [
        (NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(), "1.0"),
        (NaiveDate::from_ymd_opt(2026, 2, 2).unwrap(), "1.0"),
        (NaiveDate::from_ymd_opt(2026, 2, 3).unwrap(), "1.0"),
        (NaiveDate::from_ymd_opt(2026, 2, 4).unwrap(), "1.0"),
        (NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(), "1.0"),
    ];
    seed_fund_nav(&pool, "000001", &dates).await;

    let (run_id, _obs0) = env_create(
        &pool,
        1,
        "test",
        "tiantian",
        &["000001".to_string()],
        NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
        NaiveDate::from_ymd_opt(2026, 2, 5).unwrap(),
        Decimal::from(1000),
        0.0,
        0.0,
        2,
    )
    .await
    .expect("create env");

    // T=2/1 buy 1000 -> executes at 2/2
    let r1 = env_step(
        &pool,
        &run_id,
        &[Action {
            side: Side::Buy,
            fund_code: "000001".to_string(),
            amount: Some("1000".to_string()),
            shares: None,
        }],
    )
    .await
    .expect("step buy");
    assert_eq!(r1.date, "2026-02-02");
    assert_eq!(r1.observation.cash_available, "0");

    // T=2/2 sell 1000 shares -> executes at 2/3, cash settles at 2/5 (T+2)
    let r2 = env_step(
        &pool,
        &run_id,
        &[Action {
            side: Side::Sell,
            fund_code: "000001".to_string(),
            amount: None,
            shares: Some("1000".to_string()),
        }],
    )
    .await
    .expect("step sell");
    assert_eq!(r2.date, "2026-02-03");
    assert_eq!(r2.observation.cash_available, "0");
    assert_eq!(r2.observation.cash_receivable, "1000");

    // 2/4: still receivable
    let r3 = env_step(&pool, &run_id, &[]).await.expect("step");
    assert_eq!(r3.date, "2026-02-04");
    assert_eq!(r3.observation.cash_available, "0");
    assert_eq!(r3.observation.cash_receivable, "1000");

    // 2/5: settled
    let r4 = env_step(&pool, &run_id, &[]).await.expect("step");
    assert_eq!(r4.date, "2026-02-05");
    assert_eq!(r4.observation.cash_available, "1000");
    assert_eq!(r4.observation.cash_receivable, "0");
    assert!(r4.done);
}
