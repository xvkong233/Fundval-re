use sqlx::Row;

#[tokio::test]
async fn enqueue_estimate_tick_prioritizes_watchlists_then_positions() {
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
        INSERT INTO auth_user (id, password, username, is_staff, is_active)
        VALUES (1, 'x', 'u', 0, 1)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed user");

    for (id, code) in [("f-a", "A"), ("f-b", "B"), ("f-c", "C")] {
        sqlx::query(
            r#"
            INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
            VALUES ($1, $2, $3, '股票型', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind(id)
        .bind(code)
        .bind(format!("fund-{code}"))
        .execute(&pool)
        .await
        .expect("seed fund");
    }

    sqlx::query("INSERT INTO watchlist (id, user_id, name) VALUES ('wl-1', 1, '自选')")
        .execute(&pool)
        .await
        .expect("watchlist");
    sqlx::query(
        "INSERT INTO watchlist_item (id, watchlist_id, fund_id, \"order\") VALUES ('wli-1','wl-1','f-a',0)",
    )
    .execute(&pool)
    .await
    .expect("watchlist item");

    sqlx::query(
        "INSERT INTO account (id, user_id, name, is_default) VALUES ('acc-1', 1, '默认', 1)",
    )
    .execute(&pool)
    .await
    .expect("account");
    sqlx::query("INSERT INTO position (id, account_id, fund_id) VALUES ('pos-1','acc-1','f-b')")
        .execute(&pool)
        .await
        .expect("position");

    api::crawl::scheduler::enqueue_estimate_tick(&pool, 10, "tiantian")
        .await
        .expect("enqueue estimate tick");

    let rows = sqlx::query(
        r#"
        SELECT fund_code, priority
        FROM crawl_job
        WHERE job_type = 'estimate_sync'
        ORDER BY priority DESC, fund_code ASC
        "#,
    )
    .fetch_all(&pool)
    .await
    .expect("select jobs");

    let got: Vec<(String, i64)> = rows
        .into_iter()
        .map(|r| (r.get::<String, _>("fund_code"), r.get::<i64, _>("priority")))
        .collect();

    assert_eq!(got.len(), 3);
    // A from watchlist should have higher priority than B from positions, and C is all-funds slow seed.
    assert_eq!(got[0].0, "A");
    assert_eq!(got[1].0, "B");
    assert_eq!(got[2].0, "C");
    assert!(got[0].1 >= got[1].1);
    assert!(got[1].1 > got[2].1);
}

#[tokio::test]
async fn enqueue_estimate_tick_does_not_touch_updated_at_when_priority_unchanged() {
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
        INSERT INTO auth_user (id, password, username, is_staff, is_active)
        VALUES (1, 'x', 'u', 0, 1)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed user");

    sqlx::query(
        r#"
        INSERT INTO fund (id, fund_code, fund_name, fund_type, created_at, updated_at)
        VALUES ('f-a', 'A', 'fund-A', '股票型', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed fund");

    // 预先插入一个同 key 的任务（priority=100）并把 updated_at 固定成老时间。
    sqlx::query(
        r#"
        INSERT INTO crawl_job (
          id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at
        ) VALUES (
          'job-old', 'estimate_sync', 'A', 'tiantian', 100, CURRENT_TIMESTAMP, 'queued', 0, '2000-01-01 00:00:00', '2000-01-01 00:00:00'
        )
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed crawl job");

    sqlx::query("INSERT INTO watchlist (id, user_id, name) VALUES ('wl-1', 1, '自选')")
        .execute(&pool)
        .await
        .expect("watchlist");
    sqlx::query(
        "INSERT INTO watchlist_item (id, watchlist_id, fund_id, \"order\") VALUES ('wli-1','wl-1','f-a',0)",
    )
    .execute(&pool)
    .await
    .expect("watchlist item");

    api::crawl::scheduler::enqueue_estimate_tick(&pool, 1, "tiantian")
        .await
        .expect("enqueue estimate tick");

    let row = sqlx::query(
        "SELECT CAST(updated_at AS TEXT) as updated_at, priority FROM crawl_job WHERE job_type='estimate_sync' AND fund_code='A' AND source_name='tiantian'",
    )
    .fetch_one(&pool)
    .await
    .expect("select job");

    let updated_at: String = row.get("updated_at");
    assert_eq!(updated_at, "2000-01-01 00:00:00");
    assert_eq!(row.get::<i64, _>("priority"), 100);
}
