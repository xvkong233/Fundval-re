use axum::{Json, http::StatusCode, response::IntoResponse};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::routes::auth;
use crate::routes::errors;
use crate::sim::engine;
use crate::state::AppState;

fn quant_base_url(state: &AppState) -> String {
    state
        .config()
        .get_string("quant_service_url")
        .unwrap_or_else(|| "http://localhost:8002".to_string())
        .trim_end_matches('/')
        .to_string()
}

#[derive(Debug, Deserialize)]
pub struct CreateSimRunBody {
    pub mode: String, // "backtest" | "env"
    /// backtest 策略：buy_and_hold_equal | auto_topk_snapshot | auto_topk_ts_timing
    pub strategy: Option<String>,
    pub name: Option<String>,
    pub source: Option<String>,
    pub fund_codes: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    pub initial_cash: String,
    pub buy_fee_rate: Option<f64>,
    pub sell_fee_rate: Option<f64>,
    pub settlement_days: Option<i64>,

    // auto_topk_snapshot params
    pub top_k: Option<i64>,
    pub rebalance_every: Option<i64>,
    pub weights: Option<Vec<f64>>,

    // auto_topk_ts_timing params
    pub refer_index_code: Option<String>,
    pub sell_macd_point: Option<f64>,
    pub buy_macd_point: Option<f64>,
    pub sh_composite_index: Option<f64>,
    pub fund_position: Option<f64>,
    pub sell_at_top: Option<bool>,
    pub sell_num: Option<f64>,
    pub sell_unit: Option<String>, // "amount" | "fundPercent"
    pub profit_rate: Option<f64>,
    pub buy_amount_percent: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct CreateSimRunOut<T> {
    pub run_id: String,
    pub data: T,
}

#[derive(Debug, Serialize)]
pub struct CreatedOk {
    pub message: &'static str,
}

#[derive(Debug, Deserialize)]
pub struct EnvStepBody {
    pub actions: Vec<engine::Action>,
}

#[derive(Debug, Deserialize)]
pub struct TrainAutoBody {
    pub rounds: i64,
    pub population: Option<i64>,
    pub elite_ratio: Option<f64>,
    pub seed: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct SimRunSummary {
    pub id: String,
    pub mode: String,
    pub name: String,
    pub source_name: String,
    pub strategy: String,
    pub start_date: String,
    pub end_date: String,
    pub current_date: Option<String>,
    pub initial_cash: String,
    pub cash_available: String,
    pub cash_frozen: String,
    pub buy_fee_rate: f64,
    pub sell_fee_rate: f64,
    pub settlement_days: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d").map_err(|e| e.to_string())
}

pub async fn list_runs(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let user_id_i64 = match user_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return auth::invalid_token_response(),
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let rows = sqlx::query(
        r#"
        SELECT
          CAST(id AS TEXT) as id,
          mode,
          name,
          source_name,
          strategy,
          CAST(start_date AS TEXT) as start_date,
          CAST(end_date AS TEXT) as end_date,
          CAST(current_date AS TEXT) as current_date,
          CAST(initial_cash AS TEXT) as initial_cash,
          CAST(cash_available AS TEXT) as cash_available,
          CAST(cash_frozen AS TEXT) as cash_frozen,
          buy_fee_rate,
          sell_fee_rate,
          settlement_days,
          status,
          CAST(created_at AS TEXT) as created_at,
          CAST(updated_at AS TEXT) as updated_at
        FROM sim_run
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 200
        "#,
    )
    .bind(user_id_i64)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut out: Vec<SimRunSummary> = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(SimRunSummary {
            id: r.get("id"),
            mode: r.get("mode"),
            name: r.get::<String, _>("name"),
            source_name: r.get("source_name"),
            strategy: r.get("strategy"),
            start_date: r.get("start_date"),
            end_date: r.get("end_date"),
            current_date: r
                .try_get::<Option<String>, _>("current_date")
                .ok()
                .flatten(),
            initial_cash: r.get("initial_cash"),
            cash_available: r.get("cash_available"),
            cash_frozen: r.get("cash_frozen"),
            buy_fee_rate: r.get("buy_fee_rate"),
            sell_fee_rate: r.get("sell_fee_rate"),
            settlement_days: r.get("settlement_days"),
            status: r.get("status"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        });
    }

    (StatusCode::OK, Json(out)).into_response()
}

pub async fn delete_run(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(run_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let user_id_i64 = match user_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return auth::invalid_token_response(),
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let result = sqlx::query(
        r#"
        DELETE FROM sim_run
        WHERE user_id = $1 AND CAST(id AS TEXT) = $2
        "#,
    )
    .bind(user_id_i64)
    .bind(run_id.trim())
    .execute(pool)
    .await;

    let result = match result {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    if result.rows_affected() == 0 {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "detail": "Not found." })),
        )
            .into_response();
    }

    (StatusCode::OK, Json(json!({ "deleted": true }))).into_response()
}

pub async fn create_run(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    Json(body): Json<CreateSimRunBody>,
) -> axum::response::Response {
    let user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    let user_id_i64 = match user_id.parse::<i64>() {
        Ok(v) => v,
        Err(_) => return auth::invalid_token_response(),
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let mode = body.mode.trim();
    if mode != "env" && mode != "backtest" {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid mode (env|backtest)" })),
        )
            .into_response();
    }

    let start_date = match parse_date(&body.start_date) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid start_date" })),
            )
                .into_response();
        }
    };
    let end_date = match parse_date(&body.end_date) {
        Ok(v) => v,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid end_date" })),
            )
                .into_response();
        }
    };

    let initial_cash = body
        .initial_cash
        .trim()
        .parse::<rust_decimal::Decimal>()
        .unwrap_or_default();
    if initial_cash <= rust_decimal::Decimal::ZERO {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "invalid initial_cash" })),
        )
            .into_response();
    }

    let source_name = body.source.as_deref().unwrap_or("tiantian").trim();
    let name = body.name.as_deref().unwrap_or("").trim();

    let buy_fee_rate = body.buy_fee_rate.unwrap_or(0.0).clamp(0.0, 0.5);
    let sell_fee_rate = body.sell_fee_rate.unwrap_or(0.0).clamp(0.0, 0.5);
    let settlement_days = body.settlement_days.unwrap_or(2).clamp(0, 10);

    if mode == "env" {
        if body.fund_codes.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "env 模式需要提供 fund_codes" })),
            )
                .into_response();
        }
        let created = engine::env_create(
            pool,
            user_id_i64,
            name,
            source_name,
            &body.fund_codes,
            start_date,
            end_date,
            initial_cash,
            buy_fee_rate,
            sell_fee_rate,
            settlement_days,
        )
        .await;

        match created {
            Ok((run_id, obs)) => {
                (StatusCode::OK, Json(CreateSimRunOut { run_id, data: obs })).into_response()
            }
            Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        }
    } else {
        let strategy = body
            .strategy
            .as_deref()
            .unwrap_or("buy_and_hold_equal")
            .trim();

        let created = if strategy == "buy_and_hold_equal" {
            if body.fund_codes.is_empty() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "buy_and_hold_equal 需要提供 fund_codes" })),
                )
                    .into_response();
            }
            engine::backtest_create_buy_and_hold_equal(
                pool,
                user_id_i64,
                name,
                source_name,
                &body.fund_codes,
                start_date,
                end_date,
                initial_cash,
                buy_fee_rate,
                sell_fee_rate,
                settlement_days,
            )
            .await
        } else if strategy == "auto_topk_snapshot" {
            let top_k = body.top_k.unwrap_or(20).clamp(1, 200) as usize;
            let rebalance_every = body.rebalance_every.unwrap_or(5).clamp(1, 60);
            engine::backtest_create_auto_topk_snapshot(
                pool,
                user_id_i64,
                name,
                source_name,
                start_date,
                end_date,
                initial_cash,
                buy_fee_rate,
                sell_fee_rate,
                settlement_days,
                engine::AutoTopkSnapshotParams {
                    top_k,
                    rebalance_every,
                    weights: body.weights.clone(),
                },
            )
            .await
        } else if strategy == "auto_topk_ts_timing" {
            let top_k = body.top_k.unwrap_or(20).clamp(1, 200) as usize;
            let rebalance_every = body.rebalance_every.unwrap_or(5).clamp(1, 60);

            let refer_index_code = body
                .refer_index_code
                .as_deref()
                .unwrap_or("1.000001")
                .trim()
                .to_string();

            let sell_unit = body
                .sell_unit
                .as_deref()
                .unwrap_or("fundPercent")
                .trim()
                .to_string();

            engine::backtest_create_auto_topk_ts_timing(
                pool,
                user_id_i64,
                name,
                source_name,
                start_date,
                end_date,
                initial_cash,
                buy_fee_rate,
                sell_fee_rate,
                settlement_days,
                engine::AutoTopkTsTimingParams {
                    top_k,
                    rebalance_every,
                    weights: body.weights.clone(),
                    refer_index_code,
                    sell_macd_point: body.sell_macd_point,
                    buy_macd_point: body.buy_macd_point,
                    sh_composite_index: body.sh_composite_index.unwrap_or(3000.0),
                    fund_position: body.fund_position.unwrap_or(70.0),
                    sell_at_top: body.sell_at_top.unwrap_or(true),
                    sell_num: body.sell_num.unwrap_or(10.0),
                    sell_unit,
                    profit_rate: body.profit_rate.unwrap_or(10.0),
                    buy_amount_percent: body.buy_amount_percent.unwrap_or(20.0),
                    quant_service_url: quant_base_url(&state),
                },
            )
            .await
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("unknown backtest strategy: {strategy}") })),
            )
                .into_response();
        };

        match created {
            Ok(run_id) => (
                StatusCode::OK,
                Json(CreateSimRunOut {
                    run_id,
                    data: CreatedOk { message: "created" },
                }),
            )
                .into_response(),
            Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
        }
    }
}

pub async fn run_backtest(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(run_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    match engine::backtest_run(pool, &run_id).await {
        Ok(()) => (StatusCode::OK, Json(json!({ "message": "done" }))).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

pub async fn env_step(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(run_id): axum::extract::Path<String>,
    Json(body): Json<EnvStepBody>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    match engine::env_step(pool, &run_id, &body.actions).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

pub async fn env_observation(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(run_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    match engine::env_observation(pool, &run_id).await {
        Ok(r) => (StatusCode::OK, Json(r)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

pub async fn equity(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(run_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let rows = sqlx::query(
        r#"
        SELECT
          CAST(date AS TEXT) as date,
          total_equity,
          cash_available,
          cash_frozen,
          cash_receivable,
          positions_value
        FROM sim_daily_equity
        WHERE CAST(run_id AS TEXT) = $1
        ORDER BY date ASC
        "#,
    )
    .bind(&run_id)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut out: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(json!({
            "date": r.get::<String,_>("date"),
            "total_equity": r.get::<f64,_>("total_equity"),
            "cash_available": r.get::<f64,_>("cash_available"),
            "cash_frozen": r.get::<f64,_>("cash_frozen"),
            "cash_receivable": r.get::<f64,_>("cash_receivable"),
            "positions_value": r.get::<f64,_>("positions_value"),
        }));
    }

    (StatusCode::OK, Json(out)).into_response()
}

pub async fn train_auto(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(run_id): axum::extract::Path<String>,
    Json(body): Json<TrainAutoBody>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let rounds = body.rounds;
    let population = body.population.unwrap_or(30);
    let elite_ratio = body.elite_ratio.unwrap_or(0.2);

    match engine::train_auto_topk_snapshot(pool, &run_id, rounds, population, elite_ratio, body.seed).await {
        Ok(v) => (StatusCode::OK, Json(v)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(json!({ "error": e }))).into_response(),
    }
}

pub async fn train_rounds(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(run_id): axum::extract::Path<String>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database not configured" })),
            )
                .into_response();
        }
        Some(p) => p,
    };

    let rows = sqlx::query(
        r#"
        SELECT
          round,
          best_total_return,
          best_final_equity,
          best_weights_json,
          CAST(created_at AS TEXT) as created_at
        FROM sim_train_round
        WHERE CAST(run_id AS TEXT) = $1
        ORDER BY round ASC
        "#,
    )
    .bind(&run_id)
    .fetch_all(pool)
    .await;

    let rows = match rows {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                errors::internal_json(&state, e),
            )
                .into_response();
        }
    };

    let mut out: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
    for r in rows {
        out.push(json!({
            "round": r.get::<i64,_>("round"),
            "best_total_return": r.get::<f64,_>("best_total_return"),
            "best_final_equity": r.get::<f64,_>("best_final_equity"),
            "best_weights_json": r.get::<String,_>("best_weights_json"),
            "created_at": r.get::<String,_>("created_at"),
        }));
    }

    (StatusCode::OK, Json(out)).into_response()
}
