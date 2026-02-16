use axum::{http::StatusCode, response::IntoResponse, Json};
use sqlx::Executor;

use crate::state::{AppState, HealthResponse};

pub async fn health(state: axum::extract::State<AppState>) -> impl IntoResponse {
    let database = match state.pool() {
        None => "disconnected",
        Some(pool) => match pool.acquire().await {
            Err(_) => "disconnected",
            Ok(mut conn) => match conn.execute("SELECT 1").await {
                Ok(_) => "connected",
                Err(_) => "disconnected",
            },
        },
    };

    let body = HealthResponse {
        status: "ok",
        database,
        system_initialized: state.config().system_initialized(),
    };

    (StatusCode::OK, Json(body))
}
