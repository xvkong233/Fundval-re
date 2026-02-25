use axum::Router;

use crate::state::AppState;

pub mod accounts;
pub mod auth;
pub mod bootstrap;
pub mod crawl_config;
pub mod errors;
pub mod forecast;
pub mod fund_analytics;
pub mod fund_analysis_v2;
pub mod fund_signals;
pub mod funds;
pub mod health;
pub mod indexes;
pub mod nav_history;
pub mod positions;
pub mod rates;
pub mod settings;
pub mod sim;
pub mod quant;
pub mod sniffer;
pub mod sources;
pub mod tasks;
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
            "/api/funds/{fund_code}/analysis_v2",
            axum::routing::get(fund_analysis_v2::retrieve),
        )
        .route(
            "/api/funds/{fund_code}/analysis_v2/compute",
            axum::routing::post(fund_analysis_v2::compute),
        )
        .route(
            "/api/funds/{fund_code}/signals",
            axum::routing::get(fund_signals::retrieve),
        )
        .route(
            "/api/funds/signals/batch",
            axum::routing::post(fund_signals::batch),
        )
        .route(
            "/api/funds/signals/batch_async",
            axum::routing::post(fund_signals::batch_async),
        )
        .route(
            "/api/funds/signals/batch_async/{task_id}",
            axum::routing::get(fund_signals::batch_async_page),
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
            "/api/funds/prices/refresh_batch_async",
            axum::routing::post(funds::prices_refresh_batch_async),
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
        .route("/api/indexes/daily", axum::routing::get(indexes::daily))
        .route("/api/rates/risk-free", axum::routing::get(rates::risk_free))
        .route("/api/sniffer/status", axum::routing::get(sniffer::status))
        .route("/api/sniffer/items", axum::routing::get(sniffer::items))
        .route(
            "/api/forecast/model/train",
            axum::routing::post(forecast::train_model),
        )
        .route("/api/quant/health", axum::routing::get(quant::health))
        .route("/api/quant/macd", axum::routing::post(quant::macd))
        .route(
            "/api/quant/xalpha/metrics",
            axum::routing::post(quant::xalpha_metrics),
        )
        .route(
            "/api/quant/xalpha/metrics_batch_async",
            axum::routing::post(quant::xalpha_metrics_batch_async),
        )
        .route("/api/quant/xalpha/grid", axum::routing::post(quant::xalpha_grid))
        .route(
            "/api/quant/xalpha/grid_batch_async",
            axum::routing::post(quant::xalpha_grid_batch_async),
        )
        .route(
            "/api/quant/xalpha/scheduled",
            axum::routing::post(quant::xalpha_scheduled),
        )
        .route(
            "/api/quant/xalpha/scheduled_batch_async",
            axum::routing::post(quant::xalpha_scheduled_batch_async),
        )
        .route(
            "/api/quant/xalpha/qdiipredict",
            axum::routing::post(quant::xalpha_qdiipredict),
        )
        .route(
            "/api/quant/xalpha/qdiipredict_batch_async",
            axum::routing::post(quant::xalpha_qdiipredict_batch_async),
        )
        .route(
            "/api/quant/xalpha/backtest",
            axum::routing::post(quant::xalpha_backtest),
        )
        .route(
            "/api/quant/fund-strategies/compare",
            axum::routing::post(quant::fund_strategies_compare),
        )
        .route(
            "/api/quant/pytrader/strategies",
            axum::routing::get(quant::pytrader_strategies),
        )
        .route(
            "/api/quant/pytrader/backtest",
            axum::routing::post(quant::pytrader_backtest),
        )
        .route("/api/tasks/overview", axum::routing::get(tasks::overview))
        .route("/api/tasks/jobs/{id}", axum::routing::get(tasks::job_detail))
        .route("/api/tasks/jobs/{id}/runs", axum::routing::get(tasks::job_runs))
        .route("/api/tasks/jobs/{id}/logs", axum::routing::get(tasks::job_logs))
        .route("/api/tasks/runs/{id}/logs", axum::routing::get(tasks::run_logs))
        // sim (paper trading / RL env)
        .route(
            "/api/sim/runs",
            axum::routing::get(sim::list_runs).post(sim::create_run),
        )
        .route("/api/sim/runs/{id}", axum::routing::delete(sim::delete_run))
        .route(
            "/api/sim/runs/{id}/run",
            axum::routing::post(sim::run_backtest),
        )
        .route(
            "/api/sim/runs/{id}/train",
            axum::routing::post(sim::train_auto),
        )
        .route(
            "/api/sim/runs/{id}/train/rounds",
            axum::routing::get(sim::train_rounds),
        )
        .route(
            "/api/sim/envs/{id}/step",
            axum::routing::post(sim::env_step),
        )
        .route(
            "/api/sim/envs/{id}/observation",
            axum::routing::get(sim::env_observation),
        )
        .route("/api/sim/runs/{id}/equity", axum::routing::get(sim::equity))
        .route(
            "/api/admin/sniffer/sync",
            axum::routing::post(sniffer::admin_sync),
        )
        .route(
            "/api/admin/rates/risk-free/sync",
            axum::routing::post(rates::admin_sync_risk_free),
        )
        .route(
            "/api/admin/crawl/config",
            axum::routing::get(crawl_config::admin_get_config).put(crawl_config::admin_set_config),
        )
        .with_state(state)
}
