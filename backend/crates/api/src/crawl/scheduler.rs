use chrono::{DateTime, Duration, Utc};
use sqlx::Row;
use uuid::Uuid;

pub async fn enqueue_tick(pool: &sqlx::AnyPool, max_jobs: i64, source_name: &str) -> Result<i64, String> {
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
    F: Fn(CrawlJob) -> Fut,
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

        let attempt_now = job.attempt + 1;
        match exec(job.clone()).await {
            Ok(()) => {
                mark_ok(pool, &job.id, next_at(success_delay_seconds(job.priority))).await?;
            }
            Err(e) => {
                let backoff = backoff_seconds(attempt_now);
                mark_error(pool, &job.id, &e, next_at(backoff)).await?;
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

fn success_delay_seconds(priority: i64) -> i64 {
    // 自选更频繁，其次持仓，其余全量轮询放慢。
    if priority >= 100 {
        15 * 60
    } else if priority >= 80 {
        30 * 60
    } else {
        6 * 60 * 60
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
            last_ok_at = CURRENT_TIMESTAMP,
            last_error = NULL,
            not_before = ($2)::timestamptz,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_sqlite = r#"
        UPDATE crawl_job
        SET status = 'queued',
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

async fn enqueue_nav_for_all_funds_round_robin(
    pool: &sqlx::AnyPool,
    max_jobs: i64,
    source_name: &str,
) -> Result<i64, String> {
    if max_jobs <= 0 {
        return Ok(0);
    }

    let offset: i64 = sqlx::query("SELECT value FROM crawl_state WHERE key = 'fund_offset'")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<String, _>("value").ok())
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(0)
        .max(0);

    let rows = sqlx::query(
        r#"
        SELECT fund_code
        FROM fund
        ORDER BY fund_code ASC
        LIMIT $1
        OFFSET $2
        "#,
    )
    .bind(max_jobs)
    .bind(offset)
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

    let new_offset = if rows.is_empty() { 0 } else { offset + rows.len() as i64 };
    let _ = sqlx::query(
        r#"
        INSERT INTO crawl_state (key, value, updated_at)
        VALUES ('fund_offset', $1, CURRENT_TIMESTAMP)
        ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(new_offset.to_string())
    .execute(pool)
    .await;

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
          SET priority = CASE WHEN EXCLUDED.priority > crawl_job.priority THEN EXCLUDED.priority ELSE crawl_job.priority END,
              updated_at = CURRENT_TIMESTAMP
    "#;

    let sql_any = r#"
        INSERT INTO crawl_job (id, job_type, fund_code, source_name, priority, not_before, status, attempt, created_at, updated_at)
        VALUES ($1, 'nav_history_sync', $2, $3, $4, CURRENT_TIMESTAMP, 'queued', 0, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        ON CONFLICT (job_type, fund_code, source_name) DO UPDATE
          SET priority = CASE WHEN EXCLUDED.priority > crawl_job.priority THEN EXCLUDED.priority ELSE crawl_job.priority END,
              updated_at = CURRENT_TIMESTAMP
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
