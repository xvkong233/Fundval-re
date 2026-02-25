use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::json;

use crate::routes::{auth, errors};
use crate::sources;
use crate::state::AppState;
use crate::tasks;

#[derive(Debug, Deserialize)]
pub struct TrainForecastModelBody {
    pub source: Option<String>,
    pub model_name: Option<String>,
    pub horizon: Option<i64>,
    pub lag_k: Option<i64>,
    pub priority: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct EnqueueTaskOut {
    pub task_id: String,
}

pub async fn train_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<TrainForecastModelBody>,
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
    let source_name = match sources::normalize_source_name(source_raw) {
        Some(v) => v,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": format!("unknown source: {source_raw}") })),
            )
                .into_response();
        }
    };

    let model_name = body
        .model_name
        .as_deref()
        .unwrap_or("global_ols_v1")
        .trim()
        .to_string();
    if model_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "model_name 不能为空" })),
        )
            .into_response();
    }

    let horizon = body.horizon.unwrap_or(60).clamp(1, 5000);
    let lag_k = body.lag_k.unwrap_or(20).clamp(1, 400);
    let priority = body.priority.unwrap_or(200).clamp(1, 1000);

    let payload = json!({
      "source": source_name,
      "model_name": model_name,
      "horizon": horizon,
      "lag_k": lag_k
    });

    let task_id = match tasks::enqueue_task_job(pool, "forecast_model_train", &payload, priority, created_by).await {
        Ok(id) => id,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    // 立即唤醒后台 worker，避免等待 tick。
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
