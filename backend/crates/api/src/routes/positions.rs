use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use axum::{Json, body::Bytes, extract::Query, http::StatusCode, response::IntoResponse};
use chrono::{DateTime, Duration, NaiveDate, SecondsFormat, Utc};
use rust_decimal::{Decimal, RoundingStrategy, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

use crate::dbfmt;
use crate::position_history;
use crate::routes::auth;
use crate::routes::errors;
use crate::state::AppState;

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize)]
struct MessageResponse {
    message: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct PositionsListQuery {
    pub account: Option<Uuid>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PositionHistoryQuery {
    pub account_id: Option<String>,
    pub days: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct FundInfo {
    pub fund_code: String,
    pub fund_name: String,
    pub fund_type: Option<String>,
    pub latest_nav: Option<String>,
    pub latest_nav_date: Option<String>,
    pub estimate_nav: Option<String>,
    pub estimate_growth: Option<String>,
    pub estimate_time: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct PositionResponse {
    pub id: String,
    pub account: String,
    pub account_name: String,
    pub fund_code: String,
    pub fund_name: String,
    pub fund_type: Option<String>,
    pub fund: FundInfo,
    pub holding_share: String,
    pub holding_cost: String,
    pub holding_nav: String,
    pub pnl: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct PositionHistoryPointResponse {
    pub date: String,
    pub value: f64,
    pub cost: f64,
}

pub async fn list(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<PositionsListQuery>,
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
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let mut sql = String::from(
        r#"
        SELECT
          CAST(p.id AS TEXT) as id,
          CAST(p.account_id AS TEXT) as account,
          a.name as account_name,
          f.fund_code,
          f.fund_name,
          f.fund_type,
          CAST(f.latest_nav AS TEXT) as latest_nav,
          CAST(f.latest_nav_date AS TEXT) as latest_nav_date,
          CAST(f.estimate_nav AS TEXT) as estimate_nav,
          CAST(f.estimate_growth AS TEXT) as estimate_growth,
          CAST(f.estimate_time AS TEXT) as estimate_time,
          CAST(p.holding_share AS TEXT) as holding_share,
          CAST(p.holding_cost AS TEXT) as holding_cost,
          CAST(p.holding_nav AS TEXT) as holding_nav,
          CAST(p.updated_at AS TEXT) as updated_at
        FROM position p
        JOIN account a ON a.id = p.account_id
        JOIN fund f ON f.id = p.fund_id
        WHERE a.user_id = $1
        "#,
    );

    if q.account.is_some() {
        sql.push_str(" AND CAST(p.account_id AS TEXT) = $2");
    }
    sql.push_str(" ORDER BY f.fund_code ASC");

    let mut query = sqlx::query(&sql).bind(user_id_i64);
    if let Some(account_id) = q.account {
        query = query.bind(account_id.to_string());
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

    let mut out: Vec<PositionResponse> = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(position_from_row(row));
    }

    (StatusCode::OK, Json(out)).into_response()
}

pub async fn history(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<PositionHistoryQuery>,
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
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let Some(account_id_raw) = q
        .account_id
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "缺少 account_id 参数".to_string(),
            }),
        )
            .into_response();
    };

    let account_id = match Uuid::parse_str(&account_id_raw) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "detail": "Not found." })),
            )
                .into_response();
        }
    };
    let account_id_str = account_id.to_string();

    let days = q
        .days
        .as_ref()
        .and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(30)
        .max(0);

    // 验证账户归属 + 只支持子账户
    let row = match sqlx::query(
        "SELECT CAST(parent_id AS TEXT) as parent_id FROM account WHERE CAST(id AS TEXT) = $1 AND user_id = $2",
    )
        .bind(&account_id_str)
        .bind(user_id_i64)
        .fetch_optional(pool)
        .await
    {
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

    let parent_id: Option<String> = row.get("parent_id");
    if parent_id.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "暂不支持父账户历史查询".to_string(),
            }),
        )
            .into_response();
    }

    let end_date = Utc::now().date_naive();
    let start_date = end_date - Duration::days(days);

    // 获取所有操作流水（包含查询范围之前的操作）
    let op_rows = match sqlx::query(
        r#"
        SELECT
          CAST(fund_id AS TEXT) as fund_id,
          operation_type,
          CAST(operation_date AS TEXT) as operation_date,
          CAST(amount AS TEXT) as amount,
          CAST(share AS TEXT) as share
        FROM position_operation
        WHERE CAST(account_id AS TEXT) = $1 AND operation_date <= $2
        ORDER BY operation_date ASC, created_at ASC
        "#,
    )
    .bind(&account_id_str)
    .bind(end_date.to_string())
    .fetch_all(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    if op_rows.is_empty() {
        return (
            StatusCode::OK,
            Json(Vec::<PositionHistoryPointResponse>::new()),
        )
            .into_response();
    }

    let mut ops = Vec::with_capacity(op_rows.len());
    let mut fund_ids_set: HashSet<Uuid> = HashSet::new();

    for row in op_rows {
        let fund_id_raw: String = row.get("fund_id");
        let fund_id = match Uuid::parse_str(fund_id_raw.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        fund_ids_set.insert(fund_id);

        let op_type_raw: String = row.get("operation_type");
        let operation_type = match op_type_raw.as_str() {
            "BUY" => position_history::OperationType::Buy,
            _ => position_history::OperationType::Sell,
        };

        ops.push(position_history::Operation {
            fund_id,
            operation_type,
            operation_date: match NaiveDate::parse_from_str(
                row.get::<String, _>("operation_date").trim(),
                "%Y-%m-%d",
            ) {
                Ok(v) => v,
                Err(_) => continue,
            },
            amount: parse_decimal(row.get::<String, _>("amount")),
            share: parse_decimal(row.get::<String, _>("share")),
        });
    }

    let fund_ids: Vec<Uuid> = fund_ids_set.into_iter().collect();
    let fund_ids_text: Vec<String> = fund_ids.iter().map(|u| u.to_string()).collect();

    // 查询每日净值（范围内）
    let nav_rows = {
        let mut sql = String::from(
            r#"
            SELECT
              CAST(fund_id AS TEXT) as fund_id,
              CAST(nav_date AS TEXT) as nav_date,
              CAST(unit_nav AS TEXT) as unit_nav
            FROM fund_nav_history
            WHERE source_name = 'tiantian' AND CAST(fund_id AS TEXT) IN (
            "#,
        );
        for i in 0..fund_ids_text.len() {
            if i > 0 {
                sql.push(',');
            }
            sql.push('$');
            sql.push_str(&(i + 1).to_string());
        }
        sql.push_str(") AND nav_date >= $");
        sql.push_str(&(fund_ids_text.len() + 1).to_string());
        sql.push_str(" AND nav_date <= $");
        sql.push_str(&(fund_ids_text.len() + 2).to_string());

        let mut q = sqlx::query(&sql);
        for fid in &fund_ids_text {
            q = q.bind(fid);
        }
        q = q.bind(start_date.to_string()).bind(end_date.to_string());
        q.fetch_all(pool).await
    };

    let nav_rows = match nav_rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut nav_records = Vec::with_capacity(nav_rows.len());
    for row in nav_rows {
        let fund_id_raw: String = row.get("fund_id");
        let fund_id = match Uuid::parse_str(fund_id_raw.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let nav_date =
            match NaiveDate::parse_from_str(row.get::<String, _>("nav_date").trim(), "%Y-%m-%d") {
                Ok(v) => v,
                Err(_) => continue,
            };
        nav_records.push(position_history::NavRecord {
            fund_id,
            nav_date,
            unit_nav: parse_decimal(row.get::<String, _>("unit_nav")),
        });
    }

    // Fund.latest_nav 作为 fallback（与 Django 服务一致）
    let latest_rows = {
        let mut sql = String::from(
            r#"
            SELECT CAST(id AS TEXT) as fund_id, CAST(latest_nav AS TEXT) as latest_nav
            FROM fund
            WHERE CAST(id AS TEXT) IN (
            "#,
        );
        for i in 0..fund_ids_text.len() {
            if i > 0 {
                sql.push(',');
            }
            sql.push('$');
            sql.push_str(&(i + 1).to_string());
        }
        sql.push(')');

        let mut q = sqlx::query(&sql);
        for fid in &fund_ids_text {
            q = q.bind(fid);
        }
        q.fetch_all(pool).await
    };

    let latest_rows = match latest_rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut latest_nav_by_fund: HashMap<Uuid, Decimal> = HashMap::new();
    for row in latest_rows {
        let fund_id_raw: String = row.get("fund_id");
        let fund_id = match Uuid::parse_str(fund_id_raw.trim()) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let latest_nav_text: Option<String> = row.get("latest_nav");
        let Some(latest_nav_text) = latest_nav_text else {
            continue;
        };
        let latest_nav_text = latest_nav_text.trim().to_string();
        if latest_nav_text.is_empty() {
            continue;
        }
        latest_nav_by_fund.insert(fund_id, parse_decimal(latest_nav_text));
    }

    let points = position_history::calculate_account_history(
        &ops,
        &nav_records,
        &latest_nav_by_fund,
        start_date,
        end_date,
    );

    let out = points
        .into_iter()
        .map(|p| PositionHistoryPointResponse {
            date: p.date.to_string(),
            value: p.value.to_f64().unwrap_or(0.0),
            cost: p.cost.to_f64().unwrap_or(0.0),
        })
        .collect::<Vec<_>>();

    (StatusCode::OK, Json(out)).into_response()
}

pub async fn retrieve(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
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
          CAST(p.id AS TEXT) as id,
          CAST(p.account_id AS TEXT) as account,
          a.name as account_name,
          f.fund_code,
          f.fund_name,
          f.fund_type,
          CAST(f.latest_nav AS TEXT) as latest_nav,
          CAST(f.latest_nav_date AS TEXT) as latest_nav_date,
          CAST(f.estimate_nav AS TEXT) as estimate_nav,
          CAST(f.estimate_growth AS TEXT) as estimate_growth,
          CAST(f.estimate_time AS TEXT) as estimate_time,
          CAST(p.holding_share AS TEXT) as holding_share,
          CAST(p.holding_cost AS TEXT) as holding_cost,
          CAST(p.holding_nav AS TEXT) as holding_nav,
          CAST(p.updated_at AS TEXT) as updated_at
        FROM position p
        JOIN account a ON a.id = p.account_id
        JOIN fund f ON f.id = p.fund_id
        WHERE CAST(p.id AS TEXT) = $1 AND a.user_id = $2
        "#,
    )
    .bind(id.to_string())
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
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    };

    (StatusCode::OK, Json(position_from_row(row))).into_response()
}

pub async fn recalculate(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    let body: RecalculateRequest = if body.is_empty() {
        RecalculateRequest::default()
    } else {
        match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Invalid JSON" })),
                )
                    .into_response();
            }
        }
    };
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
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    // admin only
    let is_admin = match sqlx::query("SELECT is_superuser FROM auth_user WHERE id = $1")
        .bind(user_id_i64)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(row)) => row
            .try_get::<bool, _>("is_superuser")
            .unwrap_or_else(|_| row.try_get::<i64, _>("is_superuser").unwrap_or(0) != 0),
        Ok(None) => false,
        Err(_) => false,
    };

    if !is_admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "detail": "You do not have permission to perform this action." })),
        )
            .into_response();
    }

    if let Err(e) = recalculate_all_positions(pool, body.account_id).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response();
    }

    (
        StatusCode::OK,
        Json(MessageResponse {
            message: "重算完成",
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize, Default)]
pub struct RecalculateRequest {
    pub account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OperationsListQuery {
    pub account: Option<Uuid>,
    pub fund_code: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OperationCreateRequest {
    pub account: Uuid,
    pub fund_code: String,
    pub operation_type: String,
    pub operation_date: String,
    pub before_15: bool,
    pub amount: Value,
    pub share: Value,
    pub nav: Value,
}

#[derive(Debug, Serialize, Clone)]
pub struct OperationResponse {
    pub id: String,
    pub account: String,
    pub account_name: String,
    pub fund: String,
    pub fund_name: String,
    pub operation_type: String,
    pub operation_date: String,
    pub before_15: bool,
    pub amount: String,
    pub share: String,
    pub nav: String,
    pub created_at: String,
}

pub async fn operations_list(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Query(q): Query<OperationsListQuery>,
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
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

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

    let mut sql = String::from(
        r#"
        SELECT
          CAST(o.id AS TEXT) as id,
          CAST(o.account_id AS TEXT) as account,
          a.name as account_name,
          CAST(o.fund_id AS TEXT) as fund,
          f.fund_name,
          o.operation_type,
          CAST(o.operation_date AS TEXT) as operation_date,
          o.before_15,
          CAST(o.amount AS TEXT) as amount,
          CAST(o.share AS TEXT) as share,
          CAST(o.nav AS TEXT) as nav,
          CAST(o.created_at AS TEXT) as created_at
        FROM position_operation o
        JOIN account a ON a.id = o.account_id
        JOIN fund f ON f.id = o.fund_id
        "#,
    );

    // filter by user unless staff
    if !is_staff {
        sql.push_str(" WHERE a.user_id = $1");
    } else {
        sql.push_str(" WHERE 1=1");
    }

    let mut bind_idx = if !is_staff { 2 } else { 1 };
    if q.account.is_some() {
        sql.push_str(&format!(" AND CAST(o.account_id AS TEXT) = ${bind_idx}"));
        bind_idx += 1;
    }
    if q.fund_code
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .is_some()
    {
        sql.push_str(&format!(" AND f.fund_code = ${bind_idx}"));
    }
    sql.push_str(" ORDER BY o.operation_date ASC, o.created_at ASC");

    let mut query = sqlx::query(&sql);
    if !is_staff {
        query = query.bind(user_id_i64);
    }
    if let Some(account_id) = q.account {
        query = query.bind(account_id.to_string());
    }
    if let Some(fund_code) = q
        .fund_code
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        query = query.bind(fund_code);
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

    let mut out: Vec<OperationResponse> = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(OperationResponse {
            id: row.get::<String, _>("id"),
            account: row.get::<String, _>("account"),
            account_name: row.get::<String, _>("account_name"),
            fund: row.get::<String, _>("fund"),
            fund_name: row.get::<String, _>("fund_name"),
            operation_type: row.get::<String, _>("operation_type"),
            operation_date: row.get::<String, _>("operation_date"),
            before_15: row.get::<bool, _>("before_15"),
            amount: fmt_decimal_fixed(parse_decimal(row.get::<String, _>("amount")), 2),
            share: fmt_decimal_fixed(parse_decimal(row.get::<String, _>("share")), 4),
            nav: fmt_decimal_fixed(parse_decimal(row.get::<String, _>("nav")), 4),
            created_at: dbfmt::datetime_to_rfc3339(&row.get::<String, _>("created_at")),
        });
    }

    (StatusCode::OK, Json(out)).into_response()
}

pub async fn operations_create(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<OperationCreateRequest>,
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
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    fn field_error(field: &'static str, message: impl Into<String>) -> axum::response::Response {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ field: [message.into()] })),
        )
            .into_response()
    }

    #[allow(clippy::result_large_err)]
    fn parse_decimal_input(
        field: &'static str,
        value: &Value,
    ) -> Result<Decimal, axum::response::Response> {
        let raw = match value {
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            _ => return Err(field_error(field, "A valid number is required.")),
        };
        if raw.trim().is_empty() {
            return Err(field_error(field, "A valid number is required."));
        }
        Decimal::from_str(&raw).map_err(|_| field_error(field, "A valid number is required."))
    }

    let fund_code = body.fund_code.trim().to_string();
    if fund_code.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "fund_code": ["基金代码不能为空"] })),
        )
            .into_response();
    }

    // account 必须属于当前用户（admin/staff 可跳过）；且必须是子账户（parent_id 不能为 NULL）
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

    let account_id_str = body.account.to_string();
    let account_row = match sqlx::query(
        "SELECT user_id, CAST(parent_id AS TEXT) as parent_id, name FROM account WHERE CAST(id AS TEXT) = $1",
    )
        .bind(&account_id_str)
        .fetch_optional(pool)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };
    let Some(account_row) = account_row else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "account": ["Invalid pk - object does not exist."] })),
        )
            .into_response();
    };
    let owner_id: i64 = account_row.get("user_id");
    if !is_staff && owner_id != user_id_i64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "account": ["Invalid pk - object does not exist."] })),
        )
            .into_response();
    }
    let parent_id: Option<String> = account_row.get("parent_id");
    if parent_id.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "non_field_errors": ["持仓操作只能在子账户上进行，父账户不能进行持仓操作"] })),
        )
            .into_response();
    }
    let account_name: String = account_row.get("name");

    let fund_row = match sqlx::query(
        "SELECT CAST(id AS TEXT) as id, fund_name FROM fund WHERE fund_code = $1",
    )
    .bind(&fund_code)
    .fetch_optional(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };
    let Some(fund_row) = fund_row else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "fund_code": ["基金不存在"] })),
        )
            .into_response();
    };
    let fund_id: String = fund_row.get("id");
    let fund_name: String = fund_row.get("fund_name");

    let operation_type = body.operation_type.trim().to_string();
    if operation_type != "BUY" && operation_type != "SELL" {
        return field_error(
            "operation_type",
            format!("\"{operation_type}\" is not a valid choice."),
        );
    }
    let operation_date = match NaiveDate::parse_from_str(body.operation_date.trim(), "%Y-%m-%d") {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "operation_date": ["Date has wrong format. Use one of these formats instead: YYYY-MM-DD."] })),
            )
                .into_response();
        }
    };

    let amount = match parse_decimal_input("amount", &body.amount) {
        Ok(v) => rescale(v, 2),
        Err(resp) => return resp,
    };
    let share = match parse_decimal_input("share", &body.share) {
        Ok(v) => rescale(v, 4),
        Err(resp) => return resp,
    };
    let nav = match parse_decimal_input("nav", &body.nav) {
        Ok(v) => rescale(v, 4),
        Err(resp) => return resp,
    };

    let id = Uuid::new_v4().to_string();
    let mut tx = match pool.begin().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let sql_pg = r#"
        INSERT INTO position_operation (
          id, account_id, fund_id, operation_type, operation_date, before_15, amount, share, nav, created_at
        )
        VALUES (
          ($1)::uuid,($2)::uuid,($3)::uuid,$4,($5)::date,$6,
          ($7)::numeric,($8)::numeric,($9)::numeric,
          CURRENT_TIMESTAMP
        )
    "#;
    let sql_any = r#"
        INSERT INTO position_operation (
          id, account_id, fund_id, operation_type, operation_date, before_15, amount, share, nav, created_at
        )
        VALUES ($1,$2,$3,$4,$5,$6,CAST($7 AS NUMERIC),CAST($8 AS NUMERIC),CAST($9 AS NUMERIC),CURRENT_TIMESTAMP)
    "#;

    let r = sqlx::query(sql_pg)
        .bind(&id)
        .bind(&account_id_str)
        .bind(&fund_id)
        .bind(&operation_type)
        .bind(operation_date.to_string())
        .bind(body.before_15)
        .bind(amount.to_string())
        .bind(share.to_string())
        .bind(nav.to_string())
        .execute(&mut *tx)
        .await;

    if let Err(e) = if r.is_ok() {
        Ok(())
    } else {
        sqlx::query(sql_any)
            .bind(&id)
            .bind(&account_id_str)
            .bind(&fund_id)
            .bind(&operation_type)
            .bind(operation_date.to_string())
            .bind(body.before_15)
            .bind(amount.to_string())
            .bind(share.to_string())
            .bind(nav.to_string())
            .execute(&mut *tx)
            .await
            .map(|_| ())
    } {
        let _ = tx.rollback().await;
        return (
            StatusCode::BAD_REQUEST,
            errors::masked_json(&state, "创建操作失败", e),
        )
            .into_response();
    }

    if let Err(e) = recalculate_position(&mut tx, &account_id_str, &fund_id).await {
        let _ = tx.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response();
    }

    if tx.commit().await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "transaction commit failed" })),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(OperationResponse {
            id: id.clone(),
            account: account_id_str,
            account_name,
            fund: fund_id.clone(),
            fund_name,
            operation_type,
            operation_date: operation_date.to_string(),
            before_15: body.before_15,
            amount: fmt_decimal_fixed(amount, 2),
            share: fmt_decimal_fixed(share, 4),
            nav: fmt_decimal_fixed(nav, 4),
            created_at: format_dt(Utc::now()),
        }),
    )
        .into_response()
}

pub async fn operations_retrieve(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
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
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

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

    let row = sqlx::query(
        r#"
        SELECT
          CAST(o.id AS TEXT) as id,
          CAST(o.account_id AS TEXT) as account,
          a.name as account_name,
          CAST(o.fund_id AS TEXT) as fund,
          f.fund_name,
          o.operation_type,
          CAST(o.operation_date AS TEXT) as operation_date,
          o.before_15,
          CAST(o.amount AS TEXT) as amount,
          CAST(o.share AS TEXT) as share,
          CAST(o.nav AS TEXT) as nav,
          CAST(o.created_at AS TEXT) as created_at
        FROM position_operation o
        JOIN account a ON a.id = o.account_id
        JOIN fund f ON f.id = o.fund_id
        WHERE CAST(o.id AS TEXT) = $1
        "#,
    )
    .bind(id.to_string())
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

    if !is_staff {
        let account_id: String = row.get("account");
        let owner = sqlx::query("SELECT user_id FROM account WHERE CAST(id AS TEXT) = $1")
            .bind(&account_id)
            .fetch_optional(pool)
            .await;
        let owner = match owner {
            Ok(Some(r)) => r.get::<i64, _>("user_id"),
            _ => -1,
        };
        if owner != user_id_i64 {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "detail": "Not found." })),
            )
                .into_response();
        }
    }

    (
        StatusCode::OK,
        Json(OperationResponse {
            id: row.get::<String, _>("id"),
            account: row.get::<String, _>("account"),
            account_name: row.get::<String, _>("account_name"),
            fund: row.get::<String, _>("fund"),
            fund_name: row.get::<String, _>("fund_name"),
            operation_type: row.get::<String, _>("operation_type"),
            operation_date: row.get::<String, _>("operation_date"),
            before_15: row.get::<bool, _>("before_15"),
            amount: fmt_decimal_fixed(parse_decimal(row.get::<String, _>("amount")), 2),
            share: fmt_decimal_fixed(parse_decimal(row.get::<String, _>("share")), 4),
            nav: fmt_decimal_fixed(parse_decimal(row.get::<String, _>("nav")), 4),
            created_at: dbfmt::datetime_to_rfc3339(&row.get::<String, _>("created_at")),
        }),
    )
        .into_response()
}

pub async fn operations_destroy(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
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
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let is_admin = match sqlx::query("SELECT is_superuser FROM auth_user WHERE id = $1")
        .bind(user_id_i64)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(row)) => row
            .try_get::<bool, _>("is_superuser")
            .unwrap_or_else(|_| row.try_get::<i64, _>("is_superuser").unwrap_or(0) != 0),
        _ => false,
    };
    if !is_admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "detail": "You do not have permission to perform this action." })),
        )
            .into_response();
    }

    let mut tx = match pool.begin().await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let row = match sqlx::query(
        "SELECT CAST(account_id AS TEXT) as account_id, CAST(fund_id AS TEXT) as fund_id FROM position_operation WHERE CAST(id AS TEXT) = $1",
    )
        .bind(id.to_string())
        .fetch_optional(&mut *tx)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let Some(row) = row else {
        let _ = tx.rollback().await;
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    };

    let account_id: String = row.get("account_id");
    let fund_id: String = row.get("fund_id");

    let res = match sqlx::query("DELETE FROM position_operation WHERE CAST(id AS TEXT) = $1")
        .bind(id.to_string())
        .execute(&mut *tx)
        .await
    {
        Ok(v) => v,
        Err(e) => {
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    if res.rows_affected() == 0 {
        let _ = tx.rollback().await;
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    }

    if let Err(e) = recalculate_position(&mut tx, &account_id, &fund_id).await {
        let _ = tx.rollback().await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        )
            .into_response();
    }

    if tx.commit().await.is_err() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "transaction commit failed" })),
        )
            .into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

fn position_from_row(row: sqlx::any::AnyRow) -> PositionResponse {
    let holding_share = parse_decimal(row.get::<String, _>("holding_share"));
    let holding_cost = parse_decimal(row.get::<String, _>("holding_cost"));
    let holding_nav = parse_decimal(row.get::<String, _>("holding_nav"));
    let latest_nav = row
        .get::<Option<String>, _>("latest_nav")
        .map(parse_decimal);

    let pnl_dec = match latest_nav {
        None => Decimal::ZERO,
        Some(latest) if holding_share.is_zero() => Decimal::ZERO,
        Some(latest) => (latest - holding_nav) * holding_share,
    };

    let fund_code: String = row.get("fund_code");
    let fund_name: String = row.get("fund_name");
    let fund_type: Option<String> = row.get("fund_type");

    PositionResponse {
        id: row.get::<String, _>("id"),
        account: row.get::<String, _>("account"),
        account_name: row.get::<String, _>("account_name"),
        fund_code: fund_code.clone(),
        fund_name: fund_name.clone(),
        fund_type: fund_type.clone(),
        fund: FundInfo {
            fund_code,
            fund_name,
            fund_type,
            latest_nav: latest_nav.map(|d| fmt_decimal_fixed(d, 4)),
            latest_nav_date: row.get::<Option<String>, _>("latest_nav_date"),
            estimate_nav: row
                .get::<Option<String>, _>("estimate_nav")
                .map(parse_decimal)
                .map(|d| fmt_decimal_fixed(d, 4)),
            estimate_growth: row
                .get::<Option<String>, _>("estimate_growth")
                .map(parse_decimal)
                .map(|d| fmt_decimal_fixed(d, 4)),
            estimate_time: row.get::<Option<String>, _>("estimate_time"),
        },
        holding_share: fmt_decimal_fixed(holding_share, 4),
        holding_cost: fmt_decimal_fixed(holding_cost, 2),
        holding_nav: fmt_decimal_fixed(holding_nav, 4),
        pnl: fmt_decimal_fixed(pnl_dec, 2),
        updated_at: dbfmt::datetime_to_rfc3339(&row.get::<String, _>("updated_at")),
    }
}

fn parse_decimal(s: String) -> Decimal {
    Decimal::from_str(&s).unwrap_or(Decimal::ZERO)
}

fn rescale(value: Decimal, dp: u32) -> Decimal {
    let mut v = value.round_dp_with_strategy(dp, RoundingStrategy::MidpointNearestEven);
    v.rescale(dp);
    v
}

fn fmt_decimal_fixed(value: Decimal, dp: u32) -> String {
    let mut v = value.round_dp_with_strategy(dp, RoundingStrategy::MidpointNearestEven);
    v.rescale(dp);
    v.to_string()
}

fn format_dt(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::AutoSi, false)
}

async fn recalculate_all_positions(
    pool: &sqlx::AnyPool,
    account_id: Option<String>,
) -> Result<(), String> {
    let account_id = account_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let rows = if let Some(account_id) = account_id.as_ref() {
        sqlx::query(
            r#"
            SELECT DISTINCT
              CAST(account_id AS TEXT) as account_id,
              CAST(fund_id AS TEXT) as fund_id
            FROM position_operation
            WHERE CAST(account_id AS TEXT) = $1
            "#,
        )
        .bind(account_id)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query(
            r#"
            SELECT DISTINCT
              CAST(account_id AS TEXT) as account_id,
              CAST(fund_id AS TEXT) as fund_id
            FROM position_operation
            "#,
        )
        .fetch_all(pool)
        .await
    };

    let rows = rows.map_err(|e| e.to_string())?;

    for row in rows {
        let account_id: String = row.get("account_id");
        let fund_id: String = row.get("fund_id");
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        recalculate_position(&mut tx, &account_id, &fund_id).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
    }

    Ok(())
}

async fn recalculate_position(
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    account_id: &str,
    fund_id: &str,
) -> Result<(), String> {
    // 确认 account 是子账户（与 Django clean() 行为一致）
    let row = sqlx::query(
        "SELECT CAST(parent_id AS TEXT) as parent_id FROM account WHERE CAST(id AS TEXT) = $1",
    )
    .bind(account_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    let Some(row) = row else {
        return Err("account not found".to_string());
    };
    let parent_id: Option<String> = row.get("parent_id");
    if parent_id.is_none() {
        return Err("account is not a child account".to_string());
    }

    let ops = sqlx::query(
        r#"
        SELECT
          operation_type,
          CAST(amount AS TEXT) as amount,
          CAST(share AS TEXT) as share
        FROM position_operation
        WHERE CAST(account_id AS TEXT) = $1 AND CAST(fund_id AS TEXT) = $2
        ORDER BY operation_date ASC, created_at ASC
        "#,
    )
    .bind(account_id)
    .bind(fund_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    let mut total_share = Decimal::ZERO;
    let mut total_cost = Decimal::ZERO;

    for op in ops {
        let operation_type: String = op.get("operation_type");
        let amount = parse_decimal(op.get::<String, _>("amount"));
        let share = parse_decimal(op.get::<String, _>("share"));

        match operation_type.as_str() {
            "BUY" => {
                total_share += share;
                total_cost += amount;
            }
            "SELL" => {
                if total_share > Decimal::ZERO {
                    let cost_per_share = total_cost / total_share;
                    total_share -= share;
                    total_cost -= share * cost_per_share;
                    total_cost = rescale(total_cost, 2);
                }
            }
            _ => {}
        }
    }

    let holding_nav = if total_share > Decimal::ZERO {
        rescale(total_cost / total_share, 4)
    } else {
        Decimal::ZERO
    };

    let position_id = Uuid::new_v4().to_string();
    let sql_pg = r#"
        INSERT INTO position (id, account_id, fund_id, holding_share, holding_cost, holding_nav, updated_at)
        VALUES (($1)::uuid,($2)::uuid,($3)::uuid,($4)::numeric,($5)::numeric,($6)::numeric,CURRENT_TIMESTAMP)
        ON CONFLICT (account_id, fund_id) DO UPDATE
        SET holding_share = EXCLUDED.holding_share,
            holding_cost = EXCLUDED.holding_cost,
            holding_nav = EXCLUDED.holding_nav,
            updated_at = CURRENT_TIMESTAMP
    "#;
    let sql_any = r#"
        INSERT INTO position (id, account_id, fund_id, holding_share, holding_cost, holding_nav, updated_at)
        VALUES ($1,$2,$3,CAST($4 AS NUMERIC),CAST($5 AS NUMERIC),CAST($6 AS NUMERIC),CURRENT_TIMESTAMP)
        ON CONFLICT (account_id, fund_id) DO UPDATE
        SET holding_share = EXCLUDED.holding_share,
            holding_cost = EXCLUDED.holding_cost,
            holding_nav = EXCLUDED.holding_nav,
            updated_at = CURRENT_TIMESTAMP
    "#;

    let r = sqlx::query(sql_pg)
        .bind(&position_id)
        .bind(account_id)
        .bind(fund_id)
        .bind(rescale(total_share, 4).to_string())
        .bind(rescale(total_cost, 2).to_string())
        .bind(holding_nav.to_string())
        .execute(&mut **tx)
        .await;

    if r.is_err() {
        sqlx::query(sql_any)
            .bind(&position_id)
            .bind(account_id)
            .bind(fund_id)
            .bind(rescale(total_share, 4).to_string())
            .bind(rescale(total_cost, 2).to_string())
            .bind(holding_nav.to_string())
            .execute(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
