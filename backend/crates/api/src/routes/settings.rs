use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::state::AppState;

fn mask_token(token: &str) -> Option<String> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    if t.len() <= 8 {
        return Some("********".to_string());
    }
    Some(format!("{}****{}", &t[..4], &t[t.len() - 4..]))
}

async fn require_staff(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), axum::response::Response> {
    let pool = match state.pool() {
        Some(p) => p,
        None => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "数据库未连接" })),
            )
                .into_response());
        }
    };

    let user_id = match crate::routes::auth::authenticate(state, headers) {
        Ok(v) => v,
        Err(resp) => return Err(resp),
    };
    let user_id_i64 = user_id
        .parse::<i64>()
        .map_err(|_| crate::routes::auth::invalid_token_response())?;

    let is_staff = match sqlx::query("SELECT is_staff FROM auth_user WHERE id = $1")
        .bind(user_id_i64)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(row)) => row.get::<bool, _>("is_staff"),
        _ => false,
    };

    if !is_staff {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "detail": "You do not have permission to perform this action." })),
        )
            .into_response());
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct TokenStatusResponse {
    pub configured: bool,
    pub token_hint: Option<String>,
}

pub async fn get_tushare_token_status(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    if let Err(resp) = require_staff(&state, &headers).await {
        return resp;
    }

    let token = state.config().get_string("tushare_token").unwrap_or_default();
    let hint = mask_token(&token);
    (
        StatusCode::OK,
        Json(TokenStatusResponse {
            configured: hint.is_some(),
            token_hint: hint,
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct SetTokenRequest {
    pub token: Option<String>,
}

pub async fn set_tushare_token(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<SetTokenRequest>,
) -> axum::response::Response {
    if let Err(resp) = require_staff(&state, &headers).await {
        return resp;
    }

    let token = body.token.map(|s| {
        let t = s.trim().to_string();
        if t.is_empty() {
            None
        } else {
            Some(t)
        }
    }).flatten();

    state.config().set_string("tushare_token", token);
    if let Err(e) = state.config().save() {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("保存配置失败: {e}") })),
        )
            .into_response();
    }

    (StatusCode::OK, Json(json!({ "message": "ok" }))).into_response()
}

