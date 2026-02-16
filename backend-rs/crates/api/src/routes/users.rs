use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

use crate::state::AppState;
use crate::django_password;

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
    let exists = sqlx::query("SELECT 1 FROM auth_user WHERE username = $1")
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

    let password_hash = django_password::hash_password(password);
    let email = body.email.unwrap_or_default();

    let inserted_id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO auth_user (
          password, last_login, is_superuser, username, first_name, last_name, email, is_staff, is_active, date_joined
        )
        VALUES ($1, NULL, FALSE, $2, '', '', $3, FALSE, TRUE, NOW())
        RETURNING id
        "#,
    )
    .bind(password_hash)
    .bind(username)
    .bind(email)
    .fetch_one(pool)
    .await;

    let id = match inserted_id {
        Ok(v) => v,
        Err(e) => {
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
    };

    // 生成 token
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
