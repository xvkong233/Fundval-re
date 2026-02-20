use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::eastmoney;
use crate::routes::auth;
use crate::routes::errors;
use crate::sources;
use crate::state::AppState;

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct NavHistoryListQuery {
    pub fund_code: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct NavHistoryItem {
    pub id: String,
    pub fund_code: String,
    pub fund_name: String,
    pub nav_date: String,
    pub unit_nav: String,
    pub accumulated_nav: Option<String>,
    pub daily_growth: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn list(
    axum::extract::State(state): axum::extract::State<AppState>,
    Query(q): Query<NavHistoryListQuery>,
) -> axum::response::Response {
    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let fund_code = q
        .fund_code
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let start_date = q
        .start_date
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let end_date = q
        .end_date
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

    let source_name_raw = q.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_name_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name_raw} 不存在") })),
        )
            .into_response();
    };

    let mut sql = String::from(
        r#"
        SELECT
          h.id::text as id,
          f.fund_code,
          f.fund_name,
          h.nav_date::text as nav_date,
          h.unit_nav::text as unit_nav,
          h.accumulated_nav::text as accumulated_nav,
          h.daily_growth::text as daily_growth,
          h.created_at,
          h.updated_at
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE 1=1
        "#,
    );

    let mut bind_idx = 1;
    sql.push_str(&format!(" AND h.source_name = ${bind_idx}"));
    bind_idx += 1;
    if fund_code.is_some() {
        sql.push_str(&format!(" AND f.fund_code = ${bind_idx}"));
        bind_idx += 1;
    }
    if start_date.is_some() {
        sql.push_str(&format!(" AND h.nav_date >= ${bind_idx}"));
        bind_idx += 1;
    }
    if end_date.is_some() {
        sql.push_str(&format!(" AND h.nav_date <= ${bind_idx}"));
    }
    sql.push_str(" ORDER BY h.nav_date DESC");

    let mut query = sqlx::query(&sql);
    query = query.bind(source_name);
    if let Some(code) = fund_code {
        query = query.bind(code);
    }
    if let Some(sd) = start_date {
        query = query.bind(sd);
    }
    if let Some(ed) = end_date {
        query = query.bind(ed);
    }

    let rows = match query.fetch_all(pool).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let out = rows.into_iter().map(row_to_item).collect::<Vec<_>>();
    (StatusCode::OK, Json(out)).into_response()
}

pub async fn retrieve(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> axum::response::Response {
    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let row = sqlx::query(
        r#"
        SELECT
          h.id::text as id,
          f.fund_code,
          f.fund_name,
          h.nav_date::text as nav_date,
          h.unit_nav::text as unit_nav,
          h.accumulated_nav::text as accumulated_nav,
          h.daily_growth::text as daily_growth,
          h.created_at,
          h.updated_at
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE h.id = $1
        "#,
    )
    .bind(id)
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
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    };

    (StatusCode::OK, Json(row_to_item(row))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct BatchQueryRequest {
    pub fund_codes: Option<Vec<String>>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub nav_date: Option<String>,
    pub source: Option<String>,
}

pub async fn batch_query(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<BatchQueryRequest>,
) -> axum::response::Response {
    let fund_codes = body.fund_codes.unwrap_or_default();
    if fund_codes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "缺少 fund_codes 参数" })),
        )
            .into_response();
    }

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let start_date = body
        .start_date
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let end_date = body
        .end_date
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let nav_date = body
        .nav_date
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());

    let source_name_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_name_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name_raw} 不存在") })),
        )
            .into_response();
    };

    let mut sql = String::from(
        r#"
        SELECT
          h.id::text as id,
          f.fund_code,
          f.fund_name,
          h.nav_date::text as nav_date,
          h.unit_nav::text as unit_nav,
          h.accumulated_nav::text as accumulated_nav,
          h.daily_growth::text as daily_growth,
          h.created_at,
          h.updated_at
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = ANY($1::text[])
          AND h.source_name = $2
        "#,
    );

    let mut bind_idx = 3;
    if nav_date.is_some() {
        sql.push_str(&format!(" AND h.nav_date = ${bind_idx}"));
    } else {
        if start_date.is_some() {
            sql.push_str(&format!(" AND h.nav_date >= ${bind_idx}"));
            bind_idx += 1;
        }
        if end_date.is_some() {
            sql.push_str(&format!(" AND h.nav_date <= ${bind_idx}"));
        }
    }
    sql.push_str(" ORDER BY f.fund_code ASC, h.nav_date DESC");

    let mut query = sqlx::query(&sql).bind(&fund_codes).bind(source_name);
    if let Some(nd) = nav_date {
        query = query.bind(nd);
    } else {
        if let Some(sd) = start_date {
            query = query.bind(sd);
        }
        if let Some(ed) = end_date {
            query = query.bind(ed);
        }
    }

    let rows = match query.fetch_all(pool).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut grouped: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for code in &fund_codes {
        grouped.insert(code.clone(), json!([]));
    }
    for row in rows {
        let code: String = row.get("fund_code");
        let entry = grouped.entry(code).or_insert_with(|| json!([]));
        if let Some(arr) = entry.as_array_mut() {
            arr.push(serde_json::to_value(row_to_item(row)).unwrap_or(json!({})));
        }
    }

    (StatusCode::OK, Json(serde_json::Value::Object(grouped))).into_response()
}

#[derive(Debug, Deserialize)]
pub struct SyncRequest {
    pub fund_codes: Option<Vec<String>>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub source: Option<String>,
}

pub async fn sync(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<SyncRequest>,
) -> axum::response::Response {
    let fund_codes = body.fund_codes.unwrap_or_default();
    if fund_codes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "缺少 fund_codes 参数" })),
        )
            .into_response();
    }

    // 分级鉴权：>15 需要 is_staff
    if fund_codes.len() > 15 {
        let is_staff = match maybe_is_staff(&state, &headers).await {
            Ok(v) => v.unwrap_or(false),
            Err(resp) => return resp,
        };
        if !is_staff {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "同步超过 15 个基金需要管理员权限" })),
            )
                .into_response();
        }
    }

    let start_date = body
        .start_date
        .as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());
    let end_date = body
        .end_date
        .as_ref()
        .and_then(|s| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").ok());

    let source_name_raw = body.source.as_deref().unwrap_or(sources::SOURCE_TIANTIAN);
    let Some(source_name) = sources::normalize_source_name(source_name_raw) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("数据源 {source_name_raw} 不存在") })),
        )
            .into_response();
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let client = match eastmoney::build_client() {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("创建 HTTP 客户端失败: {e}") })),
            )
                .into_response();
        }
    };
    let tushare_token = state
        .config()
        .get_string("tushare_token")
        .unwrap_or_default();

    let mut results: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
    for code in fund_codes {
        match sync_one(
            pool,
            &client,
            source_name,
            &code,
            start_date,
            end_date,
            &tushare_token,
        )
        .await
        {
            Ok(count) => {
                results.insert(code, json!({ "success": true, "count": count }));
            }
            Err(e) => {
                results.insert(code, json!({ "success": false, "error": e }));
            }
        }
    }

    (StatusCode::OK, Json(serde_json::Value::Object(results))).into_response()
}

async fn maybe_is_staff(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<Option<bool>, axum::response::Response> {
    let Some(auth_header) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Ok(None);
    };

    let auth_str = auth_header
        .to_str()
        .map_err(|_| auth::invalid_token_response())?;

    // 非 Bearer 视为“未提供认证”，留给调用方按业务规则处理（这里用于 >15 的分级鉴权）。
    if !auth_str.starts_with("Bearer ") {
        return Ok(None);
    }

    // Bearer 但 token 无效时：应与 DRF 行为一致，直接返回 401（而不是降级为 403）。
    let user_id = auth::authenticate(state, headers)?;
    let user_id_i64 = user_id
        .parse::<i64>()
        .map_err(|_| auth::invalid_token_response())?;

    let Some(pool) = state.pool() else {
        return Ok(None);
    };

    let row = sqlx::query("SELECT is_staff FROM auth_user WHERE id = $1")
        .bind(user_id_i64)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "nav_history.maybe_is_staff query failed");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "服务器内部错误" })),
            )
                .into_response()
        })?;

    Ok(row.map(|r| r.get::<bool, _>("is_staff")))
}

async fn sync_one(
    pool: &sqlx::PgPool,
    client: &reqwest::Client,
    source_name: &str,
    fund_code: &str,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    tushare_token: &str,
) -> Result<i64, String> {
    let fund_row = sqlx::query("SELECT id FROM fund WHERE fund_code = $1")
        .bind(fund_code)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

    let Some(fund_row) = fund_row else {
        return Err(format!("基金不存在：{fund_code}"));
    };
    let fund_id: Uuid = fund_row.get("id");

    let mut effective_start = start_date;
    if effective_start.is_none() {
        let last = sqlx::query(
            "SELECT nav_date FROM fund_nav_history WHERE source_name = $1 AND fund_id = $2 ORDER BY nav_date DESC LIMIT 1",
        )
        .bind(source_name)
        .bind(fund_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        if let Some(last) = last {
            let d: NaiveDate = last.get("nav_date");
            effective_start = Some(d.succ_opt().unwrap_or(d));
        }
    }
    let effective_end = end_date.unwrap_or_else(|| Utc::now().date_naive());

    let data = match source_name {
        sources::SOURCE_TIANTIAN => {
            eastmoney::fetch_nav_history(client, fund_code, effective_start, Some(effective_end))
                .await?
        }
        sources::SOURCE_DANJUAN => {
            sources::danjuan::fetch_nav_history(
                client,
                fund_code,
                effective_start,
                Some(effective_end),
            )
            .await?
        }
        sources::SOURCE_THS => {
            let all = sources::ths::fetch_nav_series(client, fund_code).await?;
            all.into_iter()
                .filter(|r| {
                    if let Some(sd) = effective_start
                        && r.nav_date < sd
                    {
                        return false;
                    }
                    if r.nav_date > effective_end {
                        return false;
                    }
                    true
                })
                .collect::<Vec<_>>()
        }
        sources::SOURCE_TUSHARE => {
            if tushare_token.trim().is_empty() {
                return Err("tushare token 未配置（请在“设置”页面填写）".to_string());
            }
            sources::tushare::fetch_nav_history(
                client,
                tushare_token,
                fund_code,
                effective_start,
                Some(effective_end),
            )
            .await?
        }
        _ => return Err(format!("数据源 {source_name} 不存在")),
    };
    if data.is_empty() {
        return Ok(0);
    }

    let mut inserted_count: i64 = 0;
    for item in data {
        let inserted: bool = sqlx::query(
            r#"
            INSERT INTO fund_nav_history (id, source_name, fund_id, nav_date, unit_nav, accumulated_nav, daily_growth, created_at, updated_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,NOW(),NOW())
            ON CONFLICT (source_name, fund_id, nav_date) DO UPDATE
            SET unit_nav = EXCLUDED.unit_nav,
                accumulated_nav = EXCLUDED.accumulated_nav,
                daily_growth = EXCLUDED.daily_growth,
                updated_at = NOW()
            RETURNING (xmax = 0) as inserted
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(source_name)
        .bind(fund_id)
        .bind(item.nav_date)
        .bind(item.unit_nav)
        .bind(item.accumulated_nav)
        .bind(item.daily_growth)
        .fetch_one(pool)
        .await
        .map_err(|e| e.to_string())?
        .get::<bool, _>("inserted");

        if inserted {
            inserted_count += 1;
        }
    }

    Ok(inserted_count)
}

fn row_to_item(row: sqlx::postgres::PgRow) -> NavHistoryItem {
    NavHistoryItem {
        id: row.get::<String, _>("id"),
        fund_code: row.get::<String, _>("fund_code"),
        fund_name: row.get::<String, _>("fund_name"),
        nav_date: row.get::<String, _>("nav_date"),
        unit_nav: row.get::<String, _>("unit_nav"),
        accumulated_nav: row.get::<Option<String>, _>("accumulated_nav"),
        daily_growth: row.get::<Option<String>, _>("daily_growth"),
        created_at: format_dt(row.get::<DateTime<Utc>, _>("created_at")),
        updated_at: format_dt(row.get::<DateTime<Utc>, _>("updated_at")),
    }
}

fn format_dt(dt: DateTime<Utc>) -> String {
    // 对齐 DRF 常见输出（UTC 使用 Z 后缀）
    dt.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}
