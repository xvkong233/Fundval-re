//! 原生量化交易 API：基于 ML 预测的模拟交易与回测。
//!
//! - POST /api/quant-trading/backtest：对指定基金运行回测
//! - GET /api/quant-trading/signals/:fund_code：获取当前交易信号
//! - GET /api/quant-trading/trades/:fund_code：获取历史模拟交易记录

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;

use crate::ml::features::build_features;
use crate::ml::train::{MlTask, get_sector_model};
use crate::quant::backtest::backtest_single_fund;
use crate::quant::calibration::PlattCalibrator;
use crate::quant::daily::{generate_daily_recommendations, DailyRecommendation};
use crate::quant::signal::{SignalGenerator, TradingSignal};
use crate::quant::strategy::{Strategy, StrategyConfig};
use crate::routes::auth;
use crate::state::AppState;
use chrono::{Local, Timelike};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct BacktestRequest {
    pub fund_code: String,
    pub source: Option<String>,
    #[serde(default)]
    pub initial_capital: Option<f64>,
    #[serde(default)]
    pub enter_threshold: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct BacktestResponse {
    pub fund_code: String,
    pub win_rate: f64,
    pub total_trades: usize,
    pub winning_trades: usize,
    pub total_return_pct: f64,
    pub profit_factor: f64,
    pub max_drawdown_pct: f64,
    pub sharpe_ratio: f64,
    pub trades: Vec<TradeView>,
}

#[derive(Debug, Serialize)]
pub struct TradeView {
    pub entry_date: String,
    pub exit_date: String,
    pub entry_nav: f64,
    pub exit_nav: f64,
    pub return_pct: f64,
    pub days_held: usize,
    pub exit_reason: String,
    pub profitable: bool,
}

/// 运行回测
pub async fn backtest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BacktestRequest>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database not available"})),
            )
                .into_response()
        }
    };

    let source = req.source.unwrap_or_else(|| "tiantian".to_string());
    let initial_capital = req.initial_capital.unwrap_or(100_000.0);
    let enter_threshold = req.enter_threshold.unwrap_or(0.80);

    // 1. 加载基金净值历史
    let navs = match load_nav_history(&pool, &req.fund_code, &source).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e})),
            )
                .into_response()
        }
    };
    if navs.len() < 100 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "净值历史不足100个交易日，无法回测"})),
        )
            .into_response();
    }

    // 2. 加载或训练模型（简化：尝试加载板块模型，否则用全局）
    // 实际生产中应有定时训练任务；此处若无模型则返回错误提示先训练
    let peer_code = "__all__"; // 简化：用全局模型
    let dip_model = match get_sector_model(&pool, peer_code, MlTask::DipBuy, 5).await {
        Ok(Some(rec)) => rec.model,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "未找到训练好的模型，请先运行 ML 训练任务"})),
            )
                .into_response()
        }
    };
    let magic_model = match get_sector_model(&pool, peer_code, MlTask::MagicRebound, 5).await {
        Ok(Some(rec)) => rec.model,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "未找到训练好的模型，请先运行 ML 训练任务"})),
            )
                .into_response()
        }
    };

    // 3. 生成信号（使用恒等校准，生产环境应加载已拟合的校准器）
    let sig_gen = SignalGenerator {
        dip_buy_model: dip_model,
        magic_rebound_model: magic_model,
        dip_buy_calibrator: PlattCalibrator::identity(),
        magic_calibrator: PlattCalibrator::identity(),
        enter_threshold,
    };

    let labeled: Vec<(String, f64)> = navs
        .iter()
        .map(|(d, v)| (d.clone(), *v))
        .collect();
    let signals: Vec<Option<TradingSignal>> = (0..navs.len())
        .map(|i| {
            if i < 30 {
                return None;
            }
            let feat = build_features(&labeled, i);
            let vol20 = feat[3];
            sig_gen.generate(&navs[i].0, &req.fund_code, &feat, vol20)
        })
        .collect();

    // 4. 运行回测
    let mut cfg = StrategyConfig::default();
    cfg.enter_threshold = enter_threshold;
    let strategy = Strategy::new(cfg);
    let result = backtest_single_fund(
        &req.fund_code,
        &navs,
        &|i| signals[i].clone(),
        &strategy,
        initial_capital,
    );

    let trades: Vec<TradeView> = result
        .trades
        .iter()
        .map(|t| TradeView {
            entry_date: t.entry_date.clone(),
            exit_date: t.exit_date.clone(),
            entry_nav: t.entry_nav,
            exit_nav: t.exit_nav,
            return_pct: t.return_pct,
            days_held: t.days_held,
            exit_reason: t.exit_reason.as_str().to_string(),
            profitable: t.net_pnl > 0.0,
        })
        .collect();

    let m = result.metrics;
    (
        StatusCode::OK,
        Json(json!(BacktestResponse {
            fund_code: req.fund_code,
            win_rate: m.win_rate,
            total_trades: m.total_trades,
            winning_trades: m.winning_trades,
            total_return_pct: m.total_return_pct,
            profit_factor: m.profit_factor,
            max_drawdown_pct: m.max_drawdown_pct,
            sharpe_ratio: m.sharpe_ratio,
            trades,
        })),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct SignalQuery {
    pub source: Option<String>,
}

/// 获取当前交易信号
pub async fn current_signal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(fund_code): Path<String>,
    Query(q): Query<SignalQuery>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database not available"})),
            )
                .into_response()
        }
    };

    let source = q.source.unwrap_or_else(|| "tiantian".to_string());
    let navs = match load_nav_history(&pool, &fund_code, &source).await {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e})),
            )
                .into_response()
        }
    };
    if navs.len() < 30 {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "净值历史不足"})),
        )
            .into_response();
    }

    let peer_code = "__all__";
    let dip_model = match get_sector_model(&pool, peer_code, MlTask::DipBuy, 5).await {
        Ok(Some(rec)) => rec.model,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "未找到训练好的模型"})),
            )
                .into_response()
        }
    };
    let magic_model = match get_sector_model(&pool, peer_code, MlTask::MagicRebound, 5).await {
        Ok(Some(rec)) => rec.model,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "未找到训练好的模型"})),
            )
                .into_response()
        }
    };

    let sig_gen = SignalGenerator {
        dip_buy_model: dip_model,
        magic_rebound_model: magic_model,
        dip_buy_calibrator: PlattCalibrator::identity(),
        magic_calibrator: PlattCalibrator::identity(),
        enter_threshold: 0.80,
    };

    let labeled: Vec<(String, f64)> =
        navs.iter().map(|(d, v)| (d.clone(), *v)).collect();
    let idx = navs.len() - 1;
    let feat = build_features(&labeled, idx);
    let vol20 = feat[3];
    let (date, nav) = &navs[idx];

    match sig_gen.generate(date, &fund_code, &feat, vol20) {
        Some(sig) => (
            StatusCode::OK,
            Json(json!({
                "fund_code": fund_code,
                "date": date,
                "nav": nav,
                "dip_buy_proba": sig.dip_buy_proba,
                "magic_rebound_proba": sig.magic_rebound_proba,
                "strength": sig.strength.as_str(),
                "enter": sig.enter,
                "take_profit_pct": sig.take_profit_pct,
                "stop_loss_pct": sig.stop_loss_pct,
            })),
        )
            .into_response(),
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "信号生成失败"})),
        )
            .into_response(),
    }
}

async fn load_nav_history(
    pool: &sqlx::AnyPool,
    fund_code: &str,
    source: &str,
) -> Result<Vec<(String, f64)>, String> {
    let rows = sqlx::query(
        r#"
        SELECT CAST(h.nav_date AS TEXT) as nav_date, CAST(h.unit_nav AS TEXT) as unit_nav
        FROM fund_nav_history h
        JOIN fund f ON f.id = h.fund_id
        WHERE f.fund_code = $1 AND h.source_name = $2
        ORDER BY h.nav_date ASC
        LIMIT 2000
        "#,
    )
    .bind(fund_code.trim())
    .bind(source.trim())
    .fetch_all(pool)
    .await
    .map_err(|e| e.to_string())?;

    let mut navs: Vec<(String, f64)> = Vec::with_capacity(rows.len());
    for r in rows {
        let d: String = r.get("nav_date");
        let s: String = r.get("unit_nav");
        if let Ok(v) = s.trim().parse::<f64>() {
            if v > 0.0 {
                navs.push((d, v));
            }
        }
    }
    Ok(navs)
}

// ── 每日可操作建议 ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct DailyRequest {
    /// 基金代码列表
    pub fund_codes: Vec<String>,
    /// 当前持仓：fund_code → 确认日期 (YYYY-MM-DD)
    #[serde(default)]
    pub positions: HashMap<String, String>,
    #[serde(default = "default_buy_thresh")]
    pub buy_threshold: f64,
    #[serde(default = "default_sell_thresh")]
    pub sell_threshold: f64,
}

fn default_buy_thresh() -> f64 { 0.60 }
fn default_sell_thresh() -> f64 { 0.50 }

#[derive(Debug, Serialize)]
pub struct DailyResponse {
    pub date: String,
    pub is_trading_day: bool,
    pub before_cutoff: bool,
    pub recommendations: Vec<DailyRecommendation>,
    pub summary: String,
}

/// POST /api/quant-trading/daily：生成每日可操作建议（含T+1规则）。
///
/// 为每只基金计算模型信号 + 股票透视，结合持仓状态输出 Buy/Sell/Hold/Wait。
pub async fn daily_recommendations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<DailyRequest>,
) -> axum::response::Response {
    let _user_id = match auth::authenticate(&state, &headers) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let pool = match state.pool() {
        Some(p) => p.clone(),
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "database not available"})),
            )
                .into_response()
        }
    };

    let now = Local::now();
    let today = now.date_naive();
    let is_trading = crate::quant::daily::t1_rules::is_trading_day(today);
    let before_cutoff = now.hour() < crate::quant::daily::t1_rules::CUTOFF_HOUR;

    // 加载模型（复用 sector 模型）
    let peer_code = "__all__";
    let dip_model = match get_sector_model(&pool, peer_code, MlTask::DipBuy, 5).await {
        Ok(Some(rec)) => rec.model,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "未找到训练好的模型，请先运行ML训练"})),
            )
                .into_response()
        }
    };
    let magic_model = match get_sector_model(&pool, peer_code, MlTask::MagicRebound, 5).await {
        Ok(Some(rec)) => rec.model,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "未找到训练好的模型"})),
            )
                .into_response()
        }
    };

    // 解析持仓确认日期
    let mut positions: HashMap<String, chrono::NaiveDate> = HashMap::new();
    for (code, date_str) in &req.positions {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            positions.insert(code.clone(), d);
        }
    }

    // 为每只基金计算信号
    let mut signals: Vec<(String, String, f64, Option<f64>)> = Vec::new();
    for fund_code in &req.fund_codes {
        let navs = match load_nav_history(&pool, fund_code, "tiantian").await {
            Ok(v) => v,
            Err(_) => continue,
        };
        if navs.len() < 60 {
            continue;
        }
        let idx = navs.len() - 1;
        let feat = build_features(&navs, idx);
        // 简化：用模型直接预测（实际应走 SignalGenerator + 校准）
        let proba = dip_model.predict_proba(&feat).unwrap_or(0.5);
        // TODO: 股票透视需持仓数据，暂为 None
        signals.push((fund_code.clone(), fund_code.clone(), proba, None));
    }

    let recs = generate_daily_recommendations(
        now,
        &signals,
        &positions,
        req.buy_threshold,
        req.sell_threshold,
    );

    let n_buy = recs.iter().filter(|r| matches!(r.action, crate::quant::daily::Action::Buy)).count();
    let n_sell = recs.iter().filter(|r| matches!(r.action, crate::quant::daily::Action::Sell)).count();
    let summary = format!(
        "{}：建议买入{n_buy}只，卖出{n_sell}只，共评估{}只基金",
        today, recs.len()
    );

    (
        StatusCode::OK,
        Json(DailyResponse {
            date: today.to_string(),
            is_trading_day: is_trading,
            before_cutoff,
            recommendations: recs,
            summary,
        }),
    )
        .into_response()
}
