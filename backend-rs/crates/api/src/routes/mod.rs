use axum::Router;

use crate::state::AppState;

pub mod bootstrap;
pub mod auth;
pub mod users;
pub mod health;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        // 文档定义为 /api/health/，同时兼容无尾斜杠（不影响契约）
        .route("/api/health/", axum::routing::get(health::health))
        .route("/api/health", axum::routing::get(health::health))
        .route(
            "/api/admin/bootstrap/verify",
            axum::routing::post(bootstrap::verify),
        )
        .route(
            "/api/admin/bootstrap/initialize",
            axum::routing::post(bootstrap::initialize),
        )
        .route("/api/auth/login", axum::routing::post(auth::login))
        .route("/api/auth/refresh", axum::routing::post(auth::refresh))
        .route("/api/auth/me", axum::routing::get(auth::me))
        .route("/api/auth/password", axum::routing::put(auth::change_password))
        .route("/api/users/register/", axum::routing::post(users::register))
        .route("/api/users/register", axum::routing::post(users::register))
        .with_state(state)
}
