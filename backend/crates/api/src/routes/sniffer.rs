use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use serde_json::json;
use sqlx::Row;

use crate::dbfmt;
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

fn format_dt(raw: &str) -> String {
    dbfmt::datetime_to_rfc3339(raw)
}

fn parse_tags(raw: &str) -> Vec<String> {
    let s = raw.trim();
    if s.is_empty() {
        return vec![];
    }

    // Prefer JSON array if present.
    if s.starts_with('[')
        && let Ok(v) = serde_json::from_str::<Vec<String>>(s)
    {
        return v
            .into_iter()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
    }

    // Postgres array literal: {"a","b"}
    if s.starts_with('{') && s.ends_with('}') {
        let inner = &s[1..s.len() - 1];
        let mut out: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut in_quotes = false;
        let mut escape = false;
        for ch in inner.chars() {
            if escape {
                cur.push(ch);
                escape = false;
                continue;
            }
            if ch == '\\' && in_quotes {
                escape = true;
                continue;
            }
            if ch == '"' {
                in_quotes = !in_quotes;
                continue;
            }
            if ch == ',' && !in_quotes {
                let t = cur.trim().to_string();
                if !t.is_empty() {
                    out.push(t);
                }
                cur.clear();
                continue;
            }
            cur.push(ch);
        }
        let t = cur.trim().to_string();
        if !t.is_empty() {
            out.push(t);
        }
        return out;
    }

    vec![s.to_string()]
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
        SELECT
          CAST(id AS TEXT) as id,
          source_url,
          CAST(started_at AS TEXT) as started_at,
          CAST(finished_at AS TEXT) as finished_at,
          ok,
          item_count,
          error,
          CAST(snapshot_id AS TEXT) as snapshot_id
        FROM sniffer_run
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await;

    let last_run = match last_run_row {
        Ok(Some(r)) => Some(json!({
          "id": r.get::<String, _>("id"),
          "source_url": r.get::<String, _>("source_url"),
          "started_at": format_dt(&r.get::<String, _>("started_at")),
          "finished_at": r.get::<Option<String>, _>("finished_at").map(|s| format_dt(&s)),
          "ok": r.get::<bool, _>("ok"),
          "item_count": r.get::<i32, _>("item_count"),
          "error": r.get::<Option<String>, _>("error"),
          "snapshot_id": r.get::<Option<String>, _>("snapshot_id"),
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
        SELECT
          CAST(id AS TEXT) as id,
          source_url,
          CAST(fetched_at AS TEXT) as fetched_at,
          item_count,
          CAST(run_id AS TEXT) as run_id
        FROM sniffer_snapshot
        ORDER BY fetched_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await;

    let last_snapshot = match last_snapshot_row {
        Ok(Some(r)) => Some(json!({
          "id": r.get::<String, _>("id"),
          "source_url": r.get::<String, _>("source_url"),
          "fetched_at": format_dt(&r.get::<String, _>("fetched_at")),
          "item_count": r.get::<i32, _>("item_count"),
          "run_id": r.get::<Option<String>, _>("run_id"),
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
        SELECT
          CAST(id AS TEXT) as id,
          source_url,
          CAST(fetched_at AS TEXT) as fetched_at,
          item_count
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

    let snapshot_id: String = snap_row.get("id");
    let source_url: String = snap_row.get("source_url");
    let fetched_at: String = snap_row.get("fetched_at");
    let item_count: i32 = snap_row.get("item_count");

    let rows = match sqlx::query(
        r#"
        SELECT
          f.fund_code,
          f.fund_name,
          i.sector,
          i.star_count,
          CAST(i.tags AS TEXT) as tags,
          CAST(i.week_growth AS TEXT) as week_growth,
          CAST(i.year_growth AS TEXT) as year_growth,
          CAST(i.max_drawdown AS TEXT) as max_drawdown,
          i.fund_size_text
        FROM sniffer_item i
        JOIN fund f ON f.id = i.fund_id
        WHERE i.snapshot_id = $1
        ORDER BY i.sector ASC, i.star_count DESC NULLS LAST, i.week_growth DESC NULLS LAST, f.fund_code ASC
        "#,
    )
    .bind(&snapshot_id)
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

        let tags: Vec<String> = parse_tags(&r.get::<String, _>("tags"));
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
            week_growth: r.get::<Option<String>, _>("week_growth"),
            year_growth: r.get::<Option<String>, _>("year_growth"),
            max_drawdown: r.get::<Option<String>, _>("max_drawdown"),
            fund_size_text: r.get("fund_size_text"),
        });
    }

    (
        StatusCode::OK,
        Json(SnifferItemsResponse {
            has_snapshot: true,
            source_url: Some(source_url),
            fetched_at: Some(format_dt(&fetched_at)),
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
              "run_id": r.run_id,
              "snapshot_id": r.snapshot_id,
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
