use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
// sqlx::Row 暂不需要（后续 summary 会用到）

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: Option<String>,
    pub password: Option<String>,
    pub password_confirm: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct FieldErrors {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password_confirm: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub user: RegisterUser,
}

#[derive(Debug, Serialize)]
pub struct RegisterUser {
    pub id: String,
    pub username: String,
}

pub async fn register(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> axum::response::Response {
    if !state.config().allow_register() {
        return (
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "注册未开放".to_string(),
            }),
        )
            .into_response();
    }

    let username = match body.username.as_deref() {
        None | Some("") => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FieldErrors {
                    username: Some(vec!["This field is required.".to_string()]),
                    password: None,
                    password_confirm: None,
                }),
            )
                .into_response();
        }
        Some(v) => v.trim(),
    };

    if username.len() > 150 {
        return (
            StatusCode::BAD_REQUEST,
            Json(FieldErrors {
                username: Some(vec!["Ensure this field has no more than 150 characters.".to_string()]),
                password: None,
                password_confirm: None,
            }),
        )
            .into_response();
    }

    let password = match body.password.as_deref() {
        None | Some("") => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FieldErrors {
                    username: None,
                    password: Some(vec!["This field is required.".to_string()]),
                    password_confirm: None,
                }),
            )
                .into_response();
        }
        Some(v) => v,
    };

    if password.len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(FieldErrors {
                username: None,
                password: Some(vec!["Ensure this field has at least 8 characters.".to_string()]),
                password_confirm: None,
            }),
        )
            .into_response();
    }

    let password_confirm = match body.password_confirm.as_deref() {
        None | Some("") => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FieldErrors {
                    username: None,
                    password: None,
                    password_confirm: Some(vec!["This field is required.".to_string()]),
                }),
            )
                .into_response();
        }
        Some(v) => v,
    };

    if password != password_confirm {
        return (
            StatusCode::BAD_REQUEST,
            Json(FieldErrors {
                username: None,
                password: None,
                password_confirm: Some(vec!["两次密码不一致".to_string()]),
            }),
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

    // username 唯一性（对齐 Django serializer 行为）
    let exists = sqlx::query("SELECT 1 FROM users WHERE username = $1")
        .bind(username)
        .fetch_optional(pool)
        .await;
    match exists {
        Ok(Some(_)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(FieldErrors {
                    username: Some(vec!["用户名已存在".to_string()]),
                    password: None,
                    password_confirm: None,
                }),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e.to_string() }),
            )
                .into_response();
        }
    }

    let salt = SaltString::generate(&mut rand_core::OsRng);
    let password_hash = match Argon2::default().hash_password(password.as_bytes(), &salt) {
        Ok(hash) => hash.to_string(),
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse { error: e.to_string() }),
            )
                .into_response();
        }
    };

    let id = uuid::Uuid::new_v4();
    let email = body.email.unwrap_or_default();

    let inserted = sqlx::query(
        r#"
        INSERT INTO users (id, username, password_hash, email, is_superuser)
        VALUES ($1, $2, $3, $4, FALSE)
        "#,
    )
    .bind(id)
    .bind(username)
    .bind(password_hash)
    .bind(email)
    .execute(pool)
    .await;

    if let Err(e) = inserted {
        // 兜底：并发情况下仍可能触发唯一约束冲突
        let msg = e.to_string();
        if msg.to_lowercase().contains("duplicate") || msg.to_lowercase().contains("unique") {
            return (
                StatusCode::BAD_REQUEST,
                Json(FieldErrors {
                    username: Some(vec!["用户名已存在".to_string()]),
                    password: None,
                    password_confirm: None,
                }),
            )
                .into_response();
        }
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse { error: msg }),
        )
            .into_response();
    }

    let jwt = state.jwt();
    let access_token = jwt.issue_access_token(&id.to_string());
    let refresh_token = jwt.issue_refresh_token(&id.to_string());

    (
        StatusCode::CREATED,
        Json(RegisterResponse {
            access_token,
            refresh_token,
            user: RegisterUser {
                id: id.to_string(),
                username: username.to_string(),
            },
        }),
    )
        .into_response()
}
