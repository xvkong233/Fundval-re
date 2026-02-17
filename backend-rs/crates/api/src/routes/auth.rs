use axum::{http::StatusCode, response::IntoResponse, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{jwt::JwtService, state::AppState};
use crate::django_password;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: LoginUser,
}

#[derive(Debug, Serialize)]
pub struct LoginUser {
    pub id: String,
    pub username: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct NotAuthenticatedResponse {
    pub detail: &'static str,
}

#[derive(Debug, Serialize)]
pub struct TokenNotValidResponse {
    pub detail: &'static str,
    pub code: &'static str,
    pub messages: Vec<TokenMessage>,
}

#[derive(Debug, Serialize)]
pub struct TokenMessage {
    pub token_class: &'static str,
    pub token_type: &'static str,
    pub message: &'static str,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub id: String,
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: &'static str,
}

pub async fn login(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<LoginRequest>,
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
        SELECT id::text as id, username, password, email, is_superuser, date_joined
        FROM auth_user
        WHERE username = $1
        "#,
    )
    .bind(&body.username)
    .fetch_optional(pool)
    .await;

    let Some(row) = (match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response();
        }
    }) else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "用户名或密码错误".to_string(),
            }),
        )
            .into_response();
    };

    let password_hash = match row.try_get::<String, _>("password") {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "用户名或密码错误".to_string(),
                }),
            )
                .into_response();
        }
    };

    if !django_password::verify_password(&body.password, &password_hash) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "用户名或密码错误".to_string(),
            }),
        )
            .into_response();
    }

    let user_id = row.get::<String, _>("id");
    let username = row.get::<String, _>("username");
    let is_superuser = row.get::<bool, _>("is_superuser");

    let jwt = state.jwt();
    let access_token = jwt.issue_access_token(&user_id);
    let refresh_token = jwt.issue_refresh_token(&user_id);

    (
        StatusCode::OK,
        Json(LoginResponse {
            access_token,
            refresh_token,
            user: LoginUser {
                id: user_id,
                username,
                role: if is_superuser { "admin" } else { "user" }.to_string(),
            },
        }),
    )
        .into_response()
}

pub async fn refresh(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> axum::response::Response {
    let jwt = state.jwt();
    let decoded = jwt.decode(&body.refresh_token);
    let Ok(decoded) = decoded else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid refresh token".to_string(),
            }),
        )
            .into_response();
    };

    if decoded.claims.token_type != "refresh" {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid refresh token".to_string(),
            }),
        )
            .into_response();
    }

    let access_token = jwt.issue_access_token(&decoded.claims.sub);
    (StatusCode::OK, Json(RefreshResponse { access_token })).into_response()
}

pub async fn me(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
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
        SELECT id::text as id, username, email, is_superuser, date_joined
        FROM auth_user
        WHERE id::text = $1
        "#,
    )
    .bind(&user_id)
    .fetch_optional(pool)
    .await;

    let Some(row) = (match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response();
        }
    }) else {
        return invalid_token_response();
    };

    let created_at: DateTime<Utc> = row.get("date_joined");
    (
        StatusCode::OK,
        Json(MeResponse {
            id: row.get::<String, _>("id"),
            username: row.get::<String, _>("username"),
            email: row.get::<String, _>("email"),
            role: if row.get::<bool, _>("is_superuser") {
                "admin".to_string()
            } else {
                "user".to_string()
            },
            created_at: created_at.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, false),
        }),
    )
        .into_response()
}

pub async fn change_password(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<ChangePasswordRequest>,
) -> axum::response::Response {
    let user_id = match authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
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
        SELECT password
        FROM auth_user
        WHERE id::text = $1
        "#,
    )
    .bind(&user_id)
    .fetch_optional(pool)
    .await;

    let Some(row) = (match row {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
                .into_response();
        }
    }) else {
        return invalid_token_response();
    };

    let password_hash: String = row.get("password");
    if !django_password::verify_password(&body.old_password, &password_hash) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "旧密码错误".to_string(),
            }),
        )
            .into_response();
    }

    let new_hash = django_password::hash_password(&body.new_password);

    let updated = sqlx::query("UPDATE auth_user SET password = $1 WHERE id::text = $2")
        .bind(new_hash)
        .bind(&user_id)
        .execute(pool)
        .await;

    if let Err(e) = updated {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
            .into_response();
    }

    (StatusCode::OK, Json(MessageResponse { message: "密码修改成功" })).into_response()
}

pub(crate) fn authenticate(
    state: &AppState,
    headers: &axum::http::HeaderMap,
) -> Result<String, axum::response::Response> {
    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return Err(
            (
                StatusCode::UNAUTHORIZED,
                Json(NotAuthenticatedResponse {
                    detail: "Authentication credentials were not provided.",
                }),
            )
                .into_response(),
        );
    };

    let auth_str = match auth.to_str() {
        Ok(v) => v,
        Err(_) => return Err(invalid_token_response()),
    };
    let token = auth_str.strip_prefix("Bearer ").unwrap_or("");
    if token.is_empty() {
        return Err(invalid_token_response());
    }

    let jwt: &JwtService = state.jwt();
    let decoded = jwt.decode(token).map_err(|_| invalid_token_response())?;
    if decoded.claims.token_type != "access" {
        return Err(invalid_token_response());
    }
    Ok(decoded.claims.sub)
}

pub(crate) fn invalid_token_response() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(TokenNotValidResponse {
            detail: "Given token not valid for any token type",
            code: "token_not_valid",
            messages: vec![TokenMessage {
                token_class: "AccessToken",
                token_type: "access",
                message: "Token is invalid or expired",
            }],
        }),
    )
        .into_response()
}
