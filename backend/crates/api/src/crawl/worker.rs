use std::time::Duration;

use crate::crawl::scheduler::{self, CrawlJob};
use crate::eastmoney;
use crate::routes::nav_history;
use crate::sources;
use crate::state::AppState;

pub async fn run_due_jobs_with_nav_sync(
    pool: &sqlx::AnyPool,
    config: &crate::config::ConfigStore,
    max_run: i64,
    per_job_delay_ms: u64,
) -> Result<i64, String> {
    let client = eastmoney::build_client()?;
    let tushare_token = config.get_string("tushare_token").unwrap_or_default();

    scheduler::run_due_jobs(pool, max_run, |job| {
        let client = client.clone();
        let tushare_token = tushare_token.clone();
        async move { exec_one(pool, &client, &tushare_token, per_job_delay_ms, job).await }
    })
    .await
}

pub async fn background_task(state: AppState) {
    let Some(pool) = state.pool().cloned() else {
        return;
    };

    // 每轮：先补充队列（自选/持仓优先），再跑一批到期任务。
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;

        let _guard = state.crawl_lock().lock().await;

        if let Err(e) = scheduler::enqueue_tick(&pool, 200, sources::SOURCE_TIANTIAN).await {
            tracing::warn!(error = %e, "crawl enqueue_tick failed");
        }

        if let Err(e) = run_due_jobs_with_nav_sync(&pool, state.config(), 20, 250).await {
            tracing::warn!(error = %e, "crawl run_due_jobs failed");
        }
    }
}

async fn exec_one(
    pool: &sqlx::AnyPool,
    client: &reqwest::Client,
    tushare_token: &str,
    per_job_delay_ms: u64,
    job: CrawlJob,
) -> Result<(), String> {
    if job.job_type != "nav_history_sync" {
        return Err(format!("unknown job_type: {}", job.job_type));
    }
    let fund_code = job
        .fund_code
        .clone()
        .ok_or_else(|| "missing fund_code".to_string())?;

    let source_raw = job
        .source_name
        .as_deref()
        .unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_raw) else {
        return Err(format!("unknown source: {source_raw}"));
    };

    let _inserted = nav_history::sync_one(
        pool,
        client,
        source_name,
        &fund_code,
        None,
        None,
        tushare_token,
    )
    .await?;

    if per_job_delay_ms > 0 {
        tokio::time::sleep(Duration::from_millis(per_job_delay_ms)).await;
    }

    Ok(())
}
