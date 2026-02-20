use axum::{Json, http::StatusCode, response::IntoResponse};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use crate::routes::auth;
use crate::routes::errors;
use crate::sniffer;
use crate::state::AppState;

#[derive(Debug, Serialize)]
struct SnifferItemResponse {
    fund_code: String,
    fund_name: String,
    sector: String,
    star_count: Option<i32>,
    tags: Vec<String>,
    week_growth: Option<String>,
    year_growth: Option<String>,
    max_drawdown: Option<String>,
    fund_size_text: Option<String>,
}

#[derive(Debug, Serialize)]
struct SnifferItemsResponse {
    has_snapshot: bool,
    source_url: Option<String>,
    fetched_at: Option<String>,
    item_count: i32,
    sectors: Vec<String>,
    tags: Vec<String>,
    items: Vec<SnifferItemResponse>,
}

#[derive(Debug, Serialize)]
struct SnifferStatusResponse {
    last_run: Option<serde_json::Value>,
    last_snapshot: Option<serde_json::Value>,
}

fn format_dt(dt: chrono::DateTime<Utc>) -> String {
    dt.to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub async fn status(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let _user_id_i64 = match user_id.parse::<i64>() {
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

    let last_run_row = sqlx::query(
        r#"
        SELECT id, source_url, started_at, finished_at, ok, item_count, error, snapshot_id
        FROM sniffer_run
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await;

    let last_run = match last_run_row {
        Ok(Some(r)) => Some(json!({
          "id": r.get::<Uuid, _>("id").to_string(),
          "source_url": r.get::<String, _>("source_url"),
          "started_at": format_dt(r.get("started_at")),
          "finished_at": r.get::<Option<chrono::DateTime<Utc>>, _>("finished_at").map(format_dt),
          "ok": r.get::<bool, _>("ok"),
          "item_count": r.get::<i32, _>("item_count"),
          "error": r.get::<Option<String>, _>("error"),
          "snapshot_id": r.get::<Option<Uuid>, _>("snapshot_id").map(|v| v.to_string()),
        })),
        Ok(None) => None,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let last_snapshot_row = sqlx::query(
        r#"
        SELECT id, source_url, fetched_at, item_count, run_id
        FROM sniffer_snapshot
        ORDER BY fetched_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await;

    let last_snapshot = match last_snapshot_row {
        Ok(Some(r)) => Some(json!({
          "id": r.get::<Uuid, _>("id").to_string(),
          "source_url": r.get::<String, _>("source_url"),
          "fetched_at": format_dt(r.get("fetched_at")),
          "item_count": r.get::<i32, _>("item_count"),
          "run_id": r.get::<Option<Uuid>, _>("run_id").map(|v| v.to_string()),
        })),
        Ok(None) => None,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(SnifferStatusResponse {
            last_run,
            last_snapshot,
        }),
    )
        .into_response()
}

pub async fn items(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let _user_id_i64 = match user_id.parse::<i64>() {
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

    let snap_row = match sqlx::query(
        r#"
        SELECT id, source_url, fetched_at, item_count
        FROM sniffer_snapshot
        ORDER BY fetched_at DESC
        LIMIT 1
        "#,
    )
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

    let Some(snap_row) = snap_row else {
        return (
            StatusCode::OK,
            Json(SnifferItemsResponse {
                has_snapshot: false,
                source_url: None,
                fetched_at: None,
                item_count: 0,
                sectors: vec![],
                tags: vec![],
                items: vec![],
            }),
        )
            .into_response();
    };

    let snapshot_id: Uuid = snap_row.get("id");
    let source_url: String = snap_row.get("source_url");
    let fetched_at: chrono::DateTime<Utc> = snap_row.get("fetched_at");
    let item_count: i32 = snap_row.get("item_count");

    let rows = match sqlx::query(
        r#"
        SELECT
          f.fund_code,
          f.fund_name,
          i.sector,
          i.star_count,
          i.tags,
          i.week_growth,
          i.year_growth,
          i.max_drawdown,
          i.fund_size_text
        FROM sniffer_item i
        JOIN fund f ON f.id = i.fund_id
        WHERE i.snapshot_id = $1
        ORDER BY i.sector ASC, i.star_count DESC NULLS LAST, i.week_growth DESC NULLS LAST, f.fund_code ASC
        "#,
    )
    .bind(snapshot_id)
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

    let mut sectors_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut tags_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut items: Vec<SnifferItemResponse> = Vec::with_capacity(rows.len());

    for r in rows {
        let sector: String = r.get("sector");
        sectors_set.insert(sector.clone());

        let tags: Vec<String> = r.get::<Vec<String>, _>("tags");
        for t in &tags {
            if !t.trim().is_empty() {
                tags_set.insert(t.trim().to_string());
            }
        }

        items.push(SnifferItemResponse {
            fund_code: r.get("fund_code"),
            fund_name: r.get("fund_name"),
            sector,
            star_count: r.get("star_count"),
            tags,
            week_growth: r
                .get::<Option<rust_decimal::Decimal>, _>("week_growth")
                .map(|v| v.to_string()),
            year_growth: r
                .get::<Option<rust_decimal::Decimal>, _>("year_growth")
                .map(|v| v.to_string()),
            max_drawdown: r
                .get::<Option<rust_decimal::Decimal>, _>("max_drawdown")
                .map(|v| v.to_string()),
            fund_size_text: r.get("fund_size_text"),
        });
    }

    (
        StatusCode::OK,
        Json(SnifferItemsResponse {
            has_snapshot: true,
            source_url: Some(source_url),
            fetched_at: Some(format_dt(fetched_at)),
            item_count,
            sectors: sectors_set.into_iter().collect(),
            tags: tags_set.into_iter().collect(),
            items,
        }),
    )
        .into_response()
}

pub async fn admin_sync(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(v) => v,
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

    // Django IsAdminUser：要求 is_staff
    let is_admin = match sqlx::query("SELECT is_staff FROM auth_user WHERE id = $1")
        .bind(user_id_i64)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(row)) => row.get::<bool, _>("is_staff"),
        _ => false,
    };

    if !is_admin {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "detail": "You do not have permission to perform this action." })),
        )
            .into_response();
    }

    match sniffer::run_sync_once(state.clone()).await {
        Ok(r) => (
            StatusCode::OK,
            Json(json!({
              "run_id": r.run_id.to_string(),
              "snapshot_id": r.snapshot_id.to_string(),
              "item_count": r.item_count,
              "users_updated": r.users_updated,
              "watchlist_name": sniffer::SNIFFER_WATCHLIST_NAME,
              "source_url": sniffer::DEEPQ_STAR_CSV_URL,
            })),
        )
            .into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(json!({ "error": e }))).into_response(),
    }
}
