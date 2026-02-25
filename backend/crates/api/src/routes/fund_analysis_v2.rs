use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;

use crate::routes::{auth, errors};
use crate::sources;
use crate::state::AppState;
use crate::tasks;

#[derive(Debug, Deserialize, Default)]
pub struct FundAnalysisV2Query {
    pub source: Option<String>,
    pub profile: Option<String>,
    pub refer_index_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FundAnalysisV2Out {
    pub fund_code: String,
    pub source: String,
    pub profile: String,
    pub refer_index_code: String,
    pub as_of_date: Option<String>,
    pub result: Value,
    pub last_task_id: Option<String>,
    pub updated_at: String,
    pub missing: bool,
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

pub async fn retrieve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(fund_code): Path<String>,
    Query(q): Query<FundAnalysisV2Query>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

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

    let source_raw = q.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let source = match normalize_source_or_404(source_raw) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let profile = q.profile.as_deref().unwrap_or("default").trim();
    let refer_index_code = q
        .refer_index_code
        .as_deref()
        .unwrap_or("1.000001")
        .trim();

    let row = sqlx::query(
        r#"
        SELECT
          fund_code,
          source,
          profile,
          refer_index_code,
          as_of_date,
          result_json,
          CAST(last_task_id AS TEXT) as last_task_id,
          CAST(updated_at AS TEXT) as updated_at
        FROM fund_analysis_snapshot
        WHERE fund_code = $1 AND source = $2 AND profile = $3 AND refer_index_code = $4
        LIMIT 1
        "#,
    )
    .bind(fund_code.trim())
    .bind(source)
    .bind(profile)
    .bind(refer_index_code)
    .fetch_optional(pool)
    .await;

    let row = match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let Some(row) = row else {
        return (
            StatusCode::OK,
            Json(FundAnalysisV2Out {
                fund_code: fund_code.trim().to_string(),
                source: source.to_string(),
                profile: profile.to_string(),
                refer_index_code: refer_index_code.to_string(),
                as_of_date: None,
                result: json!({}),
                last_task_id: None,
                updated_at: chrono::Utc::now().to_rfc3339(),
                missing: true,
            }),
        )
            .into_response();
    };

    let result_json: String = row.get("result_json");
    let result: Value = match serde_json::from_str(&result_json) {
        Ok(v) => v,
        Err(_) => json!({ "raw": result_json }),
    };

    (
        StatusCode::OK,
        Json(FundAnalysisV2Out {
            fund_code: row.get("fund_code"),
            source: row.get("source"),
            profile: row.get("profile"),
            refer_index_code: row
                .try_get::<Option<String>, _>("refer_index_code")
                .ok()
                .flatten()
                .unwrap_or_else(|| "1.000001".to_string()),
            as_of_date: row.try_get::<Option<String>, _>("as_of_date").ok().flatten(),
            result,
            last_task_id: row.try_get::<Option<String>, _>("last_task_id").ok().flatten(),
            updated_at: row.get("updated_at"),
            missing: false,
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct FundAnalysisV2ComputeBody {
    pub source: Option<String>,
    pub profile: Option<String>,
    pub windows: Option<Vec<i64>>,
    pub risk_free_annual: Option<f64>,
    pub grid_step_pct: Option<f64>,
    pub every_n: Option<i64>,
    pub amount: Option<f64>,
    pub refer_index_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EnqueueTaskOut {
    pub task_id: String,
}

fn quant_base_url(state: &AppState) -> String {
    state
        .config()
        .get_string("quant_service_url")
        .unwrap_or_else(|| "http://localhost:8002".to_string())
}

pub async fn compute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(fund_code): Path<String>,
    Json(body): Json<FundAnalysisV2ComputeBody>,
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
    let source = match normalize_source_or_404(source_raw) {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    let profile = body.profile.as_deref().unwrap_or("default").trim().to_string();

    let mut windows = body.windows.unwrap_or_else(|| vec![60]);
    windows.retain(|w| *w >= 2 && *w <= 5000);
    if windows.is_empty() {
        windows = vec![60];
    }
    windows.truncate(8);

    let payload = json!({
      "fund_code": fund_code.trim(),
      "source": source,
      "profile": profile,
      "windows": windows,
      "risk_free_annual": body.risk_free_annual.unwrap_or(0.0),
      "grid_step_pct": body.grid_step_pct.unwrap_or(0.02),
      "every_n": body.every_n.unwrap_or(20),
      "amount": body.amount.unwrap_or(1.0),
      "refer_index_code": body.refer_index_code.as_deref().unwrap_or("1.000001").trim(),
      "quant_service_url": quant_base_url(&state),
    });

    let task_id = match tasks::enqueue_task_job(pool, "fund_analysis_v2_compute", &payload, 80, created_by).await {
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
