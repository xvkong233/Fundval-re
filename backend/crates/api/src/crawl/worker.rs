use std::sync::Arc;
use std::time::Duration;

use sqlx::Row;

use crate::crawl::scheduler::{self, CrawlJob};
use crate::eastmoney;
use crate::ml;
use crate::routes::nav_history;
use crate::sources;
use crate::state::AppState;
use crate::tiantian_h5;

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

    scheduler::run_due_jobs(pool, max_run, |job| {
        let client = client.clone();
        let tushare_token = tushare_token.clone();
        let source_fallbacks = source_fallbacks.clone();
        async move {
            exec_one(
                pool,
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
        interval.tick().await;

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
    }
}

async fn exec_one(
    pool: &sqlx::AnyPool,
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
                match nav_history::sync_one(pool, client, s, &fund_code, None, None, tushare_token)
                    .await
                {
                    Ok(_) => {
                        last_err = None;
                        ok_source = Some(s);
                        break;
                    }
                    Err(e) => {
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
            let themes = tiantian_h5::fetch_fund_relate_themes(client, &fund_code).await?;
            let _ =
                tiantian_h5::upsert_fund_relate_themes(pool, &fund_code, "tiantian_h5", &themes)
                    .await?;
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
