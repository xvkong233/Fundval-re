pub mod accuracy;
pub mod analytics;
pub mod config;
pub mod crawl;
pub mod db;
pub mod dbfmt;
pub mod django_password;
pub mod eastmoney;
pub mod jwt;
pub mod position_history;
pub mod rates;
pub mod routes;
pub mod sniffer;
pub mod sources;
pub mod state;

use axum::Router;
use tower_http::normalize_path::NormalizePath;

pub fn app(state: state::AppState) -> Router {
    Router::new()
        .merge(routes::router(state.clone()))
        .with_state(state)
}

/// 生成“可直接 serve 的 HTTP service”：在路由匹配前先做路径归一化。
///
/// - `/api/health/`、`/api/health//` => `/api/health`
pub fn service(state: state::AppState) -> NormalizePath<Router> {
    NormalizePath::trim_trailing_slash(app(state))
}
