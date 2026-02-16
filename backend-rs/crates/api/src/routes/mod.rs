use axum::Router;

use crate::state::AppState;

pub mod bootstrap;
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
        .with_state(state)
}
