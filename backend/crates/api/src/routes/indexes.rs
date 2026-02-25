use axum::{Json, extract::Query, http::HeaderMap, http::StatusCode, response::IntoResponse};
use chrono::{Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::eastmoney;
use crate::index_series;
use crate::routes::auth;
use crate::routes::errors;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct IndexDailyQuery {
    pub index_code: String,
    pub source_name: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub fetch: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct IndexDailyPoint {
    pub date: String,
    pub close: String,
}

fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub async fn daily(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
    Query(q): Query<IndexDailyQuery>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({ "error": "database not configured" }))).into_response(),
        Some(p) => p,
    };

    let index_code = q.index_code.trim().to_string();
    if index_code.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "missing index_code" }))).into_response();
    }
    let source_name = q.source_name.as_deref().unwrap_or("eastmoney").trim().to_string();

    let end_date = q
        .end_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().date_naive());
    let start_date = q
        .start_date
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok())
        .unwrap_or_else(|| end_date - Duration::days(365));

    let fetch = q.fetch.unwrap_or(true);
    let out = if fetch && source_name == "eastmoney" {
        let client = match eastmoney::build_client() {
            Ok(c) => c,
            Err(e) => return errors::internal_response(&state, e),
        };
        match index_series::load_or_fetch_index_close_series(
            pool,
            &client,
            state.db_kind(),
            &index_code,
            &source_name,
            start_date,
            end_date,
            3,
        )
        .await
        {
            Ok(v) => v,
            Err(e) => return errors::internal_response(&state, e),
        }
    } else {
        match index_series::load_index_close_series(pool, &index_code, &source_name, start_date, end_date).await {
            Ok(v) => v,
            Err(e) => return errors::internal_response(&state, e),
        }
    };

    let points: Vec<IndexDailyPoint> = out
        .into_iter()
        .map(|(d, c)| IndexDailyPoint {
            date: fmt_date(d),
            close: c.to_string(),
        })
        .collect();
    Json(serde_json::json!({ "index_code": index_code, "source_name": source_name, "points": points })).into_response()
}
