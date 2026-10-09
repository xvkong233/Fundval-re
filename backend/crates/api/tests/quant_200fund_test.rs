//! 200 基金全量回测验证：使用调优出的最优配置，
//! 在 200 只基金、1000 天全量数据上验证胜率。
//!
//! 运行前需先跑 quant_tuning_test 生成 /tmp/quant_tune_best.json。

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

#[test]
fn backtest_200_funds() {
    // 读取调优最优配置
    let cfg_str = std::fs::read_to_string("/tmp/quant_tune_best.json")
        .expect("先运行 quant_tuning_test 生成 /tmp/quant_tune_best.json");
    let cfg: serde_json::Value =
        serde_json::from_str(&cfg_str).expect("解析调优配置");
    let l2 = cfg["l2"].as_f64().unwrap();
    let epochs = cfg["epochs"].as_u64().unwrap() as usize;
    let thresh = cfg["thresh"].as_f64().unwrap();
    let tp_pct = cfg["tp"].as_f64().unwrap();
    let sl_pct = cfg["sl"].as_f64().unwrap();
    println!(
        "\n使用调优配置: l2={l2} epochs={epochs} thresh={thresh:.2} tp={tp_pct}/sl={sl_pct}"
    );

    // 200 基金，1000 天全量数据
    let n_funds = 200;
    let days = 1000;
    println!("生成 {n_funds} 只基金 × {days} 天数据...");
    let all_navs: Vec<Vec<f64>> =
        (0..n_funds).map(|fi| gen_nav(20000 + fi as u64, days)).collect();

    // Pooled 训练 + 信号生成（全量数据）
    let train_window = 400;
    let test_window = 60;
    let n = days;
    let mut all_signals: Vec<Vec<Option<TradingSignal>>> =
        (0..n_funds).map(|_| vec![None; n]).collect();

    let mut start = 0;
    let mut window_idx = 0;
    while start + train_window < n {
        let train_end = start + train_window;
        let test_end = (train_end + test_window).min(n);
        window_idx += 1;

        let mut x_tr: Vec<Vec<f64>> = Vec::new();
        let mut y_dip: Vec<f64> = Vec::new();
        let mut y_magic: Vec<f64> = Vec::new();
        for navs in &all_navs {
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

        let n_pos = y_dip.iter().filter(|v| **v >= 0.5).count() as f64;
        let n_neg = y_dip.len() as f64 - n_pos;
        let mk_cfg = |y: &[f64]| {
            let np = y.iter().filter(|v| **v >= 0.5).count() as f64;
            let nn = y.len() as f64 - np;
            LogRegTrainConfig {
                learning_rate: 0.5,
                epochs,
                l2,
                pos_weight: if np > 0.0 { nn / np } else { 1.0 },
            }
        };
        if let (Some(m_dip), Some(m_magic)) = (
            train_logreg(&x_tr, &y_dip, &mk_cfg(&y_dip)),
            train_logreg(&x_tr, &y_magic, &mk_cfg(&y_magic)),
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
                enter_threshold: 0.0,
            };
            for (fi, navs) in all_navs.iter().enumerate() {
                let labeled: Vec<(String, f64)> = navs
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| (format!("d{i}"), v))
                    .collect();
                for i in train_end..test_end {
                    let feat = build_features(&labeled, i);
                    if let Some(mut sig) =
                        sig_gen.generate(&format!("d{i}"), "TEST", &feat, feat[3])
                    {
                        sig.take_profit_pct = tp_pct;
                        sig.stop_loss_pct = sl_pct;
                        all_signals[fi][i] = Some(sig);
                    }
                }
            }
        }
        println!("  窗口 {window_idx}: 训练样本 {} 个", x_tr.len());
        let _ = n_neg;
        start += test_window;
    }

    // 回测每只基金
    println!("\n回测 {n_funds} 只基金...");
    let mut win_rates: Vec<f64> = Vec::new();
    let mut n_qualified = 0;
    let mut n_pass = 0;
    let mut total_trades_all = 0;
    let mut dist = [0; 10]; // 胜率分布直方图

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
                    if combined >= thresh {
                        Some(sig.clone())
                    } else {
                        None
                    }
                })
            })
            .collect();
        let mut scfg = StrategyConfig::default();
        scfg.enter_threshold = thresh;
        let strategy = Strategy::new(scfg);
        let result = backtest_single_fund(
            &format!("F{fi}"),
            &nav_series,
            &|i| filtered[i].clone(),
            &strategy,
            100_000.0,
        );
        let m = &result.metrics;
        if m.total_trades >= 3 {
            n_qualified += 1;
            win_rates.push(m.win_rate);
            total_trades_all += m.total_trades;
            if m.win_rate >= 0.80 {
                n_pass += 1;
            }
            let bucket = (m.win_rate * 10.0).floor() as usize;
            dist[bucket.min(9)] += 1;
        }
        if fi % 50 == 49 {
            println!("  已回测 {} 只...", fi + 1);
        }
    }

    win_rates.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg_wr = if win_rates.is_empty() {
        0.0
    } else {
        win_rates.iter().sum::<f64>() / win_rates.len() as f64
    };
    let median_wr = if win_rates.is_empty() {
        0.0
    } else {
        win_rates[win_rates.len() / 2]
    };

    println!("\n=== 200 基金回测汇总 ===");
    println!("可评估基金: {n_qualified}/{n_funds}（交易数>=3）");
    println!("胜率达标 (>=80%): {n_pass}/{n_qualified}");
    println!("平均胜率: {avg_wr:.1}%");
    println!("中位胜率: {median_wr:.1}%");
    println!("总交易数: {total_trades_all}");
    println!("胜率分布:");
    for (i, c) in dist.iter().enumerate() {
        println!("  {}-{}%: {}", i * 10, (i + 1) * 10, c);
    }

    // 写出详细结果
    let detail: Vec<serde_json::Value> = win_rates
        .iter()
        .enumerate()
        .map(|(i, wr)| serde_json::json!({"rank": i, "win_rate": wr}))
        .collect();
    std::fs::write(
        "/tmp/quant_200fund_result.json",
        serde_json::to_string_pretty(&serde_json::json!({
            "n_funds": n_funds,
            "n_qualified": n_qualified,
            "n_pass": n_pass,
            "avg_win_rate": avg_wr,
            "median_win_rate": median_wr,
            "total_trades": total_trades_all,
            "dist": dist,
        }))
        .unwrap(),
    )
    .expect("写结果");
    let _ = detail;

    assert!(
        n_qualified > 0 && n_pass as f64 / n_qualified as f64 >= 0.70,
        "至少70%的可评估基金胜率应达80%+，实际 {n_pass}/{n_qualified}"
    );
}
