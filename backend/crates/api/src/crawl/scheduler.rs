use chrono::{DateTime, Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

pub fn daily_counter_key(job_type: &str, source_name: &str, kind: &str) -> String {
    let day = Utc::now().format("%Y%m%d").to_string();
    format!("crawl_{job_type}_{source_name}_{kind}_{day}")
}

pub fn daily_counter_key_all(source_name: &str, kind: &str) -> String {
    let day = Utc::now().format("%Y%m%d").to_string();
    format!("crawl_all_{source_name}_{kind}_{day}")
}

pub async fn get_counter(pool: &sqlx::AnyPool, key: &str) -> Result<i64, String> {
    let row = sqlx::query("SELECT value FROM crawl_state WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
    let Some(row) = row else {
        return Ok(0);
    };
    let v: String = row.get("value");
    Ok(v.trim().parse::<i64>().unwrap_or(0))
}

async fn bump_counter(pool: &sqlx::AnyPool, key: &str, delta: i64) -> Result<(), String> {
    let delta = delta.to_string();

    let sql_pg = r#"
        INSERT INTO crawl_state (key, value, updated_at)
        VALUES ($1, $2, CURRENT_TIMESTAMP)
        ON CONFLICT (key) DO UPDATE
          SET value = ((crawl_state.value)::bigint + ($2)::bigint)::text,
              updated_at = CURRENT_TIMESTAMP
    "#;

    let sql_any = r#"
        INSERT INTO crawl_state (key, value, updated_at)
        VALUES ($1, $2, CURRENT_TIMESTAMP)
        ON CONFLICT (key) DO UPDATE
          SET value = (CAST(crawl_state.value as INTEGER) + CAST(excluded.value as INTEGER)),
              updated_at = CURRENT_TIMESTAMP
    "#;

    let r = sqlx::query(sql_pg)
        .bind(key)
        .bind(&delta)
        .execute(pool)
        .await;
    if r.is_ok() {
        return Ok(());
    }

    sqlx::query(sql_any)
        .bind(key)
        .bind(&delta)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn enqueue_tick(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    let max_jobs = max_jobs.clamp(0, 5000);
    if max_jobs == 0 {
        return Ok(0);
    }

    let mut remaining = max_jobs;
    remaining -= enqueue_nav_for_watchlists(pool, remaining, source_name).await?;
    if remaining <= 0 {
        return Ok(max_jobs);
    }

    remaining -= enqueue_nav_for_positions(pool, remaining, source_name).await?;
    if remaining <= 0 {
        return Ok(max_jobs);
    }

    remaining -= enqueue_nav_for_all_funds_round_robin(pool, remaining, source_name).await?;
    if remaining <= 0 {
        return Ok(max_jobs);
    }

    // 关联板块（relate theme）也做分批同步：自选/持仓优先，其余慢速覆盖。
    remaining -= enqueue_relate_theme_for_watchlists(pool, remaining, source_name).await?;
    if remaining <= 0 {
        return Ok(max_jobs);
    }

    remaining -= enqueue_relate_theme_for_positions(pool, remaining, source_name).await?;
    if remaining <= 0 {
        return Ok(max_jobs);
    }

    remaining -=
        enqueue_relate_theme_for_all_funds_round_robin(pool, remaining, source_name).await?;

    Ok(max_jobs - remaining)
}

/// 估值同步入队：优先自选，其次持仓（避免全市场高频估值导致上游封锁）。
pub async fn enqueue_estimate_tick(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    let max_jobs = max_jobs.clamp(0, 5000);
    if max_jobs == 0 {
        return Ok(0);
    }

    let mut remaining = max_jobs;
    remaining -= enqueue_estimate_for_watchlists(pool, remaining, source_name).await?;
    if remaining <= 0 {
        return Ok(max_jobs);
    }

    remaining -= enqueue_estimate_for_positions(pool, remaining, source_name).await?;
    if remaining <= 0 {
        return Ok(max_jobs);
    }

    // 全市场慢速估值：只播种“还没有 estimate_sync job 的基金”，播种完成后稳定返回 0。
    remaining -= enqueue_estimate_for_all_funds_round_robin(pool, remaining, source_name).await?;
    Ok(max_jobs - remaining)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrawlJob {
    pub id: String,
    pub job_type: String,
    pub fund_code: Option<String>,
    pub source_name: Option<String>,
    pub priority: i64,
    pub attempt: i64,
}

pub async fn run_due_jobs<F, Fut>(
    pool: &sqlx::AnyPool,
    max_run: i64,
    exec: F,
) -> Result<i64, String>
where
    F: Fn(CrawlJob, String) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let max_run = max_run.clamp(0, 5000);
    if max_run == 0 {
        return Ok(0);
    }

    // NOTE: sqlx Any + SQLite 对 `$1` 这类占位符在某些语句（尤其 LIMIT）上存在兼容性差异。
    // 这里将 LIMIT 直接内联（max_run 已 clamp 到 0..=5000），避免 pool/conn 行为不一致导致“查询不到任务”。
    let sql = format!(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          job_type,
          fund_code,
          source_name,
          priority,
          attempt
        FROM crawl_job
        WHERE status = 'queued' AND not_before <= CURRENT_TIMESTAMP
        ORDER BY priority DESC, not_before ASC
        LIMIT {}
        "#,
        max_run
    );

    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let mut ran = 0_i64;
    for r in rows {
        let job = CrawlJob {
            id: r.get::<String, _>("id"),
            job_type: r.get::<String, _>("job_type"),
            fund_code: r.get::<Option<String>, _>("fund_code"),
            source_name: r.get::<Option<String>, _>("source_name"),
            priority: r.get::<i64, _>("priority"),
            attempt: r.get::<i64, _>("attempt"),
        };

        mark_running(pool, &job.id).await?;

        let run_id = crate::tasks::create_task_run(
            pool,
            "crawl_job",
            &job.id,
            &job.job_type,
            job.fund_code.as_deref(),
            job.source_name.as_deref(),
        )
        .await?;

        let source = job.source_name.as_deref().unwrap_or("unknown");
        let _ = bump_counter(pool, &daily_counter_key(&job.job_type, source, "run"), 1).await;
        let _ = bump_counter(pool, &daily_counter_key_all(source, "run"), 1).await;

        let attempt_now = job.attempt + 1;
        match exec(job.clone(), run_id.clone()).await {
            Ok(()) => {
                mark_ok(
                    pool,
                    &job.id,
                    next_at(success_delay_seconds(&job.job_type, job.priority)),
                )
                .await?;
                let _ = crate::tasks::finish_task_run_ok(pool, &run_id).await;
                let _ =
                    bump_counter(pool, &daily_counter_key(&job.job_type, source, "ok"), 1).await;
                let _ = bump_counter(pool, &daily_counter_key_all(source, "ok"), 1).await;
            }
            Err(e) => {
                let backoff = backoff_seconds(attempt_now);
                mark_error(pool, &job.id, &e, next_at(backoff)).await?;
                let _ = crate::tasks::finish_task_run_error(pool, &run_id, &e).await;
                let _ =
                    bump_counter(pool, &daily_counter_key(&job.job_type, source, "err"), 1).await;
                let _ = bump_counter(pool, &daily_counter_key_all(source, "err"), 1).await;
            }
        }

        ran += 1;
    }

    Ok(ran)
}

fn next_at(delay_seconds: i64) -> String {
    let now: DateTime<Utc> = Utc::now();
    let dt = now + Duration::seconds(delay_seconds.max(0));
    dt.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn backoff_seconds(attempt: i64) -> i64 {
    let attempt = attempt.clamp(1, 30);
    let pow = 1_i64.checked_shl((attempt - 1) as u32).unwrap_or(i64::MAX);
    (10_i64.saturating_mul(pow)).clamp(10, 3600)
}

fn success_delay_seconds(job_type: &str, priority: i64) -> i64 {
    // 估值相对更“实时”，但仍需节流以避免上游封锁。
    if job_type == "estimate_sync" {
        if priority >= 100 {
            2 * 60
        } else if priority >= 80 {
            5 * 60
        } else if priority >= 20 {
            30 * 60
        } else {
            6 * 60 * 60
        }
    } else {
        // 默认：自选更频繁，其次持仓，其余全量轮询放慢。
        if priority >= 100 {
            15 * 60
        } else if priority >= 80 {
            30 * 60
        } else {
            6 * 60 * 60
        }
    }
}

async fn mark_running(pool: &sqlx::AnyPool, id: &str) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE crawl_job
        SET status = 'running', attempt = attempt + 1, updated_at = CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_sqlite = r#"
        UPDATE crawl_job
        SET status = 'running', attempt = attempt + 1, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?1
    "#;

    if sqlx::query(sql_pg).bind(id).execute(pool).await.is_ok() {
        return Ok(());
    }

    sqlx::query(sql_sqlite)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn mark_ok(pool: &sqlx::AnyPool, id: &str, not_before: String) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE crawl_job
        SET status = 'queued',
            attempt = 0,
            last_ok_at = CURRENT_TIMESTAMP,
            last_error = NULL,
            not_before = ($2)::timestamptz,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_sqlite = r#"
        UPDATE crawl_job
        SET status = 'queued',
            attempt = 0,
            last_ok_at = CURRENT_TIMESTAMP,
            last_error = NULL,
            not_before = ?2,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?1
    "#;

    if sqlx::query(sql_pg)
        .bind(id)
        .bind(&not_before)
        .execute(pool)
        .await
        .is_ok()
    {
        return Ok(());
    }

    sqlx::query(sql_sqlite)
        .bind(id)
        .bind(&not_before)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn mark_error(
    pool: &sqlx::AnyPool,
    id: &str,
    err: &str,
    not_before: String,
) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE crawl_job
        SET status = 'queued',
            last_error = $2,
            not_before = ($3)::timestamptz,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_sqlite = r#"
        UPDATE crawl_job
        SET status = 'queued',
            last_error = ?2,
            not_before = ?3,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ?1
    "#;

    if sqlx::query(sql_pg)
        .bind(id)
        .bind(err)
        .bind(&not_before)
        .execute(pool)
        .await
        .is_ok()
    {
        return Ok(());
    }

    sqlx::query(sql_sqlite)
        .bind(id)
        .bind(err)
        .bind(&not_before)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn enqueue_nav_for_watchlists(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT f.fund_code as fund_code
        FROM watchlist_item wi
        JOIN fund f ON f.id = wi.fund_id
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_nav_job(pool, code.trim(), source_name, 100).await?;
        inserted += 1;
    }
    Ok(inserted)
}

async fn enqueue_estimate_for_watchlists(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT f.fund_code as fund_code
        FROM watchlist_item wi
        JOIN fund f ON f.id = wi.fund_id
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_estimate_job(pool, code.trim(), source_name, 100).await?;
        inserted += 1;
    }
    Ok(inserted)
}

async fn enqueue_relate_theme_for_watchlists(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT f.fund_code as fund_code
        FROM watchlist_item wi
        JOIN fund f ON f.id = wi.fund_id
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_relate_theme_job(pool, code.trim(), source_name, 90).await?;
        inserted += 1;
    }
    Ok(inserted)
}

async fn enqueue_nav_for_positions(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT f.fund_code as fund_code
        FROM position p
        JOIN fund f ON f.id = p.fund_id
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_nav_job(pool, code.trim(), source_name, 80).await?;
        inserted += 1;
    }
    Ok(inserted)
}

async fn enqueue_estimate_for_positions(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT f.fund_code as fund_code
        FROM position p
        JOIN fund f ON f.id = p.fund_id
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_estimate_job(pool, code.trim(), source_name, 80).await?;
        inserted += 1;
    }
    Ok(inserted)
}

async fn enqueue_estimate_for_all_funds_round_robin(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let rows = sqlx::query(
        r#"
        SELECT f.fund_code as fund_code
        FROM fund f
        LEFT JOIN crawl_job cj
          ON cj.job_type = 'estimate_sync'
         AND cj.fund_code = f.fund_code
         AND cj.source_name = $2
        WHERE cj.id IS NULL
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .bind(source_name)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in &rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_estimate_job(pool, code.trim(), source_name, 5).await?;
        inserted += 1;
    }

    Ok(inserted)
}

async fn enqueue_relate_theme_for_positions(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let rows = sqlx::query(
        r#"
        SELECT DISTINCT f.fund_code as fund_code
        FROM position p
        JOIN fund f ON f.id = p.fund_id
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_relate_theme_job(pool, code.trim(), source_name, 70).await?;
        inserted += 1;
    }
    Ok(inserted)
}

async fn enqueue_nav_for_all_funds_round_robin(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }
    // 全量播种：只补“还没有 crawl_job 的基金”，避免 OFFSET 轮询导致的反复扫描/抖动。
    // 一旦全量都播种完毕，此函数将稳定返回 0。
    let rows = sqlx::query(
        r#"
        SELECT f.fund_code as fund_code
        FROM fund f
        LEFT JOIN crawl_job cj
          ON cj.job_type = 'nav_history_sync'
         AND cj.fund_code = f.fund_code
         AND cj.source_name = $2
        WHERE cj.id IS NULL
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .bind(source_name)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in &rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_nav_job(pool, code.trim(), source_name, 10).await?;
        inserted += 1;
    }

    Ok(inserted)
}

async fn enqueue_relate_theme_for_all_funds_round_robin(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let rows = sqlx::query(
        r#"
        SELECT f.fund_code as fund_code
        FROM fund f
        LEFT JOIN crawl_job cj
          ON cj.job_type = 'relate_theme_sync'
         AND cj.fund_code = f.fund_code
         AND cj.source_name = $2
        WHERE cj.id IS NULL
        ORDER BY f.fund_code ASC
        LIMIT $1
        "#,
    )
    .bind(max_jobs)
    .bind(source_name)
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut inserted = 0_i64;
    for r in &rows {
        let code: String = r.get("fund_code");
        if code.trim().is_empty() {
            continue;
        }
        upsert_relate_theme_job(pool, code.trim(), source_name, 5).await?;
        inserted += 1;
    }

    Ok(inserted)
}

async fn upsert_nav_job(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source_name: &str,
    priority: i64,
) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    let code = fund_code.trim();
    let source = source_name.trim();
    if code.is_empty() || source.is_empty() {
        return Ok(());
    }

    let sql_pg = r#"
        INSERT INTO crawl_job (id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at)
        VALUES (($1)::uuid, 'nav_history_sync', $2, $3, $4, CURRENT_TIMESTAMP, 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (job_type, fund_code, source_name) DO UPDATE
          SET priority = EXCLUDED.priority,
              -- 若优先级被提升，则把任务尽量提前到“现在”（不推迟已更早的 not_before）
              not_before = CASE
                WHEN crawl_job.not_before > CURRENT_TIMESTAMP THEN CURRENT_TIMESTAMP
                ELSE crawl_job.not_before
              END,
              updated_at = CURRENT_TIMESTAMP
          WHERE EXCLUDED.priority > crawl_job.priority
    "#;

    let sql_any = r#"
        INSERT INTO crawl_job (id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at)
        VALUES ($1, 'nav_history_sync', $2, $3, $4, CURRENT_TIMESTAMP, 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (job_type, fund_code, source_name) DO UPDATE
          SET priority = EXCLUDED.priority,
              not_before = CASE
                WHEN crawl_job.not_before > CURRENT_TIMESTAMP THEN CURRENT_TIMESTAMP
                ELSE crawl_job.not_before
              END,
              updated_at = CURRENT_TIMESTAMP
          WHERE EXCLUDED.priority > crawl_job.priority
    "#;

    let r = sqlx::query(sql_pg)
        .bind(&id)
        .bind(code)
        .bind(source)
        .bind(priority)
        .execute(pool)
        .await;

    if r.is_ok() {
        return Ok(());
    }

    sqlx::query(sql_any)
        .bind(&id)
        .bind(code)
        .bind(source)
        .bind(priority)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn upsert_nav_history_job(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source_name: &str,
    priority: i64,
) -> Result<(), String> {
    upsert_nav_job(pool, fund_code, source_name, priority).await
}

pub async fn upsert_estimate_job(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source_name: &str,
    priority: i64,
) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    let code = fund_code.trim();
    let source = source_name.trim();
    if code.is_empty() || source.is_empty() {
        return Ok(());
    }

    let sql_pg = r#"
        INSERT INTO crawl_job (id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at)
        VALUES (($1)::uuid, 'estimate_sync', $2, $3, $4, CURRENT_TIMESTAMP, 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (job_type, fund_code, source_name) DO UPDATE
          SET priority = EXCLUDED.priority,
              -- 若优先级被提升，则把任务尽量提前到“现在”（不推迟已更早的 not_before）
              not_before = CASE
                WHEN crawl_job.not_before > CURRENT_TIMESTAMP THEN CURRENT_TIMESTAMP
                ELSE crawl_job.not_before
              END,
              updated_at = CURRENT_TIMESTAMP
          WHERE EXCLUDED.priority > crawl_job.priority
    "#;

    let sql_any = r#"
        INSERT INTO crawl_job (id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at)
        VALUES ($1, 'estimate_sync', $2, $3, $4, CURRENT_TIMESTAMP, 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (job_type, fund_code, source_name) DO UPDATE
          SET priority = EXCLUDED.priority,
              not_before = CASE
                WHEN crawl_job.not_before > CURRENT_TIMESTAMP THEN CURRENT_TIMESTAMP
                ELSE crawl_job.not_before
              END,
              updated_at = CURRENT_TIMESTAMP
          WHERE EXCLUDED.priority > crawl_job.priority
    "#;

    let r = sqlx::query(sql_pg)
        .bind(&id)
        .bind(code)
        .bind(source)
        .bind(priority)
        .execute(pool)
        .await;

    if r.is_ok() {
        return Ok(());
    }

    sqlx::query(sql_any)
        .bind(&id)
        .bind(code)
        .bind(source)
        .bind(priority)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

async fn upsert_relate_theme_job(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source_name: &str,
    priority: i64,
) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    let code = fund_code.trim();
    let source = source_name.trim();
    if code.is_empty() || source.is_empty() {
        return Ok(());
    }

    let sql_pg = r#"
        INSERT INTO crawl_job (id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at)
        VALUES (($1)::uuid, 'relate_theme_sync', $2, $3, $4, CURRENT_TIMESTAMP, 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (job_type, fund_code, source_name) DO UPDATE
          SET priority = EXCLUDED.priority,
              not_before = CASE
                WHEN crawl_job.not_before > CURRENT_TIMESTAMP THEN CURRENT_TIMESTAMP
                ELSE crawl_job.not_before
              END,
              updated_at = CURRENT_TIMESTAMP
          WHERE EXCLUDED.priority > crawl_job.priority
    "#;

    let sql_any = r#"
        INSERT INTO crawl_job (id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at)
        VALUES ($1, 'relate_theme_sync', $2, $3, $4, CURRENT_TIMESTAMP, 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (job_type, fund_code, source_name) DO UPDATE
          SET priority = EXCLUDED.priority,
              not_before = CASE
                WHEN crawl_job.not_before > CURRENT_TIMESTAMP THEN CURRENT_TIMESTAMP
                ELSE crawl_job.not_before
              END,
              updated_at = CURRENT_TIMESTAMP
          WHERE EXCLUDED.priority > crawl_job.priority
    "#;

    let r = sqlx::query(sql_pg)
        .bind(&id)
        .bind(code)
        .bind(source)
        .bind(priority)
        .execute(pool)
        .await;

    if r.is_ok() {
        return Ok(());
    }

    sqlx::query(sql_any)
        .bind(&id)
        .bind(code)
        .bind(source)
        .bind(priority)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}
