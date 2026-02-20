use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::dbfmt;
use crate::routes::auth;
use crate::routes::errors;
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct RiskFreeQuery {
    pub tenor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RiskFreeResponse {
    pub tenor: String,
    pub rate_date: String,
    pub rate_percent: String,
    pub source: String,
    pub fetched_at: String,
}

pub async fn risk_free(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<RiskFreeQuery>,
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

    let tenor = q
        .tenor
        .as_deref()
        .unwrap_or("3M")
        .trim()
        .to_string();
    if tenor.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "缺少 tenor 参数" })),
        )
            .into_response();
    }

    let row = sqlx::query(
        r#"
        SELECT
          CAST(rate_date AS TEXT) as rate_date,
          CAST(rate AS TEXT) as rate,
          source,
          CAST(fetched_at AS TEXT) as fetched_at
        FROM risk_free_rate_daily
        WHERE tenor = $1
        ORDER BY rate_date DESC, fetched_at DESC
        LIMIT 1
        "#,
    )
    .bind(&tenor)
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
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "暂无无风险利率缓存" })),
        )
            .into_response();
    };

    (
        StatusCode::OK,
        Json(RiskFreeResponse {
            tenor,
            rate_date: row.get::<String, _>("rate_date"),
            rate_percent: row.get::<String, _>("rate"),
            source: row.get::<String, _>("source"),
            fetched_at: dbfmt::datetime_to_rfc3339(&row.get::<String, _>("fetched_at")),
        }),
    )
        .into_response()
}

