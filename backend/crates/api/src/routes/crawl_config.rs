use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::sources;
use crate::state::AppState;

async fn require_staff(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<(), axum::response::Response> {
    let pool = match state.pool() {
        Some(p) => p,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "数据库未连接" })),
            )
                .into_response());
        }
    };

    let user_id = match crate::routes::auth::authenticate(state, headers) {
        Ok(v) => v,
        Err(resp) => return Err(resp),
    };
    let user_id_i64 = user_id
        .parse::<i64>()
        .map_err(|_| crate::routes::auth::invalid_token_response())?;

    let is_staff = match sqlx::query("SELECT is_staff FROM auth_user WHERE id = $1")
        .bind(user_id_i64)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(row)) => row
            .try_get::<bool, _>("is_staff")
            .unwrap_or_else(|_| row.try_get::<i64, _>("is_staff").unwrap_or(0) != 0),
        _ => false,
    };

    if !is_staff {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "detail": "You do not have permission to perform this action." })),
        )
            .into_response());
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct CrawlConfigResponse {
    pub crawl_enabled: bool,
    pub crawl_source: String,
    pub crawl_tick_interval_seconds: i64,
    pub crawl_enqueue_max_jobs: i64,
    pub crawl_daily_run_limit: i64,
    pub crawl_run_max_jobs: i64,
    pub crawl_per_job_delay_ms: i64,
    pub crawl_per_job_jitter_ms: i64,
    pub crawl_source_fallbacks: String,
}

pub async fn admin_get_config(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    if let Err(resp) = require_staff(&state, &headers).await {
        return resp;
    }

    let cfg = state.config();
    let source = cfg
        .get_string("crawl_source")
        .unwrap_or_else(|| sources::SOURCE_TIANTIAN.to_string());
    let source = sources::normalize_source_name(&source)
        .unwrap_or(sources::SOURCE_TIANTIAN)
        .to_string();

    (
        StatusCode::OK,
        Json(CrawlConfigResponse {
            crawl_enabled: cfg.get_bool("crawl_enabled", true),
            crawl_source: source,
            crawl_tick_interval_seconds: cfg.get_i64("crawl_tick_interval_seconds", 30),
            crawl_enqueue_max_jobs: cfg.get_i64("crawl_enqueue_max_jobs", 200),
            crawl_daily_run_limit: cfg.get_i64("crawl_daily_run_limit", 3000),
            crawl_run_max_jobs: cfg.get_i64("crawl_run_max_jobs", 20),
            crawl_per_job_delay_ms: cfg.get_i64("crawl_per_job_delay_ms", 250),
            crawl_per_job_jitter_ms: cfg.get_i64("crawl_per_job_jitter_ms", 200),
            crawl_source_fallbacks: cfg.get_string("crawl_source_fallbacks").unwrap_or_default(),
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct SetCrawlConfigRequest {
    pub crawl_enabled: Option<bool>,
    pub crawl_source: Option<String>,
    pub crawl_tick_interval_seconds: Option<i64>,
    pub crawl_enqueue_max_jobs: Option<i64>,
    pub crawl_daily_run_limit: Option<i64>,
    pub crawl_run_max_jobs: Option<i64>,
    pub crawl_per_job_delay_ms: Option<i64>,
    pub crawl_per_job_jitter_ms: Option<i64>,
    pub crawl_source_fallbacks: Option<String>,
}

pub async fn admin_set_config(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<SetCrawlConfigRequest>,
) -> axum::response::Response {
    if let Err(resp) = require_staff(&state, &headers).await {
        return resp;
    }

    let cfg = state.config();

    if let Some(v) = body.crawl_enabled {
        cfg.set_bool("crawl_enabled", v);
    }
    if let Some(v) = body.crawl_source {
        let raw = v.trim();
        let Some(n) = sources::normalize_source_name(raw) else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("未知数据源: {raw}") })),
            )
                .into_response();
        };
        cfg.set_string("crawl_source", Some(n.to_string()));
    }

    if let Some(v) = body.crawl_tick_interval_seconds {
        cfg.set_i64(
            "crawl_tick_interval_seconds",
            Some(v.clamp(1, 24 * 60 * 60)),
        );
    }
    if let Some(v) = body.crawl_enqueue_max_jobs {
        cfg.set_i64("crawl_enqueue_max_jobs", Some(v.clamp(0, 5000)));
    }
    if let Some(v) = body.crawl_daily_run_limit {
        cfg.set_i64("crawl_daily_run_limit", Some(v.clamp(0, 1_000_000)));
    }
    if let Some(v) = body.crawl_run_max_jobs {
        cfg.set_i64("crawl_run_max_jobs", Some(v.clamp(0, 5000)));
    }
    if let Some(v) = body.crawl_per_job_delay_ms {
        cfg.set_i64("crawl_per_job_delay_ms", Some(v.clamp(0, 60_000)));
    }
    if let Some(v) = body.crawl_per_job_jitter_ms {
        cfg.set_i64("crawl_per_job_jitter_ms", Some(v.clamp(0, 60_000)));
    }
    if let Some(v) = body.crawl_source_fallbacks {
        cfg.set_string("crawl_source_fallbacks", Some(v.trim().to_string()));
    }

    if let Err(e) = cfg.save() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("保存配置失败: {e}") })),
        )
            .into_response();
    }

    (StatusCode::OK, Json(json!({ "message": "ok" }))).into_response()
}
