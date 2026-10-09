//! 量化模型调优：在 20 基金子集上网格搜索超参数，
//! 最优配置再到 200 基金全量验证。
//!
//! 高效设计：每个 (l2, epochs) 训练配置只训练一次、生成一次信号，
//! 然后在信号上评估多种 (enter_threshold, take_profit, stop_loss) 组合，
//! 无需重复训练。

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

/// 交易结果模拟（含追踪止损），与策略回测逻辑对齐
fn trade_outcome(
    navs: &[f64],
    entry_idx: usize,
    tp_pct: f64,
    sl_pct: f64,
    max_hold: usize,
) -> bool {
    if entry_idx >= navs.len() {
        return false;
    }
    let entry = navs[entry_idx];
    if entry <= 0.0 {
        return false;
    }
    let tp_nav = entry * (1.0 + tp_pct / 100.0);
    let sl_nav = entry * (1.0 - sl_pct / 100.0);
    let mut highest = entry;
    for k in 1..=max_hold {
        if entry_idx + k >= navs.len() {
            break;
        }
        let nav = navs[entry_idx + k];
        if nav > highest {
            highest = nav;
        }
        if nav >= tp_nav {
            return true;
        }
        if nav <= sl_nav {
            return false;
        }
        if nav > entry && (highest - nav) / highest * 100.0 >= 3.0 {
            return (nav / entry - 1.0) * 100.0 > 0.5;
        }
    }
    let exit_idx = (entry_idx + max_hold).min(navs.len() - 1);
    (navs[exit_idx] / entry - 1.0) * 100.0 > 0.5
}

fn balanced_cfg(y: &[f64], l2: f64, epochs: usize) -> LogRegTrainConfig {
    let n_pos = y.iter().filter(|v| **v >= 0.5).count() as f64;
    let n_neg = y.len() as f64 - n_pos;
    LogRegTrainConfig {
        learning_rate: 0.5,
        epochs,
        l2,
        pos_weight: if n_pos > 0.0 { n_neg / n_pos } else { 1.0 },
    }
}

pub struct TuneConfig {
    pub l2: f64,
    pub epochs: usize,
    pub tp_pct: f64,
    pub sl_pct: f64,
    pub enter_threshold: f64,
}

/// 在给定训练配置下，pooled 训练并生成全基金信号（只做一次）
fn train_and_signal(
    all_navs: &[Vec<f64>],
    train_window: usize,
    test_window: usize,
    l2: f64,
    epochs: usize,
    tp_pct: f64,
    sl_pct: f64,
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
                let profitable = trade_outcome(navs, i, tp_pct, sl_pct, 20);
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
            let cfg_dip = balanced_cfg(&y_dip, l2, epochs);
            let cfg_magic = balanced_cfg(&y_magic, l2, epochs);
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

                // 注意：信号里的 tp/sl 固定为训练用的值，保证标签-回测一致
                let sig_gen = SignalGenerator {
                    dip_buy_model: m_dip,
                    magic_rebound_model: m_magic,
                    dip_buy_calibrator: cal_dip,
                    magic_calibrator: cal_magic,
                    enter_threshold: 0.0, // 不在此过滤，后续按阈值筛选
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
                        // 用训练时的 tp/sl 覆盖信号默认值，保证一致
                        if let Some(mut sig) =
                            sig_gen.generate(&format!("d{i}"), "TEST", &feat, vol20)
                        {
                            sig.take_profit_pct = tp_pct;
                            sig.stop_loss_pct = sl_pct;
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

pub struct FundResult {
    pub win_rate: f64,
    pub n_trades: usize,
    pub total_ret: f64,
}

/// 在已生成信号上评估单个 (threshold) 配置，返回每只基金的结果
fn eval_threshold(
    all_navs: &[Vec<f64>],
    all_signals: &[Vec<Option<TradingSignal>>],
    enter_threshold: f64,
    tp_pct: f64,
    sl_pct: f64,
    initial_capital: f64,
) -> Vec<FundResult> {
    let mut out = Vec::with_capacity(all_navs.len());
    for (fi, navs) in all_navs.iter().enumerate() {
        let dates: Vec<String> = (0..navs.len()).map(|i| format!("d{i}")).collect();
        let nav_series: Vec<(String, f64)> =
            dates.into_iter().zip(navs.iter().cloned()).collect();
        let signals = &all_signals[fi];
        let filtered: Vec<Option<TradingSignal>> = signals
            .iter()
            .map(|s| {
                s.as_ref().and_then(|sig| {
                    let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                    if combined >= enter_threshold {
                        Some(sig.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();
        let mut cfg = StrategyConfig::default();
        cfg.enter_threshold = enter_threshold;
        let strategy = Strategy::new(cfg);
        // 策略内部用信号自带的 tp/sl（已设为训练值）
        let _ = (tp_pct, sl_pct);
        let result = backtest_single_fund(
            &format!("F{fi}"),
            &nav_series,
            &|i| filtered[i].clone(),
            &strategy,
            initial_capital,
        );
        out.push(FundResult {
            win_rate: result.metrics.win_rate,
            n_trades: result.metrics.total_trades,
            total_ret: result.metrics.total_return_pct,
        });
    }
    out
}

/// 汇总：达标基金数（胜率>=80% 且 交易数>=3）、平均胜率、总交易数
fn summarize(results: &[FundResult]) -> (usize, f64, usize) {
    let qualified: Vec<&FundResult> =
        results.iter().filter(|r| r.n_trades >= 3).collect();
    let n_pass = qualified.iter().filter(|r| r.win_rate >= 0.80).count();
    let avg_wr = if qualified.is_empty() {
        0.0
    } else {
        qualified.iter().map(|r| r.win_rate).sum::<f64>() / qualified.len() as f64
    };
    let total_trades: usize = qualified.iter().map(|r| r.n_trades).sum();
    (n_pass, avg_wr, total_trades)
}

#[test]
fn tune_on_20_funds() {
    // 20 基金子集，1000 天全量历史
    let n_funds = 20;
    let days = 1000;
    let all_navs: Vec<Vec<f64>> =
        (0..n_funds).map(|fi| gen_nav(9000 + fi as u64, days)).collect();

    // 阶段1：固定训练超参，扫阈值 × 止盈止损（无需重训练）
    let l2 = 0.1;
    let epochs = 600;
    let mut best: Option<(f64, f64, f64, usize, f64)> = None; // (thresh, tp, sl, n_pass, avg_wr)
    // 先用一组 tp/sl 训练生成信号
    for (tp_pct, sl_pct) in [(6.0, 3.0), (8.0, 4.0), (5.0, 2.5)] {
        let signals = train_and_signal(&all_navs, 400, 60, l2, epochs, tp_pct, sl_pct);
        for thresh in [0.80, 0.85, 0.90] {
            let results =
                eval_threshold(&all_navs, &signals, thresh, tp_pct, sl_pct, 100_000.0);
            let (n_pass, avg_wr, total_trades) = summarize(&results);
            let n_qualified =
                results.iter().filter(|r| r.n_trades >= 3).count();
            println!(
                "tune: tp={tp_pct}/sl={sl_pct} thresh={thresh:.2} -> 达标 {n_pass}/{n_qualified} 只, 平均胜率 {avg_wr:.1}%, 总交易 {total_trades}"
            );
            let score = n_pass as f64 * 1000.0 + avg_wr * 100.0;
            let best_score = best.map(|(_, _, _, np, aw)| np as f64 * 1000.0 + aw * 100.0).unwrap_or(-1.0);
            if score > best_score {
                best = Some((thresh, tp_pct, sl_pct, n_pass, avg_wr));
            }
        }
    }
    let (b_thresh, b_tp, b_sl, b_pass, b_wr) = best.expect("应有调优结果");
    println!(
        "\n阶段1最优: thresh={b_thresh:.2} tp={b_tp}/sl={b_sl} 达标{b_pass}只 平均胜率{b_wr:.1}%"
    );

    // 阶段2：固定阈值与止盈止损，扫训练超参 l2 × epochs
    let mut best2: Option<(f64, usize, usize, f64)> = None; // (l2, epochs, n_pass, avg_wr)
    for l2c in [0.05, 0.1, 0.3] {
        for epochs_c in [400, 800] {
            let signals =
                train_and_signal(&all_navs, 400, 60, l2c, epochs_c, b_tp, b_sl);
            let results =
                eval_threshold(&all_navs, &signals, b_thresh, b_tp, b_sl, 100_000.0);
            let (n_pass, avg_wr, _) = summarize(&results);
            println!(
                "tune2: l2={l2c} epochs={epochs_c} -> 达标 {n_pass} 只, 平均胜率 {avg_wr:.1}%"
            );
            let score = n_pass as f64 * 1000.0 + avg_wr * 100.0;
            let best_score =
                best2.map(|(_, _, np, aw)| np as f64 * 1000.0 + aw * 100.0).unwrap_or(-1.0);
            if score > best_score {
                best2 = Some((l2c, epochs_c, n_pass, avg_wr));
            }
        }
    }
    let (b_l2, b_epochs, b2_pass, b2_wr) = best2.expect("应有调优结果");
    println!(
        "\n最终最优: l2={b_l2} epochs={b_epochs} thresh={b_thresh:.2} tp={b_tp}/sl={b_sl} 达标{b2_pass}只 平均胜率{b2_wr:.1}%"
    );

    // 写出最优配置供 200 基金验证使用
    std::fs::write(
        "/tmp/quant_tune_best.json",
        format!(
            "{{\"l2\":{b_l2},\"epochs\":{b_epochs},\"thresh\":{b_thresh},\"tp\":{b_tp},\"sl\":{b_sl}}}"
        ),
    )
    .expect("写调优结果");
}
