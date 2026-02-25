use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TaskRunRow {
    pub id: String,
    pub queue_type: String,
    pub job_id: String,
    pub job_type: String,
    pub fund_code: Option<String>,
    pub source_name: Option<String>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

pub async fn create_task_run(
    pool: &sqlx::AnyPool,
    queue_type: &str,
    job_id: &str,
    job_type: &str,
    fund_code: Option<&str>,
    source_name: Option<&str>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();

    let sql_pg = r#"
        INSERT INTO task_run (
          id, queue_type, job_id, job_type, fund_code, source_name, status, started_at, created_at
        )
        VALUES (($1)::uuid,$2,($3)::uuid,$4,$5,$6,'running',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
    "#;
    let sql_any = r#"
        INSERT INTO task_run (
          id, queue_type, job_id, job_type, fund_code, source_name, status, started_at, created_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,'running',CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
    "#;

    if sqlx::query(sql_pg)
        .bind(&id)
        .bind(queue_type)
        .bind(job_id)
        .bind(job_type)
        .bind(fund_code)
        .bind(source_name)
        .execute(pool)
        .await
        .is_ok()
    {
        return Ok(id);
    }

    sqlx::query(sql_any)
        .bind(&id)
        .bind(queue_type)
        .bind(job_id)
        .bind(job_type)
        .bind(fund_code)
        .bind(source_name)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(id)
}

pub async fn finish_task_run_ok(pool: &sqlx::AnyPool, run_id: &str) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE task_run
        SET status='ok', error=NULL, finished_at=CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_any = r#"
        UPDATE task_run
        SET status='ok', error=NULL, finished_at=CURRENT_TIMESTAMP
        WHERE id = $1
    "#;

    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres { sql_pg } else { sql_any };
    sqlx::query(sql)
        .bind(run_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn finish_task_run_error(pool: &sqlx::AnyPool, run_id: &str, err: &str) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE task_run
        SET status='error', error=$2, finished_at=CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_any = r#"
        UPDATE task_run
        SET status='error', error=$2, finished_at=CURRENT_TIMESTAMP
        WHERE id = $1
    "#;

    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres { sql_pg } else { sql_any };
    sqlx::query(sql)
        .bind(run_id)
        .bind(err)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn append_task_log(pool: &sqlx::AnyPool, run_id: &str, level: &str, message: &str) -> Result<(), String> {
    let id = Uuid::new_v4().to_string();
    let sql_pg = r#"
        INSERT INTO task_run_log (id, run_id, level, message, created_at)
        VALUES (($1)::uuid,($2)::uuid,$3,$4,CURRENT_TIMESTAMP)
    "#;
    let sql_any = r#"
        INSERT INTO task_run_log (id, run_id, level, message, created_at)
        VALUES ($1,$2,$3,$4,CURRENT_TIMESTAMP)
    "#;

    if sqlx::query(sql_pg)
        .bind(&id)
        .bind(run_id)
        .bind(level)
        .bind(message)
        .execute(pool)
        .await
        .is_ok()
    {
        return Ok(());
    }
    sqlx::query(sql_any)
        .bind(&id)
        .bind(run_id)
        .bind(level)
        .bind(message)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct TaskJobRow {
    pub id: String,
    pub task_type: String,
    pub payload_json: String,
    pub priority: i64,
    pub not_before: String,
    pub status: String,
    pub attempt: i64,
    pub error: Option<String>,
    pub created_by: Option<i64>,
}

pub async fn enqueue_task_job(
    pool: &sqlx::AnyPool,
    task_type: &str,
    payload: &Value,
    priority: i64,
    created_by: Option<i64>,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let payload_json = serde_json::to_string(payload).map_err(|e| e.to_string())?;

    let sql_pg = r#"
        INSERT INTO task_job (
          id, task_type, payload_json, priority, not_before, status, attempt, error, created_by, created_at, updated_at
        )
        VALUES (($1)::uuid,$2,$3,$4,CURRENT_TIMESTAMP,'queued',0,NULL,$5,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
    "#;
    let sql_any = r#"
        INSERT INTO task_job (
          id, task_type, payload_json, priority, not_before, status, attempt, error, created_by, created_at, updated_at
        )
        VALUES ($1,$2,$3,$4,CURRENT_TIMESTAMP,'queued',0,NULL,$5,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
    "#;

    if sqlx::query(sql_pg)
        .bind(&id)
        .bind(task_type)
        .bind(&payload_json)
        .bind(priority)
        .bind(created_by)
        .execute(pool)
        .await
        .is_ok()
    {
        return Ok(id);
    }
    // created_by 可能指向不存在的用户（测试/用户被删除等）；此时降级为 NULL，避免整批任务入队失败。
    if created_by.is_some()
        && sqlx::query(sql_pg)
            .bind(&id)
            .bind(task_type)
            .bind(&payload_json)
            .bind(priority)
            .bind(None::<i64>)
            .execute(pool)
            .await
            .is_ok()
    {
        return Ok(id);
    }

    let r = sqlx::query(sql_any)
        .bind(&id)
        .bind(task_type)
        .bind(&payload_json)
        .bind(priority)
        .bind(created_by)
        .execute(pool)
        .await;
    if r.is_ok() {
        return Ok(id);
    }

    if created_by.is_some() {
        sqlx::query(sql_any)
            .bind(&id)
            .bind(task_type)
            .bind(&payload_json)
            .bind(priority)
            .bind(None::<i64>)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(id);
    }

    r.map_err(|e| e.to_string())?;
    Ok(id)
}

pub async fn get_task_job(pool: &sqlx::AnyPool, id: &str) -> Result<Option<TaskJobRow>, String> {
    let row = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          task_type,
          payload_json,
          priority,
          CAST(not_before AS TEXT) as not_before,
          status,
          attempt,
          error,
          created_by
        FROM task_job
        WHERE CAST(id AS TEXT) = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|r| TaskJobRow {
        id: r.get("id"),
        task_type: r.get("task_type"),
        payload_json: r.get("payload_json"),
        priority: r.get("priority"),
        not_before: r.get("not_before"),
        status: r.get("status"),
        attempt: r.get("attempt"),
        error: r.try_get::<Option<String>, _>("error").ok().flatten(),
        created_by: r.try_get::<Option<i64>, _>("created_by").ok().flatten(),
    }))
}

async fn mark_task_job_running(pool: &sqlx::AnyPool, id: &str) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE task_job
        SET status='running',
            attempt = attempt + 1,
            started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_any = r#"
        UPDATE task_job
        SET status='running',
            attempt = attempt + 1,
            started_at = COALESCE(started_at, CURRENT_TIMESTAMP),
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $1
    "#;

    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres { sql_pg } else { sql_any };
    sqlx::query(sql)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn mark_task_job_done(pool: &sqlx::AnyPool, id: &str) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE task_job
        SET status='done',
            error=NULL,
            finished_at = CURRENT_TIMESTAMP,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_any = r#"
        UPDATE task_job
        SET status='done',
            error=NULL,
            finished_at = CURRENT_TIMESTAMP,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $1
    "#;

    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres { sql_pg } else { sql_any };
    sqlx::query(sql)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn mark_task_job_error(pool: &sqlx::AnyPool, id: &str, err: &str) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE task_job
        SET status='error',
            error=$2,
            finished_at = CURRENT_TIMESTAMP,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = ($1)::uuid
    "#;
    let sql_any = r#"
        UPDATE task_job
        SET status='error',
            error=$2,
            finished_at = CURRENT_TIMESTAMP,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $1
    "#;

    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres { sql_pg } else { sql_any };
    sqlx::query(sql)
        .bind(id)
        .bind(err)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn run_due_task_jobs(pool: &sqlx::AnyPool, max_run: i64) -> Result<i64, String> {
    let max_run = max_run.clamp(0, 200);
    if max_run == 0 {
        return Ok(0);
    }

    let sql = format!(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          task_type,
          payload_json,
          priority,
          CAST(not_before AS TEXT) as not_before,
          status,
          attempt,
          error,
          created_by
        FROM task_job
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
        let job = TaskJobRow {
            id: r.get("id"),
            task_type: r.get("task_type"),
            payload_json: r.get("payload_json"),
            priority: r.get("priority"),
            not_before: r.get("not_before"),
            status: r.get("status"),
            attempt: r.get("attempt"),
            error: r.try_get::<Option<String>, _>("error").ok().flatten(),
            created_by: r.try_get::<Option<i64>, _>("created_by").ok().flatten(),
        };

        mark_task_job_running(pool, &job.id).await?;
        let run_id = create_task_run(
            pool,
            "task_job",
            &job.id,
            &job.task_type,
            None,
            None,
        )
        .await?;

        let exec_result = match job.task_type.as_str() {
            "signals_batch" => exec_signals_batch(pool, &run_id, &job).await,
            "nav_history_sync_batch" => exec_nav_history_sync_batch(pool, &run_id, &job).await,
            "sniffer_sync" => exec_sniffer_sync(pool, &run_id, &job).await,
            "forecast_model_train" => exec_forecast_model_train(pool, &run_id, &job).await,
            "fund_analysis_v2_compute" => exec_fund_analysis_v2_compute(pool, &run_id, &job).await,
            "prices_refresh_batch" => exec_prices_refresh_batch(pool, &run_id, &job).await,
            "quant_xalpha_metrics_batch" => exec_quant_xalpha_metrics_batch(pool, &run_id, &job).await,
            "quant_xalpha_grid_batch" => exec_quant_xalpha_grid_batch(pool, &run_id, &job).await,
            "quant_xalpha_scheduled_batch" => {
                exec_quant_xalpha_scheduled_batch(pool, &run_id, &job).await
            }
            "quant_xalpha_qdiipredict_batch" => {
                exec_quant_xalpha_qdiipredict_batch(pool, &run_id, &job).await
            }
            _ => Err(format!("unknown task_type: {}", job.task_type)),
        };

        match exec_result {
            Ok(()) => {
                let _ = append_task_log(pool, &run_id, "INFO", "任务执行完成").await;
                let _ = finish_task_run_ok(pool, &run_id).await;
                mark_task_job_done(pool, &job.id).await?;
            }
            Err(e) => {
                let _ = append_task_log(pool, &run_id, "ERROR", &format!("任务执行失败：{e}")).await;
                let _ = finish_task_run_error(pool, &run_id, &e).await;
                mark_task_job_error(pool, &job.id, &e).await?;
            }
        }

        ran += 1;
    }

    Ok(ran)
}

fn parse_source_list(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in raw.split(',') {
        let s = p.trim();
        if s.is_empty() {
            continue;
        }
        let Some(n) = crate::sources::normalize_source_name(s) else {
            continue;
        };
        let n = n.to_string();
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

async fn update_fund_latest_nav_fields(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    latest_nav: &str,
    latest_nav_date: &str,
) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE fund
        SET latest_nav = CAST($2 AS NUMERIC),
            latest_nav_date = CAST($3 AS DATE),
            updated_at = CURRENT_TIMESTAMP
        WHERE fund_code = $1
    "#;
    let sql_any = r#"
        UPDATE fund
        SET latest_nav = $2,
            latest_nav_date = $3,
            updated_at = CURRENT_TIMESTAMP
        WHERE fund_code = $1
    "#;

    if sqlx::query(sql_pg)
        .bind(fund_code)
        .bind(latest_nav)
        .bind(latest_nav_date)
        .execute(pool)
        .await
        .is_ok()
    {
        return Ok(());
    }

    sqlx::query(sql_any)
        .bind(fund_code)
        .bind(latest_nav)
        .bind(latest_nav_date)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

async fn update_fund_estimate_fields(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    estimate_nav: &str,
    estimate_growth: Option<&str>,
    estimate_time_rfc3339: &str,
) -> Result<(), String> {
    let sql_pg = r#"
        UPDATE fund
        SET estimate_nav = CAST($2 AS NUMERIC),
            estimate_growth = CAST($3 AS NUMERIC),
            estimate_time = CAST($4 AS TIMESTAMPTZ),
            updated_at = CURRENT_TIMESTAMP
        WHERE fund_code = $1
    "#;
    let sql_any = r#"
        UPDATE fund
        SET estimate_nav = $2,
            estimate_growth = $3,
            estimate_time = $4,
            updated_at = CURRENT_TIMESTAMP
        WHERE fund_code = $1
    "#;

    if sqlx::query(sql_pg)
        .bind(fund_code)
        .bind(estimate_nav)
        .bind(estimate_growth)
        .bind(estimate_time_rfc3339)
        .execute(pool)
        .await
        .is_ok()
    {
        return Ok(());
    }

    sqlx::query(sql_any)
        .bind(fund_code)
        .bind(estimate_nav)
        .bind(estimate_growth)
        .bind(estimate_time_rfc3339)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

async fn exec_prices_refresh_batch(
    pool: &sqlx::AnyPool,
    run_id: &str,
    job: &TaskJobRow,
) -> Result<(), String> {
    use chrono::{SecondsFormat, Utc};

    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;
    let fund_codes = payload
        .get("fund_codes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing fund_codes".to_string())?;

    let source_raw = payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(crate::sources::SOURCE_TIANTIAN);
    let Some(source_name) = crate::sources::normalize_source_name(source_raw) else {
        return Err(format!("unknown source: {source_raw}"));
    };

    let mut codes: Vec<String> = Vec::with_capacity(fund_codes.len());
    for v in fund_codes {
        if let Some(s) = v.as_str() {
            let t = s.trim();
            if !t.is_empty() {
                codes.push(t.to_string());
            }
        }
    }
    if codes.is_empty() {
        return Err("fund_codes empty".to_string());
    }
    if codes.len() > 2000 {
        codes.truncate(2000);
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("prices_refresh_batch: fund_codes={} source={source_name}", codes.len()),
    )
    .await;

    let client = crate::eastmoney::build_client()?;

    let mut ok = 0_i64;
    let mut failed = 0_i64;

    for (idx, code) in codes.iter().enumerate() {
        if idx % 20 == 0 {
            let _ = append_task_log(pool, run_id, "INFO", &format!("进度 {idx}/{}", codes.len())).await;
        }

        let _ = append_task_log(pool, run_id, "INFO", &format!("[{code}] 刷新开始")).await;

        let snap = match crate::eastmoney::fetch_fundgz_snapshot(&client, code).await {
            Ok(Some(v)) => v,
            Ok(None) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] fundgz 返回空")).await;
                continue;
            }
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] fundgz 请求失败: {e}")).await;
                continue;
            }
        };

        let mut did_any = false;

        if let (Some(nav), Some(nav_date)) = (snap.latest_nav, snap.latest_nav_date) {
            let _ = update_fund_latest_nav_fields(
                pool,
                code,
                &nav.to_string(),
                &nav_date.to_string(),
            )
            .await;
            did_any = true;
        }

        if source_name == crate::sources::SOURCE_TIANTIAN {
            if let Some(est_nav) = snap.estimate_nav {
                let now = Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, false);
                let growth = snap.estimate_growth.map(|g| g.to_string());
                let _ = update_fund_estimate_fields(
                    pool,
                    code,
                    &est_nav.to_string(),
                    growth.as_deref(),
                    &now,
                )
                .await;
                did_any = true;
            }
        } else {
            // 其他源：与 estimate_sync worker 保持一致，退化为 latest_nav 近似估值，避免额外上游请求。
            if let Some(nav) = snap.latest_nav {
                let now = Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, false);
                let _ = update_fund_estimate_fields(pool, code, &nav.to_string(), None, &now).await;
                did_any = true;
            }
        }

        if did_any {
            ok += 1;
            let gztime = snap.gztime_raw.unwrap_or_else(|| "-".to_string());
            let _ = append_task_log(pool, run_id, "INFO", &format!("[{code}] 刷新完成 gztime={gztime}")).await;
        } else {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 无可写入字段（可能上游返回不完整）")).await;
        }

        // 简单节流：避免批量请求触发上游封锁。
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("prices_refresh_batch 汇总：ok={ok} failed={failed}"),
    )
    .await;

    if ok == 0 {
        return Err("prices_refresh_batch: all fund_codes failed".to_string());
    }
    Ok(())
}

async fn latest_nav_date(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source_name: &str,
) -> Result<Option<String>, String> {
    let row = sqlx::query(
        r#"
        SELECT CAST(h.nav_date AS TEXT) as nav_date
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2
        ORDER BY h.nav_date DESC
        LIMIT 1
        "#,
    )
    .bind(fund_code)
    .bind(source_name)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(row.map(|r| r.get::<String, _>("nav_date")))
}

async fn exec_nav_history_sync_batch(
    pool: &sqlx::AnyPool,
    run_id: &str,
    job: &TaskJobRow,
) -> Result<(), String> {
    use chrono::NaiveDate;
    use sqlx::Row;

    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;
    let codes = payload
        .get("fund_codes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing fund_codes".to_string())?;

    let source = payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(crate::sources::SOURCE_TIANTIAN)
        .trim()
        .to_string();
    let start_date = payload
        .get("start_date")
        .and_then(|v| v.as_str())
        .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());
    let end_date = payload
        .get("end_date")
        .and_then(|v| v.as_str())
        .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());
    let compute_signals = payload
        .get("compute_signals")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let per_job_delay_ms = payload
        .get("per_job_delay_ms")
        .and_then(|v| v.as_i64())
        .unwrap_or(250)
        .clamp(0, 60_000) as u64;
    let per_job_jitter_ms = payload
        .get("per_job_jitter_ms")
        .and_then(|v| v.as_i64())
        .unwrap_or(200)
        .clamp(0, 60_000) as u64;
    let fallbacks_raw = payload
        .get("source_fallbacks")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut fallbacks = parse_source_list(&fallbacks_raw);
    fallbacks.retain(|s| s != &source);

    let tushare_token = payload
        .get("tushare_token")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut fund_codes: Vec<String> = Vec::new();
    for c in codes {
        let s = c.as_str().unwrap_or("").trim();
        if s.is_empty() {
            continue;
        }
        if !fund_codes.contains(&s.to_string()) {
            fund_codes.push(s.to_string());
        }
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!(
            "nav_history_sync_batch: fund_codes={} source={} start={:?} end={:?} compute_signals={}",
            fund_codes.len(),
            source,
            start_date.map(|d| d.to_string()),
            end_date.map(|d| d.to_string()),
            compute_signals
        ),
    )
    .await;

    let client = crate::eastmoney::build_client()?;

    for (idx, fund_code) in fund_codes.iter().enumerate() {
        if idx % 5 == 0 {
            let _ = append_task_log(
                pool,
                run_id,
                "INFO",
                &format!("进度 {idx}/{}", fund_codes.len()),
            )
            .await;
        }

        let _ = append_task_log(pool, run_id, "INFO", &format!("[{fund_code}] 开始同步")).await;

        let mut tried: Vec<String> = Vec::new();
        tried.push(source.clone());
        tried.extend(fallbacks.clone());
        for s in crate::sources::BUILTIN_SOURCES {
            let s = s.to_string();
            if !tried.contains(&s) {
                tried.push(s);
            }
        }

        let mut ok_source: Option<String> = None;
        let mut last_err: Option<String> = None;
        for s in tried {
            let Some(s_norm) = crate::sources::normalize_source_name(&s) else {
                continue;
            };
            let _ = append_task_log(pool, run_id, "INFO", &format!("[{fund_code}] 尝试数据源：{s_norm}")).await;
            match crate::routes::nav_history::sync_one(
                pool,
                &client,
                s_norm,
                fund_code,
                start_date,
                end_date,
                &tushare_token,
            )
            .await
            {
                Ok(count) => {
                    ok_source = Some(s_norm.to_string());
                    last_err = None;
                    let _ = append_task_log(
                        pool,
                        run_id,
                        "INFO",
                        &format!("[{fund_code}] 同步成功 source={s_norm} count={count}"),
                    )
                    .await;
                    break;
                }
                Err(e) => {
                    last_err = Some(e.clone());
                    let _ = append_task_log(
                        pool,
                        run_id,
                        "WARN",
                        &format!("[{fund_code}] 同步失败 source={s_norm} err={e}"),
                    )
                    .await;
                }
            }
        }

        if let Some(e) = last_err {
            return Err(e);
        }

        if compute_signals {
            if let Some(source_used) = ok_source.as_deref() {
                let _ =
                    append_task_log(pool, run_id, "INFO", &format!("[{fund_code}] 计算信号快照（全市场）"))
                        .await;
                let _ = crate::ml::compute::compute_and_store_fund_snapshot_with_opts(
                    pool,
                    fund_code,
                    crate::ml::train::PEER_CODE_ALL,
                    source_used,
                    crate::ml::compute::ComputeOpts {
                        train_if_missing: true,
                    },
                )
                .await;

                let peer_rows = sqlx::query(
                    r#"
                    SELECT sec_code
                    FROM fund_relate_theme
                    WHERE fund_code = $1
                    GROUP BY sec_code
                    ORDER BY sec_code ASC
                    LIMIT 2
                    "#,
                )
                .bind(fund_code)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                for r in peer_rows {
                    let peer_code: String = r.get("sec_code");
                    let _ = append_task_log(
                        pool,
                        run_id,
                        "INFO",
                        &format!("[{fund_code}] 计算信号快照（板块） peer_code={peer_code}"),
                    )
                    .await;
                    let _ = crate::ml::compute::compute_and_store_fund_snapshot_with_opts(
                        pool,
                        fund_code,
                        &peer_code,
                        source_used,
                        crate::ml::compute::ComputeOpts {
                            train_if_missing: false,
                        },
                    )
                    .await;
                }
            }
        }

        if per_job_delay_ms > 0 {
            let jitter = if per_job_jitter_ms == 0 {
                0
            } else {
                let mut h: u64 = 14695981039346656037;
                for b in fund_code.as_bytes() {
                    h ^= *b as u64;
                    h = h.wrapping_mul(1099511628211);
                }
                h % (per_job_jitter_ms + 1)
            };
            tokio::time::sleep(std::time::Duration::from_millis(per_job_delay_ms + jitter)).await;
        }
    }

    Ok(())
}

async fn exec_sniffer_sync(pool: &sqlx::AnyPool, run_id: &str, _job: &TaskJobRow) -> Result<(), String> {
    let _ = append_task_log(pool, run_id, "INFO", "sniffer_sync: start").await;
    let config = crate::config::ConfigStore::load();
    let jwt = crate::jwt::JwtService::from_secret("task-job-secret");
    let db_kind = crate::db::database_kind_from_pool(pool);
    let state = crate::state::AppState::new(Some(pool.clone()), config, jwt, db_kind);

    match crate::sniffer::run_sync_once(state).await {
        Ok(r) => {
            let _ = append_task_log(
                pool,
                run_id,
                "INFO",
                &format!(
                    "sniffer_sync ok: run_id={} snapshot_id={} item_count={} users_updated={}",
                    r.run_id, r.snapshot_id, r.item_count, r.users_updated
                ),
            )
            .await;
            Ok(())
        }
        Err(e) => Err(e),
    }
}

async fn exec_forecast_model_train(pool: &sqlx::AnyPool, run_id: &str, job: &TaskJobRow) -> Result<(), String> {
    use serde_json::Value;
    use sqlx::Row;
    use uuid::Uuid;

    use crate::forecast::ols_sgd::{OlsModel, OlsTrainConfig, train_ols_sgd};

    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;

    let source = payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(crate::sources::SOURCE_TIANTIAN)
        .trim()
        .to_string();
    let model_name = payload
        .get("model_name")
        .and_then(|v| v.as_str())
        .unwrap_or("global_ols_v1")
        .trim()
        .to_string();
    if model_name.is_empty() {
        return Err("missing model_name".to_string());
    }
    let horizon = payload.get("horizon").and_then(|v| v.as_i64()).unwrap_or(60).clamp(1, 5000);
    let lag_k = payload.get("lag_k").and_then(|v| v.as_i64()).unwrap_or(20).clamp(1, 400);

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("forecast_model_train: model_name={model_name} source={source} horizon={horizon} lag_k={lag_k}"),
    )
    .await;

    let _ = append_task_log(pool, run_id, "INFO", "训练全市场预测模型：开始").await;

    let fund_rows = sqlx::query("SELECT fund_code FROM fund ORDER BY fund_code ASC")
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

    let max_samples: usize = 50_000;
    let mut x: Vec<Vec<f64>> = Vec::new();
    let mut y: Vec<f64> = Vec::new();

    for (idx, r) in fund_rows.iter().enumerate() {
        if idx % 200 == 0 {
            let _ = append_task_log(
                pool,
                run_id,
                "INFO",
                &format!("训练数据扫描进度 {idx}/{}", fund_rows.len()),
            )
            .await;
        }

        let code: String = r.get("fund_code");
        let nav_rows = sqlx::query(
            r#"
            SELECT CAST(h.nav_date AS TEXT) as nav_date, CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1 AND h.source_name = $2
            ORDER BY h.nav_date DESC
            LIMIT 400
            "#,
        )
        .bind(code.trim())
        .bind(&source)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        if nav_rows.len() < (lag_k as usize + 3) {
            continue;
        }

        let mut navs: Vec<f64> = Vec::with_capacity(nav_rows.len());
        for rr in nav_rows.into_iter().rev() {
            let v: String = rr.get("unit_nav");
            if let Ok(f) = v.trim().parse::<f64>() {
                if f > 0.0 {
                    navs.push(f);
                }
            }
        }
        if navs.len() < (lag_k as usize + 3) {
            continue;
        }

        let mut rets: Vec<f64> = Vec::with_capacity(navs.len().saturating_sub(1));
        for (a, b) in navs.iter().zip(navs.iter().skip(1)) {
            if *a > 0.0 && *b > 0.0 {
                rets.push((b / a).ln());
            }
        }
        if rets.len() < (lag_k as usize + 2) {
            continue;
        }

        for i in (lag_k as usize)..rets.len() {
            if x.len() >= max_samples {
                break;
            }
            let mut feat: Vec<f64> = Vec::with_capacity(lag_k as usize);
            let start = i - lag_k as usize;
            feat.extend_from_slice(&rets[start..i]);
            x.push(feat);
            y.push(rets[i]);
        }
        if x.len() >= max_samples {
            break;
        }
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("训练样本构建完成：samples={} dim={} horizon={horizon}", x.len(), lag_k),
    )
    .await;

    let (model, sample_count) = if x.len() < 10 {
        let _ = append_task_log(pool, run_id, "WARN", "训练样本过少，使用退化预测模型（mu=0）").await;
        (
            OlsModel {
                weights: vec![0.0; lag_k as usize],
                bias: 0.0,
                mean: vec![0.0; lag_k as usize],
                std: vec![1.0; lag_k as usize],
                residual_sigma: 0.0,
            },
            x.len() as i64,
        )
    } else {
        let cfg = OlsTrainConfig {
            learning_rate: 0.01,
            epochs: 3,
            l2: 1e-4,
        };
        let m = train_ols_sgd(&x, &y, &cfg).ok_or_else(|| "模型训练失败（样本不足或数据异常）".to_string())?;
        let _ = append_task_log(pool, run_id, "INFO", &format!("训练完成：residual_sigma={:.6}", m.residual_sigma)).await;
        (m, x.len() as i64)
    };

    let id = Uuid::new_v4().to_string();
    let weights_json = serde_json::to_string(&model.weights).map_err(|e| e.to_string())?;
    let mean_json = serde_json::to_string(&model.mean).map_err(|e| e.to_string())?;
    let std_json = serde_json::to_string(&model.std).map_err(|e| e.to_string())?;

    let sql_pg = r#"
        INSERT INTO forecast_model (
          id, model_name, source, horizon, lag_k, weights_json, bias, mean_json, std_json, residual_sigma,
          sample_count, trained_at, created_at, updated_at
        )
        VALUES (($1)::uuid,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT (model_name, source, horizon, lag_k) DO UPDATE SET
          weights_json = excluded.weights_json,
          bias = excluded.bias,
          mean_json = excluded.mean_json,
          std_json = excluded.std_json,
          residual_sigma = excluded.residual_sigma,
          sample_count = excluded.sample_count,
          trained_at = excluded.trained_at,
          updated_at = CURRENT_TIMESTAMP
    "#;
    let sql_any = r#"
        INSERT INTO forecast_model (
          id, model_name, source, horizon, lag_k, weights_json, bias, mean_json, std_json, residual_sigma,
          sample_count, trained_at, created_at, updated_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT (model_name, source, horizon, lag_k) DO UPDATE SET
          weights_json = excluded.weights_json,
          bias = excluded.bias,
          mean_json = excluded.mean_json,
          std_json = excluded.std_json,
          residual_sigma = excluded.residual_sigma,
          sample_count = excluded.sample_count,
          trained_at = excluded.trained_at,
          updated_at = CURRENT_TIMESTAMP
    "#;

    if sqlx::query(sql_pg)
        .bind(&id)
        .bind(&model_name)
        .bind(&source)
        .bind(horizon)
        .bind(lag_k)
        .bind(&weights_json)
        .bind(model.bias)
        .bind(&mean_json)
        .bind(&std_json)
        .bind(model.residual_sigma)
        .bind(sample_count)
        .execute(pool)
        .await
        .is_err()
    {
        sqlx::query(sql_any)
            .bind(&id)
            .bind(&model_name)
            .bind(&source)
            .bind(horizon)
            .bind(lag_k)
            .bind(&weights_json)
            .bind(model.bias)
            .bind(&mean_json)
            .bind(&std_json)
            .bind(model.residual_sigma)
            .bind(sample_count)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("写入 forecast_model 完成：samples={sample_count} dim={lag_k}"),
    )
    .await;

    Ok(())
}

async fn exec_fund_analysis_v2_compute(
    pool: &sqlx::AnyPool,
    run_id: &str,
    job: &TaskJobRow,
) -> Result<(), String> {
    use serde_json::{Value, json};
    use sqlx::Row;
    use uuid::Uuid;

    use crate::forecast::ols_sgd::{OlsModel, OlsTrainConfig, train_ols_sgd};

    const MODEL_NAME: &str = "global_ols_v1";
    const FORECAST_HORIZON: i64 = 60;
    const LAG_K: i64 = 20;

    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;
    let fund_code = payload
        .get("fund_code")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if fund_code.is_empty() {
        return Err("missing fund_code".to_string());
    }

    let source = payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(crate::sources::SOURCE_TIANTIAN)
        .trim()
        .to_string();
    let profile = payload
        .get("profile")
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .trim()
        .to_string();

    let mut windows: Vec<i64> = payload
        .get("windows")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|x| x.as_i64()).collect::<Vec<_>>())
        .unwrap_or_else(|| vec![60]);
    windows.retain(|w| *w >= 2 && *w <= 5000);
    if windows.is_empty() {
        windows = vec![60];
    }
    windows.truncate(8);

    let risk_free_annual = payload
        .get("risk_free_annual")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let grid_step_pct = payload
        .get("grid_step_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.02);
    let every_n = payload
        .get("every_n")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(1, 10_000);
    let amount = payload
        .get("amount")
        .and_then(|v| v.as_f64())
        .unwrap_or(1.0);

    let quant_service_url = payload
        .get("quant_service_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8002")
        .trim_end_matches('/')
        .to_string();

    let refer_index_code = payload
        .get("refer_index_code")
        .and_then(|v| v.as_str())
        .unwrap_or("1.000001")
        .trim()
        .to_string();
    let refer_index_source = "eastmoney".to_string();

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!(
            "fund_analysis_v2_compute: fund_code={} source={} profile={} windows={}",
            fund_code,
            source,
            profile,
            windows
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
    )
    .await;

    if !refer_index_code.is_empty() {
        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("fund_analysis_v2_compute: refer_index_code={} (source={})", refer_index_code, refer_index_source),
        )
        .await;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let url_metrics = format!("{quant_service_url}/api/quant/xalpha/metrics");
    let url_macd = format!("{quant_service_url}/api/quant/macd");
    let url_ts = format!("{quant_service_url}/api/quant/fund-strategies/ts");
    let url_grid = format!("{quant_service_url}/api/quant/xalpha/grid");
    let url_scheduled = format!("{quant_service_url}/api/quant/xalpha/scheduled");

    async fn load_forecast_model(
        pool: &sqlx::AnyPool,
        model_name: &str,
        source: &str,
        horizon: i64,
        lag_k: i64,
    ) -> Result<Option<(OlsModel, String)>, String> {
        let row = sqlx::query(
            r#"
            SELECT
              weights_json,
              bias,
              mean_json,
              std_json,
              residual_sigma,
              CAST(trained_at AS TEXT) as trained_at
            FROM forecast_model
            WHERE model_name = $1 AND source = $2 AND horizon = $3 AND lag_k = $4
            ORDER BY trained_at DESC
            LIMIT 1
            "#,
        )
        .bind(model_name)
        .bind(source)
        .bind(horizon)
        .bind(lag_k)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        let Some(row) = row else { return Ok(None); };

        let weights_json: String = row.get("weights_json");
        let mean_json: String = row.get("mean_json");
        let std_json: String = row.get("std_json");
        let weights: Vec<f64> = serde_json::from_str(&weights_json).map_err(|e| e.to_string())?;
        let mean: Vec<f64> = serde_json::from_str(&mean_json).map_err(|e| e.to_string())?;
        let std: Vec<f64> = serde_json::from_str(&std_json).map_err(|e| e.to_string())?;

        Ok(Some((
            OlsModel {
                weights,
                bias: row.get::<f64, _>("bias"),
                mean,
                std,
                residual_sigma: row.get::<f64, _>("residual_sigma"),
            },
            row.get::<String, _>("trained_at"),
        )))
    }

    async fn upsert_forecast_model(
        pool: &sqlx::AnyPool,
        model_name: &str,
        source: &str,
        horizon: i64,
        lag_k: i64,
        model: &OlsModel,
        sample_count: i64,
    ) -> Result<(), String> {
        let id = Uuid::new_v4().to_string();
        let weights_json = serde_json::to_string(&model.weights).map_err(|e| e.to_string())?;
        let mean_json = serde_json::to_string(&model.mean).map_err(|e| e.to_string())?;
        let std_json = serde_json::to_string(&model.std).map_err(|e| e.to_string())?;

        let sql_pg = r#"
            INSERT INTO forecast_model (
              id, model_name, source, horizon, lag_k, weights_json, bias, mean_json, std_json, residual_sigma,
              sample_count, trained_at, created_at, updated_at
            )
            VALUES (($1)::uuid,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            ON CONFLICT (model_name, source, horizon, lag_k) DO UPDATE SET
              weights_json = excluded.weights_json,
              bias = excluded.bias,
              mean_json = excluded.mean_json,
              std_json = excluded.std_json,
              residual_sigma = excluded.residual_sigma,
              sample_count = excluded.sample_count,
              trained_at = excluded.trained_at,
              updated_at = CURRENT_TIMESTAMP
        "#;
        let sql_any = r#"
            INSERT INTO forecast_model (
              id, model_name, source, horizon, lag_k, weights_json, bias, mean_json, std_json, residual_sigma,
              sample_count, trained_at, created_at, updated_at
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
            ON CONFLICT (model_name, source, horizon, lag_k) DO UPDATE SET
              weights_json = excluded.weights_json,
              bias = excluded.bias,
              mean_json = excluded.mean_json,
              std_json = excluded.std_json,
              residual_sigma = excluded.residual_sigma,
              sample_count = excluded.sample_count,
              trained_at = excluded.trained_at,
              updated_at = CURRENT_TIMESTAMP
        "#;

        if sqlx::query(sql_pg)
            .bind(&id)
            .bind(model_name)
            .bind(source)
            .bind(horizon)
            .bind(lag_k)
            .bind(&weights_json)
            .bind(model.bias)
            .bind(&mean_json)
            .bind(&std_json)
            .bind(model.residual_sigma)
            .bind(sample_count)
            .execute(pool)
            .await
            .is_err()
        {
            sqlx::query(sql_any)
                .bind(&id)
                .bind(model_name)
                .bind(source)
                .bind(horizon)
                .bind(lag_k)
                .bind(&weights_json)
                .bind(model.bias)
                .bind(&mean_json)
                .bind(&std_json)
                .bind(model.residual_sigma)
                .bind(sample_count)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    async fn train_global_model(
        pool: &sqlx::AnyPool,
        run_id: &str,
        source: &str,
        horizon: i64,
        lag_k: i64,
    ) -> Result<(OlsModel, i64), String> {
        let _ = append_task_log(pool, run_id, "INFO", "训练全市场预测模型：开始").await;

        let fund_rows = sqlx::query("SELECT fund_code FROM fund ORDER BY fund_code ASC")
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

        let max_samples: usize = 50_000;
        let mut x: Vec<Vec<f64>> = Vec::new();
        let mut y: Vec<f64> = Vec::new();

        for (idx, r) in fund_rows.iter().enumerate() {
            if idx % 200 == 0 {
                let _ = append_task_log(
                    pool,
                    run_id,
                    "INFO",
                    &format!("训练数据扫描进度 {idx}/{}", fund_rows.len()),
                )
                .await;
            }

            let code: String = r.get("fund_code");
            let nav_rows = sqlx::query(
                r#"
                SELECT CAST(h.nav_date AS TEXT) as nav_date, CAST(h.unit_nav AS TEXT) as unit_nav
                FROM fund_nav_history h
                JOIN fund f ON f.id = h.fund_id
                WHERE f.fund_code = $1 AND h.source_name = $2
                ORDER BY h.nav_date DESC
                LIMIT 400
                "#,
            )
            .bind(code.trim())
            .bind(source)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

            if nav_rows.len() < (lag_k as usize + 3) {
                continue;
            }

            let mut navs: Vec<f64> = Vec::with_capacity(nav_rows.len());
            for rr in nav_rows.into_iter().rev() {
                let v: String = rr.get("unit_nav");
                if let Ok(f) = v.trim().parse::<f64>() {
                    if f > 0.0 {
                        navs.push(f);
                    }
                }
            }
            if navs.len() < (lag_k as usize + 3) {
                continue;
            }

            let mut rets: Vec<f64> = Vec::with_capacity(navs.len().saturating_sub(1));
            for (a, b) in navs.iter().zip(navs.iter().skip(1)) {
                if *a > 0.0 && *b > 0.0 {
                    rets.push((b / a).ln());
                }
            }
            if rets.len() < (lag_k as usize + 2) {
                continue;
            }

            for i in (lag_k as usize)..rets.len() {
                if x.len() >= max_samples {
                    break;
                }
                let mut feat: Vec<f64> = Vec::with_capacity(lag_k as usize);
                let start = i - lag_k as usize;
                feat.extend_from_slice(&rets[start..i]);
                x.push(feat);
                y.push(rets[i]);
            }
            if x.len() >= max_samples {
                break;
            }
        }

        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("训练样本构建完成：samples={} dim={} horizon={horizon}", x.len(), lag_k),
        )
        .await;

        if x.len() < 10 {
            let _ = append_task_log(
                pool,
                run_id,
                "WARN",
                "训练样本过少，使用退化预测模型（mu=0）",
            )
            .await;
            return Ok((
                OlsModel {
                    weights: vec![0.0; lag_k as usize],
                    bias: 0.0,
                    mean: vec![0.0; lag_k as usize],
                    std: vec![1.0; lag_k as usize],
                    residual_sigma: 0.0,
                },
                x.len() as i64,
            ));
        }

        let cfg = OlsTrainConfig {
            learning_rate: 0.01,
            epochs: 3,
            l2: 1e-4,
        };
        let model = train_ols_sgd(&x, &y, &cfg).ok_or_else(|| "模型训练失败（样本不足或数据异常）".to_string())?;

        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("训练完成：residual_sigma={:.6}", model.residual_sigma),
        )
        .await;

        Ok((model, x.len() as i64))
    }

    fn trained_date_prefix(trained_at: &str) -> &str {
        trained_at.get(0..10).unwrap_or("")
    }

    let today = chrono::Utc::now().date_naive().format("%Y-%m-%d").to_string();

    let model = match load_forecast_model(pool, MODEL_NAME, &source, FORECAST_HORIZON, LAG_K).await? {
        Some((m, trained_at)) => {
            if trained_date_prefix(&trained_at) == today {
                m
            } else {
                let _ = append_task_log(
                    pool,
                    run_id,
                    "INFO",
                    &format!(
                        "预测模型过期：trained_at={} today={}，触发重训",
                        trained_date_prefix(&trained_at),
                        today
                    ),
                )
                .await;
                let (m2, sample_count) =
                    train_global_model(pool, run_id, &source, FORECAST_HORIZON, LAG_K).await?;
                let _ = upsert_forecast_model(pool, MODEL_NAME, &source, FORECAST_HORIZON, LAG_K, &m2, sample_count).await;
                m2
            }
        }
        None => {
            let _ = append_task_log(pool, run_id, "INFO", "预测模型缺失，触发训练").await;
            let (m, sample_count) = train_global_model(pool, run_id, &source, FORECAST_HORIZON, LAG_K).await?;
            let _ = upsert_forecast_model(pool, MODEL_NAME, &source, FORECAST_HORIZON, LAG_K, &m, sample_count).await;
            m
        }
    };

    let mut windows_out: Vec<Value> = Vec::with_capacity(windows.len());
    let mut as_of_date_overall: Option<String> = None;

    for window in windows {
        let rows = sqlx::query(
            r#"
            SELECT
              CAST(h.nav_date AS TEXT) as nav_date,
              CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1 AND h.source_name = $2
            ORDER BY h.nav_date DESC
            LIMIT $3
            "#,
        )
        .bind(fund_code.trim())
        .bind(&source)
        .bind(window)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        if rows.len() < 2 {
            let _ = append_task_log(
                pool,
                run_id,
                "WARN",
                &format!("[{fund_code}] window={window} 跳过：净值点数不足"),
            )
            .await;
            continue;
        }

        let mut seed_navs: Vec<(String, f64)> = Vec::with_capacity(rows.len());
        let mut as_of_date: Option<String> = None;
        for r in rows.into_iter().rev() {
            let d: String = r.get("nav_date");
            let v: String = r.get("unit_nav");
            if let Ok(val) = v.trim().parse::<f64>() {
                as_of_date = Some(d.clone());
                seed_navs.push((d, val));
            }
        }
        if seed_navs.len() < 2 {
            let _ = append_task_log(
                pool,
                run_id,
                "WARN",
                &format!("[{fund_code}] window={window} 跳过：净值解析失败"),
            )
            .await;
            continue;
        }
        if as_of_date_overall.is_none() {
            as_of_date_overall = as_of_date.clone();
        }

        let seed_points = seed_navs.len();
        let base_nav = seed_navs.last().map(|x| x.1).unwrap_or(0.0);
        if base_nav <= 0.0 {
            let _ = append_task_log(
                pool,
                run_id,
                "WARN",
                &format!("[{fund_code}] window={window} 跳过：base_nav<=0"),
            )
            .await;
            continue;
        }

        // build last LAG_K log-returns from seed navs
        let mut rets: Vec<f64> = Vec::with_capacity(seed_navs.len().saturating_sub(1));
        for ((_, a), (_, b)) in seed_navs.iter().zip(seed_navs.iter().skip(1)) {
            if *a > 0.0 && *b > 0.0 {
                rets.push((b / a).ln());
            }
        }
        let mut hist: Vec<f64> = Vec::new();
        let k = LAG_K as usize;
        if rets.len() >= k {
            hist.extend_from_slice(&rets[rets.len() - k..]);
        } else {
            hist.extend(std::iter::repeat(0.0).take(k - rets.len()));
            hist.extend_from_slice(&rets);
        }

        // forecast horizon=60
        let mut forecast_points: Vec<Value> = Vec::with_capacity(FORECAST_HORIZON as usize);
        let mut cum_mu = 0.0_f64;
        let mut nav_min = f64::INFINITY;
        let mut nav_max = f64::NEG_INFINITY;
        let mut nav_series_mean: Vec<f64> = Vec::with_capacity(FORECAST_HORIZON as usize);

        let sigma = model.residual_sigma.max(0.0).min(0.2);
        let z = 1.96_f64;

        for step in 1..=FORECAST_HORIZON {
            let mu = model
                .predict(&hist)
                .unwrap_or(0.0)
                .max(-0.15)
                .min(0.15);

            cum_mu += mu;
            let nav_mean = base_nav * cum_mu.exp();
            let step_f = step as f64;
            let band = z * sigma * step_f.sqrt();
            let nav_low = base_nav * (cum_mu - band).exp();
            let nav_high = base_nav * (cum_mu + band).exp();

            nav_min = nav_min.min(nav_mean);
            nav_max = nav_max.max(nav_mean);
            nav_series_mean.push(nav_mean);

            forecast_points.push(json!({
              "step": step,
              "nav": nav_mean,
              "ci_low": nav_low,
              "ci_high": nav_high,
              "mu": mu
            }));

            hist.push(mu);
            if hist.len() > k {
                hist.remove(0);
            }
        }

        let mut low_idx = 0_usize;
        let mut high_idx = 0_usize;
        for (i, &v) in nav_series_mean.iter().enumerate() {
            if v <= nav_series_mean[low_idx] {
                low_idx = i;
            }
            if v >= nav_series_mean[high_idx] {
                high_idx = i;
            }
        }

        // simple swing points: local extrema
        let mut swings: Vec<Value> = Vec::new();
        for i in 1..nav_series_mean.len().saturating_sub(1) {
            let a = nav_series_mean[i - 1];
            let b = nav_series_mean[i];
            let c = nav_series_mean[i + 1];
            if (b >= a && b > c) || (b > a && b >= c) {
                swings.push(json!({ "kind": "high", "step": (i as i64) + 1, "nav": b }));
            } else if (b <= a && b < c) || (b < a && b <= c) {
                swings.push(json!({ "kind": "low", "step": (i as i64) + 1, "nav": b }));
            }
        }
        swings.truncate(12);

        let forecast = json!({
          "horizon": FORECAST_HORIZON,
          "base_nav": base_nav,
          "points": forecast_points,
          "low": { "step": (low_idx as i64) + 1, "nav": nav_series_mean[low_idx] },
          "high": { "step": (high_idx as i64) + 1, "nav": nav_series_mean[high_idx] },
          "nav_min": nav_min,
          "nav_max": nav_max,
          "swing_points": swings,
          "model": { "name": MODEL_NAME, "lag_k": LAG_K, "sigma": sigma }
        });

        // quant-service uses forecast curve, not historical curve
        let series_json = Value::Array(
            nav_series_mean
                .iter()
                .enumerate()
                .map(|(i, &v)| json!({ "index": i, "date": format!("f+{}", i + 1), "val": v }))
                .collect::<Vec<_>>(),
        );

        // macd timing uses reference index series (Qbot/fund-strategies), if available.
        let macd_series_json = {
            let end_date_str = as_of_date.clone().unwrap_or_default();
            let end_date = chrono::NaiveDate::parse_from_str(&end_date_str, "%Y-%m-%d").ok();
            let mut series: Vec<Value> = Vec::new();

            if let Some(end_date) = end_date {
                let start_date = end_date - chrono::Duration::days(450);
                let rows = sqlx::query(
                    r#"
                    SELECT
                      CAST(trade_date AS TEXT) as trade_date,
                      CAST(close AS TEXT) as close
                    FROM index_daily_price
                    WHERE index_code = $1 AND source_name = $2
                      AND CAST(trade_date AS TEXT) >= $3 AND CAST(trade_date AS TEXT) <= $4
                    ORDER BY trade_date ASC
                    "#,
                )
                .bind(refer_index_code.trim())
                .bind(refer_index_source.as_str())
                .bind(start_date.format("%Y-%m-%d").to_string())
                .bind(end_date.format("%Y-%m-%d").to_string())
                .fetch_all(pool)
                .await;

                if let Ok(rows) = rows {
                    for (i, r) in rows.iter().enumerate() {
                        let d: String = r.get("trade_date");
                        let c: String = r.get("close");
                        if let Ok(v) = c.trim().parse::<f64>() {
                            series.push(json!({ "index": i, "date": d, "val": v }));
                        }
                    }
                }

                if series.len() < 20 {
                    // Try fetch from Eastmoney then reload (best-effort, fallback to forecast series if still insufficient).
                    if let Ok(client2) = crate::eastmoney::build_client() {
                        let fetched = crate::eastmoney::fetch_index_kline_daily(
                            &client2,
                            refer_index_code.trim(),
                            start_date,
                            end_date,
                        )
                        .await;
                        if let Ok(list) = fetched {
                            let sql_pg = r#"
                                INSERT INTO index_daily_price (
                                  id, index_code, source_name, trade_date, close, created_at, updated_at
                                )
                                VALUES (($1)::uuid,$2,$3,($4)::date,($5)::numeric,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                                ON CONFLICT (index_code, source_name, trade_date) DO UPDATE SET
                                  close = excluded.close,
                                  updated_at = CURRENT_TIMESTAMP
                            "#;
                            let sql_any = r#"
                                INSERT INTO index_daily_price (
                                  id, index_code, source_name, trade_date, close, created_at, updated_at
                                )
                                VALUES ($1,$2,$3,$4,$5,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
                                ON CONFLICT (index_code, source_name, trade_date) DO UPDATE SET
                                  close = excluded.close,
                                  updated_at = CURRENT_TIMESTAMP
                            "#;

                            for it in list {
                                let id = Uuid::new_v4().to_string();
                                let trade_date = it.trade_date.format("%Y-%m-%d").to_string();
                                let close = it.close.to_string();
                                let r = sqlx::query(sql_pg)
                                    .bind(&id)
                                    .bind(refer_index_code.trim())
                                    .bind(refer_index_source.as_str())
                                    .bind(&trade_date)
                                    .bind(&close)
                                    .execute(pool)
                                    .await;
                                if r.is_err() {
                                    let _ = sqlx::query(sql_any)
                                        .bind(&id)
                                        .bind(refer_index_code.trim())
                                        .bind(refer_index_source.as_str())
                                        .bind(&trade_date)
                                        .bind(&close)
                                        .execute(pool)
                                        .await;
                                }
                            }

                            // reload
                            series.clear();
                            if let Ok(rows2) = sqlx::query(
                                r#"
                                SELECT
                                  CAST(trade_date AS TEXT) as trade_date,
                                  CAST(close AS TEXT) as close
                                FROM index_daily_price
                                WHERE index_code = $1 AND source_name = $2
                                  AND CAST(trade_date AS TEXT) >= $3 AND CAST(trade_date AS TEXT) <= $4
                                ORDER BY trade_date ASC
                                "#,
                            )
                            .bind(refer_index_code.trim())
                            .bind(refer_index_source.as_str())
                            .bind(start_date.format("%Y-%m-%d").to_string())
                            .bind(end_date.format("%Y-%m-%d").to_string())
                            .fetch_all(pool)
                            .await
                            {
                                for (i, r) in rows2.iter().enumerate() {
                                    let d: String = r.get("trade_date");
                                    let c: String = r.get("close");
                                    if let Ok(v) = c.trim().parse::<f64>() {
                                        series.push(json!({ "index": i, "date": d, "val": v }));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if series.len() >= 3 { Value::Array(series) } else { series_json.clone() }
        };

        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!(
                "[{fund_code}] window={window} 计算：seed_points={} as_of_date={}",
                seed_points,
                as_of_date.clone().unwrap_or_default()
            ),
        )
        .await;

        let metrics_resp = client
            .post(&url_metrics)
            .json(&json!({
              "series": series_json.clone(),
              "risk_free_annual": risk_free_annual
            }))
            .send()
            .await
            .map_err(|e| format!("metrics request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("metrics http error: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("metrics json failed: {e}"))?;

        let macd_resp = client
            .post(&url_macd)
            .json(&json!({
              "series": macd_series_json.clone(),
              "sell_position": 0.75,
              "buy_position": 0.5
            }))
            .send()
            .await
            .map_err(|e| format!("macd request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("macd http error: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("macd json failed: {e}"))?;

        let ts_resp = {
            let start_date_str = seed_navs.first().map(|x| x.0.clone()).unwrap_or_default();
            let end_date_str = as_of_date.clone().unwrap_or_default();
            let start_date = chrono::NaiveDate::parse_from_str(&start_date_str, "%Y-%m-%d").ok();
            let end_date = chrono::NaiveDate::parse_from_str(&end_date_str, "%Y-%m-%d").ok();

            let shangzheng_series_json = if let (Some(sd), Some(ed)) = (start_date, end_date) {
                let list = crate::index_series::load_or_fetch_index_close_series(
                    pool,
                    &client,
                    crate::db::database_kind_from_pool(pool),
                    "1.000001",
                    refer_index_source.as_str(),
                    sd,
                    ed,
                    3,
                )
                .await
                .unwrap_or_default();
                Value::Array(
                    list.iter()
                        .enumerate()
                        .map(|(i, (d, v))| {
                            let val = v.to_string().parse::<f64>().unwrap_or(0.0);
                            json!({ "index": i, "date": d.format("%Y-%m-%d").to_string(), "val": val })
                        })
                        .collect::<Vec<_>>(),
                )
            } else {
                Value::Array(vec![])
            };

            let fund_series_json = Value::Array(
                seed_navs
                    .iter()
                    .map(|(d, v)| json!({ "date": d, "val": v }))
                    .collect::<Vec<_>>(),
            );

            let refer_points = macd_resp.get("points").cloned().unwrap_or(Value::Array(vec![]));

            let resp = client
                .post(&url_ts)
                .json(&json!({
                  "fund_series": fund_series_json,
                  "shangzheng_series": shangzheng_series_json,
                  "refer_index_points": refer_points,
                  "cfg": {
                    "total_amount": 10000.0,
                    "salary": 10000.0,
                    "purchased_fund_amount": 0.0,
                    "fixed_amount": 1000.0,
                    "period": ["monthly", 1],
                    "sh_composite_index": 3000.0,
                    "fund_position": 70.0,
                    "sell_at_top": true,
                    "sell_num": 10.0,
                    "sell_unit": "fundPercent",
                    "profit_rate": 5.0,
                    "sell_macd_point": 75.0,
                    "buy_macd_point": 50.0,
                    "buy_amount_percent": 20.0
                  }
                }))
                .send()
                .await;

            match resp {
                Ok(r) => match r.error_for_status() {
                    Ok(r2) => r2
                        .json::<Value>()
                        .await
                        .unwrap_or_else(|e| json!({ "error": format!("{e}") })),
                    Err(e) => json!({ "error": format!("{e}") }),
                },
                Err(e) => json!({ "error": format!("{e}") }),
            }
        };

        let grid_resp = client
            .post(&url_grid)
            .json(&json!({
              "series": series_json.clone(),
              "grid_step_pct": grid_step_pct
            }))
            .send()
            .await
            .map_err(|e| format!("grid request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("grid http error: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("grid json failed: {e}"))?;

        let scheduled_resp = client
            .post(&url_scheduled)
            .json(&json!({
              "series": series_json.clone(),
              "every_n": every_n,
              "amount": amount
            }))
            .send()
            .await
            .map_err(|e| format!("scheduled request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("scheduled http error: {e}"))?
            .json::<Value>()
            .await
            .map_err(|e| format!("scheduled json failed: {e}"))?;

        windows_out.push(json!({
          "window": window,
          "as_of_date": as_of_date,
          "seed_points": seed_points,
          "forecast": forecast,
          "metrics": metrics_resp,
          "macd": macd_resp,
          "fund_strategies_ts": ts_resp,
          "grid": grid_resp,
          "scheduled": scheduled_resp
        }));
    }

    if windows_out.is_empty() {
        return Err("no eligible windows computed".to_string());
    }

    let result = json!({
      "fund_code": fund_code,
      "source": source,
      "profile": profile,
      "refer_index_code": refer_index_code,
      "as_of_date": as_of_date_overall,
      "windows": windows_out
    });

    let snapshot_id = Uuid::new_v4().to_string();
    let result_json = result.to_string();

    let sql_pg = r#"
        INSERT INTO fund_analysis_snapshot (
          id, fund_code, source, profile, refer_index_code, as_of_date, result_json, last_task_id, created_at, updated_at
        )
        VALUES (($1)::uuid,$2,$3,$4,$5,$6,$7,($8)::uuid,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT (fund_code, source, profile, refer_index_code) DO UPDATE SET
          as_of_date = excluded.as_of_date,
          result_json = excluded.result_json,
          last_task_id = excluded.last_task_id,
          updated_at = CURRENT_TIMESTAMP
    "#;
    let sql_any = r#"
        INSERT INTO fund_analysis_snapshot (
          id, fund_code, source, profile, refer_index_code, as_of_date, result_json, last_task_id, created_at, updated_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,$7,$8,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT (fund_code, source, profile, refer_index_code) DO UPDATE SET
          as_of_date = excluded.as_of_date,
          result_json = excluded.result_json,
          last_task_id = excluded.last_task_id,
          updated_at = CURRENT_TIMESTAMP
    "#;

    if sqlx::query(sql_pg)
        .bind(&snapshot_id)
        .bind(fund_code.trim())
        .bind(result["source"].as_str().unwrap_or(crate::sources::SOURCE_TIANTIAN))
        .bind(result["profile"].as_str().unwrap_or("default"))
        .bind(result["refer_index_code"].as_str().unwrap_or("1.000001"))
        .bind(result["as_of_date"].as_str())
        .bind(&result_json)
        .bind(job.id.as_str())
        .execute(pool)
        .await
        .is_err()
    {
        sqlx::query(sql_any)
            .bind(&snapshot_id)
            .bind(fund_code.trim())
            .bind(result["source"].as_str().unwrap_or(crate::sources::SOURCE_TIANTIAN))
            .bind(result["profile"].as_str().unwrap_or("default"))
            .bind(result["refer_index_code"].as_str().unwrap_or("1.000001"))
            .bind(result["as_of_date"].as_str())
            .bind(&result_json)
            .bind(job.id.as_str())
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
    }

    let _ = append_task_log(pool, run_id, "INFO", &format!("[{fund_code}] 快照写入完成")).await;
    Ok(())
}

async fn exec_signals_batch(pool: &sqlx::AnyPool, run_id: &str, job: &TaskJobRow) -> Result<(), String> {
    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;
    let codes = payload
        .get("fund_codes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing fund_codes".to_string())?;
    let source = payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("tiantian")
        .trim()
        .to_string();

    let mut fund_codes: Vec<String> = Vec::new();
    for c in codes {
        let s = c.as_str().unwrap_or("").trim();
        if s.is_empty() {
            continue;
        }
        if !fund_codes.contains(&s.to_string()) {
            fund_codes.push(s.to_string());
        }
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("signals_batch: fund_codes={} source={}", fund_codes.len(), source),
    )
    .await;

    // 清理旧结果（允许同一个 task 重跑时覆盖）
    let _ = sqlx::query("DELETE FROM fund_signals_batch_item WHERE CAST(task_id AS TEXT) = $1")
        .bind(job.id.as_str())
        .execute(pool)
        .await;

    for (idx, code) in fund_codes.iter().enumerate() {
        if idx % 10 == 0 {
            let _ = append_task_log(pool, run_id, "INFO", &format!("进度 {idx}/{}", fund_codes.len())).await;
        }

        let Some(as_of_date) = latest_nav_date(pool, code, &source).await? else {
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：无净值数据")).await;
            continue;
        };

        let _ = append_task_log(pool, run_id, "INFO", &format!("[{code}] 计算信号 as_of_date={as_of_date}")).await;

        // 确保全市场快照可用（模型缺失时会训练一次）；异步任务允许更重的计算。
        let _ = crate::ml::compute::compute_and_store_fund_snapshot_with_opts(
            pool,
            code,
            crate::ml::train::PEER_CODE_ALL,
            &source,
            crate::ml::compute::ComputeOpts { train_if_missing: true },
        )
        .await;

        // 读取快照
        let snap = sqlx::query(
            r#"
            SELECT
              position_percentile_0_100,
              position_bucket,
              dip_buy_proba_5t,
              dip_buy_proba_20t,
              magic_rebound_proba_5t,
              magic_rebound_proba_20t,
              CAST(computed_at AS TEXT) as computed_at
            FROM fund_signal_snapshot
            WHERE fund_code = $1 AND peer_code = $2 AND CAST(as_of_date AS TEXT) = $3
            "#,
        )
        .bind(code)
        .bind(crate::ml::train::PEER_CODE_ALL)
        .bind(&as_of_date)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        let Some(snap) = snap else {
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：无快照")).await;
            continue;
        };

        let best_peer = serde_json::json!({
          "peer_code": crate::ml::train::PEER_CODE_ALL,
          "peer_name": "全市场",
          "position_percentile_0_100": snap.try_get::<Option<f64>, _>("position_percentile_0_100").ok().flatten(),
          "position_bucket": snap.try_get::<Option<String>, _>("position_bucket").ok().flatten(),
          "dip_buy": {
            "p_5t": snap.try_get::<Option<f64>, _>("dip_buy_proba_5t").ok().flatten(),
            "p_20t": snap.try_get::<Option<f64>, _>("dip_buy_proba_20t").ok().flatten()
          },
          "magic_rebound": {
            "p_5t": snap.try_get::<Option<f64>, _>("magic_rebound_proba_5t").ok().flatten(),
            "p_20t": snap.try_get::<Option<f64>, _>("magic_rebound_proba_20t").ok().flatten()
          },
          "model_sample_size_20t": serde_json::Value::Null,
          "computed_at": snap.try_get::<Option<String>, _>("computed_at").ok().flatten()
        });

        let best_peer_json = best_peer.to_string();

        let sql_pg = r#"
            INSERT INTO fund_signals_batch_item (task_id, fund_code, source, as_of_date, best_peer_json, computed_at)
            VALUES (($1)::uuid,$2,$3,$4,($5)::jsonb,CURRENT_TIMESTAMP)
            ON CONFLICT (task_id, fund_code) DO UPDATE SET
              as_of_date = excluded.as_of_date,
              best_peer_json = excluded.best_peer_json,
              computed_at = CURRENT_TIMESTAMP
        "#;
        let sql_any = r#"
            INSERT INTO fund_signals_batch_item (task_id, fund_code, source, as_of_date, best_peer_json, computed_at)
            VALUES ($1,$2,$3,$4,$5,CURRENT_TIMESTAMP)
            ON CONFLICT (task_id, fund_code) DO UPDATE SET
              as_of_date = excluded.as_of_date,
              best_peer_json = excluded.best_peer_json,
              computed_at = CURRENT_TIMESTAMP
        "#;

        if sqlx::query(sql_pg)
            .bind(job.id.as_str())
            .bind(code)
            .bind(&source)
            .bind(&as_of_date)
            .bind(&best_peer_json)
            .execute(pool)
            .await
            .is_err()
        {
            let _ = sqlx::query(sql_any)
                .bind(job.id.as_str())
                .bind(code)
                .bind(&source)
                .bind(&as_of_date)
                .bind(&best_peer_json)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }

        let _ = append_task_log(pool, run_id, "INFO", &format!("[{code}] 完成")).await;
    }

    Ok(())
}

async fn exec_quant_xalpha_metrics_batch(
    pool: &sqlx::AnyPool,
    run_id: &str,
    job: &TaskJobRow,
) -> Result<(), String> {
    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;

    let codes = payload
        .get("fund_codes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing fund_codes".to_string())?;
    let source = payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(crate::sources::SOURCE_TIANTIAN)
        .trim()
        .to_string();
    let window = payload
        .get("window")
        .and_then(|v| v.as_i64())
        .unwrap_or(252)
        .clamp(2, 5000);
    let risk_free_annual = payload
        .get("risk_free_annual")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let quant_service_url = payload
        .get("quant_service_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8002")
        .trim_end_matches('/')
        .to_string();

    let mut fund_codes: Vec<String> = Vec::new();
    for c in codes {
        let s = c.as_str().unwrap_or("").trim();
        if s.is_empty() {
            continue;
        }
        if !fund_codes.contains(&s.to_string()) {
            fund_codes.push(s.to_string());
        }
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!(
            "quant_xalpha_metrics_batch: fund_codes={} source={} window={} rf_annual={}",
            fund_codes.len(),
            source,
            window,
            risk_free_annual
        ),
    )
    .await;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{quant_service_url}/api/quant/xalpha/metrics");

    let mut ok = 0_i64;
    let mut failed = 0_i64;

    for (idx, code) in fund_codes.iter().enumerate() {
        if idx % 20 == 0 {
            let _ = append_task_log(
                pool,
                run_id,
                "INFO",
                &format!("进度 {idx}/{}", fund_codes.len()),
            )
            .await;
        }

        let Some(as_of_date) = latest_nav_date(pool, code, &source).await? else {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：无净值数据")).await;
            continue;
        };

        let rows = sqlx::query(
            r#"
            SELECT
              CAST(h.nav_date AS TEXT) as nav_date,
              CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1 AND h.source_name = $2
            ORDER BY h.nav_date DESC
            LIMIT $3
            "#,
        )
        .bind(code.trim())
        .bind(&source)
        .bind(window)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        if rows.len() < 2 {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：净值点数不足")).await;
            continue;
        }

        let mut series: Vec<Value> = Vec::with_capacity(rows.len());
        for r in rows.into_iter().rev() {
            let d: String = r.get("nav_date");
            let v: String = r.get("unit_nav");
            if let Ok(val) = v.trim().parse::<f64>() {
                series.push(json!({ "date": d, "val": val }));
            }
        }
        if series.len() < 2 {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：净值解析失败")).await;
            continue;
        }

        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("[{code}] 计算指标 as_of_date={as_of_date} points={}", series.len()),
        )
        .await;

        let resp = client
            .post(&url)
            .json(&json!({
              "series": series,
              "risk_free_annual": risk_free_annual,
            }))
            .send()
            .await;

        let resp = match resp {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] 请求 quant-service 失败: {e}")).await;
                continue;
            }
        };
        let resp = match resp.error_for_status() {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] quant-service 返回错误: {e}")).await;
                continue;
            }
        };
        let v = match resp.json::<Value>().await {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] quant-service 返回非 JSON: {e}")).await;
                continue;
            }
        };

        let tr = v
            .get("metrics")
            .and_then(|m| m.get("total_return"))
            .and_then(|x| x.as_f64());
        let mdd = v
            .get("metrics")
            .and_then(|m| m.get("max_drawdown"))
            .and_then(|x| x.as_f64());
        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("[{code}] 指标完成 total_return={:?} max_drawdown={:?}", tr, mdd),
        )
        .await;

        ok += 1;
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("quant_xalpha_metrics_batch 汇总：ok={ok} failed={failed}"),
    )
    .await;

    if ok == 0 {
        return Err("quant_xalpha_metrics_batch: all fund_codes failed".to_string());
    }

    Ok(())
}

async fn exec_quant_xalpha_grid_batch(
    pool: &sqlx::AnyPool,
    run_id: &str,
    job: &TaskJobRow,
) -> Result<(), String> {
    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;

    let codes = payload
        .get("fund_codes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing fund_codes".to_string())?;
    let source = payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(crate::sources::SOURCE_TIANTIAN)
        .trim()
        .to_string();
    let window = payload
        .get("window")
        .and_then(|v| v.as_i64())
        .unwrap_or(252)
        .clamp(2, 5000);
    let grid_step_pct = payload
        .get("grid_step_pct")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.02);
    let quant_service_url = payload
        .get("quant_service_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8002")
        .trim_end_matches('/')
        .to_string();

    let mut fund_codes: Vec<String> = Vec::new();
    for c in codes {
        let s = c.as_str().unwrap_or("").trim();
        if s.is_empty() {
            continue;
        }
        if !fund_codes.contains(&s.to_string()) {
            fund_codes.push(s.to_string());
        }
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!(
            "quant_xalpha_grid_batch: fund_codes={} source={} window={} grid_step_pct={}",
            fund_codes.len(),
            source,
            window,
            grid_step_pct
        ),
    )
    .await;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{quant_service_url}/api/quant/xalpha/grid");

    let mut ok = 0_i64;
    let mut failed = 0_i64;

    for (idx, code) in fund_codes.iter().enumerate() {
        if idx % 20 == 0 {
            let _ = append_task_log(
                pool,
                run_id,
                "INFO",
                &format!("进度 {idx}/{}", fund_codes.len()),
            )
            .await;
        }

        let Some(as_of_date) = latest_nav_date(pool, code, &source).await? else {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：无净值数据")).await;
            continue;
        };

        let rows = sqlx::query(
            r#"
            SELECT
              CAST(h.nav_date AS TEXT) as nav_date,
              CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1 AND h.source_name = $2
            ORDER BY h.nav_date DESC
            LIMIT $3
            "#,
        )
        .bind(code.trim())
        .bind(&source)
        .bind(window)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        if rows.len() < 2 {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：净值点数不足")).await;
            continue;
        }

        let mut series: Vec<Value> = Vec::with_capacity(rows.len());
        for r in rows.into_iter().rev() {
            let d: String = r.get("nav_date");
            let v: String = r.get("unit_nav");
            if let Ok(val) = v.trim().parse::<f64>() {
                series.push(json!({ "date": d, "val": val }));
            }
        }
        if series.len() < 2 {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：净值解析失败")).await;
            continue;
        }

        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("[{code}] 计算网格 as_of_date={as_of_date} points={}", series.len()),
        )
        .await;

        let resp = client
            .post(&url)
            .json(&json!({
              "series": series,
              "grid_step_pct": grid_step_pct,
            }))
            .send()
            .await;

        let resp = match resp {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] 请求 quant-service 失败: {e}")).await;
                continue;
            }
        };
        let resp = match resp.error_for_status() {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] quant-service 返回错误: {e}")).await;
                continue;
            }
        };
        let v = match resp.json::<Value>().await {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] quant-service 返回非 JSON: {e}")).await;
                continue;
            }
        };

        let action_count = v.get("actions").and_then(|a| a.as_array()).map(|a| a.len());
        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("[{code}] 网格完成 actions={action_count:?}"),
        )
        .await;

        ok += 1;
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("quant_xalpha_grid_batch 汇总：ok={ok} failed={failed}"),
    )
    .await;

    if ok == 0 {
        return Err("quant_xalpha_grid_batch: all fund_codes failed".to_string());
    }
    Ok(())
}

async fn exec_quant_xalpha_scheduled_batch(
    pool: &sqlx::AnyPool,
    run_id: &str,
    job: &TaskJobRow,
) -> Result<(), String> {
    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;

    let codes = payload
        .get("fund_codes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing fund_codes".to_string())?;
    let source = payload
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or(crate::sources::SOURCE_TIANTIAN)
        .trim()
        .to_string();
    let window = payload
        .get("window")
        .and_then(|v| v.as_i64())
        .unwrap_or(252)
        .clamp(2, 5000);
    let every_n = payload
        .get("every_n")
        .and_then(|v| v.as_i64())
        .unwrap_or(20)
        .clamp(1, 1000);
    let amount = payload.get("amount").and_then(|v| v.as_f64()).unwrap_or(1.0);
    let quant_service_url = payload
        .get("quant_service_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8002")
        .trim_end_matches('/')
        .to_string();

    let mut fund_codes: Vec<String> = Vec::new();
    for c in codes {
        let s = c.as_str().unwrap_or("").trim();
        if s.is_empty() {
            continue;
        }
        if !fund_codes.contains(&s.to_string()) {
            fund_codes.push(s.to_string());
        }
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!(
            "quant_xalpha_scheduled_batch: fund_codes={} source={} window={} every_n={} amount={}",
            fund_codes.len(),
            source,
            window,
            every_n,
            amount
        ),
    )
    .await;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{quant_service_url}/api/quant/xalpha/scheduled");

    let mut ok = 0_i64;
    let mut failed = 0_i64;

    for (idx, code) in fund_codes.iter().enumerate() {
        if idx % 20 == 0 {
            let _ = append_task_log(
                pool,
                run_id,
                "INFO",
                &format!("进度 {idx}/{}", fund_codes.len()),
            )
            .await;
        }

        let Some(as_of_date) = latest_nav_date(pool, code, &source).await? else {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：无净值数据")).await;
            continue;
        };

        let rows = sqlx::query(
            r#"
            SELECT
              CAST(h.nav_date AS TEXT) as nav_date,
              CAST(h.unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history h
            JOIN fund f ON f.id = h.fund_id
            WHERE f.fund_code = $1 AND h.source_name = $2
            ORDER BY h.nav_date DESC
            LIMIT $3
            "#,
        )
        .bind(code.trim())
        .bind(&source)
        .bind(window)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        if rows.len() < 2 {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：净值点数不足")).await;
            continue;
        }

        let mut series: Vec<Value> = Vec::with_capacity(rows.len());
        for r in rows.into_iter().rev() {
            let d: String = r.get("nav_date");
            let v: String = r.get("unit_nav");
            if let Ok(val) = v.trim().parse::<f64>() {
                series.push(json!({ "date": d, "val": val }));
            }
        }
        if series.len() < 2 {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", &format!("[{code}] 跳过：净值解析失败")).await;
            continue;
        }

        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("[{code}] 计算定投 as_of_date={as_of_date} points={}", series.len()),
        )
        .await;

        let resp = client
            .post(&url)
            .json(&json!({
              "series": series,
              "every_n": every_n,
              "amount": amount,
            }))
            .send()
            .await;

        let resp = match resp {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] 请求 quant-service 失败: {e}")).await;
                continue;
            }
        };
        let resp = match resp.error_for_status() {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] quant-service 返回错误: {e}")).await;
                continue;
            }
        };
        let v = match resp.json::<Value>().await {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] quant-service 返回非 JSON: {e}")).await;
                continue;
            }
        };

        let action_count = v.get("actions").and_then(|a| a.as_array()).map(|a| a.len());
        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("[{code}] 定投完成 actions={action_count:?}"),
        )
        .await;

        ok += 1;
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("quant_xalpha_scheduled_batch 汇总：ok={ok} failed={failed}"),
    )
    .await;

    if ok == 0 {
        return Err("quant_xalpha_scheduled_batch: all fund_codes failed".to_string());
    }
    Ok(())
}

async fn exec_quant_xalpha_qdiipredict_batch(
    pool: &sqlx::AnyPool,
    run_id: &str,
    job: &TaskJobRow,
) -> Result<(), String> {
    let payload: Value = serde_json::from_str(&job.payload_json).map_err(|e| e.to_string())?;
    let items = payload
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "missing items".to_string())?;
    let quant_service_url = payload
        .get("quant_service_url")
        .and_then(|v| v.as_str())
        .unwrap_or("http://localhost:8002")
        .trim_end_matches('/')
        .to_string();

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("quant_xalpha_qdiipredict_batch: items={}", items.len()),
    )
    .await;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!("{quant_service_url}/api/quant/xalpha/qdiipredict");

    let mut ok = 0_i64;
    let mut failed = 0_i64;

    for (idx, it) in items.iter().enumerate() {
        if idx % 20 == 0 {
            let _ = append_task_log(pool, run_id, "INFO", &format!("进度 {idx}/{}", items.len())).await;
        }

        let code = it
            .get("fund_code")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        if code.is_empty() {
            failed += 1;
            let _ = append_task_log(pool, run_id, "WARN", "跳过：fund_code 为空").await;
            continue;
        }

        let last_value = it.get("last_value").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let legs = it.get("legs").cloned().unwrap_or(Value::Array(vec![]));

        let _ = append_task_log(pool, run_id, "INFO", &format!("[{code}] 预测 QDII last_value={last_value}")).await;

        let resp = client
            .post(&url)
            .json(&json!({
              "last_value": last_value,
              "legs": legs,
            }))
            .send()
            .await;

        let resp = match resp {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] 请求 quant-service 失败: {e}")).await;
                continue;
            }
        };
        let resp = match resp.error_for_status() {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] quant-service 返回错误: {e}")).await;
                continue;
            }
        };
        let v = match resp.json::<Value>().await {
            Ok(v) => v,
            Err(e) => {
                failed += 1;
                let _ = append_task_log(pool, run_id, "ERROR", &format!("[{code}] quant-service 返回非 JSON: {e}")).await;
                continue;
            }
        };

        let delta = v.get("delta").and_then(|x| x.as_f64());
        let predicted_value = v.get("predicted_value").and_then(|x| x.as_f64());

        let _ = append_task_log(
            pool,
            run_id,
            "INFO",
            &format!("[{code}] 完成 delta={delta:?} predicted_value={predicted_value:?}"),
        )
        .await;

        ok += 1;
    }

    let _ = append_task_log(
        pool,
        run_id,
        "INFO",
        &format!("quant_xalpha_qdiipredict_batch 汇总：ok={ok} failed={failed}"),
    )
    .await;

    if ok == 0 {
        return Err("quant_xalpha_qdiipredict_batch: all items failed".to_string());
    }
    Ok(())
}
