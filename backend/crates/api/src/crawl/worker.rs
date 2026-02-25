use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::Decimal;
use sqlx::Row;
use uuid::Uuid;

use crate::crawl::scheduler::{self, CrawlJob};
use crate::eastmoney;
use crate::ml;
use crate::routes::nav_history;
use crate::sources;
use crate::state::AppState;
use crate::tiantian_h5;
use crate::tasks;

fn format_dt(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::AutoSi, false)
}

async fn upsert_estimate_accuracy(
    pool: &sqlx::AnyPool,
    source_name: &str,
    fund_id: &str,
    estimate_date: &str,
    estimate_nav: Decimal,
) -> Result<(), sqlx::Error> {
    let is_postgres = crate::db::database_kind_from_pool(pool) == crate::db::DatabaseKind::Postgres;
    let sql = if is_postgres {
        r#"
            INSERT INTO estimate_accuracy (id, source_name, fund_id, estimate_date, estimate_nav, created_at)
            VALUES (($1)::uuid, $2, ($3)::uuid, ($4)::date, ($5)::numeric, CURRENT_TIMESTAMP)
            ON CONFLICT (source_name, fund_id, estimate_date) DO UPDATE
              SET estimate_nav = EXCLUDED.estimate_nav
        "#
    } else {
        r#"
            INSERT INTO estimate_accuracy (id, source_name, fund_id, estimate_date, estimate_nav, created_at)
            VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
            ON CONFLICT (source_name, fund_id, estimate_date) DO UPDATE
              SET estimate_nav = excluded.estimate_nav
        "#
    };

    sqlx::query(sql)
        .bind(Uuid::new_v4().to_string())
        .bind(source_name)
        .bind(fund_id)
        .bind(estimate_date)
        .bind(estimate_nav.to_string())
        .execute(pool)
        .await?;

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

    let r = sqlx::query(sql_pg)
        .bind(fund_code)
        .bind(estimate_nav)
        .bind(estimate_growth)
        .bind(estimate_time_rfc3339)
        .execute(pool)
        .await;
    if r.is_ok() {
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

    let r = sqlx::query(sql_pg)
        .bind(fund_code)
        .bind(latest_nav)
        .bind(latest_nav_date)
        .execute(pool)
        .await;
    if r.is_ok() {
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

fn parse_source_list(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in raw.split(',') {
        let s = p.trim();
        if s.is_empty() {
            continue;
        }
        let Some(n) = sources::normalize_source_name(s) else {
            continue;
        };
        let n = n.to_string();
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

pub async fn run_due_jobs_with_nav_sync(
    pool: &sqlx::AnyPool,
    config: &crate::config::ConfigStore,
    max_run: i64,
    per_job_delay_ms: u64,
    per_job_jitter_ms: u64,
    source_fallbacks: Arc<Vec<String>>,
) -> Result<i64, String> {
    let client = eastmoney::build_client()?;
    let tushare_token = config.get_string("tushare_token").unwrap_or_default();

    scheduler::run_due_jobs(pool, max_run, |job, run_id| {
        let client = client.clone();
        let tushare_token = tushare_token.clone();
        let source_fallbacks = source_fallbacks.clone();
        async move {
            exec_one(
                pool,
                &run_id,
                &client,
                &tushare_token,
                per_job_delay_ms,
                per_job_jitter_ms,
                &source_fallbacks,
                job,
            )
            .await
        }
    })
    .await
}

pub async fn background_task(state: AppState) {
    let Some(pool) = state.pool().cloned() else {
        return;
    };

    // 每轮：先补充队列（自选/持仓优先），再跑一批到期任务。
    let tick_seconds = state
        .config()
        .get_i64("crawl_tick_interval_seconds", 30)
        .max(1) as u64;
    let mut interval = tokio::time::interval(Duration::from_secs(tick_seconds));
    loop {
        tokio::select! {
            _ = interval.tick() => {},
            _ = state.crawl_notify().notified() => {},
        }

        let _guard = state.crawl_lock().lock().await;

        if !state.config().get_bool("crawl_enabled", true) {
            continue;
        }

        let source_raw = state
            .config()
            .get_string("crawl_source")
            .unwrap_or_else(|| sources::SOURCE_TIANTIAN.to_string());
        let Some(source_name) = sources::normalize_source_name(&source_raw) else {
            tracing::warn!(source = %source_raw, "crawl disabled due to unknown source");
            continue;
        };

        let enqueue_max = state
            .config()
            .get_i64("crawl_enqueue_max_jobs", 200)
            .clamp(0, 5000);
        if let Err(e) = scheduler::enqueue_tick(&pool, enqueue_max, source_name).await {
            tracing::warn!(error = %e, "crawl enqueue_tick failed");
        }

        // 估值：自选/持仓优先，全市场慢速覆盖（可通过 estimate_enqueue_max_jobs 控制）。
        let estimate_enqueue_max = state
            .config()
            .get_i64("estimate_enqueue_max_jobs", 50)
            .clamp(0, 5000);
        if estimate_enqueue_max > 0 {
            if let Err(e) =
                scheduler::enqueue_estimate_tick(&pool, estimate_enqueue_max, source_name).await
            {
                tracing::warn!(error = %e, "crawl enqueue_estimate_tick failed");
            }
        }

        let mut run_max = state
            .config()
            .get_i64("crawl_run_max_jobs", 20)
            .clamp(0, 5000);

        let daily_limit = state
            .config()
            .get_i64("crawl_daily_run_limit", 3000)
            .clamp(0, 1_000_000);
        if daily_limit > 0 && run_max > 0 {
            let key = scheduler::daily_counter_key_all(source_name, "run");
            if let Ok(used) = scheduler::get_counter(&pool, &key).await {
                let remaining = (daily_limit - used).max(0);
                if remaining <= 0 {
                    continue;
                }
                run_max = run_max.min(remaining);
            }
        }
        if run_max <= 0 {
            continue;
        }

        let per_job_delay_ms = state
            .config()
            .get_i64("crawl_per_job_delay_ms", 250)
            .clamp(0, 60_000) as u64;
        let per_job_jitter_ms = state
            .config()
            .get_i64("crawl_per_job_jitter_ms", 200)
            .clamp(0, 60_000) as u64;

        let fallbacks_raw = state
            .config()
            .get_string("crawl_source_fallbacks")
            .unwrap_or_default();
        let mut fallbacks = parse_source_list(&fallbacks_raw);
        fallbacks.retain(|s| s != source_name);
        let fallbacks = Arc::new(fallbacks);

        if let Err(e) = run_due_jobs_with_nav_sync(
            &pool,
            state.config(),
            run_max,
            per_job_delay_ms,
            per_job_jitter_ms,
            fallbacks,
        )
        .await
        {
            tracing::warn!(error = %e, "crawl run_due_jobs failed");
        }

        let task_run_max = state
            .config()
            .get_i64("task_run_max_jobs", 5)
            .clamp(0, 200);
        if task_run_max > 0 {
            if let Err(e) = tasks::run_due_task_jobs(&pool, task_run_max).await {
                tracing::warn!(error = %e, "task queue run_due_task_jobs failed");
            }
        }
    }
}

async fn exec_one(
    pool: &sqlx::AnyPool,
    run_id: &str,
    client: &reqwest::Client,
    tushare_token: &str,
    per_job_delay_ms: u64,
    per_job_jitter_ms: u64,
    source_fallbacks: &[String],
    job: CrawlJob,
) -> Result<(), String> {
    let fund_code = job
        .fund_code
        .clone()
        .ok_or_else(|| "missing fund_code".to_string())?;

    let _ = crate::tasks::append_task_log(
        pool,
        run_id,
        "INFO",
        &format!(
            "开始执行 job_type={} fund_code={} source={}",
            job.job_type,
            fund_code,
            job.source_name.clone().unwrap_or_else(|| "tiantian".to_string())
        ),
    )
    .await;

    match job.job_type.as_str() {
        "nav_history_sync" => {
            let source_raw = job
                .source_name
                .as_deref()
                .unwrap_or(sources::SOURCE_TIANTIAN);
            let Some(source_name) = sources::normalize_source_name(source_raw) else {
                return Err(format!("unknown source: {source_raw}"));
            };

            let mut last_err: Option<String> = None;
            let mut ok_source: Option<&str> = None;
            let mut tried: Vec<&str> = Vec::new();
            tried.push(source_name);
            for fb in source_fallbacks {
                if tried.contains(&fb.as_str()) {
                    continue;
                }
                tried.push(fb);
            }

            for s in tried {
                let _ = crate::tasks::append_task_log(
                    pool,
                    run_id,
                    "INFO",
                    &format!("尝试同步净值：source={s}"),
                )
                .await;
                match nav_history::sync_one(pool, client, s, &fund_code, None, None, tushare_token)
                    .await
                {
                    Ok(_) => {
                        last_err = None;
                        ok_source = Some(s);
                        let _ = crate::tasks::append_task_log(
                            pool,
                            run_id,
                            "INFO",
                            &format!("净值同步成功：source={s}"),
                        )
                        .await;
                        break;
                    }
                    Err(e) => {
                        let _ = crate::tasks::append_task_log(
                            pool,
                            run_id,
                            "WARN",
                            &format!("净值同步失败：source={s} err={e}"),
                        )
                        .await;
                        last_err = Some(e);
                        continue;
                    }
                }
            }

            if let Some(e) = last_err {
                return Err(e);
            }

            // best-effort：净值同步后顺便计算信号快照（不强制训练，避免拖慢爬取节奏）。
            if let Some(source_used) = ok_source {
                // 全市场兜底：即使没有关联板块，也能基于全量基金数据生成信号（模型缺失时会训练一次）。
                let _ = ml::compute::compute_and_store_fund_snapshot_with_opts(
                    pool,
                    &fund_code,
                    ml::train::PEER_CODE_ALL,
                    source_used,
                    ml::compute::ComputeOpts {
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
                .bind(&fund_code)
                .fetch_all(pool)
                .await;

                if let Ok(rows) = peer_rows {
                    for r in rows {
                        let peer_code: String = r.get("sec_code");
                        let _ = crate::tasks::append_task_log(
                            pool,
                            run_id,
                            "INFO",
                            &format!("计算信号快照（板块）：peer_code={peer_code}"),
                        )
                        .await;
                        let _ = ml::compute::compute_and_store_fund_snapshot_with_opts(
                            pool,
                            &fund_code,
                            &peer_code,
                            source_used,
                            ml::compute::ComputeOpts {
                                train_if_missing: false,
                            },
                        )
                        .await;
                    }
                }
            }
        }
        "relate_theme_sync" => {
            let _ = crate::tasks::append_task_log(pool, run_id, "INFO", "拉取关联板块").await;
            let themes = tiantian_h5::fetch_fund_relate_themes(client, &fund_code).await?;
            let _ = crate::tasks::append_task_log(
                pool,
                run_id,
                "INFO",
                &format!("关联板块条数：{}", themes.len()),
            )
            .await;
            let _ =
                tiantian_h5::upsert_fund_relate_themes(pool, &fund_code, "tiantian_h5", &themes)
                    .await?;
        }
        "estimate_sync" => {
            let source_raw = job
                .source_name
                .as_deref()
                .unwrap_or(sources::SOURCE_TIANTIAN);
            let Some(source_name) = sources::normalize_source_name(source_raw) else {
                return Err(format!("unknown source: {source_raw}"));
            };

            // 估值只对 tiantian（eastmoney fundgz）走实时接口；其他源退化为“最新净值”近似，避免额外上游请求。
            if source_name == sources::SOURCE_TIANTIAN {
                let _ = crate::tasks::append_task_log(pool, run_id, "INFO", "请求实时估值（fundgz）").await;
                let snap = eastmoney::fetch_fundgz_snapshot(client, &fund_code).await?;
                let Some(snap) = snap else {
                    return Err("fundgz empty".to_string());
                };

                let latest_nav = snap.latest_nav;
                let latest_nav_date = snap.latest_nav_date;
                let estimate_nav = snap.estimate_nav;
                let estimate_growth = snap.estimate_growth;
                let estimate_time = snap.estimate_time;
                let gztime_raw = snap.gztime_raw;

                if let (Some(nav), Some(nav_date)) = (latest_nav, latest_nav_date) {
                    let _ = update_fund_latest_nav_fields(
                        pool,
                        &fund_code,
                        &nav.to_string(),
                        &nav_date.to_string(),
                    )
                    .await;
                }

                let Some(estimate_nav) = estimate_nav else {
                    return Err("estimate empty".to_string());
                };

                let now = Utc::now();
                let growth_s = estimate_growth.map(|g| g.to_string());
                update_fund_estimate_fields(
                    pool,
                    &fund_code,
                    &estimate_nav.to_string(),
                    growth_s.as_deref(),
                    &format_dt(now),
                )
                .await?;
                let _ = crate::tasks::append_task_log(
                    pool,
                    run_id,
                    "INFO",
                    &format!(
                        "估值更新：nav={} growth={:?} gztime={:?}",
                        estimate_nav, growth_s, gztime_raw
                    ),
                )
                .await;

                // best-effort：记录估值准确度（用于后续评估数据源质量）
                if let Ok(Some(row)) = sqlx::query("SELECT CAST(id AS TEXT) as id FROM fund WHERE fund_code = $1")
                    .bind(&fund_code)
                    .fetch_optional(pool)
                    .await
                {
                    let fund_id: String = row.get("id");
                    if let Some(et) = estimate_time {
                        let _ = upsert_estimate_accuracy(
                            pool,
                            sources::SOURCE_TIANTIAN,
                            &fund_id,
                            &et.date().to_string(),
                            estimate_nav,
                        )
                        .await;
                    }
                }
            } else {
                // fallback：用 fund.latest_nav 作为近似估值
                let _ = crate::tasks::append_task_log(pool, run_id, "INFO", "非 tiantian：退化为 latest_nav").await;
                let row = sqlx::query(
                    r#"
                    SELECT
                      CAST(latest_nav AS TEXT) as latest_nav
                    FROM fund
                    WHERE fund_code = $1
                    "#,
                )
                .bind(&fund_code)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                let Some(row) = row else {
                    return Ok(());
                };
                let Some(latest_nav) = row.get::<Option<String>, _>("latest_nav") else {
                    return Ok(());
                };

                let now = Utc::now();
                update_fund_estimate_fields(pool, &fund_code, &latest_nav, None, &format_dt(now))
                    .await?;
                let _ = crate::tasks::append_task_log(
                    pool,
                    run_id,
                    "INFO",
                    &format!("估值更新（fallback latest_nav）：nav={latest_nav}"),
                )
                .await;
            }
        }
        _ => return Err(format!("unknown job_type: {}", job.job_type)),
    }

    if per_job_delay_ms > 0 {
        let jitter = if per_job_jitter_ms == 0 {
            0
        } else {
            // 稳定抖动：避免引入随机源（便于测试/复现），只用于分散节奏。
            let mut h: u64 = 14695981039346656037;
            for b in fund_code.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(1099511628211);
            }
            h % (per_job_jitter_ms + 1)
        };
        tokio::time::sleep(Duration::from_millis(per_job_delay_ms + jitter)).await;
    }

    Ok(())
}
