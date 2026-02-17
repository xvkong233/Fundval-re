pub mod routes;
pub mod config;
pub mod django_password;
pub mod jwt;
pub mod state;
pub mod eastmoney;
pub mod position_history;

use axum::Router;

pub fn app(state: state::AppState) -> Router {
    Router::new()
        .merge(routes::router(state.clone()))
        .with_state(state)
}
