use axum::{Json, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

use crate::django_password;
use crate::routes::errors;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BootstrapVerifyRequest {
    pub bootstrap_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BootstrapVerifyOk {
    pub valid: bool,
    pub message: &'static str,
}

#[derive(Debug, Serialize)]
pub struct BootstrapVerifyBad {
    pub valid: bool,
    pub error: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct BootstrapInitializeRequest {
    pub bootstrap_key: Option<String>,
    pub admin_username: Option<String>,
    pub admin_password: Option<String>,
    pub allow_register: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BootstrapInitializeOk {
    pub message: &'static str,
    pub admin_created: bool,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn verify(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<BootstrapVerifyRequest>,
) -> axum::response::Response {
    if state.config().system_initialized() {
        return (
            StatusCode::GONE,
            Json(ErrorResponse {
                error: "System already initialized".to_string(),
            }),
        )
            .into_response();
    }

    if state
        .config()
        .verify_bootstrap_key(body.bootstrap_key.as_deref())
    {
        (
            StatusCode::OK,
            Json(BootstrapVerifyOk {
                valid: true,
                message: "密钥验证成功",
            }),
        )
            .into_response()
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(BootstrapVerifyBad {
                valid: false,
                error: "密钥无效",
            }),
        )
            .into_response()
    }
}

pub async fn initialize(
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<BootstrapInitializeRequest>,
) -> axum::response::Response {
    if state.config().system_initialized() {
        return (
            StatusCode::GONE,
            Json(ErrorResponse {
                error: "System already initialized".to_string(),
            }),
        )
            .into_response();
    }

    if !state
        .config()
        .verify_bootstrap_key(body.bootstrap_key.as_deref())
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "密钥无效".to_string(),
            }),
        )
            .into_response();
    }

    let admin_username = match body.admin_username.as_deref() {
        None | Some("") => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "创建管理员失败: missing admin_username".to_string(),
                }),
            )
                .into_response();
        }
        Some(v) => v,
    };
    let admin_password = match body.admin_password.as_deref() {
        None | Some("") => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "创建管理员失败: missing admin_password".to_string(),
                }),
            )
                .into_response();
        }
        Some(v) => v,
    };

    // 为后续 auth 模块做准备：此处先写入 users 表。
    // 若未配置 DB，则返回与 Django 类似的 400（创建管理员失败）。
    if let Some(pool) = state.pool() {
        let password_hash = django_password::hash_password(admin_password);
        let email = format!("{admin_username}@fundval.local");

        let result = sqlx::query(
            r#"
            INSERT INTO auth_user (
              password, last_login, is_superuser, username, first_name, last_name, email, is_staff, is_active, date_joined
            )
            VALUES ($1, NULL, TRUE, $2, '', '', $3, TRUE, TRUE, NOW())
            "#,
        )
        .bind(password_hash)
        .bind(admin_username)
        .bind(email)
        .execute(pool)
        .await;

        if let Err(e) = result {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: errors::masked_message(&state, "创建管理员失败", e),
                }),
            )
                .into_response();
        }
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "创建管理员失败: database not configured".to_string(),
            }),
        )
            .into_response();
    }

    state.config().set_system_initialized(true);
    state
        .config()
        .set_allow_register(body.allow_register.unwrap_or(false));
    let _ = state.config().save();

    (
        StatusCode::OK,
        Json(BootstrapInitializeOk {
            message: "系统初始化成功",
            admin_created: true,
        }),
    )
        .into_response()
}
