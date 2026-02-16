pub mod routes;
pub mod config;
pub mod state;

use axum::Router;

pub fn app(state: state::AppState) -> Router {
    Router::new()
        .merge(routes::router(state.clone()))
        .with_state(state)
}
