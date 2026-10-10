//! 市场状态过滤验证：只在沪深300处于上升趋势时交易，看胜率是否提升。

mod common;

use api::ml::dataset::trade_profitable;
use api::ml::features::build_features;
use api::ml::logreg::{train_logreg, LogRegTrainConfig};
use api::quant::backtest::backtest_single_fund;
use api::quant::calibration::PlattCalibrator;
use api::quant::signal::{SignalGenerator, TradingSignal};
use api::quant::strategy::{Strategy, StrategyConfig};
use std::collections::HashMap;

fn load_index() -> HashMap<String, f64> {
    let text = std::fs::read_to_string(format!("{}/klines_index_000300.json", common::DATA_DIR)).unwrap_or_default();
    let items: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
    let mut out = HashMap::new();
    for item in items {
        let date = item.get("date").and_then(|d| d.as_str()).unwrap_or("").to_string();
        let close: f64 = item.get("close").and_then(|c| c.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
        if !date.is_empty() && close > 0.0 {
            out.insert(date, close);
        }
    }
    out
}

/// 市场是否处于上升趋势：沪深300的20日收益 > 0
fn is_bull_market(index: &HashMap<String, f64>, date: &str) -> bool {
    let mut sorted: Vec<&String> = index.keys().collect();
    sorted.sort();
    let pos = match sorted.iter().position(|d| d.as_str() == date) {
        Some(p) => p,
        None => return false,
    };
    if pos < 20 {
        return false;
    }
    let curr = index[sorted[pos].as_str()];
    let prev = index[sorted[pos - 20].as_str()];
    prev > 0.0 && curr / prev - 1.0 > 0.02 // 20日涨超2%算牛市
}

#[test]
fn regime_filter_test() {
    let index = load_index();
    println!("\n沪深300数据: {} 条", index.len());

    // 加载基金（随机抽样200只，可复现）
    let funds = common::load_random_funds(200, 42, 800);
    println!("加载 {} 只基金（随机抽样）", funds.len());

    // 训练（16特征）
    let mut all_x = Vec::new();
    let mut all_y = Vec::new();
    for (_, navs, lt) in &funds {
        let n = navs.len();
        let train_end = n * 70 / 100;
        let labeled = navs.clone();
        for idx in 60..train_end {
            let mut x = build_features(navs, idx);
            x.extend_from_slice(lt);
            let y = if trade_profitable(&labeled, idx, 20, 8.0, 4.0) { 1.0 } else { 0.0 };
            all_x.push(x);
            all_y.push(y);
        }
    }
    let n_pos = all_y.iter().filter(|v| **v >= 0.5).count() as f64;
    let cfg = LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 400,
        l2: 0.05,
        pos_weight: (all_y.len() as f64 - n_pos) / n_pos.max(1.0),
    };
    let model = train_logreg(&all_x, &all_y, &cfg).expect("train");
    let probs: Vec<f64> = all_x.iter().map(|x| model.predict_proba(x).unwrap_or(0.5)).collect();
    let cal = PlattCalibrator::fit(&probs, &all_y).unwrap_or_else(PlattCalibrator::identity);
    let sig_gen = SignalGenerator {
        dip_buy_model: model.clone(),
        magic_rebound_model: model,
        dip_buy_calibrator: cal.clone(),
        magic_calibrator: cal,
        enter_threshold: 0.0,
    };

    // 对比：不同阈值 × 是否牛市过滤
    for thresh in [0.55, 0.58, 0.60, 0.62] {
    for use_regime in [false, true] {
        let mut qualified = 0;
        let mut total_wr = 0.0;
        let mut total_trades = 0;
        let mut total_ret = 0.0;
        let mut total_pf = 0.0;
        let mut scfg = StrategyConfig::default();
        scfg.enter_threshold = thresh;
        let strategy = Strategy::new(scfg);

        for (code, navs, lt) in &funds {
            let n = navs.len();
            let test_start = n * 70 / 100;
            if n - test_start < 100 {
                continue;
            }
            let test_navs = &navs[test_start..];
            let signals: Vec<Option<TradingSignal>> = (0..test_navs.len())
                .map(|i| {
                    let mut feat = build_features(test_navs, i);
                    feat.extend_from_slice(lt);
                    // 市场状态过滤
                    if use_regime && !is_bull_market(&index, &test_navs[i].0) {
                        return None;
                    }
                    sig_gen.generate(&test_navs[i].0, code, &feat, feat[3]).and_then(|sig| {
                        let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                        if combined >= thresh { Some(sig) } else { None }
                    })
                })
                .collect();
            let result = backtest_single_fund(code, test_navs, &|i| signals[i].clone(), &strategy, 100_000.0);
            let m = &result.metrics;
            if m.total_trades >= 3 {
                qualified += 1;
                total_wr += m.win_rate;
                total_trades += m.total_trades;
                total_ret += m.total_return_pct;
                total_pf += m.profit_factor;
            }
        }
        let label = format!("阈值{thresh} {}", if use_regime { "牛市过滤" } else { "无过滤" });
        println!(
            "{label}: 可评估{qualified} 胜率{:.1}% 平均收益{:.1}% 盈亏比{:.2} 总交易{total_trades}",
            if qualified > 0 { total_wr / qualified as f64 * 100.0 } else { 0.0 },
            if qualified > 0 { total_ret / qualified as f64 * 100.0 } else { 0.0 },
            if qualified > 0 { total_pf / qualified as f64 } else { 0.0 },
        );
    }
    }
}
