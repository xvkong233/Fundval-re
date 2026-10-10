//! ATR自适应止损 vs 固定止损对比（学习 Trader 海龟交易法）。
//!
//! 固定：8%止盈 / 4%止损
//! ATR：止盈 = 3*ATR，止损 = 2*ATR（随波动率自适应）

use api::ml::dataset::trade_profitable;
use api::ml::features::build_features;
use api::ml::logreg::{train_logreg, LogRegTrainConfig};
use api::quant::backtest::backtest_single_fund;
use api::quant::calibration::PlattCalibrator;
use api::quant::regime::entry_allowed;
use api::quant::signal::{SignalGenerator, TradingSignal};
use api::quant::strategy::{Strategy, StrategyConfig};

mod common;

#[test]
fn atr_stop_comparison() {
    // 随机抽样150只（seed=7，与v2测试不同的种子，验证泛化）
    let funds = common::load_random_funds(150, 7, 800);
    println!("\n随机抽样 {} 只基金（seed=7）", funds.len());

    // 训练模型
    let mut all_x = Vec::new();
    let mut all_y = Vec::new();
    for (_, navs, lt) in &funds {
        let n = navs.len();
        let train_end = n * 70 / 100;
        let labeled = (*navs).clone();
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

    // 两种策略配置
    let mut cfg_fixed = StrategyConfig::default();
    cfg_fixed.enter_threshold = 0.55;
    cfg_fixed.use_atr_stop = false;
    let strat_fixed = Strategy::new(cfg_fixed);

    let mut cfg_atr = StrategyConfig::default();
    cfg_atr.enter_threshold = 0.55;
    cfg_atr.use_atr_stop = true;
    cfg_atr.atr_stop_mult = 4.0; // 对标固定4%：典型ATR~1%，4x=4%
    cfg_atr.atr_profit_mult = 8.0; // 对标固定8%：8x=8%
    let strat_atr = Strategy::new(cfg_atr);

    struct Agg {
        wr: f64,
        n: usize,
        trades: usize,
        ret: f64,
    }
    let mut agg_fixed = Agg { wr: 0.0, n: 0, trades: 0, ret: 0.0 };
    let mut agg_atr = Agg { wr: 0.0, n: 0, trades: 0, ret: 0.0 };

    for (code, navs, lt) in &funds {
        let n = navs.len();
        let test_start = n * 70 / 100;
        if n - test_start < 100 {
            continue;
        }
        let test_navs = &navs[test_start..];

        // 信号（含regime过滤）
        let sigs: Vec<Option<TradingSignal>> = (0..test_navs.len())
            .map(|i| {
                let gi = test_start + i;
                if !entry_allowed(navs, gi).0 {
                    return None;
                }
                let mut feat = build_features(test_navs, i);
                feat.extend_from_slice(lt);
                sig_gen.generate(&test_navs[i].0, code, &feat, feat[3]).and_then(|sig| {
                    let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                    if combined >= 0.55 {
                        Some(sig)
                    } else {
                        None
                    }
                })
            })
            .collect();

        for (strat, agg) in [(&strat_fixed, &mut agg_fixed), (&strat_atr, &mut agg_atr)] {
            let result =
                backtest_single_fund(code, test_navs, &|i| sigs[i].clone(), strat, 100_000.0);
            if result.metrics.total_trades >= 3 {
                agg.n += 1;
                agg.wr += result.metrics.win_rate;
                agg.trades += result.metrics.total_trades;
                agg.ret += result.metrics.total_return_pct;
            }
        }
    }

    println!("\n=== 固定止损 vs ATR自适应止损（含regime过滤，阈值0.55） ===");
    for (name, a) in [("固定8%/4%", &agg_fixed), ("ATR 4x/8x", &agg_atr)] {
        println!(
            "{:<12} 可评估{:>3} 平均胜率{:>5.1}% 平均交易{:>5.1} 平均收益{:>7.1}%",
            name,
            a.n,
            if a.n > 0 { a.wr / a.n as f64 * 100.0 } else { 0.0 },
            if a.n > 0 { a.trades as f64 / a.n as f64 } else { 0.0 },
            if a.n > 0 { a.ret / a.n as f64 } else { 0.0 },
        );
    }
}
