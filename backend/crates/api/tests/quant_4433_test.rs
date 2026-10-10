//! 4433选基 + ML择时 双层策略验证。

mod common;

use api::ml::dataset::trade_profitable;
use api::ml::features::build_features;
use api::ml::logreg::{train_logreg, LogRegTrainConfig};
use api::quant::backtest::backtest_single_fund;
use api::quant::calibration::PlattCalibrator;
use api::quant::screen4433::{screen_4433, FundReturns};
use api::quant::signal::{SignalGenerator, TradingSignal};
use api::quant::strategy::{Strategy, StrategyConfig};
use std::collections::HashSet;

#[test]
fn dual_layer_4433_ml() {
    // 1. 加载基金（随机抽样200只，可复现）
    let funds = common::load_random_funds(200, 42, 800);
    println!("\n加载 {} 只基金（随机抽样，>=800天）", funds.len());

    // 2. 4433筛选
    let returns: Vec<FundReturns> = funds
        .iter()
        .map(|(code, navs, _)| FundReturns::from_navs(code, navs))
        .collect();
    let screened = screen_4433(&returns);
    let pass_codes: HashSet<String> = screened.iter().filter(|r| r.pass_all).map(|r| r.code.clone()).collect();
    let high_score: HashSet<String> = screened.iter().filter(|r| r.score >= 5).map(|r| r.code.clone()).collect();
    println!("4433完全通过: {} 只", pass_codes.len());
    println!("4433评分>=5: {} 只", high_score.len());

    // 打印通过的基金
    for r in screened.iter().filter(|r| r.pass_all) {
        println!("  通过: {} (score={})", r.code, r.score);
    }

    // 3. 在两组基金上分别训练+回测
    for (label, filter) in [
        ("全部基金", None),
        ("4433通过", Some(&pass_codes)),
        ("4433评分>=5", Some(&high_score)),
    ] {
        let subset: Vec<_> = funds
            .iter()
            .filter(|(code, _, _)| filter.map_or(true, |s: &HashSet<String>| s.contains(code)))
            .collect();
        if subset.len() < 10 {
            println!("{label}: 基金太少({})，跳过", subset.len());
            continue;
        }

        // 训练
        let mut all_x = Vec::new();
        let mut all_y = Vec::new();
        for (_, navs, lt) in &subset {
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

        // 回测
        let mut qualified = 0;
        let mut total_wr = 0.0;
        let mut total_trades = 0;
        let mut scfg = StrategyConfig::default();
        scfg.enter_threshold = 0.55;
        let strategy = Strategy::new(scfg);

        for (code, navs, lt) in &subset {
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
                    sig_gen.generate(&test_navs[i].0, code, &feat, feat[3]).and_then(|sig| {
                        let combined = sig.dip_buy_proba * 0.6 + sig.magic_rebound_proba * 0.4;
                        if combined >= 0.55 { Some(sig) } else { None }
                    })
                })
                .collect();
            let result = backtest_single_fund(code, test_navs, &|i| signals[i].clone(), &strategy, 100_000.0);
            if result.metrics.total_trades >= 3 {
                qualified += 1;
                total_wr += result.metrics.win_rate;
                total_trades += result.metrics.total_trades;
            }
        }
        println!(
            "{label}({}): 可评估{qualified} 胜率{:.1}% 总交易{total_trades}",
            subset.len(),
            if qualified > 0 { total_wr / qualified as f64 * 100.0 } else { 0.0 },
        );
    }
}
