use axum::{extract::Query, http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct FundListQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub search: Option<String>,
    pub fund_type: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FundItem {
    pub id: String,
    pub fund_code: String,
    pub fund_name: String,
    pub fund_type: Option<String>,
    pub latest_nav: Option<String>,
    pub latest_nav_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct FundListResponse {
    pub count: i64,
    pub results: Vec<FundItem>,
}

pub async fn list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(q): Query<FundListQuery>,
) -> axum::response::Response {
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

    let page_size = q.page_size.unwrap_or(20).clamp(1, 200);
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * page_size;

    let mut where_sql = String::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(search) = q.search.as_ref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
    }) {
        where_sql.push_str(" WHERE (fund_code ILIKE $1 OR fund_name ILIKE $1)");
        binds.push(format!("%{search}%"));
    }

    if let Some(ft) = q.fund_type.as_ref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() { None } else { Some(t.to_string()) }
    }) {
        let idx = binds.len() + 1;
        if where_sql.is_empty() {
            where_sql.push_str(&format!(" WHERE fund_type = ${idx}"));
        } else {
            where_sql.push_str(&format!(" AND fund_type = ${idx}"));
        }
        binds.push(ft);
    }

    // count
    let count_sql = format!("SELECT COUNT(*)::bigint as cnt FROM fund{where_sql}");
    let mut count_q = sqlx::query(&count_sql);
    for b in &binds {
        count_q = count_q.bind(b);
    }
    let count_row = match count_q.fetch_one(pool).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };
    let count: i64 = count_row.get::<i64, _>("cnt");

    // page data
    let list_sql = format!(
        r#"
        SELECT
          id::text as id,
          fund_code,
          fund_name,
          fund_type,
          latest_nav::text as latest_nav,
          latest_nav_date::text as latest_nav_date,
          created_at,
          updated_at
        FROM fund
        {where_sql}
        ORDER BY fund_code ASC
        LIMIT {page_size} OFFSET {offset}
        "#
    );

    let mut list_q = sqlx::query(&list_sql);
    for b in &binds {
        list_q = list_q.bind(b);
    }

    let rows = match list_q.fetch_all(pool).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let results = rows
        .into_iter()
        .map(|row| FundItem {
            id: row.get::<String, _>("id"),
            fund_code: row.get::<String, _>("fund_code"),
            fund_name: row.get::<String, _>("fund_name"),
            fund_type: row.get::<Option<String>, _>("fund_type"),
            latest_nav: row.get::<Option<String>, _>("latest_nav"),
            latest_nav_date: row.get::<Option<String>, _>("latest_nav_date"),
            created_at: format_dt(row.get::<DateTime<Utc>, _>("created_at")),
            updated_at: format_dt(row.get::<DateTime<Utc>, _>("updated_at")),
        })
        .collect::<Vec<_>>();

    (StatusCode::OK, Json(FundListResponse { count, results })).into_response()
}

pub async fn retrieve(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(fund_code): axum::extract::Path<String>,
) -> axum::response::Response {
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
          id::text as id,
          fund_code,
          fund_name,
          fund_type,
          latest_nav::text as latest_nav,
          latest_nav_date::text as latest_nav_date,
          created_at,
          updated_at
        FROM fund
        WHERE fund_code = $1
        "#,
    )
    .bind(&fund_code)
    .fetch_optional(pool)
    .await;

    let row = match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let Some(row) = row else {
        // 对齐 DRF 默认 404 响应格式
        return (StatusCode::NOT_FOUND, Json(json!({ "detail": "Not found." }))).into_response();
    };

    let item = FundItem {
        id: row.get::<String, _>("id"),
        fund_code: row.get::<String, _>("fund_code"),
        fund_name: row.get::<String, _>("fund_name"),
        fund_type: row.get::<Option<String>, _>("fund_type"),
        latest_nav: row.get::<Option<String>, _>("latest_nav"),
        latest_nav_date: row.get::<Option<String>, _>("latest_nav_date"),
        created_at: format_dt(row.get::<DateTime<Utc>, _>("created_at")),
        updated_at: format_dt(row.get::<DateTime<Utc>, _>("updated_at")),
    };

    (StatusCode::OK, Json(item)).into_response()
}

fn format_dt(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

