//! 全策略对比验证：8大开源项目量化思想在真实基金数据上的大比武。
//!
//! 对比策略：
//! 1. buy_hold — 买入持有（基准）
//! 2. trend — 趋势跟踪（Trader式）
//! 3. portfolio_majority — 多策略投票（VNPy式）
//! 4. portfolio_weighted — 多策略加权（VNPy式）
//! 5. ml — 现有ML择时模型
//!
//! 另做单因子 IC 测试（QUANTAXIS式因子研究）。

use api::ml::dataset::trade_profitable;
use api::ml::features::build_features;
use api::ml::logreg::{train_logreg, LogRegTrainConfig};
use api::quant::calibration::PlattCalibrator;
use api::quant::factors::{test_all_factors, FACTOR_NAMES};
use api::quant::portfolio::{backtest_portfolio, CombineMode};
use api::quant::signal::SignalGenerator;
use api::quant::trend::{backtest_buy_hold, backtest_trend, TrendParams};
use std::collections::HashMap;

const DATA_DIR: &str = "/home/hatch/workspace/fund_data";

fn load_funds() -> HashMap<String, Vec<(String, f64)>> {
    let mut funds = HashMap::new();
    let entries = std::fs::read_dir(DATA_DIR).expect("read dir");
    for entry in entries.flatten() {
        let name = entry.file_name().into_string().unwrap_or_default();
        if !name.starts_with("nav_") || !name.ends_with(".json") {
            continue;
        }
        let code = name[4..name.len() - 5].to_string();
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let items: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
        let mut navs: Vec<(String, f64)> = Vec::new();
        for item in items {
            let date = item.get("date").and_then(|d| d.as_str()).unwrap_or("").to_string();
            let nav: f64 = item.get("nav").and_then(|n| n.as_str()).and_then(|s| s.parse().ok()).unwrap_or(0.0);
            if !date.is_empty() && nav > 0.0 {
                navs.push((date, nav));
            }
        }
        if navs.len() >= 800 {
            funds.insert(code, navs);
        }
    }
    funds
}

#[test]
fn strategy_showdown() {
    let funds = load_funds();
    println!("\n加载 {} 只基金", funds.len());

    // ── 1. 单因子 IC 测试 ──
    println!("\n=== 单因子 IC 测试（horizon=20天） ===");
    let ic_results = test_all_factors(&funds, 20, 20);
    for r in ic_results.iter().take(10) {
        println!(
            "  {:16} IC={:+.4} ICIR={:.3} 胜率={:.0}%",
            r.factor_name, r.ic_mean, r.icir, r.ic_hit_rate * 100.0
        );
    }
    let _ = FACTOR_NAMES;

    // ── 2. 训练ML模型（用于ml策略和组合策略） ──
    println!("\n训练ML模型...");
    let mut all_x = Vec::new();
    let mut all_y = Vec::new();
    let mut fund_list: Vec<(&String, &Vec<(String, f64)>)> = funds.iter().collect();
    fund_list.sort_by(|a, b| a.0.cmp(b.0));
    for (_, navs) in &fund_list {
        let n = navs.len();
        let train_end = n * 70 / 100;
        let labeled = (*navs).clone();
        for idx in 60..train_end {
            let x = build_features(navs, idx);
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

    // ── 3. 全策略回测对比 ──
    #[derive(Default)]
    struct Agg {
        ret: f64,
        dd: f64,
        trades: usize,
        n: usize,
    }
    let mut agg_bh = Agg::default();
    let mut agg_trend = Agg::default();
    let mut agg_maj = Agg::default();
    let mut agg_wtd = Agg::default();
    let mut agg_ml = Agg::default();

    let trend_params = TrendParams::default();

    for (code, navs) in &fund_list {
        let n = navs.len();
        let start = n * 70 / 100;
        if n - start < 100 {
            continue;
        }

        // ML 概率序列
        let ml_probas: Vec<Option<f64>> = (0..n)
            .map(|i| {
                if i < start {
                    return None;
                }
                let feat = build_features(&navs[..=i.min(n - 1)].to_vec(), i.min(n - 1));
                sig_gen
                    .generate(&navs[i.min(n - 1)].0, code, &feat, feat[3])
                    .map(|sig| Some(sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4))
                    .unwrap_or(None)
            })
            .collect();

        // 1. 买入持有
        let (r, dd) = backtest_buy_hold(navs, start);
        agg_bh.ret += r;
        agg_bh.dd += dd;
        agg_bh.n += 1;

        // 2. 趋势跟踪
        let (r, t, dd) = backtest_trend(navs, &trend_params, start);
        agg_trend.ret += r;
        agg_trend.dd += dd;
        agg_trend.trades += t;
        agg_trend.n += 1;

        // 3. 组合-多数投票
        let (r, t, dd) = backtest_portfolio(navs, CombineMode::Majority, &ml_probas, start);
        agg_maj.ret += r;
        agg_maj.dd += dd;
        agg_maj.trades += t;
        agg_maj.n += 1;

        // 4. 组合-加权
        let (r, t, dd) = backtest_portfolio(navs, CombineMode::Weighted { threshold: 0.05 }, &ml_probas, start);
        agg_wtd.ret += r;
        agg_wtd.dd += dd;
        agg_wtd.trades += t;
        agg_wtd.n += 1;

        // 5. 纯ML（阈值0.55）
        let mut cash = 1.0;
        let mut shares = 0.0;
        let mut peak = 1.0;
        let mut mdd = 0.0;
        let mut trades = 0;
        for idx in start..n {
            let price = navs[idx].1;
            let hold = ml_probas[idx].map_or(false, |p| p >= 0.55);
            let value = cash + shares * price;
            if value > peak {
                peak = value;
            }
            let dd = (peak - value) / peak;
            if dd > mdd {
                mdd = dd;
            }
            if hold && shares == 0.0 {
                shares = cash * (1.0 - 0.0015) / price;
                cash = 0.0;
                trades += 1;
            } else if !hold && shares > 0.0 {
                cash = shares * price * (1.0 - 0.005);
                shares = 0.0;
                trades += 1;
            }
        }
        let final_v = cash + shares * navs[n - 1].1;
        agg_ml.ret += final_v - 1.0;
        agg_ml.dd += mdd;
        agg_ml.trades += trades;
        agg_ml.n += 1;
    }

    println!("\n=== 全策略对比（{}只基金，后30%数据，含费用） ===", agg_bh.n);
    println!("{:<20} {:>10} {:>10} {:>10}", "策略", "平均收益", "平均回撤", "平均交易");
    let show = |name: &str, a: &Agg| {
        println!(
            "{:<20} {:>9.1}% {:>9.1}% {:>10.1}",
            name,
            a.ret / a.n as f64 * 100.0,
            a.dd / a.n as f64 * 100.0,
            a.trades as f64 / a.n as f64,
        );
    };
    show("买入持有(基准)", &agg_bh);
    show("趋势跟踪", &agg_trend);
    show("多策略-多数投票", &agg_maj);
    show("多策略-加权", &agg_wtd);
    show("纯ML择时", &agg_ml);
}
