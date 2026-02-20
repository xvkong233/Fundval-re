use std::collections::HashMap;
use std::str::FromStr;

use axum::{http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, SecondsFormat, Utc};
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::routes::auth;
use crate::routes::errors;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct AccountCreateRequest {
    pub name: String,
    pub parent: Option<Uuid>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AccountUpdateRequest {
    pub name: Option<String>,
    #[serde(default)]
    pub parent: Option<Option<Uuid>>,
    pub is_default: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AccountResponse {
    pub id: String,
    pub name: String,
    pub parent: Option<String>,
    pub is_default: bool,

    pub holding_cost: String,
    pub holding_value: String,
    pub pnl: String,
    pub pnl_rate: Option<String>,
    pub estimate_value: Option<String>,
    pub estimate_pnl: Option<String>,
    pub estimate_pnl_rate: Option<String>,
    pub today_pnl: Option<String>,
    pub today_pnl_rate: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<AccountResponse>>,

    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct PositionFundInfo {
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
    pub fund: PositionFundInfo,
    pub fund_code: String,
    pub fund_name: String,
    pub fund_type: Option<String>,
    pub holding_share: String,
    pub holding_cost: String,
    pub holding_nav: String,
    pub pnl: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Clone, Debug)]
struct AccountRow {
    id: String,
    name: String,
    parent_id: Option<String>,
    is_default: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Clone, Debug, Default)]
struct Summary {
    holding_cost: Decimal,
    holding_value: Decimal,
    pnl: Decimal,
    pnl_rate: Option<Decimal>,
    estimate_value: Option<Decimal>,
    estimate_pnl: Option<Decimal>,
    estimate_pnl_rate: Option<Decimal>,
    today_pnl: Option<Decimal>,
    today_pnl_rate: Option<Decimal>,
}

#[derive(Clone, Debug)]
struct PositionAggRow {
    holding_share: Decimal,
    holding_cost: Decimal,
    latest_nav: Option<Decimal>,
    estimate_nav: Option<Decimal>,
}

pub async fn list(
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
                Json(ErrorResponse {
                    error: "database not configured".to_string(),
                }),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let rows = match sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          name,
          CAST(parent_id AS TEXT) as parent_id,
          is_default,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM account
        WHERE user_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(user_id_i64)
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

    let mut accounts: Vec<AccountRow> = Vec::with_capacity(rows.len());
    for row in rows {
        accounts.push(AccountRow {
            id: row.get::<String, _>("id"),
            name: row.get::<String, _>("name"),
            parent_id: row.get::<Option<String>, _>("parent_id"),
            is_default: row.get::<bool, _>("is_default"),
            created_at: row.get::<String, _>("created_at"),
            updated_at: row.get::<String, _>("updated_at"),
        });
    }

    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for a in &accounts {
        if let Some(parent_id) = a.parent_id.as_ref() {
            children_map
                .entry(parent_id.clone())
                .or_default()
                .push(a.id.clone());
        }
    }

    let child_ids = accounts
        .iter()
        .filter_map(|a| a.parent_id.as_ref().map(|_| a.id.clone()))
        .collect::<Vec<_>>();

    let positions_by_account = match load_positions(&state, pool, &child_ids).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let mut summary_by_id: HashMap<String, Summary> = HashMap::new();
    for a in &accounts {
        if a.parent_id.is_some() {
            let positions = positions_by_account
                .get(&a.id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            summary_by_id.insert(a.id.clone(), compute_child_summary(positions));
        }
    }

    for a in &accounts {
        if a.parent_id.is_none() {
            let child_summaries = children_map
                .get(&a.id)
                .into_iter()
                .flat_map(|v| v.iter())
                .filter_map(|id| summary_by_id.get(id));
            summary_by_id.insert(a.id.clone(), compute_parent_summary(child_summaries));
        }
    }

    let row_by_id: HashMap<String, AccountRow> = accounts
        .iter()
        .cloned()
        .map(|a| (a.id.clone(), a))
        .collect();

    let mut out: Vec<AccountResponse> = Vec::with_capacity(accounts.len());
    for a in accounts {
        let summary = summary_by_id.get(&a.id).cloned().unwrap_or_default();
        let mut resp = to_account_response(&a, &summary);

        if a.parent_id.is_none() {
            let children = children_map
                .get(&a.id)
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| row_by_id.get(id))
                        .map(|row| {
                            let s = summary_by_id.get(&row.id).cloned().unwrap_or_default();
                            to_account_response(row, &s)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            resp.children = Some(children);
        }

        out.push(resp);
    }

    (StatusCode::OK, Json(out)).into_response()
}

pub async fn create(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<AccountCreateRequest>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let user_id_i64 = match user_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return auth::invalid_token_response(),
    };

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "name": ["This field may not be blank."] })),
        )
            .into_response();
    }

    let is_default = body.is_default.unwrap_or(false);
    if is_default && body.parent.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "non_field_errors": ["默认账户必须是父账户（parent 必须为 NULL）"] })),
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

    if let Some(parent_id) = body.parent {
        match sqlx::query(
            "SELECT CAST(parent_id AS TEXT) as parent_id FROM account WHERE CAST(id AS TEXT) = $1",
        )
            .bind(parent_id.to_string())
            .fetch_optional(pool)
            .await
        {
            Ok(Some(row)) => {
                let pp: Option<String> = row.get("parent_id");
                if pp.is_some() {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "non_field_errors": ["账户层级最多两层：父账户 -> 子账户，不支持孙账户"] })),
                    )
                        .into_response();
                }
            }
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "parent": ["Invalid pk - object does not exist."] })),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    errors::internal_json(&state, e),
                )
                    .into_response();
            }
        }
    }

    let exists = match sqlx::query("SELECT 1 FROM account WHERE user_id = $1 AND name = $2 LIMIT 1")
        .bind(user_id_i64)
        .bind(&name)
        .fetch_optional(pool)
        .await
    {
        Ok(v) => v.is_some(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };
    if exists {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "name": ["账户名已存在"] })),
        )
            .into_response();
    }

    let id = Uuid::new_v4().to_string();
    let parent_id = body.parent.map(|v| v.to_string());
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

    if is_default
        && let Err(e) = sqlx::query("UPDATE account SET is_default = FALSE WHERE user_id = $1 AND is_default = TRUE")
            .bind(user_id_i64)
            .execute(&mut *tx)
            .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            errors::internal_json(&state, e),
        )
            .into_response();
    }

    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO account (id, user_id, name, parent_id, is_default, created_at, updated_at)
        VALUES (CAST($1 AS uuid), $2, $3, CAST($4 AS uuid), $5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#,
    )
    .bind(&id)
    .bind(user_id_i64)
    .bind(&name)
    .bind(parent_id.clone())
    .bind(is_default)
    .execute(&mut *tx)
    .await
    {
        let _ = tx.rollback().await;
        return (
            StatusCode::BAD_REQUEST,
            errors::masked_json(&state, "创建账户失败", e),
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

    let row = AccountRow {
        id,
        name,
        parent_id,
        is_default,
        created_at: format_now(Utc::now()),
        updated_at: format_now(Utc::now()),
    };

    let summary = compute_child_summary(&[]);
    let mut resp = to_account_response(&row, &summary);
    if row.parent_id.is_none() {
        resp.children = Some(vec![]);
    }

    (StatusCode::CREATED, Json(resp)).into_response()
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

    let row = match sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          name,
          CAST(parent_id AS TEXT) as parent_id,
          is_default,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM account
        WHERE CAST(id AS TEXT) = $1 AND user_id = $2
        "#,
    )
    .bind(id.to_string())
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
        return (StatusCode::NOT_FOUND, Json(json!({ "detail": "Not found." }))).into_response();
    };

    let account = AccountRow {
        id: row.get::<String, _>("id"),
        name: row.get::<String, _>("name"),
        parent_id: row.get::<Option<String>, _>("parent_id"),
        is_default: row.get::<bool, _>("is_default"),
        created_at: row.get::<String, _>("created_at"),
        updated_at: row.get::<String, _>("updated_at"),
    };

    if account.parent_id.is_some() {
        let positions_by_account = match load_positions(&state, pool, std::slice::from_ref(&account.id)).await {
            Ok(v) => v,
            Err(resp) => return resp,
        };
        let positions = positions_by_account.get(&account.id).map(Vec::as_slice).unwrap_or(&[]);
        let summary = compute_child_summary(positions);
        return (StatusCode::OK, Json(to_account_response(&account, &summary))).into_response();
    }

    let child_rows = match sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          name,
          CAST(parent_id AS TEXT) as parent_id,
          is_default,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM account
        WHERE CAST(parent_id AS TEXT) = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(&account.id)
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

    let mut children: Vec<AccountRow> = Vec::with_capacity(child_rows.len());
    for r in child_rows {
        children.push(AccountRow {
            id: r.get::<String, _>("id"),
            name: r.get::<String, _>("name"),
            parent_id: r.get::<Option<String>, _>("parent_id"),
            is_default: r.get::<bool, _>("is_default"),
            created_at: r.get::<String, _>("created_at"),
            updated_at: r.get::<String, _>("updated_at"),
        });
    }

    let child_ids = children.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
    let positions_by_account = match load_positions(&state, pool, &child_ids).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let mut child_responses: Vec<AccountResponse> = Vec::with_capacity(children.len());
    let mut child_summaries: Vec<Summary> = Vec::with_capacity(children.len());
    for c in &children {
        let positions = positions_by_account.get(&c.id).map(Vec::as_slice).unwrap_or(&[]);
        let s = compute_child_summary(positions);
        child_summaries.push(s.clone());
        child_responses.push(to_account_response(c, &s));
    }

    let parent_summary = compute_parent_summary(child_summaries.iter());
    let mut resp = to_account_response(&account, &parent_summary);
    resp.children = Some(child_responses);

    (StatusCode::OK, Json(resp)).into_response()
}

pub async fn update_put(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(body): Json<AccountUpdateRequest>,
) -> axum::response::Response {
    let Some(name) = body.name.as_ref() else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "name": ["This field is required."] })),
        )
            .into_response();
    };
    if name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "name": ["This field may not be blank."] })),
        )
            .into_response();
    }

    update_internal(state, headers, id, body).await
}

pub async fn update_patch(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(body): Json<AccountUpdateRequest>,
) -> axum::response::Response {
    update_internal(state, headers, id, body).await
}

async fn update_internal(
    state: AppState,
    headers: axum::http::HeaderMap,
    id: Uuid,
    body: AccountUpdateRequest,
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

    let existing = match sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          name,
          CAST(parent_id AS TEXT) as parent_id,
          is_default,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM account
        WHERE CAST(id AS TEXT) = $1 AND user_id = $2
        "#,
    )
    .bind(id.to_string())
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

    let Some(row) = existing else {
        return (StatusCode::NOT_FOUND, Json(json!({ "detail": "Not found." }))).into_response();
    };

    let existing_row = AccountRow {
        id: row.get::<String, _>("id"),
        name: row.get::<String, _>("name"),
        parent_id: row.get::<Option<String>, _>("parent_id"),
        is_default: row.get::<bool, _>("is_default"),
        created_at: row.get::<String, _>("created_at"),
        updated_at: row.get::<String, _>("updated_at"),
    };

    let next_name = match body.name.as_ref() {
        None => existing_row.name.clone(),
        Some(v) if v.trim().is_empty() => existing_row.name.clone(),
        Some(v) => v.trim().to_string(),
    };

    let next_parent: Option<String> = match body.parent {
        None => existing_row.parent_id.clone(),
        Some(v) => v.map(|u| u.to_string()),
    };

    let next_is_default = body.is_default.unwrap_or(existing_row.is_default);

    if next_is_default && next_parent.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "non_field_errors": ["默认账户必须是父账户（parent 必须为 NULL）"] })),
        )
            .into_response();
    }

    if let Some(parent_id) = next_parent.as_ref() {
        if parent_id == &existing_row.id {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "parent": ["Invalid pk - object does not exist."] })),
            )
                .into_response();
        }
        match sqlx::query(
            "SELECT CAST(parent_id AS TEXT) as parent_id FROM account WHERE CAST(id AS TEXT) = $1",
        )
            .bind(parent_id)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(row)) => {
                let pp: Option<String> = row.get("parent_id");
                if pp.is_some() {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "non_field_errors": ["账户层级最多两层：父账户 -> 子账户，不支持孙账户"] })),
                    )
                        .into_response();
                }
            }
            Ok(None) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "parent": ["Invalid pk - object does not exist."] })),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    errors::internal_json(&state, e),
                )
                    .into_response();
            }
        }
    }

    let dup = match sqlx::query(
        "SELECT 1 FROM account WHERE user_id = $1 AND name = $2 AND CAST(id AS TEXT) <> $3 LIMIT 1",
    )
    .bind(user_id_i64)
    .bind(&next_name)
    .bind(&existing_row.id)
    .fetch_optional(pool)
    .await
    {
        Ok(v) => v.is_some(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };
    if dup {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "name": ["账户名已存在"] })),
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

    if next_is_default
        && let Err(e) = sqlx::query(
            "UPDATE account SET is_default = FALSE WHERE user_id = $1 AND is_default = TRUE AND CAST(id AS TEXT) <> $2",
        )
        .bind(user_id_i64)
        .bind(&existing_row.id)
        .execute(&mut *tx)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            errors::internal_json(&state, e),
        )
            .into_response();
    }

    if let Err(e) = sqlx::query(
        r#"
        UPDATE account
        SET name = $1,
            parent_id = CAST($2 AS uuid),
            is_default = $3,
            updated_at = CURRENT_TIMESTAMP
        WHERE CAST(id AS TEXT) = $4 AND user_id = $5
        "#,
    )
    .bind(&next_name)
    .bind(next_parent.clone())
    .bind(next_is_default)
    .bind(&existing_row.id)
    .bind(user_id_i64)
    .execute(&mut *tx)
    .await
    {
        let _ = tx.rollback().await;
        return (
            StatusCode::BAD_REQUEST,
            errors::masked_json(&state, "更新账户失败", e),
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

    let id_uuid = Uuid::parse_str(&existing_row.id).unwrap_or(id);
    retrieve(axum::extract::State(state), headers, axum::extract::Path(id_uuid)).await
}

pub async fn destroy(
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

    let res = match sqlx::query("DELETE FROM account WHERE CAST(id AS TEXT) = $1 AND user_id = $2")
        .bind(id.to_string())
        .bind(user_id_i64)
        .execute(pool)
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

    if res.rows_affected() == 0 {
        return (StatusCode::NOT_FOUND, Json(json!({ "detail": "Not found." }))).into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}

pub async fn positions(
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

    let account_row = match sqlx::query("SELECT name FROM account WHERE CAST(id AS TEXT) = $1 AND user_id = $2")
        .bind(id.to_string())
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
    let Some(account_row) = account_row else {
        return (StatusCode::NOT_FOUND, Json(json!({ "detail": "Not found." }))).into_response();
    };
    let account_name: String = account_row.get("name");

    let rows = match sqlx::query(
        r#"
        SELECT
          CAST(p.id AS TEXT) as id,
          CAST(p.account_id AS TEXT) as account,
          CAST(p.holding_share AS TEXT) as holding_share,
          CAST(p.holding_cost AS TEXT) as holding_cost,
          CAST(p.holding_nav AS TEXT) as holding_nav,
          CAST(p.updated_at AS TEXT) as updated_at,
          f.fund_code,
          f.fund_name,
          f.fund_type,
          CAST(f.latest_nav AS TEXT) as latest_nav,
          CAST(f.latest_nav_date AS TEXT) as latest_nav_date,
          CAST(f.estimate_nav AS TEXT) as estimate_nav,
          CAST(f.estimate_growth AS TEXT) as estimate_growth,
          CAST(f.estimate_time AS TEXT) as estimate_time
        FROM position p
        JOIN fund f ON f.id = p.fund_id
        WHERE CAST(p.account_id AS TEXT) = $1
        ORDER BY f.fund_code ASC
        "#,
    )
    .bind(id.to_string())
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

    let mut out: Vec<PositionResponse> = Vec::with_capacity(rows.len());
    for row in rows {
        let holding_share = parse_decimal(row.get::<String, _>("holding_share"));
        let holding_cost = parse_decimal(row.get::<String, _>("holding_cost"));
        let holding_nav = parse_decimal(row.get::<String, _>("holding_nav"));
        let latest_nav = row.get::<Option<String>, _>("latest_nav").map(parse_decimal);

        let pnl_dec = match latest_nav {
            None => Decimal::ZERO,
            Some(latest) if holding_share.is_zero() => Decimal::ZERO,
            Some(latest) => (latest - holding_nav) * holding_share,
        };

        let fund_code: String = row.get("fund_code");
        let fund_name: String = row.get("fund_name");
        let fund_type: Option<String> = row.get("fund_type");

        out.push(PositionResponse {
            id: row.get::<String, _>("id"),
            account: row.get::<String, _>("account"),
            account_name: account_name.clone(),
            fund: PositionFundInfo {
                fund_code: fund_code.clone(),
                fund_name: fund_name.clone(),
                fund_type: fund_type.clone(),
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
            fund_code,
            fund_name,
            fund_type,
            holding_share: fmt_decimal_fixed(holding_share, 4),
            holding_cost: fmt_decimal_fixed(holding_cost, 2),
            holding_nav: fmt_decimal_fixed(holding_nav, 4),
            pnl: fmt_decimal_fixed(pnl_dec, 2),
            updated_at: crate::dbfmt::datetime_to_rfc3339(&row.get::<String, _>("updated_at")),
        });
    }

    (StatusCode::OK, Json(out)).into_response()
}

async fn load_positions(
    state: &AppState,
    pool: &sqlx::AnyPool,
    account_ids: &[String],
) -> Result<HashMap<String, Vec<PositionAggRow>>, axum::response::Response> {
    if account_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut sql = String::from(
        r#"
        SELECT
          CAST(p.account_id AS TEXT) as account_id,
          CAST(p.holding_share AS TEXT) as holding_share,
          CAST(p.holding_cost AS TEXT) as holding_cost,
          CAST(f.latest_nav AS TEXT) as latest_nav,
          CAST(f.estimate_nav AS TEXT) as estimate_nav
        FROM position p
        JOIN fund f ON f.id = p.fund_id
        WHERE CAST(p.account_id AS TEXT) IN (
        "#,
    );
    for (i, _) in account_ids.iter().enumerate() {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("${}", i + 1));
    }
    sql.push_str(")\n");
    let mut q = sqlx::query(&sql);
    for id in account_ids {
        q = q.bind(id);
    }
    let rows = q.fetch_all(pool).await;

    let rows = match rows {
        Ok(v) => v,
        Err(e) => {
            return Err(
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    errors::internal_json(state, e),
                )
                    .into_response(),
            );
        }
    };

    let mut map: HashMap<String, Vec<PositionAggRow>> = HashMap::new();
    for row in rows {
        let account_id = row.get::<String, _>("account_id");
        let holding_share = parse_decimal(row.get::<String, _>("holding_share"));
        let holding_cost = parse_decimal(row.get::<String, _>("holding_cost"));
        let latest_nav = row.get::<Option<String>, _>("latest_nav").map(parse_decimal);
        let estimate_nav = row.get::<Option<String>, _>("estimate_nav").map(parse_decimal);

        map.entry(account_id)
            .or_default()
            .push(PositionAggRow {
                holding_share,
                holding_cost,
                latest_nav,
                estimate_nav,
            });
    }
    Ok(map)
}

fn compute_child_summary(positions: &[PositionAggRow]) -> Summary {
    let mut holding_cost = Decimal::ZERO;
    let mut holding_value = Decimal::ZERO;

    let mut estimate_value: Option<Decimal> = Some(Decimal::ZERO);
    let mut today_pnl: Option<Decimal> = Some(Decimal::ZERO);

    if positions.is_empty() {
        let pnl = holding_value - holding_cost;
        return finalize_summary(holding_cost, holding_value, pnl, estimate_value, today_pnl);
    }

    for p in positions {
        holding_cost += p.holding_cost;
        if let Some(latest) = p.latest_nav {
            holding_value += latest * p.holding_share;
        }

        if estimate_value.is_some() {
            match p.estimate_nav {
                None => estimate_value = None,
                Some(estimate) => {
                    estimate_value = Some(estimate_value.unwrap_or_default() + estimate * p.holding_share)
                }
            }
        }

        if today_pnl.is_some() {
            match (p.estimate_nav, p.latest_nav) {
                (Some(estimate), Some(latest)) => {
                    today_pnl = Some(today_pnl.unwrap_or_default() + p.holding_share * (estimate - latest));
                }
                _ => today_pnl = None,
            }
        }
    }

    let pnl = holding_value - holding_cost;
    finalize_summary(holding_cost, holding_value, pnl, estimate_value, today_pnl)
}

fn compute_parent_summary<'a>(child_summaries: impl Iterator<Item = &'a Summary>) -> Summary {
    let mut holding_cost = Decimal::ZERO;
    let mut holding_value = Decimal::ZERO;

    let mut estimate_value: Option<Decimal> = Some(Decimal::ZERO);
    let mut today_pnl: Option<Decimal> = Some(Decimal::ZERO);

    for s in child_summaries {
        holding_cost += s.holding_cost;
        holding_value += s.holding_value;

        if estimate_value.is_some() {
            match s.estimate_value {
                None => estimate_value = None,
                Some(v) => estimate_value = Some(estimate_value.unwrap_or_default() + v),
            }
        }

        if today_pnl.is_some() {
            match s.today_pnl {
                None => today_pnl = None,
                Some(v) => today_pnl = Some(today_pnl.unwrap_or_default() + v),
            }
        }
    }

    let pnl = holding_value - holding_cost;
    finalize_summary(holding_cost, holding_value, pnl, estimate_value, today_pnl)
}

fn finalize_summary(
    holding_cost: Decimal,
    holding_value: Decimal,
    pnl: Decimal,
    estimate_value: Option<Decimal>,
    today_pnl: Option<Decimal>,
) -> Summary {
    let pnl_rate = if holding_cost.is_zero() {
        None
    } else {
        Some(div_round(pnl, holding_cost, 4))
    };

    let estimate_pnl = estimate_value.map(|v| v - holding_cost);
    let estimate_pnl_rate = match (estimate_pnl, holding_cost.is_zero()) {
        (Some(_), true) => None,
        (Some(v), false) => Some(div_round(v, holding_cost, 4)),
        _ => None,
    };

    let today_pnl_rate = match (today_pnl, holding_value.is_zero()) {
        (Some(_), true) => None,
        (Some(v), false) => Some(div_round(v, holding_value, 4)),
        _ => None,
    };

    Summary {
        holding_cost,
        holding_value,
        pnl,
        pnl_rate,
        estimate_value,
        estimate_pnl,
        estimate_pnl_rate,
        today_pnl,
        today_pnl_rate,
    }
}

fn to_account_response(row: &AccountRow, s: &Summary) -> AccountResponse {
    AccountResponse {
        id: row.id.clone(),
        name: row.name.clone(),
        parent: row.parent_id.clone(),
        is_default: row.is_default,

        holding_cost: fmt_decimal_fixed(s.holding_cost, 2),
        holding_value: fmt_decimal_fixed(s.holding_value, 2),
        pnl: fmt_decimal_fixed(s.pnl, 2),
        pnl_rate: s.pnl_rate.map(|d| fmt_decimal_fixed(d, 4)),
        estimate_value: s.estimate_value.map(|d| fmt_decimal_fixed(d, 2)),
        estimate_pnl: s.estimate_pnl.map(|d| fmt_decimal_fixed(d, 2)),
        estimate_pnl_rate: s.estimate_pnl_rate.map(|d| fmt_decimal_fixed(d, 4)),
        today_pnl: s.today_pnl.map(|d| fmt_decimal_fixed(d, 2)),
        today_pnl_rate: s.today_pnl_rate.map(|d| fmt_decimal_fixed(d, 4)),

        children: None,
        created_at: crate::dbfmt::datetime_to_rfc3339(&row.created_at),
        updated_at: crate::dbfmt::datetime_to_rfc3339(&row.updated_at),
    }
}

fn parse_decimal(s: String) -> Decimal {
    Decimal::from_str(&s).unwrap_or(Decimal::ZERO)
}

fn div_round(n: Decimal, d: Decimal, dp: u32) -> Decimal {
    let q = n / d;
    q.round_dp_with_strategy(dp, RoundingStrategy::MidpointNearestEven)
}

fn fmt_decimal_fixed(value: Decimal, dp: u32) -> String {
    let mut v = value.round_dp_with_strategy(dp, RoundingStrategy::MidpointNearestEven);
    v.rescale(dp);
    v.to_string()
}

fn format_now(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::AutoSi, false)
}
