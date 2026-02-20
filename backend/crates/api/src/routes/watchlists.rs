use std::collections::HashMap;

use axum::{Json, http::StatusCode, response::IntoResponse};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

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
pub struct WatchlistCreateRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct WatchlistUpdateRequest {
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct WatchlistItemResponse {
    pub id: String,
    pub fund: String,
    pub fund_code: String,
    pub fund_name: String,
    pub fund_type: Option<String>,
    pub order: i32,
    pub created_at: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct WatchlistResponse {
    pub id: String,
    pub name: String,
    pub items: Vec<WatchlistItemResponse>,
    pub created_at: String,
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
        SELECT id, name, created_at
        FROM watchlist
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

    let mut watchlists: Vec<(Uuid, String, DateTime<Utc>)> = Vec::with_capacity(rows.len());
    let mut ids: Vec<Uuid> = Vec::with_capacity(rows.len());
    for row in rows {
        let id: Uuid = row.get("id");
        ids.push(id);
        watchlists.push((id, row.get("name"), row.get("created_at")));
    }

    let items_by_watchlist = load_items(&state, pool, &ids).await;
    let items_by_watchlist = match items_by_watchlist {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    let mut out: Vec<WatchlistResponse> = Vec::with_capacity(watchlists.len());
    for (id, name, created_at) in watchlists {
        out.push(WatchlistResponse {
            id: id.to_string(),
            name,
            items: items_by_watchlist.get(&id).cloned().unwrap_or_default(),
            created_at: format_dt(created_at),
        });
    }

    (StatusCode::OK, Json(out)).into_response()
}

pub async fn create(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<WatchlistCreateRequest>,
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

    let exists =
        match sqlx::query("SELECT 1 FROM watchlist WHERE user_id = $1 AND name = $2 LIMIT 1")
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
            Json(json!({ "name": ["自选列表名已存在"] })),
        )
            .into_response();
    }

    let id = Uuid::new_v4();
    let created_at = Utc::now();
    let inserted = sqlx::query(
        r#"
        INSERT INTO watchlist (id, user_id, name, created_at)
        VALUES ($1,$2,$3,NOW())
        "#,
    )
    .bind(id)
    .bind(user_id_i64)
    .bind(&name)
    .execute(pool)
    .await;

    if let Err(e) = inserted {
        return (
            StatusCode::BAD_REQUEST,
            errors::masked_json(&state, "创建自选列表失败", e),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(WatchlistResponse {
            id: id.to_string(),
            name,
            items: vec![],
            created_at: format_dt(created_at),
        }),
    )
        .into_response()
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
        SELECT id, name, created_at
        FROM watchlist
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(id)
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

    let items_by_watchlist = match load_items(&state, pool, &[id]).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    (
        StatusCode::OK,
        Json(WatchlistResponse {
            id: row.get::<Uuid, _>("id").to_string(),
            name: row.get::<String, _>("name"),
            items: items_by_watchlist.get(&id).cloned().unwrap_or_default(),
            created_at: format_dt(row.get::<DateTime<Utc>, _>("created_at")),
        }),
    )
        .into_response()
}

pub async fn update_put(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(body): Json<WatchlistUpdateRequest>,
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
    Json(body): Json<WatchlistUpdateRequest>,
) -> axum::response::Response {
    update_internal(state, headers, id, body).await
}

async fn update_internal(
    state: AppState,
    headers: axum::http::HeaderMap,
    id: Uuid,
    body: WatchlistUpdateRequest,
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

    let row =
        match sqlx::query("SELECT name, created_at FROM watchlist WHERE id = $1 AND user_id = $2")
            .bind(id)
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

    let current_name: String = row.get("name");
    let created_at: DateTime<Utc> = row.get("created_at");

    let next_name = body
        .name
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(current_name.clone());

    let dup = match sqlx::query(
        "SELECT 1 FROM watchlist WHERE user_id = $1 AND name = $2 AND id <> $3 LIMIT 1",
    )
    .bind(user_id_i64)
    .bind(&next_name)
    .bind(id)
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
            Json(json!({ "name": ["自选列表名已存在"] })),
        )
            .into_response();
    }

    if let Err(e) = sqlx::query("UPDATE watchlist SET name = $1 WHERE id = $2 AND user_id = $3")
        .bind(&next_name)
        .bind(id)
        .bind(user_id_i64)
        .execute(pool)
        .await
    {
        return (
            StatusCode::BAD_REQUEST,
            errors::masked_json(&state, "更新自选列表失败", e),
        )
            .into_response();
    }

    let items_by_watchlist = match load_items(&state, pool, &[id]).await {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    (
        StatusCode::OK,
        Json(WatchlistResponse {
            id: id.to_string(),
            name: next_name,
            items: items_by_watchlist.get(&id).cloned().unwrap_or_default(),
            created_at: format_dt(created_at),
        }),
    )
        .into_response()
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

    let res = match sqlx::query("DELETE FROM watchlist WHERE id = $1 AND user_id = $2")
        .bind(id)
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
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}

#[derive(Debug, Deserialize)]
pub struct AddItemRequest {
    pub fund_code: Option<String>,
}

pub async fn items_add(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(body): Json<AddItemRequest>,
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

    // ensure watchlist exists & owned
    let owned = match sqlx::query("SELECT 1 FROM watchlist WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id_i64)
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
    if !owned {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    }

    let Some(fund_code) = body
        .fund_code
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "基金代码不能为空" })),
        )
            .into_response();
    };

    let fund_row = match sqlx::query("SELECT id FROM fund WHERE fund_code = $1")
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
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "基金不存在" })),
        )
            .into_response();
    };
    let fund_id: Uuid = fund_row.get("id");

    let exists = match sqlx::query(
        "SELECT 1 FROM watchlist_item WHERE watchlist_id = $1 AND fund_id = $2 LIMIT 1",
    )
    .bind(id)
    .bind(fund_id)
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
            Json(json!({ "error": "基金已在自选列表中" })),
        )
            .into_response();
    }

    let max_order_row = match sqlx::query(
        "SELECT MAX(\"order\")::int as max_order FROM watchlist_item WHERE watchlist_id = $1",
    )
    .bind(id)
    .fetch_one(pool)
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
    let max_order: Option<i32> = max_order_row.get("max_order");
    let next_order = max_order.unwrap_or(-1) + 1;

    let item_id = Uuid::new_v4();
    if let Err(e) = sqlx::query(
        r#"
        INSERT INTO watchlist_item (id, watchlist_id, fund_id, "order", created_at)
        VALUES ($1,$2,$3,$4,NOW())
        "#,
    )
    .bind(item_id)
    .bind(id)
    .bind(fund_id)
    .bind(next_order)
    .execute(pool)
    .await
    {
        return (
            StatusCode::BAD_REQUEST,
            errors::masked_json(&state, "添加基金到自选失败", e),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(json!({ "id": item_id.to_string(), "fund_code": fund_code })),
    )
        .into_response()
}

pub async fn items_remove(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path((id, fund_code)): axum::extract::Path<(Uuid, String)>,
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

    let owned = match sqlx::query("SELECT 1 FROM watchlist WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id_i64)
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
    if !owned {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    }

    let fund_row = match sqlx::query("SELECT id FROM fund WHERE fund_code = $1")
        .bind(fund_code.trim())
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
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "基金不在自选列表中" })),
        )
            .into_response();
    };
    let fund_id: Uuid = fund_row.get("id");

    let res =
        match sqlx::query("DELETE FROM watchlist_item WHERE watchlist_id = $1 AND fund_id = $2")
            .bind(id)
            .bind(fund_id)
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
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "基金不在自选列表中" })),
        )
            .into_response();
    }

    StatusCode::NO_CONTENT.into_response()
}

#[derive(Debug, Deserialize)]
pub struct ReorderRequest {
    pub fund_codes: Option<Vec<String>>,
}

pub async fn reorder(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(body): Json<ReorderRequest>,
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

    let owned = match sqlx::query("SELECT 1 FROM watchlist WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id_i64)
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
    if !owned {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    }

    let fund_codes = body.fund_codes.unwrap_or_default();
    if fund_codes.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "基金代码列表不能为空" })),
        )
            .into_response();
    }

    for (idx, code) in fund_codes.iter().enumerate() {
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        let fund_row = sqlx::query("SELECT id FROM fund WHERE fund_code = $1")
            .bind(code)
            .fetch_optional(pool)
            .await;
        let fund_row: Option<sqlx::postgres::PgRow> = fund_row.unwrap_or_default();
        let Some(fund_row) = fund_row else {
            continue;
        };
        let fund_id: Uuid = fund_row.get("id");
        let _ = sqlx::query(
            "UPDATE watchlist_item SET \"order\" = $1 WHERE watchlist_id = $2 AND fund_id = $3",
        )
        .bind(idx as i32)
        .bind(id)
        .bind(fund_id)
        .execute(pool)
        .await;
    }

    (
        StatusCode::OK,
        Json(MessageResponse {
            message: "排序已更新",
        }),
    )
        .into_response()
}

async fn load_items(
    state: &AppState,
    pool: &sqlx::PgPool,
    watchlist_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<WatchlistItemResponse>>, axum::response::Response> {
    if watchlist_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = match sqlx::query(
        r#"
        SELECT
          i.watchlist_id,
          i.id::text as id,
          i.fund_id::text as fund,
          f.fund_code,
          f.fund_name,
          f.fund_type,
          i."order" as "order",
          i.created_at
        FROM watchlist_item i
        JOIN fund f ON f.id = i.fund_id
        WHERE i.watchlist_id = ANY($1::uuid[])
        ORDER BY i.watchlist_id ASC, i."order" ASC, i.created_at ASC
        "#,
    )
    .bind(watchlist_ids)
    .fetch_all(pool)
    .await
    {
        Ok(v) => v,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(state, e),
            )
                .into_response());
        }
    };

    let mut map: HashMap<Uuid, Vec<WatchlistItemResponse>> = HashMap::new();
    for row in rows {
        let watchlist_id: Uuid = row.get("watchlist_id");
        map.entry(watchlist_id)
            .or_default()
            .push(WatchlistItemResponse {
                id: row.get::<String, _>("id"),
                fund: row.get::<String, _>("fund"),
                fund_code: row.get::<String, _>("fund_code"),
                fund_name: row.get::<String, _>("fund_name"),
                fund_type: row.get::<Option<String>, _>("fund_type"),
                order: row.get::<i32, _>("order"),
                created_at: format_dt(row.get::<DateTime<Utc>, _>("created_at")),
            });
    }
    Ok(map)
}

fn format_dt(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::AutoSi, false)
}
