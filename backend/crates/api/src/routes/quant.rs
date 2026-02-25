use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::routes::auth;
use crate::routes::errors;
use crate::sources;
use crate::state::AppState;
use crate::tasks;

fn quant_base_url(state: &AppState) -> String {
    state
        .config()
        .get_string("quant_service_url")
        .unwrap_or_else(|| "http://localhost:8002".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn bad_gateway(message: String) -> axum::response::Response {
    (StatusCode::BAD_GATEWAY, Json(json!({ "error": message }))).into_response()
}

pub async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let url = format!("{}/health", quant_base_url(&state));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
            )
                .into_response();
        }
    };

    let resp = client.get(url).send().await;
    let resp = match resp {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 不可达: {e}")),
    };
    let resp = match resp.error_for_status() {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 返回错误: {e}")),
    };
    let json = match resp.json::<Value>().await {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 返回非 JSON: {e}")),
    };
    (StatusCode::OK, Json(json)).into_response()
}

pub async fn macd(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let url = format!("{}/api/quant/macd", quant_base_url(&state));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
            )
                .into_response();
        }
    };

    let resp = client.post(url).json(&body).send().await;
    let resp = match resp {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 不可达: {e}")),
    };
    let resp = match resp.error_for_status() {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 返回错误: {e}")),
    };
    let json = match resp.json::<Value>().await {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 返回非 JSON: {e}")),
    };
    (StatusCode::OK, Json(json)).into_response()
}

async fn proxy_xalpha_like(
    state: AppState,
    headers: HeaderMap,
    path: &'static str,
    body: Value,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let url = format!("{}/{}", quant_base_url(&state), path.trim_start_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
            )
                .into_response();
        }
    };

    let resp = client.post(url).json(&body).send().await;
    let resp = match resp {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 不可达: {e}")),
    };
    let resp = match resp.error_for_status() {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 返回错误: {e}")),
    };
    let json = match resp.json::<Value>().await {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 返回非 JSON: {e}")),
    };
    (StatusCode::OK, Json(json)).into_response()
}

async fn proxy_quant_get(
    state: AppState,
    headers: HeaderMap,
    path: &'static str,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let url = format!("{}/{}", quant_base_url(&state), path.trim_start_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
            )
                .into_response();
        }
    };

    let resp = client.get(url).send().await;
    let resp = match resp {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 不可达: {e}")),
    };
    let resp = match resp.error_for_status() {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 返回错误: {e}")),
    };
    let json = match resp.json::<Value>().await {
        Ok(v) => v,
        Err(e) => return bad_gateway(format!("quant-service 返回非 JSON: {e}")),
    };
    (StatusCode::OK, Json(json)).into_response()
}

pub async fn xalpha_metrics(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    proxy_xalpha_like(state, headers, "/api/quant/xalpha/metrics", body).await
}

pub async fn xalpha_grid(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    proxy_xalpha_like(state, headers, "/api/quant/xalpha/grid", body).await
}

pub async fn xalpha_scheduled(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    proxy_xalpha_like(state, headers, "/api/quant/xalpha/scheduled", body).await
}

pub async fn xalpha_qdiipredict(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    proxy_xalpha_like(state, headers, "/api/quant/xalpha/qdiipredict", body).await
}

pub async fn xalpha_backtest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    proxy_xalpha_like(state, headers, "/api/quant/xalpha/backtest", body).await
}

pub async fn fund_strategies_compare(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    proxy_xalpha_like(state, headers, "/api/quant/fund-strategies/compare", body).await
}

pub async fn pytrader_strategies(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> axum::response::Response {
    proxy_quant_get(state, headers, "/api/quant/pytrader/strategies").await
}

pub async fn pytrader_backtest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> axum::response::Response {
    proxy_xalpha_like(state, headers, "/api/quant/pytrader/backtest", body).await
}

#[derive(Debug, Deserialize)]
pub struct MetricsBatchAsyncBody {
    pub fund_codes: Vec<String>,
    pub source: Option<String>,
    pub window: Option<i64>,
    pub risk_free_annual: Option<f64>,
}

#[derive(Debug, serde::Serialize)]
pub struct EnqueueTaskOut {
    pub task_id: String,
}

fn sanitize_fund_codes(mut codes: Vec<String>) -> Result<Vec<String>, axum::response::Response> {
    let mut fund_codes: Vec<String> = Vec::new();
    for c in codes.drain(..) {
        let s = c.trim();
        if s.is_empty() {
            continue;
        }
        if !fund_codes.contains(&s.to_string()) {
            fund_codes.push(s.to_string());
        }
    }
    if fund_codes.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "fund_codes 不能为空" })),
        )
            .into_response());
    }
    fund_codes.truncate(2000);
    Ok(fund_codes)
}

fn normalize_source_or_404(source: &str) -> Result<&'static str, axum::response::Response> {
    let source_raw = source.trim();
    let Some(source_name) = sources::normalize_source_name(source_raw) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("unknown source: {source_raw}") })),
        )
            .into_response());
    };
    Ok(source_name)
}

pub async fn xalpha_metrics_batch_async(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<MetricsBatchAsyncBody>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let created_by = user_id.parse::<i64>().ok();

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let source_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let source_name = match normalize_source_or_404(source_raw) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let fund_codes = match sanitize_fund_codes(body.fund_codes) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let payload = json!({
      "fund_codes": fund_codes,
      "source": source_name,
      "window": body.window.unwrap_or(252).clamp(2, 5000),
      "risk_free_annual": body.risk_free_annual.unwrap_or(0.0),
      "quant_service_url": quant_base_url(&state),
    });

    let task_id = match tasks::enqueue_task_job(pool, "quant_xalpha_metrics_batch", &payload, 90, created_by).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    // best-effort：立即触发一次执行，减少等待下一轮 tick 的延迟。
    state.crawl_notify().notify_one();
    let pool2 = pool.clone();
    if !cfg!(test) {
        tokio::spawn(async move {
            if let Err(e) = tasks::run_due_task_jobs(&pool2, 1).await {
                tracing::warn!(error = %e, "task queue run_due_task_jobs failed (route trigger)");
            }
        });
    }

    (StatusCode::ACCEPTED, Json(EnqueueTaskOut { task_id })).into_response()
}

#[derive(Debug, Deserialize)]
pub struct GridBatchAsyncBody {
    pub fund_codes: Vec<String>,
    pub source: Option<String>,
    pub window: Option<i64>,
    pub grid_step_pct: Option<f64>,
}

pub async fn xalpha_grid_batch_async(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<GridBatchAsyncBody>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let created_by = user_id.parse::<i64>().ok();

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let source_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let source_name = match normalize_source_or_404(source_raw) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let fund_codes = match sanitize_fund_codes(body.fund_codes) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let payload = json!({
      "fund_codes": fund_codes,
      "source": source_name,
      "window": body.window.unwrap_or(252).clamp(2, 5000),
      "grid_step_pct": body.grid_step_pct.unwrap_or(0.02),
      "quant_service_url": quant_base_url(&state),
    });

    let task_id = match tasks::enqueue_task_job(pool, "quant_xalpha_grid_batch", &payload, 80, created_by).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    state.crawl_notify().notify_one();
    let pool2 = pool.clone();
    if !cfg!(test) {
        tokio::spawn(async move {
            if let Err(e) = tasks::run_due_task_jobs(&pool2, 1).await {
                tracing::warn!(error = %e, "task queue run_due_task_jobs failed (route trigger)");
            }
        });
    }

    (StatusCode::ACCEPTED, Json(EnqueueTaskOut { task_id })).into_response()
}

#[derive(Debug, Deserialize)]
pub struct ScheduledBatchAsyncBody {
    pub fund_codes: Vec<String>,
    pub source: Option<String>,
    pub window: Option<i64>,
    pub every_n: Option<i64>,
    pub amount: Option<f64>,
}

pub async fn xalpha_scheduled_batch_async(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<ScheduledBatchAsyncBody>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let created_by = user_id.parse::<i64>().ok();

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let source_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let source_name = match normalize_source_or_404(source_raw) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let fund_codes = match sanitize_fund_codes(body.fund_codes) {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let payload = json!({
      "fund_codes": fund_codes,
      "source": source_name,
      "window": body.window.unwrap_or(252).clamp(2, 5000),
      "every_n": body.every_n.unwrap_or(20).clamp(1, 1000),
      "amount": body.amount.unwrap_or(1.0),
      "quant_service_url": quant_base_url(&state),
    });

    let task_id = match tasks::enqueue_task_job(pool, "quant_xalpha_scheduled_batch", &payload, 80, created_by).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    state.crawl_notify().notify_one();
    let pool2 = pool.clone();
    if !cfg!(test) {
        tokio::spawn(async move {
            if let Err(e) = tasks::run_due_task_jobs(&pool2, 1).await {
                tracing::warn!(error = %e, "task queue run_due_task_jobs failed (route trigger)");
            }
        });
    }

    (StatusCode::ACCEPTED, Json(EnqueueTaskOut { task_id })).into_response()
}

#[derive(Debug, Deserialize)]
pub struct QdiiPredictBatchItemBody {
    pub fund_code: String,
    pub last_value: f64,
    pub legs: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub struct QdiiPredictBatchAsyncBody {
    pub items: Vec<QdiiPredictBatchItemBody>,
}

pub async fn xalpha_qdiipredict_batch_async(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<QdiiPredictBatchAsyncBody>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let created_by = user_id.parse::<i64>().ok();

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let mut items: Vec<Value> = Vec::new();
    for it in body.items {
        let fund_code = it.fund_code.trim().to_string();
        if fund_code.is_empty() {
            continue;
        }
        items.push(json!({
          "fund_code": fund_code,
          "last_value": it.last_value,
          "legs": it.legs,
        }));
    }
    if items.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "items 不能为空" })),
        )
            .into_response();
    }
    if items.len() > 2000 {
        items.truncate(2000);
    }

    let payload = json!({
      "items": items,
      "quant_service_url": quant_base_url(&state),
    });

    let task_id = match tasks::enqueue_task_job(pool, "quant_xalpha_qdiipredict_batch", &payload, 70, created_by).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    state.crawl_notify().notify_one();
    let pool2 = pool.clone();
    if !cfg!(test) {
        tokio::spawn(async move {
            if let Err(e) = tasks::run_due_task_jobs(&pool2, 1).await {
                tracing::warn!(error = %e, "task queue run_due_task_jobs failed (route trigger)");
            }
        });
    }

    (StatusCode::ACCEPTED, Json(EnqueueTaskOut { task_id })).into_response()
}
