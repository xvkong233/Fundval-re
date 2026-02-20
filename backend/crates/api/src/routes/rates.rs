use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::dbfmt;
use crate::routes::auth;
use crate::routes::errors;
use crate::rates::treasury_3m;
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

#[derive(Debug, Serialize)]
pub struct AdminSyncRiskFreeResponse {
    pub ok: bool,
    pub tenor: String,
    pub rate_date: String,
    pub rate_percent: String,
    pub source: String,
    pub fetched_at: String,
}

pub async fn admin_sync_risk_free(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let user_id_i64 = match user_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return auth::invalid_token_response(),
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

    let row = sqlx::query(
        r#"
        SELECT
          CASE WHEN is_superuser THEN 1 ELSE 0 END as is_superuser,
          CASE WHEN is_staff THEN 1 ELSE 0 END as is_staff
        FROM auth_user
        WHERE id = $1
        "#,
    )
        .bind(user_id_i64)
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
        return auth::invalid_token_response();
    };

    let is_admin = row.get::<i64, _>("is_superuser") != 0 || row.get::<i64, _>("is_staff") != 0;
    if !is_admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "需要管理员权限" })),
        )
            .into_response();
    }

    let url = std::env::var("RISK_FREE_CHINABOND_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| treasury_3m::DEFAULT_CHINABOND_CURVE_URL.to_string());

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Fundval-re rates/1.0")
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

    let got = match treasury_3m::fetch_chinabond_3m(&client, &url).await {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_GATEWAY, Json(json!({ "error": e }))).into_response(),
    };

    if let Err(e) = treasury_3m::upsert_risk_free_rate_3m(pool, &got, "chinabond").await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(AdminSyncRiskFreeResponse {
            ok: true,
            tenor: "3M".to_string(),
            rate_date: got.rate_date,
            rate_percent: format!("{:.4}", got.rate_percent),
            source: "chinabond".to_string(),
            fetched_at: Utc::now().to_rfc3339_opts(SecondsFormat::AutoSi, false),
        }),
    )
        .into_response()
}
