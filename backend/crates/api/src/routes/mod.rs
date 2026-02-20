use axum::Router;

use crate::state::AppState;

pub mod accounts;
pub mod auth;
pub mod bootstrap;
pub mod errors;
pub mod fund_analytics;
pub mod funds;
pub mod health;
pub mod nav_history;
pub mod positions;
pub mod rates;
pub mod settings;
pub mod sniffer;
pub mod sources;
pub mod users;
pub mod watchlists;

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        // 统一在 `api::service()`（以及 runtime main）中做路径归一化以去除尾斜杠：
        // 这里仅维护“不带尾斜杠”的 canonical 路径，`/foo/`、`/foo//` 等会自动归一化。
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
        .route(
            "/api/auth/password",
            axum::routing::put(auth::change_password),
        )
        .route(
            "/api/settings/tushare_token",
            axum::routing::get(settings::get_tushare_token_status).put(settings::set_tushare_token),
        )
        .route("/api/users/register", axum::routing::post(users::register))
        .route(
            "/api/users/me/summary",
            axum::routing::get(users::me_summary),
        )
        .route("/api/sources", axum::routing::get(sources::list))
        .route("/api/sources/health", axum::routing::get(sources::health))
        .route(
            "/api/sources/{source}/accuracy",
            axum::routing::get(sources::accuracy),
        )
        .route(
            "/api/sources/{source}/accuracy/calculate",
            axum::routing::post(sources::calculate_accuracy),
        )
        .route("/api/funds", axum::routing::get(funds::list))
        .route(
            "/api/funds/{fund_code}",
            axum::routing::get(funds::retrieve),
        )
        .route(
            "/api/funds/{fund_code}/estimate",
            axum::routing::get(funds::estimate),
        )
        .route(
            "/api/funds/{fund_code}/analytics",
            axum::routing::get(fund_analytics::retrieve),
        )
        .route(
            "/api/funds/{fund_code}/accuracy",
            axum::routing::get(funds::accuracy),
        )
        .route(
            "/api/funds/batch_estimate",
            axum::routing::post(funds::batch_estimate),
        )
        .route(
            "/api/funds/batch_update_nav",
            axum::routing::post(funds::batch_update_nav),
        )
        .route(
            "/api/funds/query_nav",
            axum::routing::post(funds::query_nav),
        )
        .route("/api/funds/sync", axum::routing::post(funds::sync))
        .route(
            "/api/accounts",
            axum::routing::get(accounts::list).post(accounts::create),
        )
        .route(
            "/api/accounts/{id}",
            axum::routing::get(accounts::retrieve)
                .put(accounts::update_put)
                .patch(accounts::update_patch)
                .delete(accounts::destroy),
        )
        .route(
            "/api/accounts/{id}/positions",
            axum::routing::get(accounts::positions),
        )
        .route("/api/positions", axum::routing::get(positions::list))
        .route(
            "/api/positions/history",
            axum::routing::get(positions::history),
        )
        .route(
            "/api/positions/{id}",
            axum::routing::get(positions::retrieve),
        )
        .route(
            "/api/positions/recalculate",
            axum::routing::post(positions::recalculate),
        )
        .route(
            "/api/positions/operations",
            axum::routing::get(positions::operations_list).post(positions::operations_create),
        )
        .route(
            "/api/positions/operations/{id}",
            axum::routing::get(positions::operations_retrieve)
                .delete(positions::operations_destroy),
        )
        .route(
            "/api/watchlists",
            axum::routing::get(watchlists::list).post(watchlists::create),
        )
        .route(
            "/api/watchlists/{id}",
            axum::routing::get(watchlists::retrieve)
                .put(watchlists::update_put)
                .patch(watchlists::update_patch)
                .delete(watchlists::destroy),
        )
        .route(
            "/api/watchlists/{id}/items",
            axum::routing::post(watchlists::items_add),
        )
        .route(
            "/api/watchlists/{id}/items/{fund_code}",
            axum::routing::delete(watchlists::items_remove),
        )
        .route(
            "/api/watchlists/{id}/reorder",
            axum::routing::put(watchlists::reorder),
        )
        .route("/api/nav-history", axum::routing::get(nav_history::list))
        .route(
            "/api/nav-history/{id}",
            axum::routing::get(nav_history::retrieve),
        )
        .route(
            "/api/nav-history/batch_query",
            axum::routing::post(nav_history::batch_query),
        )
        .route(
            "/api/nav-history/sync",
            axum::routing::post(nav_history::sync),
        )
        .route("/api/rates/risk-free", axum::routing::get(rates::risk_free))
        .route("/api/sniffer/status", axum::routing::get(sniffer::status))
        .route("/api/sniffer/items", axum::routing::get(sniffer::items))
        .route(
            "/api/admin/sniffer/sync",
            axum::routing::post(sniffer::admin_sync),
        )
        .route(
            "/api/admin/rates/risk-free/sync",
            axum::routing::post(rates::admin_sync_risk_free),
        )
        .with_state(state)
}
