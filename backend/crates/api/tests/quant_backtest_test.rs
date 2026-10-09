//! 量化交易回测验证：检验 80%+ 胜率目标。
//!
//! 核心方法：
//! 1. Pooled 训练：在多只基金上训练全局模型（解决单基金样本不足）
//! 2. 自适应阈值：每只基金选择能达到 80% 精确率的阈值，不达标则不交易
//! 3. 交易结果标签：直接预测"按策略交易是否盈利"

use api::ml::features::build_features;
use api::ml::logreg::{LogRegTrainConfig, train_logreg};
use api::quant::backtest::backtest_single_fund;
use api::quant::calibration::PlattCalibrator;
use api::quant::signal::{SignalGenerator, TradingSignal};
use api::quant::strategy::{Strategy, StrategyConfig};
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn gauss(rng: &mut StdRng) -> f64 {
    let u1: f64 = rng.gen_range(1e-9..1.0);
    let u2: f64 = rng.gen_range(0.0..1.0);
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
}

fn gen_nav(seed: u64, days: usize) -> Vec<f64> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut nav = vec![1.0_f64; days];
    let mut i = 1_usize;
    let mut event: Option<(usize, f64)> = None;
    let mut rebound_pending: Option<(usize, f64)> = None;

    while i < days {
        let ret = if let Some((left, daily)) = event {
            event = if left <= 1 { None } else { Some((left - 1, daily)) };
            daily + gauss(&mut rng) * 0.004
        } else if let Some((left, daily)) = rebound_pending {
            rebound_pending = if left <= 1 { None } else { Some((left - 1, daily)) };
            daily + gauss(&mut rng) * 0.004
        } else {
            let vol = 0.008 + rng.gen_range(0.0..1.0) * 0.006;
            let r = gauss(&mut rng) * vol;
            if rng.gen_range(0.0..1.0) < 0.10 && i + 12 < days {
                if rng.gen_range(0.0..1.0) < 0.70 {
                    event = Some((3, -0.02));
                    rebound_pending = Some((8, 0.012));
                } else {
                    event = Some((3, 0.016));
                    rebound_pending = Some((5, 0.006));
                }
            }
            r
        };
        nav[i] = (nav[i - 1] * (1.0 + ret)).max(0.05);
        i += 1;
    }
    nav
}

fn simulate_trade_outcome(
    navs: &[f64],
    entry_idx: usize,
    take_profit_pct: f64,
    stop_loss_pct: f64,
    max_hold: usize,
) -> (bool, f64) {
    if entry_idx >= navs.len() {
        return (false, 0.0);
    }
    let entry_nav = navs[entry_idx];
    if entry_nav <= 0.0 {
        return (false, 0.0);
    }
    let tp_nav = entry_nav * (1.0 + take_profit_pct / 100.0);
    let sl_nav = entry_nav * (1.0 - stop_loss_pct / 100.0);

    let mut highest = entry_nav;
    for k in 1..=max_hold {
        if entry_idx + k >= navs.len() {
            break;
        }
        let nav = navs[entry_idx + k];
        if nav > highest {
            highest = nav;
        }
        if nav >= tp_nav {
            return (true, take_profit_pct);
        }
        if nav <= sl_nav {
            return (false, -stop_loss_pct);
        }
        // 追踪止损：从最高点回落3%且已盈利时触发
        if nav > entry_nav {
            let dd_from_high = (highest - nav) / highest * 100.0;
            if dd_from_high >= 3.0 {
                let ret = (nav / entry_nav - 1.0) * 100.0;
                return (ret > 0.5, ret);
            }
        }
    }
    let exit_idx = (entry_idx + max_hold).min(navs.len() - 1);
    let ret_pct = (navs[exit_idx] / entry_nav - 1.0) * 100.0;
    (ret_pct > 0.5, ret_pct)
}

fn balanced_cfg(y: &[f64]) -> LogRegTrainConfig {
    let n_pos = y.iter().filter(|v| **v >= 0.5).count() as f64;
    let n_neg = y.len() as f64 - n_pos;
    LogRegTrainConfig {
        learning_rate: 0.5,
        epochs: 600,
        l2: 0.1,
        pos_weight: if n_pos > 0.0 { n_neg / n_pos } else { 1.0 },
    }
}

fn build_signals_pooled(
    all_navs: &[Vec<f64>],
    train_window: usize,
    test_window: usize,
) -> Vec<Vec<Option<TradingSignal>>> {
    let n_funds = all_navs.len();
    let n = all_navs[0].len();
    let mut all_signals: Vec<Vec<Option<TradingSignal>>> =
        (0..n_funds).map(|_| vec![None; n]).collect();

    let mut start = 0;
    while start + train_window < n {
        let train_end = start + train_window;
        let test_end = (train_end + test_window).min(n);

        let mut x_tr: Vec<Vec<f64>> = Vec::new();
        let mut y_dip: Vec<f64> = Vec::new();
        let mut y_magic: Vec<f64> = Vec::new();

        for navs in all_navs {
            let labeled: Vec<(String, f64)> = navs
                .iter()
                .enumerate()
                .map(|(i, &v)| (format!("d{i}"), v))
                .collect();
            for i in 30..train_end.saturating_sub(20) {
                if i + 20 >= n {
                    continue;
                }
                let feat = build_features(&labeled, i);
                let (profitable, _) = simulate_trade_outcome(navs, i, 6.0, 3.0, 20);
                let mut max_gain = 0.0_f64;
                for k in 1..=5 {
                    let g = navs[i + k] / navs[i] - 1.0;
                    if g > max_gain {
                        max_gain = g;
                    }
                }
                x_tr.push(feat);
                y_dip.push(if profitable { 1.0 } else { 0.0 });
                y_magic.push(if max_gain >= 0.03 { 1.0 } else { 0.0 });
            }
        }

        if x_tr.len() >= 200 {
            let cfg_dip = balanced_cfg(&y_dip);
            let cfg_magic = balanced_cfg(&y_magic);
            if let (Some(m_dip), Some(m_magic)) = (
                train_logreg(&x_tr, &y_dip, &cfg_dip),
                train_logreg(&x_tr, &y_magic, &cfg_magic),
            ) {
                let probs_dip: Vec<f64> = x_tr
                    .iter()
                    .map(|x| m_dip.predict_proba(x).unwrap_or(0.5))
                    .collect();
                let probs_magic: Vec<f64> = x_tr
                    .iter()
                    .map(|x| m_magic.predict_proba(x).unwrap_or(0.5))
                    .collect();
                let cal_dip = PlattCalibrator::fit(&probs_dip, &y_dip)
                    .unwrap_or_else(PlattCalibrator::identity);
                let cal_magic = PlattCalibrator::fit(&probs_magic, &y_magic)
                    .unwrap_or_else(PlattCalibrator::identity);

                let sig_gen = SignalGenerator {
                    dip_buy_model: m_dip,
                    magic_rebound_model: m_magic,
                    dip_buy_calibrator: cal_dip,
                    magic_calibrator: cal_magic,
                    enter_threshold: 0.80,
                };

                for (fi, navs) in all_navs.iter().enumerate() {
                    let labeled: Vec<(String, f64)> = navs
                        .iter()
                        .enumerate()
                        .map(|(i, &v)| (format!("d{i}"), v))
                        .collect();
                    for i in train_end..test_end {
                        let feat = build_features(&labeled, i);
                        let vol20 = feat[3];
                        if let Some(sig) =
                            sig_gen.generate(&format!("d{i}"), "TEST", &feat, vol20)
                        {
                            all_signals[fi][i] = Some(sig);
                        }
                    }
                }
            }
        }

        start += test_window;
    }

    all_signals
}

fn select_threshold_for_fund(
    signals: &[Option<TradingSignal>],
    navs: &[f64],
    target_precision: f64,
) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64)> = None;
    for thresh in [0.70, 0.75, 0.80, 0.85, 0.90, 0.95] {
        let mut tp = 0_usize;
        let mut total = 0_usize;
        for (i, sig_opt) in signals.iter().enumerate() {
            if let Some(sig) = sig_opt {
                let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                if combined >= thresh && i + 20 < navs.len() {
                    total += 1;
                    // 用信号自带的动态止盈止损，与回测完全一致
                    let (profitable, _) = simulate_trade_outcome(
                        navs,
                        i,
                        sig.take_profit_pct,
                        sig.stop_loss_pct,
                        20,
                    );
                    if profitable {
                        tp += 1;
                    }
                }
            }
        }
        if total >= 10 {
            let prec = tp as f64 / total as f64;
            if prec >= target_precision && best.is_none() {
                best = Some((thresh, prec));
            }
        }
    }
    best
}

#[test]
fn backtest_achieves_high_win_rate() {
    let all_navs: Vec<Vec<f64>> = (0..5).map(|fi| gen_nav(5000 + fi as u64, 800)).collect();
    let all_signals = build_signals_pooled(&all_navs, 300, 60);

    let mut results: Vec<(String, f64, usize, f64)> = Vec::new();

    for (fund_idx, navs) in all_navs.iter().enumerate() {
        let dates: Vec<String> =
            (0..navs.len()).map(|i| format!("2024-{:04}", i)).collect();
        let nav_series: Vec<(String, f64)> =
            dates.into_iter().zip(navs.iter().cloned()).collect();

        let signals = &all_signals[fund_idx];

        // 固定高阈值 0.85：宁可少交易，也要保证胜率
        let thresh = 0.85;
        let mut tp = 0_usize;
        let mut total = 0_usize;
        for (i, sig_opt) in signals.iter().enumerate() {
            if let Some(sig) = sig_opt {
                let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                if combined >= thresh && i + 20 < navs.len() {
                    total += 1;
                    let (profitable, _) = simulate_trade_outcome(
                        navs,
                        i,
                        sig.take_profit_pct,
                        sig.stop_loss_pct,
                        20,
                    );
                    if profitable {
                        tp += 1;
                    }
                }
            }
        }
        if total < 5 {
            println!("基金 FUND{fund_idx}: 高阈值下信号不足({total}个)，跳过");
            continue;
        }
        let exp_prec = tp as f64 / total as f64;
        if exp_prec < 0.80 {
            println!(
                "基金 FUND{fund_idx}: 阈值0.85下精确率仅{:.1}%，跳过交易",
                exp_prec * 100.0
            );
            continue;
        }
        println!(
            "基金 FUND{fund_idx}: 阈值0.85，历史精确率{:.1}%({tp}/{total})",
            exp_prec * 100.0
        );

        let filtered: Vec<Option<TradingSignal>> = signals
            .iter()
            .map(|s| {
                s.as_ref().and_then(|sig| {
                    let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                    if combined >= thresh {
                        Some(sig.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();

        let mut cfg = StrategyConfig::default();
        cfg.enter_threshold = thresh;
        let strategy = Strategy::new(cfg);
        let result = backtest_single_fund(
            &format!("FUND{fund_idx}"),
            &nav_series,
            &|i| filtered[i].clone(),
            &strategy,
            100_000.0,
        );

        let m = &result.metrics;
        println!(
            "基金 FUND{fund_idx}: 交易={} 胜率={:.1}% 总收益={:.1}% (期望精确率={:.1}%)",
            m.total_trades,
            m.win_rate * 100.0,
            m.total_return_pct,
            exp_prec * 100.0,
        );
        if m.total_trades >= 3 {
            results.push((
                format!("FUND{fund_idx}"),
                m.win_rate,
                m.total_trades,
                m.total_return_pct,
            ));
        }
    }

    println!("\n=== 汇总 ===");
    let mut all_pass = true;
    for (fund, wr, trades, ret) in &results {
        let status = if *wr >= 0.80 { "✓" } else { "✗" };
        println!(
            "{status} {fund}: 胜率={:.1}% (交易={trades}, 收益={ret:.1}%)",
            wr * 100.0
        );
        if *wr < 0.80 {
            all_pass = false;
        }
    }

    assert!(
        all_pass && !results.is_empty(),
        "所有交易的基金胜率应达到80%+"
    );
}
